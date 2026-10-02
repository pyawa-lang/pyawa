//! **M2 对拍 harness —— 减配首版**（`docs/PLAN-milestones.md` §5，`MS-6`…`MS-15`／`MS-24`）。
//!
//! 判据落在 `MS-6`…`MS-15`；本文件只实现**首版能实现的那部分**，缺口一律写在下面与报告里，
//! **不**登记成差异（`MS-19`：`"尚未实现"不是差异`）。
//!
//! # 减配在哪（**不是**差异登记）
//!
//! - `MS-8` 要求比对"退出码／stdout／stderr／未捕获异常"。首版只比**退出码 ＋ 未捕获异常的
//!   类型与消息 ＋ 探针的值**：`print` 未落地（要 `sys.stdout` → `_io` → `fs` 域，`CM-26`）
//!   ⇒ **stdout／stderr 不比对**。参照侧的 stdout 只用来取探针值（观测手段，不是比对项）。
//! - 探针的值用**标量渲染**（`str` 语义）：`str`／`int`／`bool`／`None` 之外的标签如实渲染成
//!   `<unrenderable:tag>`（通用 `repr` 要类型面接上之后才有）。
//! - `MS-9` 的规范化只做了"行尾／末尾换行 ＋ `0x…` 地址"——首版语料里没有路径／临时目录／耗时。
//! - `MS-13` ③：扩展模式（`pyawa`）**没有参照实现** ⇒ 首版语料只有纯 Python 模式；
//!   manifest 里出现非 `python` 模式会**直接失败**（不许静默跳过）。
//! - 语料**不得**使用内建名（`len`／`ValueError`／`print`…）：ABI 实例还没有 `builtins`
//!   映射（`builtins` 模块归 `P3-14`／`CM-14`）⇒ 那是"尚未落地"，不是差异（`MS-19`）。
//! - 探针只在**两侧都成功**时比对：执行失败时探针行根本没跑到，两侧都观测不到；
//!   `pa_getglobal` 对不存在的名字给 `None`（不是错误），拿它比会造出假阳性。
//! - 探针注入**不写圆括号**：本层编译器还没有括号表达式（`x = (1)` 会报未接线）。
//!
//! # 首版实测到的"尚未实现"边界（**不进**差异清单，`MS-19` 的适用范围）
//!
//! - 编译器的**下标表达式**：`x = a[1]` ⇒ `语句结尾多出了 Some(LeftBracket)`
//! - **类对象上的属性读**：`class C: v = 5` 之后 `x = C.v` ⇒ `'type' object has no attribute 'v'`
//!   （实例路径的属性读是通的；类对象那一格没接线）
//! - ABI 实例**没有 `builtins` 映射** ⇒ `ValueError`／`len`／`print` 一类名字取不到
//! - **大整数没有 ABI 通道**：`pa_tointeger` 对超出 `i64` 的整数如实返 `PA_ERR_NOTIMPLEMENTED`
//!   （不是 0），`pa_tostring` 目前只认 `str` ⇒ 语料里暂时**放不了**大整数探针（放进去会红，
//!   但那是"ABI 通道缺失"而不是语义差异）
//!
//! 三条都记在 `crates/pyawa-core/src/lib.rs` 的待做清单与 `tests/conformance/README.md`；
//! 语料里**不放**它们（放了就该红——这是设计，不是跳过）。
//!
//! # 怎么跑
//!
//! `cargo test -p pyawa-abi --test conformance` —— 两个用例：
//! [`the_corpus_has_no_new_divergences`]（`MS-10` 的三分类；有新差异即红）与
//! [`the_harness_self_check_is_green`]（`MS-12`／`T-MS-3`：两侧都指向参照实现时必须全绿）。
//! 报告落在 `target/conformance/report.md`（`MS-14`：含参照版本、语料清单与模式、三分类计数、
//! 差异清单快照），命令行上也打一份。
//!
//! # 两侧怎么跑（`MS-7`：各跑一次）
//!
//! 探针是**表达式**：参照侧注入 `print(<expr>)`，Pyawa 侧注入 `__probe_<i> = (<expr>)`（然后
//! 用 `pa_getglobal` 取回）。探针只负责把值取出来看，不改被测源码。
//! Pyawa 侧在**子进程**里跑（本测试二进制被重新 exec 成 [`pyawa_side_runner`]）⇒ `MS-15` 的
//! 超时对两侧都成立，且 Pyawa 侧崩溃只毁那一个 case（不会带走整个套件）。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pyawa_abi::status::PA_OK;
use pyawa_abi::tag::*;
use pyawa_abi::*;

