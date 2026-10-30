//! `executor` 的 `step_checks` 域：边界检查与赋值检查（`BC-23`／`BC-65`）。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_checks<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "CHECK_BOUNDARY_IN" | "CHECK_BOUNDARY_OUT" => {
                    boundary_check(instance, frame.get(), oparg as u8, opcode_number)?;
                }
                // `BC-65`：变量赋值的运行期检查（`TS-47`）——读栈顶**不**弹。
                // oparg 是常量表下标：按 `usize` 传（`EXTENDED_ARG` 折出来的长下标不截断）。
                "CHECK_ASSIGNMENT" => {
                    assignment_check(instance, frame.get(), oparg, opcode_number)?;
                }
        _ => unreachable!("step_checks 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
