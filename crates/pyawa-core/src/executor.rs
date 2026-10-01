//! 执行器：`SPEC-bytecode.md` §10 的**起步指令集**（`BC-49`）。
//!
//! **本片覆盖到哪**（其余一律返回 [`ExecError::NotImplemented`]，**不静默跳过**）：
//!
//! - 常量与局部：`RESUME`、`NOP`、`LOAD_CONST`、`LOAD_FAST`、`LOAD_FAST_CHECK`、
//!   `STORE_FAST`、`DELETE_FAST`、`POP_TOP`
//! - 运算符：`BINARY_OP`（只做整数能做的几项，见下）、`UNARY_NEGATIVE`、`UNARY_NOT`、
//!   `UNARY_INVERT`、`TO_BOOL`、`COMPARE_OP`（六元组，顺序取自 `opcode.cmp_op`）、`IS_OP`
//! - 控制流：`JUMP_FORWARD`、`JUMP_BACKWARD`、`JUMP_BACKWARD_NO_INTERRUPT`、
//!   `POP_JUMP_IF_TRUE`／`_FALSE`／`_NONE`／`_NOT_NONE`（目标按 **`BC-55`** 算）
//! - 返回：`RETURN_VALUE`
//!
//! **尚未接线**：`FOR_ITER`（要迭代器协议）、调用、容器、属性与下标、异常、生成器。
//!
//! **一处临时口径**（`OM-11` 的槽位接线后应改走协议）：判定"这是不是整数"按**类型身份**，
//! 不走协议。`TS-40` 的 `bool ⊂ int` **已接线**：`True + 1` 算 2、`-True` 算 −1
//! （两种载荷分开读，布局不同，不能互相强转）。
//!
//! 整数的**值域**：结果必须落在单例区间内；超出一律 [`ExecError::IntOutOfRange`]——
//! 大整数对象随 `SPEC-type-system.md` 落地，**禁止**在这里悄悄回绕。

use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::decode::{DecodeError, Decoder};
use crate::frame::{Frame, FrameError};
use crate::header::Header;
use crate::instance::Instance;
use crate::opcode;
use crate::refcount::{Owned, PyRef};
use crate::singleton::{BoolObject, IntObject, SMALL_INT_MAX, SMALL_INT_MIN};
use crate::value::Value;

/// 执行失败的形态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecError {
    /// 解码失败（`BC-32`…`BC-36`）。
    Decode(DecodeError),
    /// 帧操作失败（`BC-43`：值栈越界等）。
    Frame(FrameError),
    /// 指令还没接线（`BC-49` 的全表随 M2 补齐）。
    NotImplemented { opcode: u8 },
    /// 指令接线了，但这个形态／类型还没接线（协议槽位、大整数、容器……）。
    Unsupported { opcode: u8, what: &'static str },
    /// 读到未绑定的局部槽（CPython 的 `UnboundLocalError` 时机）。
    UnboundLocal { slot: usize },
    /// 结果超出本层能表示的范围：需要大整数对象。
    IntOutOfRange { value: i64 },
    /// 码元跑完却没有 `RETURN_VALUE`（码元一定被改坏了）。
    FellOffEnd,
}

impl From<DecodeError> for ExecError {
    fn from(error: DecodeError) -> Self {
        ExecError::Decode(error)
    }
}

impl From<FrameError> for ExecError {
    fn from(error: FrameError) -> Self {
        ExecError::Frame(error)
    }
}

fn opcode_of(name: &str) -> u8 {
    opcode::opcode(name).unwrap_or_else(|| panic!("opmap 缺 {name}")) as u8
}

/// 释放一份引用（`OM-20`）。
fn release(instance: &Instance, raw: NonNull<Header>) {
    // SAFETY: 调用方交出的是一份新引用。
    unsafe { instance.release_object(raw.as_ptr()) };
}

/// 把一份**新引用**交给帧的值栈（`BC-43`）。
fn push(instance: &Instance, frame: &Frame, raw: NonNull<Header>) -> Result<(), ExecError> {
    frame.push(instance.own(raw).into_raw())?;
    Ok(())
}

