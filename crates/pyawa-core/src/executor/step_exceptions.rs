//! `executor` 的 `step_exceptions` 域：异常表与 `raise` 族。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_exceptions<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
        "PUSH_EXC_INFO" => step_exceptions_push_exc_info(instance, frame, name),
        "POP_EXCEPT" => step_exceptions_pop_except(instance, frame, name),
        "CHECK_EXC_MATCH" => step_exceptions_check_exc_match(instance, frame, name),
        "RERAISE" => step_exceptions_reraise(instance, frame, oparg, name),
        "WITH_EXCEPT_START" => step_exceptions_with_except_start(instance, frame, opcode_number, name),
        "RAISE_VARARGS" => step_exceptions_raise_varargs(instance, frame, oparg, opcode_number, name),
        _ => unreachable!("step_exceptions 的 opcode 分组之外的情况"),
    }
}

/// `PUSH_EXC_INFO` 的分派臂（从 `step_exceptions` 原样搬来）。
fn step_exceptions_push_exc_info<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
            "PUSH_EXC_INFO" => {
                // 实测骨架：处理块入口第一条就是它；栈效果 `(new_exc -- prev_exc, new_exc)`
                let exception = frame.get().pop()?;
                // **上一次的"当前异常"要**取走**（不是偷看）** ✗（task-26 真 bug 修 ✗）：
                // `exception_state` 是**栈**（`current_exception()` ＝ 栈顶 ✓）⇒ 进入处理块＝
                // 「弹掉外层、压入本层」✓、退出＝「弹掉本层、把外层压回」✓（见 `POP_EXCEPT` 臂）。
                // 先前用 `current_exception()` **偷看**再 push ✗ ⇒ 外层那项留在栈里 ＋ 退出时又压回
                // 一次 ⇒ **每经一层嵌套 handler 就多一层重复** ✗ ⇒ `sys.exc_info()`（task-26 真实现）
                // 在"handler 里再套 try/except"之后仍报旧的异常 ✓（实测 `ValueError` 泄漏 ✓）。
                let previous = instance.pop_exception();
                let previous_owned = match previous {
                    Some(value) => value, // `pop_exception` 交出的就是**新引用** ✓
                    None => {
                        let none = instance.singletons().none();
                        // SAFETY: 单例由实例持有。
                        unsafe { instance.incref_object(none.as_ptr()) };
                        none
                    }
                };
                // 状态接手本层异常（"正在处理的异常"＝新压入的这层 ✓）
                instance.push_exception(exception);
                frame.get().push(previous_owned)?;
                push(instance, frame.get(), exception)?;
            }
        _ => unreachable!("step_exceptions_push_exc_info 只接收 PUSH_EXC_INFO"),
    }
    Ok(Step::Continue)
}

/// `POP_EXCEPT` 的分派臂（从 `step_exceptions` 原样搬来）。
fn step_exceptions_pop_except<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
            "POP_EXCEPT" => {
                // 栈顶是 `PUSH_EXC_INFO` 压下的"上一个异常"；把它还原成当前异常
                if crate::diag::flag("PYAWA_POP_DEBUG") {
                    eprintln!("[pop_except] 弹前深={} site={}", frame.get().depth(), instance.current_site());
                }
                let previous = frame.get().pop()?;
                if let Some(current) = instance.pop_exception() {
                    release(instance, current);
                }
                if unsafe { previous.as_ref() }.ty() == instance.singletons().none_type() {
                    release(instance, previous);
                } else {
                    instance.push_exception(previous);
                }
            }
        _ => unreachable!("step_exceptions_pop_except 只接收 POP_EXCEPT"),
    }
    Ok(Step::Continue)
}

