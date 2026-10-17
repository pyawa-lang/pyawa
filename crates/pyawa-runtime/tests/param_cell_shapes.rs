//! **形参 cell 不得走 `STORE_FAST`／`LOAD_FAST` 快路**（task-1 ✓）。
//!
//! 形参被内层闭包捕获 ⇒ 是 **cell** ✓，但它**仍留在 `co_varnames`** 里（槽就是它的实参槽 ✓）⇒
//! 任何"这个名字在 `varnames` 里 ⇒ 发 `STORE_FAST`／`LOAD_FAST`"的快路都会把 localsplus 槽里的
//! **cell 对象**当普通值打掉 ✗。参照口径：对 cell 目标一律发 `STORE_DEREF`／`LOAD_DEREF` ✓
//! （本机 `python3` 3.14 `dis` 实测：`def f2(cls): a, cls = (7, 8)` ⇒ `STORE_FAST a` ＋
//! `STORE_DEREF cls` ✓，且**不融** `STORE_FAST_STORE_FAST` ✓）。
//!
//! 实测原形 ✓（`Lib/functools.py::singledispatch.register`）：`argname, cls = next(…)` 的 `cls`
//! 被内部 lambda 捕获 ⇒ 形参 cell ✓ ⇒ 先前被写成普通值（或写坏）⇒ `typing.get_type_hints` 拿到
//! **`<cell object at 0x…>`** ⇒ `importlib.resources` 一族报
//! `TypeError: <cell object at 0x…> does not have annotations` ✗。
//!
//! 五处口径（全在 `crates/pyawa-core/src/compile/emitter.rs` ✓）：
//! ① 元组解包的两名融合守卫（cell ⇒ 不融 ✓）② 元组解包逐目标存（cell ⇒ `STORE_DEREF` ✓）
//! ③ 赋值右值同名快路（cell ⇒ 落普通发射 ⇒ `LOAD_DEREF` ✓）④ `for` 目标（单名 ＋ 元组融合守卫 ✓）
//! ⑤ `emit_operand` 的 boolop 直接操作数快路（cell ⇒ `LOAD_DEREF` ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段脚本**取得 ✓（逐行一致 ✓）。

use pyawa_core::compile::{compile, CheckTier, Constant, Mode};

/// 跑一段脚本（CLI 子进程，工作目录在临时目录 ⇒ `__pyawa__` 缓存不串场 ✓）。
fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-param-cell-{}-{name}", std::process::id()));
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

/// 摊平一个编译单元的指令名序列（含嵌套 code 常量），按单元名分组 ✓。
fn opnames(source: &str) -> Vec<(String, Vec<String>)> {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编译");
    let mut out = Vec::new();
    collect(&unit, &mut out);
    out
}

fn collect(unit: &pyawa_core::compile::CompiledUnit, out: &mut Vec<(String, Vec<String>)>) {
    let mut ops = Vec::new();
    let mut offset = 0usize;
    while offset + 1 < unit.code.len() {
        let opcode = u16::from(unit.code[offset]);
        let name = pyawa_core::opcode::opname(opcode).expect("指令在表里").to_owned();
        let caches = pyawa_core::opcode::inline_cache_entries(opcode) as usize;
        ops.push(name);
        offset += 2 + 2 * caches;
    }
    out.push((unit.name.clone(), ops));
    for constant in &unit.constants {
        if let Constant::Code(inner) = constant {
            collect(inner, out);
        }
    }
}

fn ops_of<'a>(units: &'a [(String, Vec<String>)], name: &str) -> &'a [String] {
    units
        .iter()
        .find(|(unit, _)| unit == name)
        .map(|(_, ops)| ops.as_slice())
        .unwrap_or_else(|| panic!("没有名为 {name} 的编译单元（实有 {:?}）", units.iter().map(|(n, _)| n).collect::<Vec<_>>()))
}

fn count(ops: &[String], name: &str) -> usize {
    ops.iter().filter(|op| op.as_str() == name).count()
}

