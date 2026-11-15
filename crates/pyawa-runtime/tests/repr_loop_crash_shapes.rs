//! **`P3-41` 官方套件的崩溃面**（2026-10-31 第 1 轮 ✓）：两处**不得中止**的形态。
//!
//! ① **深嵌套容器的 `repr`** ✗✗：`Lib/test/mapping_tests.py::test_repr_deep`（`test_frame`
//!    的 `FrameLocalsProxyMappingTests` 走它 ✓）先建 `support.exceeds_recursion_limit()` ＝
//!    **150,000** 层嵌套 `dict`，再断言 `repr` 报 `RecursionError` ✓。本层的 `repr` 是原生递归 ✗
//!    ⇒ 先前直接顶穿 8 MiB 原生栈、进程 `SIGABRT`（exit 134 ✗，实测 29.85–34 s）。
//!    修法：`OM-11` 的递归守卫**同时当深度计**（额度＝`sys.getrecursionlimit()`，与参照的
//!    `Py_EnterRecursiveCall` 同口径 ✓）⇒ 超限报 `RecursionError` ✓，并改 **RAII** 摘守卫
//!    （`?` 提前返回不再把地址留在守卫里 ✓）。**如实**：本层额度默认 64（参照 1000）；
//!    所以本测试**不与参照对拍** 200 层的结论，只钉"必定 `RecursionError`、不中止" ✓。
//!
//! ② **`for` 体末尾 `if …: break` 前面还夹着 `try`** ✗：`try` 的余部重放会让末尾那条 `if`
//!    走**非反转**形态（不回发回边 ✗），而循环臂按"末尾 if 会代发回边"**让位** ✗ ⇒ 整段
//!    **没有回边** ⇒ 第二圈 `FOR_ITER` 弹空栈 ⇒ `帧操作失败：StackUnderflow` ✗（实测原形
//!    `Lib/unittest/case.py::assertSequenceEqual` 的循环 ✓ ⇒ 任何**列表**断言失败都中止整个
//!    测试文件 ✗；最小复现 `target/recon/p341/r/r2.py` ✓）。修法：只有"末尾 `if` 真被直接发出"
//!    时才让位 ✓。本测试用**参照 python3 3.14 实跑**的输出逐行钉住 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-repr-loop-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 深嵌套 `dict`／`list`／`tuple`／`frozenset` 的 `repr` ⇒ `RecursionError`，进程不中止。
const DEEP_REPR_SCRIPT: &str = r#"d = {}
for _ in range(200):
    previous = d
    d = {}
    d[1] = previous
try:
    repr(d)
except RecursionError:
    print("dict: caught")
else:
    print("dict: no-error")

items = []
for _ in range(200):
    items = [items]
try:
    repr(items)
except RecursionError:
    print("list: caught")
else:
    print("list: no-error")

values = ()
for _ in range(200):
    values = (values,)
try:
    repr(values)
except RecursionError:
    print("tuple: caught")
else:
    print("tuple: no-error")

frozen = frozenset()
for _ in range(200):
    frozen = frozenset((frozen,))
try:
    repr(frozen)
except RecursionError:
    print("frozenset: caught")
else:
    print("frozenset: no-error")

print(repr({1: {2: 3}}))
print(repr([[[1]]]))
"#;

/// 浅层容器照旧（递归守卫不能把正常 `repr` 也挡掉）。
const DEEP_REPR_EXPECTED: &str = "dict: caught\nlist: caught\ntuple: caught\nfrozenset: caught\n{1: {2: 3}}\n[[[1]]]\n";

#[test]
fn deep_container_repr_raises_recursion_error_instead_of_aborting() {
    assert_eq!(run("deep", DEEP_REPR_SCRIPT), DEEP_REPR_EXPECTED);
}

/// `for` 体末尾 `if …: break`，前面两个 `try`（照 `assertSequenceEqual` 的形状）；
/// 外加 `while` 的同形（同一个让位判据 ✓）。
const LOOP_TAIL_IF_SCRIPT: &str = r#"def f(seq1, seq2):
    out = 0
    for i in range(min(len(seq1), len(seq2))):
        try:
            a = seq1[i]
        except (TypeError, IndexError):
            out += 100
            break
        try:
            b = seq2[i]
        except (TypeError, IndexError):
            out += 200
            break
        if a != b:
            out += 1
            break
    return out


def w(limit):
    out = 0
    i = 0
    while i < limit:
        try:
            value = i
        except (TypeError, IndexError):
            out += 100
            break
        i += 1
        if value == 2:
            out += 1
            break
    return out


print(f([1, 2, 3], [1, 9, 3]))
print(f([1, 2], [1, 2]))
print(f([1], [1, 2]))
print(w(5), w(2), w(0))
"#;

/// 参照：本机 `python3` 3.14 跑同一段 ⇒ `1`／`0`／`0`／`1 0 0`。
const LOOP_TAIL_IF_EXPECTED: &str = "1\n0\n0\n1 0 0\n";

#[test]
fn loop_tail_if_after_try_keeps_its_back_edge() {
    assert_eq!(run("loop", LOOP_TAIL_IF_SCRIPT), LOOP_TAIL_IF_EXPECTED);
}
