//! **编译产物不变量检查** ✓（第 207 轮）。
//!
//! 由来（**一处真相** ✓）：口径不一致这类 bug —— 符号表说"3 个局部槽"、发射器却写到第 4 个 ——
//! 先前只有**运行到那一步**才炸（`frame.rs` 报 `SlotOutOfRange` ✗），于是像 `Lib/os.py` 这样的大文件
//! 要一路逼近才能发现 ✓。这里把"发射器**实际**写出的操作数"与"符号表**声明**的容量"当场对起来 ✓。
//!
//! **指令怎么走**：用本仓**自己的解码器** ✓（`decode::Decoder` ✓，它已经把 `EXTENDED_ARG` 前缀
//! 与 cache 槽折进 `size` ✓、`jump_target()` 已按 `BC-55` 算好 ✓）—— 不另写一份走路器 ✓
//!（第 207 轮教训：手写走路器在**类体／生成式**上错位 ✗ ⇒ 满屏**假警报** ✗）。
//!
//! 种类**从表里取** ✓（`opcode_metadata` 的 `HAS_LOCAL`／`HAS_FREE`／`HAS_CONST` ✓）。
//!
//! 何时跑：`cfg(debug_assertions)`（即 `cargo test` ✓）**或** `PYAWA_COMPILE_CHECK=1` ✓。

use super::{CompileError, CompiledUnit};
use crate::code::{localsplus_kinds_from, SlotKind};
use crate::decode::Decoder;
use crate::opcode_metadata::{HAS_CONST, HAS_FREE, HAS_LOCAL};

/// 要不要跑检查 ✓。
pub(super) fn enabled() -> bool {
    cfg!(debug_assertions) || std::env::var_os("PYAWA_COMPILE_CHECK").is_some()
}

/// 跑一遍检查 ✓；不成立就返回 `CompileError::Unsupported` —— **报错而不是 panic** ✓：
/// 这样"全量扫 `Lib/`"能一次收集**所有**坏文件 ✓。
pub(super) fn check(unit: &CompiledUnit) -> Result<(), CompileError> {
    if !enabled() {
        return Ok(());
    }
    if let Err(why) = walk(unit) {
        // **元数据一起报** ✓（第 210 轮）：`nlocals`／`varnames`／`cellvars`／`freevars` 是定位
        // "差一" 这类账的关键 ✓（只报现象会让人反复追 ✗）。
        return Err(CompileError::Unsupported(format!(
            "内部不变量（编译期检查）：作用域 `{}` {why}\n  nlocals={} varnames={:?} cellvars={:?} freevars={:?}",
            unit.name,
            unit.nlocals,
            unit.varnames,
            unit.cellvars,
            unit.freevars
        )));
    }
    Ok(())
}

/// **配对指令**：oparg 是"两个 4 位局部下标" ✓（第 207 轮实测：当单下标读会把 `0x41` 看成槽 65 ✗）。
/// 判据**从名字推导** ✓（不列清单 ✗——3.14 的 `LOAD_FAST_BORROW_LOAD_FAST_BORROW` 就是列清单漏掉的 ✗）：
/// 名字里出现**两个** `FAST` 才算配对 ✓。
fn is_paired_local(name: &str) -> bool {
    name.matches("FAST").count() >= 2
}

