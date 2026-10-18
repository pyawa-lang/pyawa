//! `executor` 的 `step_misc` 域：杂项（空操作、`CACHE`、出栈、小整数）。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_misc<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "RESUME" | "NOP" => {}
                // `BC-23`：边界检查的两条**专有**指令（`TS-10`…`TS-13`）
                "POP_TOP" => {
                    if crate::diag::flag("PYAWA_POP_DEBUG") {
                        eprintln!("[pop_top] 弹前深={} site={}", frame.get().depth(), instance.current_site());
                    }
                    release(instance, frame.get().pop()?)
                }
                "NOT_TAKEN" => {
                    // §10 三分类②：参照实现**会发**这条（跟在 `POP_JUMP_*` 之后），
                    // 但它是给专门化解释器用的提示；VM **必须容受**它（净 0，什么也不做）。
                }
                "LOAD_SMALL_INT" => {
                    // 3.14 的新指令：直接把 oparg 当小整数压栈（不走常量表）。实测效果 +1。
                    push_int(instance, frame.get(), oparg as i64)?;
                }
                "CACHE" => {}
        _ => unreachable!("step_misc 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
