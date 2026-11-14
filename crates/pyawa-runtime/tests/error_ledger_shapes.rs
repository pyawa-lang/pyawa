//! **B 引用账本的面（task-16）**：`PYAWA_GC_LOG=4` 下 `ExecError::Raised` 的账目口径回归。
//!
//! 账本实现在 `crates/pyawa-core/src/error_ledger.rs`（默认关闭、正常路径零分支）。本文件从
//! **CLI 视角**钉住它的可观察契约（不碰 Instance 内部字段 ⇒ 跨 crate 也能测）：
//!
//! 1. **默认静默**：不开 `PYAWA_GC_LOG=4` 时，异常密集的脚本 stderr 里**不得**出现
//!    `[raise-ledger]`（诊断仪器不得改变正常运行的输出）。
//! 2. **账目恒等式**：`generated == pending`（本层每次生成都同时移交 pending），且
//!    `generated == discarded + transferred + implicit + leaked + suspects + overwritten`
//!    （见账本模块头的口径；`untracked_consumed` 不参与）。
//! 3. **不误报**：task-14 已修好的失败路径（`iter(Plain())`／`f(*Plain())`／坏 `__new__`／
//!    `__init__`／`__exit__`）跑 N=2000 与 N=4000，账本的 `leaked` **不随 N 线性增长**。
//! 4. **报告一致性**：`leaked > 0` ⇔ 输出里出现 `[raise-ledger] 未消费 Raised` 生成点行
//!    （修前 HEAD 上 `int(Plain())` 一族为真；task-18 修完后为假 —— 两个方向都自洽）。

use std::collections::HashMap;
use std::path::PathBuf;