/// ① 形参 cell 被**元组解包**赋值 ＋ ② 形参 cell 单名赋值后经闭包读 ＋ ④ 形参 cell 作右值。
const UNPACK_AND_READ_SCRIPT: &str = r#"def tuple_unpack_cell(cls):
    def read(): return cls
    a, cls = (7, 8)
    return read(), cls, a


def assign_rhs_cell(cls):
    def read(): return cls
    p = cls
    return p, read()


def tuple_then_chain(cls):
    def read(): return cls
    x, cls = cls, 5
    return read(), cls, x


print(tuple_unpack_cell(1))
print(tuple_unpack_cell(2))
print(assign_rhs_cell(9))
print(tuple_then_chain(4))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const UNPACK_AND_READ_EXPECTED: &str = "(8, 8, 7)\n(8, 8, 7)\n(9, 9)\n(5, 5, 4)\n";

#[test]
fn unpack_and_rhs_cell_shapes_agree_with_the_reference() {
    let stdout = run("unpack_and_rhs_cell_shapes", UNPACK_AND_READ_SCRIPT);
    assert_eq!(stdout, UNPACK_AND_READ_EXPECTED, "{stdout}");
}

/// ③ `for` 目标 cell（单名 ⇒ `STORE_DEREF` ✓；元组目标里含 cell ⇒ 不融 ✓）。
const FOR_TARGET_SCRIPT: &str = r#"def for_target_cell():
    def read(): return y
    y = 1
    for y in range(3):
        pass
    return read()


def for_tuple_cell(cls):
    def read(): return cls
    for a, cls in [(1, 2), (3, 4)]:
        pass
    return read(), cls, a


print(for_target_cell())
print(for_tuple_cell(0))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const FOR_TARGET_EXPECTED: &str = "2\n(4, 4, 3)\n";

#[test]
fn for_target_cell_shapes_agree_with_the_reference() {
    let stdout = run("for_target_cell_shapes", FOR_TARGET_SCRIPT);
    assert_eq!(stdout, FOR_TARGET_EXPECTED, "{stdout}");
}

/// ⑤ 形参 cell 作 `and`／`or` 的**直接操作数**（`emit_operand` 快路 ⇒ 必须 `LOAD_DEREF` ✓）。
const BOOLOP_OPERAND_SCRIPT: &str = r#"def boolop_operand_cell(cls):
    def read(): return cls
    return (cls and 1), read()


def boolop_operand_false(cls):
    def read(): return cls
    return (cls or 2), read()


print(boolop_operand_cell(7))
print(boolop_operand_cell(0))
print(boolop_operand_false(0))
print(boolop_operand_false(3))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const BOOLOP_OPERAND_EXPECTED: &str = "(1, 7)\n(0, 0)\n(2, 0)\n(3, 3)\n";

#[test]
fn boolop_operand_cell_shapes_agree_with_the_reference() {
    let stdout = run("boolop_operand_cell_shapes", BOOLOP_OPERAND_SCRIPT);
    assert_eq!(stdout, BOOLOP_OPERAND_EXPECTED, "{stdout}");
}

/// ⑤ **防回退**：非 cell 的两名解包（赋值 ＋ `for`）**仍走融合路** `STORE_FAST_STORE_FAST` ✓。
const FUSION_GUARD_SCRIPT: &str = r#"def two_name_unpack(xs):
    a, b = xs
    return a, b


def two_name_for(pairs):
    for a, b in pairs:
        pass
    return a, b


print(two_name_unpack([3, 4]))
print(two_name_for([(1, 2), (5, 6)]))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const FUSION_GUARD_EXPECTED: &str = "(3, 4)\n(5, 6)\n";

#[test]
fn two_name_unpack_without_cells_agree_with_the_reference() {
    let stdout = run("two_name_unpack_without_cells", FUSION_GUARD_SCRIPT);
    assert_eq!(stdout, FUSION_GUARD_EXPECTED, "{stdout}");
}

/// 全部形态合成一段源码 ⇒ 逐单元断言**指令级口径**（与参照 `dis` 同一口径 ✓）。
fn all_units() -> Vec<(String, Vec<String>)> {
    opnames(&format!(
        "{UNPACK_AND_READ_SCRIPT}{FOR_TARGET_SCRIPT}{BOOLOP_OPERAND_SCRIPT}{FUSION_GUARD_SCRIPT}"
    ))
}