/// `CHECK_EXC_MATCH` 的分派臂（从 `step_exceptions` 原样搬来）。
fn step_exceptions_check_exc_match<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
            "CHECK_EXC_MATCH" => {
                // **实测**：它**弹掉类**、压回布尔（净 0）——参照实现原话是
                // "Pops TOS and pushes the boolean result of the test"。
                let class_object = frame.get().pop()?;
                if crate::diag::flag("PYAWA_EXC_MATCH_DEBUG") {
                    eprintln!(
                        "[exc_match] 弹掉类之后 栈深={} site={}",
                        frame.get().depth(),
                        instance.current_site()
                    );
                }
                let exception = frame.get().peek()?;
                // SAFETY: class_object 是刚出栈的存活对象。
                let class_type = unsafe { class_object.as_ref() }.ty();
                // SAFETY: exception 在帧值栈上，存活。
                let exception_type = unsafe { exception.as_ref() }.ty();
                let truth = if class_type == builtin_type(instance, "type") {
                    let class = class_object.cast::<TypeObject>();
                    if !is_exception_type(instance, class) {
                        release(instance, class_object);
                        return Err(raise_builtin(
                            instance,
                            "TypeError",
                            "catching classes that do not inherit from BaseException is not allowed",
                        ));
                    }
                    let matched = instance.is_subtype(exception_type, class);
                    release(instance, class_object);
                    matched
                } else if Some(class_type) == instance.type_named("tuple") {
                    // `except (A, B)`：**任一命中即匹配**（实测）。
                    //
                    // 元组里放**非类**、放**嵌套元组**、或放不是 `BaseException` 子类的类
                    // （`except str`）都报同一句 `TypeError`（实测原话见下面那条 assert）；
                    // 且**只在真的要匹配时**才报——没异常发生时该子句根本不执行。
                    // SAFETY: 类型身份已确认。
                    let items = unsafe { &*class_object.as_ptr().cast::<TupleObject>() };
                    let mut matched = false;
                    for index in 0..items.len() {
                        let item = items.item(index).expect("下标在范围内");
                        // SAFETY: item 由元组持有，存活。
                        let item_type = unsafe { item.as_ref() }.ty();
                        if item_type != builtin_type(instance, "type") {
                            release(instance, class_object);
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                "catching classes that do not inherit from BaseException is not allowed",
                            ));
                        }
                        let candidate = item.cast::<TypeObject>();
                        if !is_exception_type(instance, candidate) {
                            release(instance, class_object);
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                "catching classes that do not inherit from BaseException is not allowed",
                            ));
                        }
                        if instance.is_subtype(exception_type, candidate) {
                            matched = true;
                        }
                    }
                    release(instance, class_object);
                    matched
                } else {
                    release(instance, class_object);
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        "catching classes that do not inherit from BaseException is not allowed",
                    ));
                };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
        _ => unreachable!("step_exceptions_check_exc_match 只接收 CHECK_EXC_MATCH"),
    }
    Ok(Step::Continue)
}

/// `RERAISE` 的分派臂（从 `step_exceptions` 原样搬来）。
fn step_exceptions_reraise<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
            "RERAISE" => {
                // **`RERAISE n`：先取 TOS 当异常，再弹那 `n` 个额外值** ✓（第 172 轮定案 ✓）。
                //   我们先前写成了「先弹 `n` 个、再抛 TOS」✗ —— 一弹就把**异常本身**弹掉 ✗，
                //   于是抛出去的是下面的 `lasti`（一个 **`int`** ✓）⇒ 症状就是本层那句
                //   `未捕获（状态 1）：int` ✓（参照报 `ZeroDivisionError: division by zero` ✓）。
                //   实测栈（顶在前）：`[ZeroDivisionError, NoneType, int, CM, function]` ✓ ——
                //   TOS 是异常 ✓、下面那两格是 `prev`／`lasti` ✓，与 `WITH_EXCEPT_START` 的注释一致 ✓。
                let exception = frame.get().pop()?;
                for _ in 0..oparg {
                    release(instance, frame.get().pop()?);
                }
                return Err(raise(instance, exception));
            }
        _ => unreachable!("step_exceptions_reraise 只接收 RERAISE"),
    }
}

