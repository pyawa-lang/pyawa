//! **核心运行期错误分支的引用泄漏判据回归（task-14 #f）**：用 `PYAWA_GC_LOG=1` 的收支做判据。
//!
//! 背景（`docs/rounds/04-post-m6-01.md` round 171/192/194/195）：
//! `iter_value` 对"不可迭代对象"会抛 `TypeError`（`raise()` 给 `Err` 与 `pending_exception`
//! 槽各一份引用）。round 195 修好了 `iter_value` 自己的探针异常（"只查不抛 + discard_raised"），
//! 但**调用方** `Instance::iterable_items` 在 `Ok(None)` 路径把原始 `Err(Raised)` 整个丢掉 ⇒
//! `f(*Plain())` 每次迭代漏一整条 `TypeError→traceback→帧` 链（round 194 的判据：2 万次 ⇒
//! `tb_created−tb_freed` 净增 2 万）。本文件是修后的**回归判据**（修前会红）。
//!
//! 判据（两条都基于显式 `gc.collect()` 的 `[gc]` 行，见 `PYAWA_GC_LOG`）：
//! 1. **异常收支**：`tb_created − tb_freed` 与迭代次数**无关**（固定几次在飞，阈值 ≤ 8；
//!    泄漏时恰为 N）；
//! 2. **对象收支（缩放法）**：同一探针跑 N=10000 与 N=20000，最后一次回收的 `live_after`
//!    差值 ≤ 512（泄漏时差 ≈ N；普通 `__new__` 失败路径漏的是实参对象，没有 traceback ⇒
//!    必须靠这条抓）。
//!
//! 覆盖的修复点：
//! - `crates/pyawa-core/src/instance/convert.rs::iterable_items`：原始 `Err(Raised)` 归还
//!   （`f(*Plain())`／`dict(Plain())`／`sum(Plain())`／`max(Plain())` 一族）；
//! - `crates/pyawa-core/src/executor/call.rs`：`__new__` 覆写支与 `__init__` 抛错路径归还
//!   "本次调用持有"的实参／实例（`tuple(Plain())` 走 `tuple_new_native` 覆写支；用户
//!   `__new__` 抛错走同一支）。
//!
//! **写域外、未修（如实，判据见 task-14 报告）**：`builtin/int.rs` 的 `dunder_int_value`
//! （`Err(_) => continue`）、`builtin/set.rs::set_new`（先分配再失败）等 —— 本测试**不覆盖**它们，
//! 避免把已知未修项伪装成绿。

/// 跑一个探针：脚本从 `sys.argv[1]` 拿迭代次数；另在脚本目录建好 `extra_files`
/// （相对路径 → 内容，用于 `pkg/__init__.py` 这类目录夹具）。
fn run_probe(
    name: &str,
    script: &str,
    n: usize,
    extra_files: &[(&str, &str)],
) -> (String, GcLine) {
    let root = std::env::temp_dir().join(format!("pyawa-refleak-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    for (relative, content) in extra_files {
        let file = root.join(relative);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("建子目录");
        }
        std::fs::write(&file, content).expect("写附加文件");
    }
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .arg(n.to_string())
        .env("PYAWA_GC_LOG", "1")
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        output.status
    );
    let line = stderr
        .lines()
        .filter(|line| line.starts_with("[gc] index="))
        .next_back()
        .unwrap_or_else(|| panic!("PYAWA_GC_LOG=1 没给 [gc] 行：\n{stderr}"));
    (stdout, GcLine::parse(line))
}

/// `[gc] index=… live=… live_after=… freed=… ms=… tb_created=… tb_freed=…` 的解析结果。
struct GcLine {
    live_after: i64,
    tb_created: i64,
    tb_freed: i64,
}

impl GcLine {
    fn parse(line: &str) -> Self {
        let field = |name: &str| -> i64 {
            line.split_whitespace()
                .find_map(|token| token.strip_prefix(name))
                .and_then(|raw| raw.strip_prefix('='))
                .and_then(|raw| raw.parse::<i64>().ok())
                .unwrap_or_else(|| panic!("[gc] 行缺 {name}：{line}"))
        };
        Self {
            live_after: field("live_after"),
            tb_created: field("tb_created"),
            tb_freed: field("tb_freed"),
        }
    }

    fn tb_in_flight(&self) -> i64 {
        self.tb_created - self.tb_freed
    }
}

/// 核心断言：异常收支恒定 ＋ 常驻对象不随 N 线性增长。
fn assert_no_leak(name: &str, script: &str) {
    assert_no_leak_with_files(name, script, &[]);
}

