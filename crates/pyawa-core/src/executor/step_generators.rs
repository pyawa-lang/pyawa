//! `executor` 的 `step_generators` 域：返回／让出／生成器创建。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_generators<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, code: &CodeObject, instruction: &Instruction, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "RETURN_VALUE" => {
                    let returned = frame.get().pop()?;
                    // **返回现场诊断** ✓（第 349 轮，门控 `PYAWA_RETURN_DEBUG=1`）：只看 `_find_new_` ✓，
                    // 用来分辨"没走到 return"✗ 与"调用机制把返回值丢了"✗（第 348 轮的设计 ✓）。
                    // **NULL 返回值诊断** ✓（第 628 轮，门控 `PYAWA_NULL_TRACE=1`）：谁把 VM 的 NULL 哨兵当值
                    // 返回了 ✓ —— 比"解包时才发现 NULL"近一步 ✓（`Lib/re/_compiler.py:541` 的 NULL 就是它 ✓）。
                    if crate::diag::flag("PYAWA_NULL_TRACE")
                        && returned == instance.singletons().null()
                    {
                        eprintln!(
                            "[null-return] code={} site={}\n{}",
                            code.name(),
                            instance.current_site(),
                            std::backtrace::Backtrace::force_capture()
                        );
                    }
                    if crate::diag::flag("PYAWA_RETURN_DEBUG") && code.name() == "_find_new_" {
                        // SAFETY: returned 由本帧值栈持有，存活。
                        let tn = unsafe { (&*returned.as_ptr()).ty().as_ref() }.name().to_owned();
                        eprintln!("[return] _find_new_ -> {tn} site={}", instance.current_site());
                    }
                    return Ok(Step::Return(value_from_raw(instance, returned)));
                }
                "YIELD_VALUE" => {
                    // 实测骨架：`YIELD_VALUE` 之后是 `RESUME`／`POP_TOP`。让出时把值栈交给
                    // 帧的恢复点（`BC-47`），并让 ip 指向**下一条**指令——恢复就从那里继续。
                    let value = frame.get().pop()?;
                    frame
                        .get()
                        .set_instruction_pointer(instruction.offset + instruction.size);
                    frame.get().suspend()?;
                    return Ok(Step::Yield(value));
                }
                "RETURN_GENERATOR" => {
                    // 本层的 `CALL` 在见到 `CO_GENERATOR` 时**已经**把帧包成生成器了，
                    // 所以这条指令在恢复执行时是空操作（它只负责"造并返回生成器"那一半）。
                }
        _ => unreachable!("step_generators 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