/// 把一个小整数压栈（内部表示是单例，`OM-23`／`OM-39`）。
fn push_small_int(instance: &Instance, frame: &Frame, value: i64) -> Result<(), ExecError> {
    let raw = instance
        .singletons()
        .small_int(value)
        .ok_or(ExecError::IntOutOfRange { value })?;
    push(instance, frame, raw)
}

/// 判定真假——*临时*只覆盖单例表里的类型（`OM-11` 的 `__bool__` 槽位接线后改走协议）。
fn truthiness(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<bool, ExecError> {
    // SAFETY: raw 是帧值栈上的存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        return Ok(false);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 类型身份已确认（见本模块顶部"临时口径"）。
        return Ok(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value);
    }
    if ty == singletons.int_type() {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value != 0);
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "真假判定只接线了 None／bool／int（__bool__ 协议未接线）",
    })
}

/// 取出整数载荷——*临时*按类型身份判定（见本模块顶部"临时口径"）。
///
/// **`TS-40`**：`bool ⊂ int`，所以 `True`／`False` 在这里按 0／1 参与运算；
/// 但**两种载荷的布局不同**，必须分开读，不能互相强转。
fn as_int(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<i64, ExecError> {
    // SAFETY: raw 是帧值栈上的存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.int_type() {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 同上。
        return Ok(i64::from(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value));
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "整数运算只接线了 int 与 bool（数值塔的其余类型与协议槽位未接线）",
    })
}

/// 把小整数结果压栈，越界即报错（**不**回绕）。
fn push_int_result(instance: &Instance, frame: &Frame, value: i64) -> Result<(), ExecError> {
    if !(SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
        return Err(ExecError::IntOutOfRange { value });
    }
    push_small_int(instance, frame, value)
}

/// 把返回值从"栈上的裸引用"转成 [`Value`]：单例落回内联表示，其余包成守卫。
fn value_from_raw<'a>(instance: &'a Instance, raw: NonNull<Header>) -> Value<'a> {
    // SAFETY: raw 是刚出栈的新引用，对象存活。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        release(instance, raw);
        return Value::None;
    }
    if ty == singletons.bool_type() {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value;
        release(instance, raw);
        return Value::Bool(value);
    }
    if ty == singletons.int_type() {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<IntObject>() }.value;
        if (SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
            release(instance, raw); // 单例由实例持有，交回我们这份即可
            return Value::small_int(value);
        }
    }
    // SAFETY: 我们持有 raw 的那份新引用，转交给守卫。
    Value::Object(unsafe { PyRef::from_raw(raw, instance) })
}

/// `BC-49` 的整数二元运算：只做不涉及协议与值域扩张的几项。
fn binary_op(name: &str, left: i64, right: i64) -> Result<i64, ExecError> {
    let result = match name {
        "NB_ADD" => left.checked_add(right),
        "NB_SUBTRACT" => left.checked_sub(right),
        "NB_MULTIPLY" => left.checked_mul(right),
        "NB_AND" => Some(left & right),
        "NB_OR" => Some(left | right),
        "NB_XOR" => Some(left ^ right),
        "NB_LSHIFT" if (0..64).contains(&right) => left.checked_shl(right as u32),
        "NB_RSHIFT" if (0..64).contains(&right) => Some(left >> right),
        _ => {
            return Err(ExecError::Unsupported {
                opcode: opcode_of("BINARY_OP"),
                what: "该 NB_* 运算尚未接线（就地运算、除法族、下标与协议运算随后补）",
            })
        }
    };
    result.ok_or(ExecError::IntOutOfRange { value: i64::MAX })
}