/// `WITH_EXCEPT_START` 的分派臂（从 `step_exceptions` 原样搬来）。
fn step_exceptions_with_except_start<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
            "WITH_EXCEPT_START" => {
                // 异常出口（实测骨架）：`PUSH_EXC_INFO; WITH_EXCEPT_START; TO_BOOL;
                // POP_JUMP_IF_TRUE L1; NOT_TAKEN; RERAISE 2; POP_TOP; POP_EXCEPT; POP_TOP×3; …`
                // 净栈效应 **+1**：以 `(类型, 异常, traceback)` 调 `__exit__`，**只压结果**，
                // 栈上原有的 `[self, 可调用, 异常, 上一个异常]` 一个都不动
                // （`RERAISE` 与抑制分支都要用它们）。
                // 栈（自顶向下）：**异常**、`prev`、`self`、**可调用**
                // —— `PUSH_EXC_INFO` 在本层压的是 `(prev, exc)`（`exc` 在 TOS，与 `handlers.rs`
                // 里 `CHECK_EXC_MATCH` 的取项一致）；`__exit__` 那一份在 `self` 的**下面**
                // （参照实现的文档说"调用栈上**第 4 项**"，第 4 项就是可调用）。
                // 栈（自顶向下）：**异常、prev、lasti、self、可调用**——`with` 的异常表条目
                // **带 `lasti`**（实测参照的 `depth<<1|lasti` 低位是 1），派发时压了那个偏移，
                // 而 `PUSH_EXC_INFO` 又把 `prev` 插在它上面 ⇒ `self`／可调用要再往下两格
                // （第 231 轮实测修正：原来按 3／4 取，`__exit__` 根本调不到）
                let exception = frame.get().peek()?;
                let prev = frame.get().peek_from_top(2)?;
                let lasti = frame.get().peek_from_top(3)?;
                let self_object = frame.get().peek_from_top(4)?;
                let callable = frame.get().peek_from_top(5)?;
                let _ = lasti;
                let exception_type = unsafe { exception.as_ref() }.ty();
                // `__exit__(type, exc, tb)`：`tb` 本层给 `None`（`__traceback__` 尚无对象，`DIV-6`）
                let mut arguments: Vec<NonNull<Header>> = Vec::with_capacity(3);
                unsafe {
                    instance.incref_object(exception_type.cast::<Header>().as_ptr());
                    instance.incref_object(exception.as_ptr());
                }
                arguments.push(exception_type.cast::<Header>());
                arguments.push(exception);
                arguments.push(instance.new_none());
                let result = call_callable(
                    instance,
                    callable,
                    Some(self_object),
                    arguments,
                    Vec::new(),
                    opcode_number,
                )?;
                push(instance, frame.get(), result)?;
                release(instance, result);
                let _ = prev;
            }
        _ => unreachable!("step_exceptions_with_except_start 只接收 WITH_EXCEPT_START"),
    }
    Ok(Step::Continue)
}

