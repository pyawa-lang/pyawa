//! **失败路径的 GC 收支回归（task-16 D ／ task-18 验收）**：11 条失败路径的"引用不随迭代增长"判据。
//!
//! 判据（与 `tests/ci/failure_path_leaks.py` 同一口径，见那一份脚本的文档）：
//! 1. **异常收支**：最后一次回收的 `tb_created − tb_freed ≤ 8`（泄漏时 ≈ N）；
//! 2. **对象收支（缩放法）**：N=10000 与 N=20000 两档 `live_after` 差 ∈ [−512, +512]（泄漏时 ≈ N）。
//!
//! 覆盖（task-16 的 D 清单）：不可迭代（`f(*Plain())`／`tuple`／`set`／`frozenset`／`int`）、
//! `sorted` 比较失败、坏 `__new__`／`__init__`／`__exit__`／`__del__`、惰性注解。
//!
//! **修前（HEAD 82257fc1 + 阶段 0 账本）基线**：6/11 RED —— `set`／`frozenset`／`int`／
//! `sorted_compare`／`bad_del`／`lazy_annotations`；task-18 修完后本文件 **11/11 全绿**。
//! 各泄漏的根因/修法见 `docs/PLAN-milestones.md` 阶段 1 报告与 task-18 报告。

fn run_probe(name: &str, body: &str, n: usize) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-failleak-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let script = format!(
        "import gc\n\n{body}\n\nN = {n}\ngc.collect()\nfor _ in range(N):\n    call()\ngc.collect()\ngc.collect()\nprint(\"done\")\n"
    );
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .env("PYAWA_GC_LOG", "1")
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success() && stdout.contains("done"),
        "探针没跑完（{name}，N={n}）：退出 {:?}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        output.status
    );
    let line = stderr
        .lines()
        .filter(|line| line.starts_with("[gc] index="))
        .next_back()
        .unwrap_or_else(|| panic!("没解析到 [gc] 行（{name}）：\n{stderr}"));
    line.to_owned()
}

/// `(live_after, tb_created, tb_freed)`。
fn gc_numbers(line: &str) -> (i64, i64, i64) {
    let field = |name: &str| -> i64 {
        line.split_whitespace()
            .find_map(|token| token.strip_prefix(&format!("{name}=")))
            .and_then(|raw| raw.parse::<i64>().ok())
            .unwrap_or_else(|| panic!("[gc] 行缺 {name}：{line}"))
    };
    (field("live_after"), field("tb_created"), field("tb_freed"))
}

/// 跑两档 N，断言两条判据。
fn assert_probe_does_not_leak(name: &str, body: &str) {
    let small = gc_numbers(&run_probe(name, body, 10_000));
    let large = gc_numbers(&run_probe(name, body, 20_000));
    let tb_small = small.1 - small.2;
    let tb_large = large.1 - large.2;
    let live_growth = large.0 - small.0;
    assert!(
        tb_small.abs() <= 8 && tb_large.abs() <= 8,
        "[{name}] 异常收支随 N 增长（泄漏时 ≈ N）：tb {tb_small}/{tb_large}"
    );
    assert!(
        live_growth.abs() <= 512,
        "[{name}] 常驻对象随 N 增长：live_after {} → {}（差 {live_growth}）",
        small.0,
        large.0
    );
}

const FSTAR: &str = r#"class Plain:
    pass

def f(*a, **k):
    return a

def call():
    try:
        f(*Plain())
    except TypeError:
        pass
"#;

const TUPLE: &str = r#"class Plain:
    pass

def call():
    try:
        tuple(Plain())
    except TypeError:
        pass
"#;

const SET: &str = r#"class Plain:
    pass

def call():
    try:
        set(Plain())
    except TypeError:
        pass
"#;

const FROZENSET: &str = r#"class Plain:
    pass

def call():
    try:
        frozenset(Plain())
    except TypeError:
        pass
"#;

const INT: &str = r#"class Plain:
    pass

def call():
    try:
        int(Plain())
    except TypeError:
        pass
"#;

const SORTED: &str = r#"class NoCompare:
    pass

def call():
    try:
        sorted([NoCompare(), NoCompare()])
    except TypeError:
        pass
"#;

const BAD_NEW: &str = r#"class Boom:
    def __new__(cls):
        raise ValueError("new boom")

def call():
    try:
        Boom()
    except ValueError:
        pass
"#;

const BAD_INIT: &str = r#"class Boom:
    def __init__(self):
        raise ValueError("init boom")

def call():
    try:
        Boom()
    except ValueError:
        pass
"#;

const BAD_EXIT: &str = r#"class CM:
    def __enter__(self):
        return self

    def __exit__(self, *args):
        raise ValueError("exit boom")

def call():
    try:
        with CM():
            pass
    except ValueError:
        pass
"#;

const BAD_DEL: &str = r#"class D:
    def __del__(self):
        raise ValueError("del boom")

def call():
    D()
"#;

const LAZY_ANNOTATIONS: &str = r#"import sys

def bad_annotate(*args, **kwargs):
    raise ValueError("annot boom")

sys.modules[__name__].__annotate_func__ = bad_annotate

def call():
    try:
        sys.modules[__name__].__annotations__
    except BaseException:
        pass
"#;

#[test]
fn noniterable_fstar_does_not_leak() {
    assert_probe_does_not_leak("noniterable-fstar", FSTAR);
}

#[test]
fn noniterable_tuple_does_not_leak() {
    assert_probe_does_not_leak("noniterable-tuple", TUPLE);
}

#[test]
fn noniterable_set_does_not_leak() {
    assert_probe_does_not_leak("noniterable-set", SET);
    assert_probe_does_not_leak("noniterable-frozenset", FROZENSET);
}

#[test]
fn noniterable_int_does_not_leak() {
    assert_probe_does_not_leak("noniterable-int", INT);
}

#[test]
fn sorted_compare_failure_does_not_leak() {
    assert_probe_does_not_leak("sorted-compare", SORTED);
}

#[test]
fn bad_new_does_not_leak() {
    assert_probe_does_not_leak("bad-new", BAD_NEW);
}

#[test]
fn bad_init_does_not_leak() {
    assert_probe_does_not_leak("bad-init", BAD_INIT);
}

#[test]
fn bad_exit_does_not_leak() {
    assert_probe_does_not_leak("bad-exit", BAD_EXIT);
}

#[test]
fn bad_del_does_not_leak() {
    assert_probe_does_not_leak("bad-del", BAD_DEL);
}

#[test]
fn lazy_annotations_does_not_leak() {
    assert_probe_does_not_leak("lazy-annotations", LAZY_ANNOTATIONS);
}