fn assert_no_leak_with_files(name: &str, script: &str, extra_files: &[(&str, &str)]) {
    let (small_stdout, small) = run_probe(name, script, 10_000, extra_files);
    let (large_stdout, large) = run_probe(name, script, 20_000, extra_files);
    assert!(small_stdout.contains("done"), "探针没跑完：{small_stdout}");
    assert!(large_stdout.contains("done"), "探针没跑完：{large_stdout}");
    assert!(
        small.tb_in_flight().abs() <= 8,
        "[{name}] N=10000 的 traceback 收支不平：created={} freed={}（泄漏时 ≈ +N）",
        small.tb_created,
        small.tb_freed
    );
    assert!(
        large.tb_in_flight().abs() <= 8,
        "[{name}] N=20000 的 traceback 收支不平：created={} freed={}（泄漏时 ≈ +N）",
        large.tb_created,
        large.tb_freed
    );
    let growth = large.live_after - small.live_after;
    assert!(
        growth.abs() <= 512,
        "[{name}] 常驻对象随 N 线性增长：live_after {} → {}（差 {growth}；泄漏时 ≈ +N）",
        small.live_after,
        large.live_after
    );
}

/// `f(*Plain())`：`iterable_items` 的 `iter_value` Err 分支（round 194 的原始最小复现）。
const ITER_VALUE_ERR_PROBE: &str = r#"import gc
import sys
N = int(sys.argv[1])
class Plain:
    pass
def f(*a, **k):
    return a, k
gc.collect()
for _ in range(N):
    try:
        f(*Plain())
    except TypeError:
        pass
gc.collect()
print("done", N)
"#;

/// `dict(Plain())`：同一条 `iterable_items` 路（另测 `sum`／`max` 的同一路径）。
const CONTAINER_ERR_PROBE: &str = r#"import gc
import sys
N = int(sys.argv[1])
class Plain:
    pass
gc.collect()
for _ in range(N):
    try:
        dict(Plain())
    except TypeError:
        pass
    try:
        sum(Plain())
    except TypeError:
        pass
    try:
        max(Plain())
    except TypeError:
        pass
gc.collect()
print("done", N)
"#;

/// `tuple(Plain())`：`call.rs` 的 `__new__` 覆写支错误路径（`tuple_new_native` 不在
/// `is_default_new` 名单 ⇒ 走覆写支；修前每失败一次漏一个 `Plain` 实参）。
const TUPLE_NEW_ERR_PROBE: &str = r#"import gc
import sys
N = int(sys.argv[1])
class Plain:
    pass
gc.collect()
for _ in range(N):
    try:
        tuple(Plain())
    except TypeError:
        pass
gc.collect()
print("done", N)
"#;

/// 用户 `__new__` 抛错：`call.rs` 覆写支的"实参归还"（修前每失败一次漏一个实参对象）。
const USER_NEW_ERR_PROBE: &str = r#"import gc
import sys
N = int(sys.argv[1])
class Arg:
    pass
class Boom:
    def __new__(cls, value):
        raise ValueError("boom")
gc.collect()
for _ in range(N):
    try:
        Boom(Arg())
    except ValueError:
        pass
gc.collect()
print("done", N)
"#;

/// 用户 `__init__` 抛错：`call.rs` 的"实例归还"（修前每失败一次漏一个实例对象）。
const USER_INIT_ERR_PROBE: &str = r#"import gc
import sys
N = int(sys.argv[1])
class Boom:
    def __init__(self):
        raise ValueError("boom")
gc.collect()
for _ in range(N):
    try:
        Boom()
    except ValueError:
        pass
gc.collect()
print("done", N)
"#;

/// `from pkg import *` 里"尝试导入子模块失败"的分支（`executor/step_calls.rs` 的
/// `load_module(...).is_ok()`）：修前每失败一次漏一颗 `ModuleNotFoundError→traceback→帧` 链。
const IMPORT_STAR_ERR_PROBE: &str = r#"import gc
import sys
N = int(sys.argv[1])
gc.collect()
for _ in range(N):
    try:
        from pkg import *
    except (ImportError, AttributeError):
        pass
gc.collect()
print("done", N)
"#;

#[test]
fn iter_value_err_branch_does_not_leak() {
    assert_no_leak("iter-value-err", ITER_VALUE_ERR_PROBE);
}

#[test]
fn import_star_submodule_probe_err_branch_does_not_leak() {
    assert_no_leak_with_files(
        "import-star-err",
        IMPORT_STAR_ERR_PROBE,
        &[("pkg/__init__.py", "__all__ = [\"nope\"]\n")],
    );
}

#[test]
fn container_constructor_err_branch_does_not_leak() {
    assert_no_leak("container-err", CONTAINER_ERR_PROBE);
}

#[test]
fn override_new_err_branch_does_not_leak() {
    assert_no_leak("tuple-new-err", TUPLE_NEW_ERR_PROBE);
    assert_no_leak("user-new-err", USER_NEW_ERR_PROBE);
}

#[test]
fn override_init_err_branch_does_not_leak() {
    assert_no_leak("user-init-err", USER_INIT_ERR_PROBE);
}
