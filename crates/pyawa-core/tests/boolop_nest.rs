//! **嵌套 boolop 的跳转标签**（task-6）：`emit_test_value` 的 `BoolOp` 分支。
//!
//! 原 bug（编译期 Rust panic ✗）：`a and (b and c) or d` ⇒
//! `跳转目标标签 2 从未落点（emitter.rs:5778）` ⇒ `signal.py` 编不了 ⇒ `multiprocessing` 26 个卡住。
//! 根因：该分支收 `cleanup` 形参却**从不落点它** ✗ —— 本式作为外层的**末操作数**时外层传进来的
//! 标签被吞 ✗。顺带实测出**同极性嵌套的语义错**（不 panic 但值错 ✗）：`(a and b) and c` 在 `a` 假时
//! 错返回 `c`；`a and (b and c) and d`／`a or (b and c) and d`／`(a or b) or c` 同理。
//!
//! 修法（一处，照参照 `dis` 实测 ✓）：
//! - 落点用传入的 `cleanup`（没有才新开 `fresh`）✓；
//! - 非末操作数的短路口：**本子式短路极性 == 外层要求的极性** ⇒ 直接跳 `target`（保持值 ✓），
//!   否则跳 `fresh`（= `cleanup`）✓。
//!
//! ⚠ **必须用名字**：字面量会被常量折叠绕开 ⇒ 测不到（实测）。

use pyawa_core::compile::{compile, CheckTier, Constant, Mode};

/// 触发矩阵（`target/recon/jump_label_A.md` §4）＋ 同极性嵌套补充。表达式一律用**名字** ✓。
const SHAPES: &[(&str, &str, &str)] = &[
    // 触发的 3 形态（修前：编译 panic ✗）
    ("T1", "a and (b and c) or d", "a, b, c, d"),
    ("T2", "a and (b or c) or d", "a, b, c, d"),
    ("T3", "a and (b and c) or d and e", "a, b, c, d, e"),
    // 不触发的 7 形态（参照矩阵；其中 N5／N6 修前**语义错** ✗）
    ("N1", "a and (b and c)", "a, b, c"),
    ("N2", "a and (b or c)", "a, b, c"),
    ("N3", "(a and b) or c", "a, b, c"),
    ("N4", "a and b and (c and d)", "a, b, c, d"),
    ("N5", "a and (b and c) and d", "a, b, c, d"),
    ("N6", "a or (b and c) and d", "a, b, c, d"),
    ("N7", "(a or b) and c", "a, b, c"),
    // 同极性嵌套补充（修前语义错 ✗）
    ("X1", "(a and b) and c", "a, b, c"),
    ("X2", "(a or b) or c", "a, b, c"),
    ("X3", "(a and b) or c or d", "a, b, c, d"),
    ("X4", "a or (b or c)", "a, b, c"),
];

fn source_for(shape: &str, params: &str) -> String {
    format!("def f({params}):\n    return {shape}\n")
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
        .unwrap_or_else(|| {
            panic!(
                "没有名为 {name} 的编译单元（实有 {:?}）",
                units.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )
        })
}

/// 所有形态都必须**编译成功** ✓（修前 T1／T2／T3 编译期 panic ✗）。
#[test]
fn every_boolop_nesting_shape_compiles() {
    for (name, shape, params) in SHAPES {
        let source = source_for(shape, params);
        let outcome = std::panic::catch_unwind(|| {
            compile(&source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
        });
        assert!(outcome.is_ok(), "{name}：`{shape}` 编译入口 panic 了（标签必须全部落点）");
        assert!(outcome.unwrap().is_ok(), "{name}：`{shape}` 应编译成功");
    }
}

/// **跳转极性序列**逐形态钉住（与参照 `dis` 同一口径 ✓）：
/// 触发的 3 形态分别是 `[F, F, T]`／`[F, T, T]`／`[F, F, T, F]`（最后一个是 `d and e` 的 `d`）。
#[test]
fn boolop_jump_polarities_match_the_reference() {
    let expected: &[(&str, &[&str])] = &[
        ("T1", &["FALSE", "FALSE", "TRUE"]),
        ("T2", &["FALSE", "TRUE", "TRUE"]),
        ("T3", &["FALSE", "FALSE", "TRUE", "FALSE"]),
        ("N1", &["FALSE", "FALSE"]),
        ("N2", &["FALSE", "TRUE"]),
        ("N3", &["FALSE", "TRUE"]),
        ("N4", &["FALSE", "FALSE", "FALSE"]),
        ("N5", &["FALSE", "FALSE", "FALSE"]),
        ("N6", &["TRUE", "FALSE", "FALSE"]),
        ("N7", &["TRUE", "FALSE"]),
        ("X1", &["FALSE", "FALSE"]),
        ("X2", &["TRUE", "TRUE"]),
        ("X3", &["FALSE", "TRUE", "TRUE"]),
        ("X4", &["TRUE", "TRUE"]),
    ];
    for (name, shape, params) in SHAPES {
        let source = source_for(shape, params);
        let units = opnames(&source);
        let ops = ops_of(&units, "f");
        let got: Vec<String> = ops
            .iter()
            .filter(|op| op.starts_with("POP_JUMP_IF_"))
            .map(|op| op.trim_start_matches("POP_JUMP_IF_").to_owned())
            .collect();
        let want = expected
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, want)| *want)
            .unwrap_or_else(|| panic!("{name} 没有登记期望极性"));
        assert_eq!(got, want, "{name}：`{shape}` 的跳转极性序列\n完整指令：{ops:?}");
    }
}