/// `MS-15`：单侧单 case 的墙钟上限；超时 ⇒ 失败（新差异），**禁止**重试。
const TIMEOUT: Duration = Duration::from_secs(20);
/// 子进程协议：观测块的两个哨兵。
const BEGIN: &str = "PYAWA-OBSERVATION-BEGIN";
const END: &str = "PYAWA-OBSERVATION-END";

// --------------------------------------------------------------------------- #
// 语料与观测
// --------------------------------------------------------------------------- #

/// 一个 case（`MS-6`：源码 ＋ **显式模式**；探针是本 harness 的观测通道）。
#[derive(Debug, Clone)]
struct Case {
    name: String,
    /// 首版只允许 `python`（见文件头）。
    mode: String,
    source: String,
    /// 探针表达式（可为空）。
    probes: Vec<String>,
    /// manifest 里声明的已知差异编号（`MS-10` 的第二类）。
    known_divergence: Option<String>,
}

/// 减配版的观测项（`MS-8` 的子集）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    exit_code: i32,
    exception: Option<(String, String)>,
    probes: Vec<String>,
    /// 超时／缺前置一类的事故：**计入失败**，不是"跳过"。
    accident: Option<String>,
}

impl Observation {
    fn timeout(probe_count: usize) -> Self {
        Self {
            exit_code: -1,
            exception: None,
            probes: vec!["<timeout>".to_owned(); probe_count],
            accident: Some("超时（`MS-15`：计新差异，禁止重试）".to_owned()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    Pyawa,
    Cpython,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Verdict {
    Pass,
    KnownDivergence(String),
    NewDivergence,
}

#[derive(Debug, Default)]
struct Summary {
    total: usize,
    pass: usize,
    known: Vec<(String, String)>,
    new: Vec<(String, String)>,
}

// --------------------------------------------------------------------------- #
// 语料加载
// --------------------------------------------------------------------------- #

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn corpus_directory() -> PathBuf {
    workspace().join("tests").join("conformance").join("corpus")
}

fn load_cases() -> Vec<Case> {
    let manifest = corpus_directory().join("manifest.tsv");
    let text = fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("读不到语料清单 {}：{error}", manifest.display()));
    let mut cases = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        assert!(
            cells.len() == 4,
            "{} 第 {} 行：清单是 4 列（文件／模式／探针／已知差异），实际 {} 列",
            manifest.display(),
            number + 1,
            cells.len()
        );
        let file = cells[0].trim();
        let mode = cells[1].trim();
        assert!(
            mode == "python",
            "{}：模式 {mode:?} —— 减配首版没有扩展模式的参照实现（`MS-13` ③），\
             扩展模式语料在定案前必须为空；**不得**静默跳过",
            manifest.display()
        );
        let path = corpus_directory().join(file);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("读不到语料 {}：{error}", path.display()));
        let probes = match cells[2].trim() {
            "-" | "" => Vec::new(),
            list => list.split(',').map(|probe| probe.trim().to_owned()).collect(),
        };
        let known_divergence = match cells[3].trim() {
            "-" | "" => None,
            id => Some(id.to_owned()),
        };
        let name = file.trim_end_matches(".py").to_owned();
        cases.push(Case { name, mode: mode.to_owned(), source, probes, known_divergence });
    }
    assert!(!cases.is_empty(), "语料不能是空的（`MS-13` ①）");
    cases
}

/// 差异清单里的编号（`MS-10` 的第二类靠它判定；墓碑不算）。
fn known_divergence_ids() -> Vec<String> {
    let path = workspace().join("tests").join("conformance").join("divergences.md");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不到差异清单 {}：{error}", path.display()));
    let mut ids = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        for cell in trimmed.split('|') {
            if let Some(id) = cell.trim().strip_prefix('`').and_then(|rest| rest.strip_suffix('`')) {
                // 只认 `DIV-<数字>`：表头格式示例里的 `DIV-n` 不算条目
                if let Some(number) = id.strip_prefix("DIV-") {
                    if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
                        ids.push(id.to_owned());
                    }
                }
            }
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

// --------------------------------------------------------------------------- #
// 规范化（`MS-9`，减配：行尾 ＋ `0x…` 地址）
// --------------------------------------------------------------------------- #

fn normalize(text: &str) -> String {
    let mut out = String::new();
    for (index, line) in text.replace("\r\n", "\n").lines().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end());
    }
    normalize_addresses(&out)
}

