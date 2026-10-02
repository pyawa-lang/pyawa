//! `_opcode` 的 Rust 侧实现（`docs/SPEC-bytecode.md` §2.1、§8；`BC-27`／`BC-30`…`BC-42`）。
//!
//! 纯函数 ＋ 纯数据：**不依赖**其他 crate，也不碰 `std` 的平台能力（`CX-4`）。
//! 表与数值的唯一出处在 [`crate::opcode_metadata`]（`BC-38`）；本 crate 即指令表的归属 crate。

use crate::opcode_metadata::{
    self as metadata, StackRule, HAS_ARG, HAS_CONST, HAS_EXC, HAS_FREE, HAS_JUMP, HAS_LOCAL,
    HAS_NAME, INLINE_CACHE_ENTRIES, OPMAP, OPNAME, STACK_EFFECTS, STACK_EFFECTS_NOT_JUMP,
};

/// `BC-38`：`stack_effect` 的失败形态；Python 层将来映射为
/// `ValueError("invalid opcode or oparg")`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackEffectError {
    /// 编号不在指令表里（含空隙编号：`BC-30` 的基线表里没有它）。
    UnknownOpcode(u16),
    /// 规则计算溢出 `i32`。CPython 对极端 oparg 也会拒绝，但**各指令的边界不一致**
    /// （`CALL` 在 `i32::MAX - 1` 就报错，`BUILD_TUPLE` 到 `i32::MAX` 仍接受），
    /// 这里只保证"溢出即报错"，不逐点对齐那条边界。
    OpargOutOfRange(i64),
}

/// `_opcode.stack_effect(opcode, oparg=None, *, jump=None)`。
///
/// - `oparg = None` 与 `oparg = 0` 等价（实测 CPython 3.14.4 的行为）
/// - `jump = None` 与 `jump = Some(true)` 同义；只有 `SETUP_*` 一族在
///   `jump = Some(false)` 时取另一条规则（`SPEC-bytecode.md` §8.4）
/// - `BC-27`：`BC-23` 的两条 Pyawa 专有指令（`CHECK_BOUNDARY_IN`／`CHECK_BOUNDARY_OUT`）
///   也在表里，返回 `0`——它们只读类型元数据，不动值栈
pub fn stack_effect(
    opcode: u16,
    oparg: Option<i64>,
    jump: Option<bool>,
) -> Result<i32, StackEffectError> {
    let index = STACK_EFFECTS
        .binary_search_by_key(&opcode, |(op, _)| *op)
        .map_err(|_| StackEffectError::UnknownOpcode(opcode))?;
    let mut rule = STACK_EFFECTS[index].1;

    if jump == Some(false) {
        if let Ok(position) = STACK_EFFECTS_NOT_JUMP.binary_search_by_key(&opcode, |(op, _)| *op) {
            rule = STACK_EFFECTS_NOT_JUMP[position].1;
        }
    }

    let oparg = oparg.unwrap_or(0);
    evaluate(rule, oparg).ok_or(StackEffectError::OpargOutOfRange(oparg))
}

fn evaluate(rule: StackRule, oparg: i64) -> Option<i32> {
    let value = match rule {
        StackRule::Const(value) => i64::from(value),
        StackRule::Linear { factor, base } => {
            oparg.checked_mul(i64::from(factor))?.checked_add(i64::from(base))?
        }
        StackRule::Parity { even, odd } => {
            i64::from(if oparg % 2 == 0 { even } else { odd })
        }
        // BC-38／UNPACK_EX：低 8 位是前置个数、高 8 位是后置个数
        StackRule::UnpackEx => (oparg & 0xFF) + (oparg >> 8),
    };
    i32::try_from(value).ok()
}

/// `BC-37`：`has_arg`。
pub fn has_arg(op: u16) -> bool {
    HAS_ARG.binary_search(&op).is_ok()
}

/// `BC-37`：`has_const`。
pub fn has_const(op: u16) -> bool {
    HAS_CONST.binary_search(&op).is_ok()
}

/// `BC-37`：`has_name`。
pub fn has_name(op: u16) -> bool {
    HAS_NAME.binary_search(&op).is_ok()
}

/// `BC-37`：`has_jump`。
pub fn has_jump(op: u16) -> bool {
    HAS_JUMP.binary_search(&op).is_ok()
}

/// `BC-37`：`has_free`。
pub fn has_free(op: u16) -> bool {
    HAS_FREE.binary_search(&op).is_ok()
}

/// `BC-37`：`has_local`。
pub fn has_local(op: u16) -> bool {
    HAS_LOCAL.binary_search(&op).is_ok()
}

/// `BC-37`：`has_exc`。
pub fn has_exc(op: u16) -> bool {
    HAS_EXC.binary_search(&op).is_ok()
}

/// 名字 → 编号（`BC-30`：与 CPython 3.14 的 `opmap` 全等）。
pub fn opcode(name: &str) -> Option<u16> {
    OPMAP
        .binary_search_by(|(candidate, _)| (*candidate).cmp(name))
        .ok()
        .map(|index| OPMAP[index].1)
}

/// 编号 → 名字。
pub fn opname(op: u16) -> Option<&'static str> {
    OPNAME
        .binary_search_by_key(&op, |(candidate, _)| *candidate)
        .ok()
        .map(|index| OPNAME[index].1)
}

/// `BC-35`：该指令的 inline cache 宽度（码元数）；无 cache 的指令为 0。
pub fn inline_cache_entries(op: u16) -> u32 {
    INLINE_CACHE_ENTRIES
        .binary_search_by_key(&op, |(candidate, _)| *candidate)
        .map(|index| INLINE_CACHE_ENTRIES[index].1)
        .unwrap_or(0)
}

/// `BC-31`：Pyawa 专有指令的编号（名字，编号）；`BC-32` 要求它们**不**出现在 `OPMAP` 里。
pub fn pyawa_specific(name: &str) -> Option<u16> {
    metadata::PYAWA_SPECIFIC
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, op)| *op)
}

/// `_opcode.get_nb_ops()`：`BC-39` 的 `BINARY_OP` oparg 顺序。
/// `BC-39`：`COMPARE_OP` 的 oparg 对应的六元组（顺序即 oparg）。
pub fn get_cmp_op() -> &'static [&'static str] {
    crate::opcode_metadata::CMP_OP
}

/// `BC-39`：`BINARY_OP` 的 oparg 顺序。
pub fn get_nb_ops() -> &'static [(&'static str, &'static str)] {
    metadata::NB_OPS
}

/// `_opcode.get_intrinsic1_descs()`。
pub fn get_intrinsic1_descs() -> &'static [&'static str] {
    metadata::INTRINSIC1_DESCS
}

/// `_opcode.get_intrinsic2_descs()`。
pub fn get_intrinsic2_descs() -> &'static [&'static str] {
    metadata::INTRINSIC2_DESCS
}

/// `_opcode.get_special_method_names()`。
pub fn get_special_method_names() -> &'static [&'static str] {
    metadata::SPECIAL_METHOD_NAMES
}

/// `dis.py` 读它取 executor。Pyawa **无 JIT**，且 `BC-32` 禁止发射 `ENTER_EXECUTOR`，
/// 故**恒**返回 `None`；参数形态等 Python 桥接层落地时再定（现为占位）。
pub fn get_executor<T>(_code: &T, _offset: i64) -> Option<()> {
    None
}