fn walk(unit: &CompiledUnit) -> Result<(), String> {
    if unit.code.len() % 2 != 0 {
        return Err(format!("码元数不是偶数（{} 字节）", unit.code.len()));
    }
    let units = unit.code.len() / 2;
    // **`localsplus`** ✓：局部在前、cell 居中、free 在后（`MAKE_CELL`／`LOAD_DEREF` 的 oparg 落在这条数组上 ✓）。
    // **一条上界：`localsplus`** ✓（第 207 轮实测两回才定下来 ✗）：3.14 的 `LOAD_FAST*` 的 oparg
    // 同样落在这条数组上 ✓ —— 它**可以指向 cell**（实测夹具 `def a(): x = 1; def b(): def c():
    // return x`：作用域 `b` 里 `LOAD_FAST_BORROW 1` ✓ 合法，`nlocals` 却只有 1 ✓）。
    // 先前按"`CO_OPTIMIZED` ⇒ 只碰真局部"判 ⇒ **假警报** ✗（把一条与参照**逐字节相同**的夹具打红 ✗）。
    // ⇒ 统一用 `nlocals + cellvars + freevars` ✓；`*_DEREF` 一族也在这条数组上 ✓ ⇒ 同一把尺 ✓。
    // **`localsplus` 的真实宽度** ✓（第 209 轮修 ✗）：`cellvars` 里**已经是形参**的那些**复用**它
    // 的 `varnames` 槽 ✓、**不**加宽数组 ⇒ 直接按 `nlocals + cellvars + freevars` 算会**算大** ✗
    // ⇒ 真 bug 从指缝漏过去 ✗（本轮实测：`os.py` 就是这么漏的 ✓）。规矩与 [`crate::CodeObject::localsplus_kinds`]
    // **同一条** ✓（一处真相 ✓）：只有**非形参**的 cell 才追加 ✓。
    let parameter_cells = unit
        .cellvars
        .iter()
        .filter(|name| unit.varnames.contains(name))
        .count();
    let appended_cells = unit.cellvars.len() - parameter_cells;
    let plus_limit = unit.nlocals + appended_cells + unit.freevars.len();
    let local_limit = plus_limit;
    let const_limit = unit.constants.len();
    // **最终布局** ✓（与运行期帧**同一处真相** ✓）。
    let kinds = localsplus_kinds_from(
        unit.nlocals,
        &unit.varnames,
        &unit.cellvars,
        unit.freevars.len(),
    );
    let mut decoder = Decoder::new(&unit.code);
    while let Some(instruction) = decoder
        .next_instruction()
        .map_err(|error| format!("解码失败：{error:?}"))?
    {
        let op = u16::from(instruction.opcode);
        let arg = instruction.oparg as usize;
        let name = crate::opcode::opname(op).unwrap_or("<未知>");
        // **`localsplus` 一族**（`*_DEREF`／`MAKE_CELL` …）用 `localsplus` 上界 ✓、真局部用 `local_limit` ✓。
        // 判据**看名字** ✓：表里 `HAS_LOCAL` 同时挂着 `*_DEREF` ✗、而 `HAS_FREE` 又不含它们 ✗
        //（第 207 轮实测 ✓）⇒ 与其猜表的边界，不如按 opcode 名字分 ✓（`DEREF`／`CELL` 即 cell／free ✓）。
        if HAS_LOCAL.contains(&op) || HAS_FREE.contains(&op) {
            let is_cell = name.contains("DEREF") || name.contains("CELL");
            let limit = if is_cell { plus_limit } else { local_limit };
            let too_big = if is_cell {
                arg >= limit
            } else {
                let paired = is_paired_local(name);
                let first = if paired { arg & 0xF } else { arg };
                let second = (arg >> 4) & 0xF;
                // **与运行期 `Frame::local()` 对齐** ✓（第 266 轮）：普通局部槽要 `< nlocals` ✓，
                // **或者**这一格本身是 cell／free ✓（那时运行期走 `cell_at` ✓ —— 实测夹具里
                // `LOAD_FAST_BORROW <cell 槽>` 合法 ✓）。先前一律拿 `localsplus` 当上界 ✗ ⇒
                // 「槽号在范围内、却既不是局部也不是 cell」这类**漏过去** ✗（实测 `import os` 的
                // `SlotOutOfRange { slot: 5, count: 5 }` 就是它 ✓）。
                let bad = |slot: usize| {
                    slot >= unit.nlocals
                        && !matches!(kinds.get(slot), Some(SlotKind::Cell) | Some(SlotKind::Free))
                };
                bad(first) || (paired && bad(second))
            };
            if too_big {
                return Err(format!(
                    "槽位越界：码元 {} 的 {name} 要槽 {arg}{}，而{}只有 {limit}",
                    instruction.offset,
                    if is_cell { "" } else { "（配对指令：两个 4 位下标都要查）" },
                    if is_cell { " localsplus " } else { " `nlocals` " }
                ));
            }
        }
        // **cell／free 指令必须落在 Cell／Free 格上** ✓（第 261 轮真 bug ✗）：槽号落在**范围内**、
        // 却指着 `Local` 格 ⇒ 运行期走 `Frame::set_cell()` ⇒ `SlotOutOfRange` ✗（实测
        // `Lib/os.py` 的 `_create_environ_mapping`：cell 在 5／6／7，发射器却发了 **4** ✗）。
        if name.contains("DEREF") || name.contains("CELL") {
            let kind = kinds.get(arg);
            if !matches!(kind, Some(SlotKind::Cell) | Some(SlotKind::Free)) {
                return Err(format!(
                    "cell／free 指令落错格：码元 {} 的 {name} 要槽 {arg}，而那一格是 {:?} ✗",
                    instruction.offset, kind
                ));
            }
        }
        if HAS_CONST.contains(&op) && arg >= const_limit {
            return Err(format!(
                "常量下标越界：码元 {} 的 {name} 要常量 {arg}，而常量表只有 {const_limit} 项",
                instruction.offset
            ));
        }
        if let Some(target) = instruction.jump_target() {
            if target > units {
                return Err(format!(
                    "跳转目标越界：码元 {} 的 {name} 跳到 {target}（共 {units} 码元）",
                    instruction.offset
                ));
            }
        }
    }
    Ok(())
}