/// 跑一段 code object，直到 `RETURN_VALUE`。
///
/// **BC-42**：指令指针沿途写回帧（码元单位），因此挂起／恢复有据可依。
pub fn execute<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>) -> Result<Value<'a>, ExecError> {
    let code_header = frame.get().code().expect("BC-42：帧必须持有 code object");
    // SAFETY: 帧持有一份对 code object 的引用（BC-42），因此它在帧存活期间有效；
    // 帧由本函数的调用方持有。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    let mut decoder = Decoder::new(code.code());
    while let Some(instruction) = decoder.next_instruction()? {
        let opcode_number = instruction.opcode;
        frame.get().set_instruction_pointer(instruction.offset);
        let oparg = instruction.oparg as usize;

        // BC-50：一律按名字分派，**禁止**依赖具体编号
        let Some(name) = opcode::opname(u16::from(opcode_number)) else {
            return Err(ExecError::NotImplemented { opcode: opcode_number });
        };

        match name {
            "RESUME" | "NOP" => {}
            "LOAD_CONST" => {
                let raw = code
                    .constant(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "常量表下标越界",
                    })?;
                push(instance, frame.get(), raw)?;
            }
            // BC-51：LOAD_FAST 假定槽位已绑定（编译器保证）；这里宁可报错也不读垃圾
            "LOAD_FAST" | "LOAD_FAST_CHECK" => {
                match frame.get().local(oparg)? {
                    Some(raw) => push(instance, frame.get(), raw)?,
                    None => return Err(ExecError::UnboundLocal { slot: oparg }),
                }
            }
            "STORE_FAST" => {
                let value = frame.get().pop()?;
                if let Some(old) = frame.get().set_local(oparg, Some(value))? {
                    release(instance, old);
                }
            }
            "DELETE_FAST" => match frame.get().set_local(oparg, None)? {
                Some(old) => release(instance, old),
                None => return Err(ExecError::UnboundLocal { slot: oparg }),
            },
            "POP_TOP" => release(instance, frame.get().pop()?),
            "TO_BOOL" | "UNARY_NOT" => {
                let value = frame.get().pop()?;
                let truth = truthiness(instance, value, opcode_number)?;
                release(instance, value);
                let truth = if name == "UNARY_NOT" { !truth } else { truth };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "UNARY_NEGATIVE" | "UNARY_INVERT" => {
                let value = frame.get().pop()?;
                let number = as_int(instance, value, opcode_number)?;
                release(instance, value);
                let result = if name == "UNARY_NEGATIVE" {
                    number.checked_neg()
                } else {
                    Some(!number)
                };
                let result = result.ok_or(ExecError::IntOutOfRange { value: number })?;
                push_int_result(instance, frame.get(), result)?;
            }
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
            "IS_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;
                // OM-39：`is` 就是对象身份——栈上放的是真对象，直接比指针
                let identical = left == right;
                release(instance, left);
                release(instance, right);
                let truth = if oparg == 0 { identical } else { !identical };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "COMPARE_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;
                let left_value = as_int(instance, left, opcode_number);
                let right_value = as_int(instance, right, opcode_number);
                release(instance, left);
                release(instance, right);
                let (left_value, right_value) = (left_value?, right_value?);

                // BC-39：oparg 对应 `opcode.cmp_op` 的六元组（顺序从表里取，不写死）
                let operator = opcode::get_cmp_op().get(oparg).copied().ok_or(
                    ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "COMPARE_OP 的 oparg 超出 cmp_op 的六元组（BC-39）",
                    },
                )?;
                let truth = match operator {
                    "<" => left_value < right_value,
                    "<=" => left_value <= right_value,
                    "==" => left_value == right_value,
                    "!=" => left_value != right_value,
                    ">" => left_value > right_value,
                    ">=" => left_value >= right_value,
                    _ => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "cmp_op 里出现了未接线的运算符",
                        })
                    }
                };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "BINARY_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;
                let left_value = as_int(instance, left, opcode_number);
                let right_value = as_int(instance, right, opcode_number);
                release(instance, left);
                release(instance, right);
                let (left_value, right_value) = (left_value?, right_value?);

                // BC-39：oparg 对应 `get_nb_ops()` 的顺序；BC-50：名字从表里取，不写死
                let name = opcode::get_nb_ops()
                    .get(oparg)
                    .map(|(name, _)| *name)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BINARY_OP 的 oparg 超出 get_nb_ops() 的范围（BC-39）",
                    })?;
                let result = binary_op(name, left_value, right_value)?;
                push_int_result(instance, frame.get(), result)?;
            }
            "RETURN_VALUE" => {
                return Ok(value_from_raw(instance, frame.get().pop()?));
            }
            _ => return Err(ExecError::NotImplemented { opcode: opcode_number }),
        }
    }
    Err(ExecError::FellOffEnd)
}
