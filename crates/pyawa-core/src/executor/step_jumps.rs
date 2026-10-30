//! `executor` 的 `step_jumps` 域：跳转与条件跳转。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_jumps<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, decoder: &mut Decoder, instruction: &Instruction, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "JUMP_FORWARD" | "JUMP_BACKWARD" | "JUMP_BACKWARD_NO_INTERRUPT" => {
                    let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BC-55：这条指令没有跳转目标",
                    })?;
                    decoder.set_position(target);
                }
                "POP_JUMP_IF_TRUE" | "POP_JUMP_IF_FALSE" => {
                    let value = frame.get().pop()?;
                    let truth = truthiness(instance, value, opcode_number);
                    release(instance, value);
                    let jump = truth? == (name == "POP_JUMP_IF_TRUE");
                    if jump {
                        let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "BC-55：这条指令没有跳转目标",
                        })?;
                        decoder.set_position(target);
                    }
                }
                "POP_JUMP_IF_NONE" | "POP_JUMP_IF_NOT_NONE" => {
                    let value = frame.get().pop()?;
                    // SAFETY: value 是刚出栈的存活对象。
                    let is_none = unsafe { value.as_ref() }.ty() == instance.singletons().none_type();
                    release(instance, value);
                    if is_none == (name == "POP_JUMP_IF_NONE") {
                        let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "BC-55：这条指令没有跳转目标",
                        })?;
                        decoder.set_position(target);
                    }
                }
        _ => unreachable!("step_jumps 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