/// `RAISE_VARARGS` 的分派臂（从 `step_exceptions` 原样搬来）。
fn step_exceptions_raise_varargs<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
            "RAISE_VARARGS" => {
                // 参照实现：0 ＝ 重抛当前异常、1 ＝ `raise X`、2 ＝ `raise X from Y`（Y 在 TOS）
                return match oparg {
                    0 => {
                        // BC-60 ②：当前异常按**实例**存
                        let current = instance.current_exception().ok_or_else(|| {
                            raise_builtin(
                                instance,
                                "RuntimeError",
                                "No active exception to reraise",
                            )
                        })?;
                        // SAFETY: current 由本实例的异常状态持有，存活。
                        unsafe { instance.incref_object(current.as_ptr()) };
                        Err(raise(instance, current))
                    }
                    1 | 2 => {
                        let cause = if oparg == 2 {
                            Some(frame.get().pop()?)
                        } else {
                            None
                        };
                        let operand = frame.get().pop()?;
                        // SAFETY: operand 在帧值栈上，存活。
                        let operand_type = unsafe { operand.as_ref() }.ty();

                        let exception = if is_exception_type(instance, operand_type) {
                            operand // 已经是异常实例
                        } else if operand_type == builtin_type(instance, "type") {
                            // 是类型对象：必须是异常类，实例化它（`raise ValueError`）
                            let class = operand.cast::<TypeObject>();
                            if !is_exception_type(instance, class) {
                                release(instance, operand);
                                if let Some(cause) = cause {
                                    release(instance, cause);
                                }
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    "exceptions must derive from BaseException",
                                ));
                            }
                            release(instance, operand);
                            let object = instance.alloc(ExceptionObject::new(
                                class,
                                RefCell::new(Vec::new()),
                                RefCell::new(None),
                                RefCell::new(None),
                                Cell::new(false),
                                RefCell::new(None),
                            ));
                            object.into_raw().cast::<Header>()
                        } else {
                            release(instance, operand);
                            if let Some(cause) = cause {
                                release(instance, cause);
                            }
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                "exceptions must derive from BaseException",
                            ));
                        };

                        // **载荷形状先核** ✓（第 722 轮真 bug 修 ✗）：**类型是异常 ≠ 载荷是
                        // `ExceptionObject`** ✗ —— Python 层定义、带**实例字典**的异常子类
                        // （`Lib/json/decoder.py` 的 `JSONDecodeError` ✓）走 `AttributeObject` 载荷。
                        // 实测：`JSONDecodeError` 的 `instance_size` 是 **56**，而 `ExceptionObject`
                        // 是 **128** ✗ ⇒ 先前一律硬转 ⇒ 读到别的字段当 `RefCell` ⇒ `set_context`
                        // 报 `RefCell already borrowed`（而且是**越界写** ✓）。口径与
                        // [`Instance::exception_message_of`] 的核对**同一处** ✓（第 193 轮就核了 ✓）。
                        let payload = {
                            // SAFETY: exception 是刚拿到的新引用，存活。
                            let ty = unsafe { exception.as_ref() }.ty();
                            // SAFETY: ty 由注册表持有。
                            if unsafe { ty.as_ref() }.instance_size()
                                == core::mem::size_of::<ExceptionObject>()
                            {
                                // SAFETY: 形状刚核过。
                                Some(unsafe { &*exception.as_ptr().cast::<ExceptionObject>() })
                            } else {
                                None
                            }
                        };

                        // 隐式上下文 ＝ 当前正在处理的异常（参照实现始终设，展示与否看抑制位）
                        if let Some(current) = instance.current_exception() {
                            // SAFETY: current 由实例的异常状态持有。
                            unsafe { instance.incref_object(current.as_ptr()) };
                            match payload {
                                Some(object) => {
                                    if let Some(old) = object.set_context(Some(current)) {
                                        release(instance, old);
                                    }
                                }
                                // 非 `ExceptionObject` 载荷 ⇒ 走**实例字典** ✓（它有 `__dict__` ✓）。
                                None => {
                                    let _ = instance.set_attribute_value(
                                        exception, "__context__", current,
                                    );
                                    release(instance, current);
                                }
                            }
                        }

                        if let Some(cause) = cause {
                            // SAFETY: cause 是刚出栈的新引用。
                            let cause_type = unsafe { cause.as_ref() }.ty();
                            // 起因可以是**异常实例**，也可以是**异常类**（实测 `raise ValueError from TypeError` 合法）
                            let cause_is_class = cause_type == builtin_type(instance, "type")
                                && is_exception_type(instance, cause.cast::<TypeObject>());
                            if cause_type == instance.singletons().none_type() {
                                // `raise X from None`：抑制上下文，但没有 __cause__
                                release(instance, cause);
                            } else if is_exception_type(instance, cause_type) {
                                match payload {
                                    Some(object) => {
                                        if let Some(old) = object.set_cause(Some(cause)) {
                                            release(instance, old);
                                        }
                                    }
                                    None => {
                                        let _ = instance.set_attribute_value(
                                            exception, "__cause__", cause,
                                        );
                                        release(instance, cause);
                                    }
                                }
                            } else if cause_is_class {
                                // 实测：起因是**类**时，参照实现会**实例化**它（`__cause__` 是 `TypeError()`），
                                // 不是把类本身存进去
                                let class = cause.cast::<TypeObject>();
                                release(instance, cause);
                                let created = instance.alloc(ExceptionObject::new(
                                    class,
                                    RefCell::new(Vec::new()),
                                    RefCell::new(None),
                                    RefCell::new(None),
                                    Cell::new(false),
                                    RefCell::new(None),
                                ));
                                let created = created.into_raw().cast::<Header>();
                                match payload {
                                    Some(object) => {
                                        if let Some(old) = object.set_cause(Some(created)) {
                                            release(instance, old);
                                        }
                                    }
                                    None => {
                                        let _ = instance.set_attribute_value(
                                            exception, "__cause__", created,
                                        );
                                        release(instance, created);
                                    }
                                }
                            } else {
                                release(instance, cause);
                                release(instance, exception);
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    "exception causes must derive from BaseException",
                                ));
                            }
                            match payload {
                                Some(object) => object.set_suppress_context(true),
                                None => {
                                    let flag = instance.retain(instance.singletons().boolean(true));
                                    let _ = instance.set_attribute_value(
                                        exception, "__suppress_context__", flag,
                                    );
                                    release(instance, flag);
                                }
                            }
                        }

                                                // **挂 `traceback`** ✓（第 213 轮：`BC-60` 的最小起步 ✓）—— `tb_frame` ＝ 抛出处的帧 ✓。
                        let traceback = instance.new_traceback(frame.as_ptr().cast::<Header>());
                        // **尽力而为** ✓（异常类型若没有实例字典，如实不挂 ✓）。
                        let _ = instance.set_attribute_value(exception, "__traceback__", traceback);
                        release(instance, traceback);
Err(raise(instance, exception))
                    }
                    _ => Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "RAISE_VARARGS 的 oparg 只能是 0／1／2",
                    }),
                };
            }
        _ => unreachable!("step_exceptions_raise_varargs 只接收 RAISE_VARARGS"),
    }
}