/// 相邻指令子序列（按名字）；`false` ＝ 该单元里**不出现**这条链 ✓。
fn has_run(ops: &[String], run: &[&str]) -> bool {
    ops.windows(run.len()).any(|window| window.iter().zip(run).all(|(op, want)| op == want))
}

/// **形参 cell 的目标一律 `STORE_DEREF`** ✓（元组解包 ＋ `for`），**且不与邻名融合** ✗。
#[test]
fn cell_targets_store_via_deref_and_never_fuse() {
    let units = all_units();
    // ①＋② `a, cls = (7, 8)`（`cls` 是形参 cell）⇒ `STORE_FAST a`；`STORE_DEREF cls`，不融 ✓
    let unpack = ops_of(&units, "tuple_unpack_cell");
    assert!(has_run(unpack, &["SWAP", "STORE_FAST", "STORE_DEREF"]), "{unpack:?}");
    assert_eq!(count(unpack, "STORE_FAST_STORE_FAST"), 0, "{unpack:?}");
    // ④ `for a, cls in […]`（`cls` 是 cell）⇒ 解包后逐名发，不融 ✓
    let for_tuple = ops_of(&units, "for_tuple_cell");
    assert!(
        has_run(for_tuple, &["UNPACK_SEQUENCE", "STORE_FAST", "STORE_DEREF"]),
        "{for_tuple:?}"
    );
    assert_eq!(count(for_tuple, "STORE_FAST_STORE_FAST"), 0, "{for_tuple:?}");
    // ③ `for cls in …`（单名 cell）⇒ `STORE_DEREF`；`y = 1` 也是 cell ⇒ 同样 ✓
    let for_single = ops_of(&units, "for_target_cell");
    assert!(has_run(for_single, &["FOR_ITER", "STORE_DEREF"]), "{for_single:?}");
    assert_eq!(count(for_single, "STORE_FAST_STORE_FAST"), 0, "{for_single:?}");
}

/// **形参 cell 作右值一律 `LOAD_DEREF`** ✓（③ 赋值右值 ＋ ⑤ boolop 直接操作数）。
#[test]
fn cell_reads_load_via_deref() {
    let units = all_units();
    // ③ `p = cls` ⇒ `LOAD_DEREF cls; STORE_FAST p`（先前是 `LOAD_FAST` ⇒ 压上 cell 对象 ✗）
    let assign = ops_of(&units, "assign_rhs_cell");
    assert!(has_run(assign, &["LOAD_DEREF", "STORE_FAST"]), "{assign:?}");
    assert_eq!(count(assign, "LOAD_FAST"), 0, "{assign:?}");
    // ⑤ `(cls and 1)` 的**直接操作数**走 `emit_operand` 快路 ⇒ 仍须 `LOAD_DEREF` ✓
    let and_op = ops_of(&units, "boolop_operand_cell");
    assert!(has_run(and_op, &["LOAD_DEREF", "COPY", "TO_BOOL"]), "{and_op:?}");
    assert_eq!(count(and_op, "LOAD_FAST"), 0, "{and_op:?}");
    let or_op = ops_of(&units, "boolop_operand_false");
    assert!(has_run(or_op, &["LOAD_DEREF", "COPY", "TO_BOOL"]), "{or_op:?}");
    assert_eq!(count(or_op, "LOAD_FAST"), 0, "{or_op:?}");
}

/// **防回退** ✗：非 cell 的两名解包（赋值 ＋ `for`）**仍须融合** `STORE_FAST_STORE_FAST` ✓。
#[test]
fn non_cell_two_name_targets_still_fuse() {
    let units = all_units();
    for name in ["two_name_unpack", "two_name_for"] {
        let ops = ops_of(&units, name);
        assert!(has_run(ops, &["UNPACK_SEQUENCE", "STORE_FAST_STORE_FAST"]), "{ops:?}");
        assert_eq!(count(ops, "STORE_DEREF"), 0, "{ops:?}");
    }
}