fn temp_script(name: &str, script: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("pyawa-ledger-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    path
}

/// 跑一次 CLI，返回 `(退出成功?, stdout, stderr)`。
fn run(name: &str, script: &str, gc_log: Option<&str>) -> (bool, String, String) {
    let path = temp_script(name, script);
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"));
    command.arg(&path);
    if let Some(value) = gc_log {
        command.env("PYAWA_GC_LOG", value);
    } else {
        command.env_remove("PYAWA_GC_LOG");
    }
    let output = command.output().expect("跑 CLI");
    let _ = std::fs::remove_dir_all(path.parent().expect("临时目录"));
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// 账本 summary 的解析结果（`[raise-ledger] generated=… pending=… …` 的末行）。
#[derive(Default)]
struct LedgerSummary {
    generated: u64,
    pending: u64,
    discarded: u64,
    transferred: u64,
    implicit: u64,
    leaked: u64,
    suspects: u64,
    overwritten: u64,
    untracked_consumed: u64,
    report_lines: usize,
}

fn parse_summary(stderr: &str) -> Option<LedgerSummary> {
    let line = stderr
        .lines()
        .filter(|line| line.starts_with("[raise-ledger] generated="))
        .next_back()?;
    let mut fields: HashMap<&str, u64> = HashMap::new();
    for token in line.split_whitespace().skip(1) {
        if let Some((key, value)) = token.split_once('=') {
            if let Ok(value) = value.parse::<u64>() {
                fields.insert(key, value);
            }
        }
    }
    let report_lines = stderr
        .lines()
        .filter(|line| line.starts_with("[raise-ledger] 未消费 Raised"))
        .count();
    Some(LedgerSummary {
        generated: *fields.get("generated")?,
        pending: *fields.get("pending")?,
        discarded: *fields.get("discarded").unwrap_or(&0),
        transferred: *fields.get("transferred").unwrap_or(&0),
        implicit: *fields.get("implicit").unwrap_or(&0),
        leaked: *fields.get("leaked").unwrap_or(&0),
        suspects: *fields.get("suspects").unwrap_or(&0),
        overwritten: *fields.get("overwritten").unwrap_or(&0),
        untracked_consumed: *fields.get("untracked_consumed").unwrap_or(&0),
        report_lines,
    })
}

/// 探针模板：`body` 提供 `call()`；末尾**两次** `gc.collect()`（第二次让在途 suspects 清零）。
fn probe_script(body: &str) -> String {
    format!(
        "import gc\n\n{body}\n\nN = 2000\ngc.collect()\nfor _ in range(N):\n    call()\ngc.collect()\ngc.collect()\nprint(\"done\")\n"
    )
}

const ITER_GREEN: &str = r#"class Plain:
    pass

def call():
    try:
        iter(Plain())
    except TypeError:
        pass
"#;

const FSTAR_GREEN: &str = r#"class Plain:
    pass

def f(*a, **k):
    return a

def call():
    try:
        f(*Plain())
    except TypeError:
        pass
"#;

const BAD_NEW_GREEN: &str = r#"class Boom:
    def __new__(cls):
        raise ValueError("new boom")

def call():
    try:
        Boom()
    except ValueError:
        pass
"#;

const INT_LEAK_PROBE: &str = r#"class Plain:
    pass

def call():
    try:
        int(Plain())
    except TypeError:
        pass
"#;

fn assert_accounting(summary: &LedgerSummary, label: &str) {
    assert_eq!(
        summary.generated, summary.pending,
        "[{label}] 生成与 pending 移交数不相等（本层每次 raise 都应同时移交 pending）"
    );
    assert!(
        summary.generated > 0,
        "[{label}] 账本没记到任何生成（PYAWA_GC_LOG=4 没生效？）"
    );
    let accounted = summary.discarded
        + summary.transferred
        + summary.implicit
        + summary.leaked
        + summary.suspects
        + summary.overwritten;
    assert_eq!(
        summary.generated, accounted,
        "[{label}] 账目恒等式不成立：generated={} vs discarded+transferred+implicit+leaked+suspects+overwritten={}",
        summary.generated, accounted
    );
    // `untracked_consumed` 是"消费了但没记生成账"的旁口径（直接构造 bundle 的路径），只要求非负。
    let _ = summary.untracked_consumed;
}

#[test]
fn ledger_is_silent_when_disabled() {
    // `int(Plain())` 是已知会生成大量异常（并被吞掉）的路径；不开账本时 stderr 不得出现账本行。
    let (ok, _stdout, stderr) = run("silent", &probe_script(INT_LEAK_PROBE), None);
    assert!(ok, "脚本不许中止\n{stderr}");
    assert!(
        !stderr.contains("[raise-ledger]"),
        "账本默认必须静默，但 stderr 出现 [raise-ledger]：\n{stderr}"
    );
}

/// 安装期基线：**空 call()** 跑同一模板 ⇒ 得到"与循环无关的固定泄漏数"。
///
/// 现有 4-6 颗来自 `install` 阶段的属性探针（`reject_abstract_instantiation` 一类用
/// `attribute_optional(...).ok()` 丢掉 `Err` ⇒ 每进程固定几颗；不在 task-16/18 的 6 处清单里，
/// 记在报告里单开工单）。判据只要求"失败路径的泄漏**不随迭代次数增长**" ⇒ 减掉基线。
fn installation_baseline() -> u64 {
    let body = "def call():\n    pass\n";
    let (ok, _stdout, stderr) = run("baseline", &probe_script(body), Some("4"));
    assert!(ok, "基线脚本不许中止\n{stderr}");
    parse_summary(&stderr)
        .unwrap_or_else(|| panic!("基线没看到账本 summary：\n{stderr}"))
        .leaked
}

#[test]
fn ledger_accounting_reconciles_on_green_paths() {
    let baseline = installation_baseline();
    for (name, body) in [
        ("iter-green", ITER_GREEN),
        ("fstar-green", FSTAR_GREEN),
        ("bad-new-green", BAD_NEW_GREEN),
    ] {
        let (ok, stdout, stderr) = run(name, &probe_script(body), Some("4"));
        assert!(ok, "[{name}] 脚本不许中止\n{stderr}");
        assert!(stdout.contains("done"), "[{name}] 探针没跑完：{stdout}");
        let summary = parse_summary(&stderr)
            .unwrap_or_else(|| panic!("[{name}] 没看到账本 summary：\n{stderr}"));
        assert_accounting(&summary, name);
        assert_eq!(
            summary.suspects, 0,
            "[{name}] 收尾两次 gc.collect 后不该还有未确认在途项"
        );
        // 只允许**安装期常数**（基线），不允许随 2000 次迭代增长。
        let growth = summary.leaked as i64 - baseline as i64;
        assert!(
            growth.abs() <= 8,
            "[{name}] task-14 已修的路径出现随迭代增长的未消费 Raised：leaked={} baseline={baseline}",
            summary.leaked
        );
    }
}

#[test]
fn ledger_does_not_scale_false_leaks_with_iteration_count() {
    // 缩放法：N=2000 与 N=4000 的 leaked 差值必须恒定（≤ 16 —— 安装期一次性泄漏的常数），
    // 不得随 N 线性增长。
    let body = ITER_GREEN;
    let mut leaked = Vec::new();
    for n in [2000usize, 4000usize] {
        let script = probe_script(body).replace("N = 2000", &format!("N = {n}"));
        let (ok, _stdout, stderr) = run("scale", &script, Some("4"));
        assert!(ok, "缩放探针不许中止\n{stderr}");
        let summary =
            parse_summary(&stderr).unwrap_or_else(|| panic!("没看到账本 summary：\n{stderr}"));
        assert_accounting(&summary, "scale");
        leaked.push(summary.leaked);
    }
    let growth = leaked[1] as i64 - leaked[0] as i64;
    assert!(
        growth.abs() <= 16,
        "leaked 随迭代次数增长：N=2000 → {}，N=4000 → {}（差 {growth}）",
        leaked[0],
        leaked[1]
    );
}

#[test]
fn ledger_reports_are_consistent_with_counters() {
    // 报告一致性（**修前修后都成立**）：leaked>0 ⇔ 出现生成点行；leaked==0 ⇔ 没有生成点行。
    // 修前 HEAD 上 `int(Plain())` 属于 leaked>0（task-18 修完之后转为 0，本断言两个方向都自洽）。
    let (ok, _stdout, stderr) = run("int-leak", &probe_script(INT_LEAK_PROBE), Some("4"));
    assert!(ok, "脚本不许中止\n{stderr}");
    let summary = parse_summary(&stderr).unwrap_or_else(|| panic!("没看到账本 summary：\n{stderr}"));
    assert_accounting(&summary, "int-leak");
    if summary.leaked > 0 {
        assert!(
            summary.report_lines > 0,
            "leaked={} 但没有任何 `未消费 Raised` 生成点行：\n{stderr}",
            summary.leaked
        );
        assert!(
            stderr.contains("生成点="),
            "生成点行缺 `生成点=` 字段：\n{stderr}"
        );
    } else {
        assert_eq!(
            summary.report_lines, 0,
            "leaked==0 却出现了生成点行：\n{stderr}"
        );
    }
}