/// `0x…` 形式的内存地址 ⇒ `0xADDR`（`MS-9`）。
///
/// 按 **char** 切，不按字节：字节级 `as char` 会把 UTF-8 拆成拉丁-1（本轮实测踩过）。
fn normalize_addresses(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(position) = rest.find("0x") {
        let after = &rest[position + 2..];
        let digits = after.chars().take_while(|character| character.is_ascii_hexdigit()).count();
        if digits == 0 {
            out.push_str(&rest[..position + 2]);
            rest = after;
            continue;
        }
        out.push_str(&rest[..position]);
        out.push_str("0xADDR");
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// 从"异常行"里取（类型，消息）：`ValueError: boom` ⇒ `("ValueError", "boom")`。
fn parse_exception(text: &str) -> Option<(String, String)> {
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?.trim();
    match line.split_once(": ") {
        Some((kind, message)) => Some((normalize(kind.trim()), normalize(message.trim()))),
        None => Some((normalize(line), String::new())),
    }
}

// --------------------------------------------------------------------------- #
// 参照侧（CPython；`MS-16`）
// --------------------------------------------------------------------------- #

fn reference_version() -> String {
    match Command::new("python3").arg("--version").output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        Err(error) => panic!("参照实现起不来（`MS-7` 要求两侧各跑一次）：{error}"),
    }
}

fn run_cpython(case: &Case) -> Observation {
    let mut program = case.source.clone();
    if !program.ends_with('\n') {
        program.push('\n');
    }
    for probe in &case.probes {
        program.push_str(&format!("print({probe})\n"));
    }
    let directory = workspace().join("target").join("conformance");
    fs::create_dir_all(&directory).expect("建 target/conformance");
    let path = directory.join(format!("{}.reference.py", case.name));
    fs::write(&path, &program).expect("写参照侧程序");

    let mut command = Command::new("python3");
    command.arg(&path);
    let Some(output) = output_with_timeout(command, TIMEOUT) else {
        return Observation::timeout(case.probes.len());
    };
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut probes: Vec<String> =
        stdout.lines().map(|line| normalize(line.trim_end())).collect();
    while probes.len() < case.probes.len() {
        probes.push("<missing>".to_owned());
    }
    probes.truncate(case.probes.len());
    let exception = if exit_code == 0 {
        None
    } else {
        parse_exception(&String::from_utf8_lossy(&output.stderr))
    };
    Observation { exit_code, exception, probes, accident: None }
}

// --------------------------------------------------------------------------- #
// Pyawa 侧：本测试二进制重新 exec 成 [`pyawa_side_runner`]
// --------------------------------------------------------------------------- #

/// 子进程入口（平时没有 `PYAWA_CONFORMANCE_SOURCE` 就什么都不做）。
#[test]
fn pyawa_side_runner() {
    let Ok(path) = std::env::var("PYAWA_CONFORMANCE_SOURCE") else {
        return;
    };
    let probes: usize = std::env::var("PYAWA_CONFORMANCE_PROBES")
        .ok()
        .and_then(|value| value.parse().ok())
        .expect("子进程缺 PYAWA_CONFORMANCE_PROBES");
    let source = fs::read_to_string(&path).expect("子进程读源码");
    let observation = execute_pyawa(&source, probes);
    println!("{BEGIN}");
    println!("exit={}", observation.exit_code);
    match &observation.exception {
        Some((kind, message)) => {
            println!("exception_type={}", escape(kind));
            println!("exception_message={}", escape(message));
        }
        None => println!("exception_type="),
    }
    for probe in &observation.probes {
        println!("probe={}", escape(probe));
    }
    if let Some(accident) = &observation.accident {
        println!("accident={}", escape(accident));
    }
    println!("{END}");
}

fn run_pyawa(case: &Case) -> Observation {
    let mut program = case.source.clone();
    if !program.ends_with('\n') {
        program.push('\n');
    }
    for (index, probe) in case.probes.iter().enumerate() {
        // **不加圆括号**：本层编译器还没有"括号表达式"（本轮实测：`x = (1)` 报
        // "表达式解析到尾出现了 Some(LeftParen)"）⇒ 探针注入不能引入它
        program.push_str(&format!("__probe_{index} = {probe}\n"));
    }
    let directory = workspace().join("target").join("conformance");
    fs::create_dir_all(&directory).expect("建 target/conformance");
    let path = directory.join(format!("{}.subject.py", case.name));
    fs::write(&path, &program).expect("写被测侧程序");

    let executable = std::env::current_exe().expect("测试二进制路径");
    let mut command = Command::new(executable);
    command.args(["--exact", "pyawa_side_runner", "--nocapture"]);
    command.env("PYAWA_CONFORMANCE_SOURCE", &path);
    command.env("PYAWA_CONFORMANCE_PROBES", case.probes.len().to_string());
    let Some(output) = output_with_timeout(command, TIMEOUT) else {
        return Observation::timeout(case.probes.len());
    };
    if !output.status.success() {
        // 崩溃也算失败（`MS-10` 的"新差异"），不是"跳过"
        return Observation {
            exit_code: -1,
            exception: None,
            probes: vec!["<crash>".to_owned(); case.probes.len()],
            accident: Some(format!(
                "Pyawa 侧子进程退出码 {:?}：{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        };
    }
    parse_observation(&String::from_utf8_lossy(&output.stdout))
}

fn parse_observation(stdout: &str) -> Observation {
    let mut exit_code = -1;
    let mut kind = String::new();
    let mut message = String::new();
    let mut probes = Vec::new();
    let mut accident = None;
    let mut inside = false;
    for line in stdout.lines() {
        if line.trim() == BEGIN {
            inside = true;
            continue;
        }
        if line.trim() == END {
            inside = false;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(value) = line.strip_prefix("exit=") {
            exit_code = value.trim().parse().unwrap_or(-1);
        } else if let Some(value) = line.strip_prefix("exception_type=") {
            kind = unescape(value);
        } else if let Some(value) = line.strip_prefix("exception_message=") {
            message = unescape(value);
        } else if let Some(value) = line.strip_prefix("probe=") {
            probes.push(unescape(value));
        } else if let Some(value) = line.strip_prefix("accident=") {
            accident = Some(unescape(value));
        }
    }
    let exception = if kind.is_empty() { None } else { Some((kind, message)) };
    Observation { exit_code, exception, probes, accident }
}

/// 观测块里的值一律单行：转义换行与反斜杠。
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// 在 Pyawa 实例里执行一段源码（**进程内**；由子进程入口调用）。
fn execute_pyawa(program: &str, probe_count: usize) -> Observation {
    // SAFETY: 指针都是本函数自己的局部量；实例随用随销。
    unsafe {
        let host = pa_host {
            abi_size: core::mem::size_of::<pa_host>(),
            abi_version: PA_ABI_VERSION,
            capabilities: core::ptr::null(),
        };
        let mut state: *mut pa_state = core::ptr::null_mut();
        assert_eq!(pa_create(&host, &mut state), PA_OK, "建实例失败");
        let mode = b"python\0";
        let status = pa_exec_string(
            state,
            program.as_ptr().cast(),
            program.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        );
        let exit_code = if status == PA_OK { 0 } else { 1 };
        let exception = if status == PA_OK {
            None
        } else {
            parse_exception(&errmsg_of(state))
        };
        let mut probes = Vec::new();
        for index in 0..probe_count {
            let name = format!("__probe_{index}\0");
            if pa_getglobal(state, name.as_ptr().cast()) == PA_OK {
                probes.push(render_top(state));
            } else {
                probes.push("<missing>".to_owned());
            }
        }
        pa_destroy(state);
        Observation { exit_code, exception, probes, accident: None }
    }
}

/// 当前错误信息（`pa_errmsg` 的借用）。
unsafe fn errmsg_of(state: *mut pa_state) -> String {
    let pointer = unsafe { pa_errmsg(state) };
    if pointer.is_null() {
        return String::new();
    }
    unsafe { core::ffi::CStr::from_ptr(pointer) }.to_string_lossy().into_owned()
}

/// 把栈顶渲染成文本（减配版的标量渲染，见文件头）。
unsafe fn render_top(state: *mut pa_state) -> String {
    let tag_value = unsafe { pa_type(state, -1) };
    if tag_value == PA_TSTRING {
        let mut length = 0usize;
        let pointer = unsafe { pa_tostring(state, -1, &mut length) };
        if pointer.is_null() {
            return "<unrenderable>".to_owned();
        }
        let bytes = unsafe { core::slice::from_raw_parts(pointer.cast::<u8>(), length) };
        return normalize(&String::from_utf8_lossy(bytes));
    }
    if tag_value == PA_TINTEGER {
        // `AB-62` 的整数桥**覆盖全部整数**（`i64` 内的也走它）⇒ 渲染只有这一条路，
        // 不再"先试 `pa_tointeger`、失败再回落"（那正是 `AB-62` 要消掉的两种真相）。
        let mut length = 0usize;
        let pointer = unsafe { pa_tointstring(state, -1, &mut length) };
        if pointer.is_null() {
            // 只有"超位数上限"会走到这里（`TS-45` 的 4300）：如实标出来，不当成 0／空串
            return "<int-over-digit-limit>".to_owned();
        }
        let bytes = unsafe { core::slice::from_raw_parts(pointer.cast::<u8>(), length) };
        return normalize(&String::from_utf8_lossy(bytes));
    }
    if tag_value == PA_TBOOLEAN {
        return if unsafe { pa_toboolean(state, -1) } == 1 { "True" } else { "False" }.to_owned();
    }
    if tag_value == PA_TNIL {
        return "None".to_owned();
    }
    // `bytes`：`AB-62` 说它走 `pa_tobytes`（`pa_tag` 里没有 bytes ⇒ 用 `NULL` 判类型）。
    let mut byte_length = 0usize;
    let byte_pointer = unsafe { pa_tobytes(state, -1, &mut byte_length) };
    // 这里渲染成 `<bytes:十六进制>`——**故意**不是参照的 `b'…'` 字面形态：
    // 跨语言可比的走法是让**探针**把 bytes 转成可比的东西（例如 `x.hex()`），
    // 而不是在 harness 里重写一遍 `repr` 的引号规则（那是第二处真相）。
    if !unsafe { pa_tobytes(state, -1, &mut byte_length) }.is_null() {
        let bytes = unsafe { core::slice::from_raw_parts(byte_pointer.cast::<u8>(), byte_length) };
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        return format!("<bytes:{hex}>");
    }
    format!("<unrenderable:{tag_value}>")
}

// --------------------------------------------------------------------------- #
// 超时（`MS-15`）
// --------------------------------------------------------------------------- #

fn output_with_timeout(mut command: Command, limit: Duration) -> Option<std::process::Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("起不了子进程（缺 `python3` 也是缺前置 ⇒ 红）：{error}"));
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(error) => panic!("等子进程失败：{error}"),
        }
    }
}

// --------------------------------------------------------------------------- #
// 比对与报告（`MS-10`／`MS-14`）
// --------------------------------------------------------------------------- #

fn compare(case: &Case, reference: &Observation, subject: &Observation) -> Verdict {
    // 探针只在**两侧都成功**时才比：执行失败时附录的探针行根本没跑，两侧都"观测不到"；
    // 而 `pa_getglobal` 对不存在的名字给 `None`（不是错误）⇒ 失败了还去比探针只会造出假阳性。
    let probes_comparable = reference.exit_code == 0 && subject.exit_code == 0;
    let same = reference.exit_code == subject.exit_code
        && reference.exception == subject.exception
        && reference.accident == subject.accident
        && (!probes_comparable || reference.probes == subject.probes);
    if same {
        return Verdict::Pass;
    }
    match &case.known_divergence {
        Some(id) => {
            assert!(
                known_divergence_ids().contains(id),
                "manifest 引用了差异清单里没有的编号 {id}（`MS-19`：依据必填）"
            );
            Verdict::KnownDivergence(id.clone())
        }
        None => Verdict::NewDivergence,
    }
}

fn render(reference: &Observation, subject: &Observation) -> String {
    format!(
        "退出码 {} vs {}；异常 {:?} vs {:?}；探针 {:?} vs {:?}",
        reference.exit_code,
        subject.exit_code,
        reference.exception,
        subject.exception,
        reference.probes,
        subject.probes
    )
}

fn run_all(subject: Subject) -> Summary {
    let cases = load_cases();
    let mut summary = Summary::default();
    let mut report = String::new();
    report.push_str("# 对拍报告（M2 harness 减配首版）\n\n");
    report.push_str(&format!("- 参照实现：**{}**\n", reference_version()));
    report.push_str(&format!("- 被测侧：{}\n", match subject {
        Subject::Pyawa => "Pyawa（`pa_exec_string`，进程内 ＋ 子进程隔离）",
        Subject::Cpython => "**自检**：参照实现本身（`MS-12`）",
    }));
    report.push_str(&format!("- 超时上限：{} s（`MS-15`）\n", TIMEOUT.as_secs()));
    report.push_str("- 比对项：退出码 ＋ 未捕获异常（类型／消息）＋ 探针值 —— **stdout／stderr 不比**\n");
    report.push_str("  （`print` 未落地，要 `sys.stdout` → `_io` → `fs` 域，`CM-26`；这是**尚未落地**，\n");
    report.push_str("   不是差异登记（`MS-19`））\n");
    report.push_str(&format!(
        "- 差异清单快照：{}\n\n",
        known_divergence_ids().join("、")
    ));
    report.push_str("| case | 模式 | 结果 | 观测（参照 vs 被测） |\n|---|---|---|---|\n");

    for case in &cases {
        summary.total += 1;
        let reference = run_cpython(case);
        let observed = match subject {
            Subject::Pyawa => run_pyawa(case),
            Subject::Cpython => run_cpython(case),
        };
        let verdict = compare(case, &reference, &observed);
        let detail = render(&reference, &observed);
        let label = match &verdict {
            Verdict::Pass => "通过".to_owned(),
            Verdict::KnownDivergence(id) => format!("已知差异（{id}）"),
            Verdict::NewDivergence => "**新差异**".to_owned(),
        };
        match &verdict {
            Verdict::Pass => summary.pass += 1,
            Verdict::KnownDivergence(id) => summary.known.push((case.name.clone(), id.clone())),
            Verdict::NewDivergence => summary.new.push((case.name.clone(), detail.clone())),
        }
        println!(
            "[{}] {}（{}）⇒ {}",
            match subject { Subject::Pyawa => "对拍", Subject::Cpython => "自检" },
            case.name,
            case.mode,
            label
        );
        if !matches!(verdict, Verdict::Pass) {
            println!("    {detail}");
        }
        report.push_str(&format!("| `{}` | {} | {} | {} |\n", case.name, case.mode, label, detail));
    }

    report.push_str(&format!(
        "\n**计数**：共 {} ⇒ 通过 **{}** · 已知差异 **{}** · 新差异 **{}**\n",
        summary.total,
        summary.pass,
        summary.known.len(),
        summary.new.len()
    ));
    let path = workspace().join("target").join("conformance").join(match subject {
        // 两个用例并行跑 ⇒ **各写各的报告**，别互相覆盖（本轮实测踩过）
        Subject::Pyawa => "report.md",
        Subject::Cpython => "self-check.md",
    });
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, &report);
    println!(
        "计数：共 {} ⇒ 通过 {} · 已知差异 {} · 新差异 {}（报告：{}）",
        summary.total,
        summary.pass,
        summary.known.len(),
        summary.new.len(),
        path.display()
    );
    summary
}

#[test]
fn the_corpus_has_no_new_divergences() {
    // `MS-10`：三分类；新差异即失败（M2 的判据就是这条）
    let summary = run_all(Subject::Pyawa);
    assert!(
        summary.new.is_empty(),
        "有新差异（`MS-10` 的第三类）：{:?}",
        summary.new
    );
    assert_eq!(summary.pass + summary.known.len(), summary.total);
}

#[test]
fn the_harness_self_check_is_green() {
    // `MS-12`／`T-MS-3`：两侧都指向参照实现 ⇒ 必须全绿；不过说明 harness 本身坏了，
    // 此时**禁止**采信任何对拍结论。
    let summary = run_all(Subject::Cpython);
    assert_eq!(
        summary.pass, summary.total,
        "自检不过 ⇒ harness 坏了；新差异：{:?}",
        summary.new
    );
}
