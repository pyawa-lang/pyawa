//! `executor` 的 `step_iterators` 域：迭代器／异步迭代协议。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_iterators<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, decoder: &mut Decoder, instruction: &Instruction, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "GET_ITER" => {
                    // 实测：GET_ITER 净 0（弹被迭代对象、压迭代器）；语义全在 `iter_value` 里
                    // （`itertools.islice` 一类走同一处实现 ⇒ 消息与行为不会分叉）
                    let iterable = frame.get().pop()?;
                    match iter_value(instance, iterable) {
                        Ok(iterator) => {
                            // 迭代器可能**就是**入参（`iter(迭代器) is 它自己`）⇒ 别释放
                            if iterator != iterable {
                                release(instance, iterable);
                            }
                            push(instance, frame.get(), iterator)?;
                            release(instance, iterator);
                        }
                        Err(error) => {
                            release(instance, iterable);
                            return Err(error);
                        }
                    }
                }
                "FOR_ITER" => {
                    let iterator = frame.get().peek()?;
                    // SAFETY: iterator 在帧值栈上，存活。
                    let ty = unsafe { iterator.as_ref() }.ty();
                    if ty == builtin_type(instance, "generator") {
                        // 生成器：`FOR_ITER` 的"取下一个"就是**恢复生成器的帧**（驱动实测骨架
                        // `CALL → GET_ITER → FOR_ITER`）；让出就压让出的值，跑完就走耗尽路径。
                        // SAFETY: 类型身份已确认。
                        let generator = unsafe { &*iterator.as_ptr().cast::<GeneratorObject>() };
                        if !generator.finished() {
                            let frame_header = generator.frame();
                            let generator_frame = Owned::new(
                                // SAFETY: frame_header 由生成器持有，存活；这里新增一份引用。
                                {
                                    unsafe { instance.incref_object(frame_header.as_ptr()) };
                                    frame_header.cast::<Frame>()
                                },
                                instance,
                            );
                            // 顺序**不能反**：`resume` 会用恢复点里的值栈**覆盖**当前值栈，
                            // 所以先恢复，再压"送进去的值"（首轮是 `None`，会被序言的 `POP_TOP` 丢掉；
                            // 之后 `x = yield v` 的取值就来自这里）。
                            if generator_frame.get().is_suspended() {
                                generator_frame.get().resume()?;
                            }
                            push(instance, generator_frame.get(), instance.singletons().none())?;
                            let outcome = execute(instance, &generator_frame);
                            match outcome {
                                Ok(ExecOutcome::Yielded(value)) => {
                                    frame.get().push(value)?;
                                }
                                Ok(ExecOutcome::Returned(_)) => {
                                    generator.mark_finished();
                                    push(instance, frame.get(), instance.singletons().null())?;
                                    let target = instruction.jump_target().ok_or(
                                        ExecError::Unsupported {
                                            opcode: opcode_number,
                                            what: "BC-55：这条指令没有跳转目标",
                                        },
                                    )?;
                                    decoder.set_position(target);
                                }
                                Err(error) => return Err(error),
                            }
                        } else {
                            push(instance, frame.get(), instance.singletons().null())?;
                            let target =
                                instruction
                                    .jump_target()
                                    .ok_or(ExecError::Unsupported {
                                        opcode: opcode_number,
                                        what: "BC-55：这条指令没有跳转目标",
                                    })?;
                            decoder.set_position(target);
                        }
                        return Ok(Step::Continue);
                    }
                    // **协议迭代器也接** ✓（task-18 真 bug 修 ✗）：先前这里对"非本层接线的迭代器"**直接报
                    // 未接线** ✗ ⇒ 用户类（`__iter__`／`__next__`）与 `open()` 的文件对象在 `for` 里全进不去 ✓。
                    // `advance_iterator` 自己就带 `__next__` 协议分支 ✓（`OM-11` 属性通道 ✓，`next()` 同一条 ✓）
                    // ⇒ 这一格判断本来就是**多余**的 ✓；耗尽仍照下面那条 FOR_ITER 的跳转收尾 ✓。
                    match advance_iterator(instance, iterator, opcode_number)? {
                        Some(item) => frame.get().push(item)?,
                        None => {
                            // 实测：**耗尽时 FOR_ITER 仍然 +1**（接着 END_FOR／POP_ITER 各 −1 收尾）
                            // ⇒ 这里压一个占位（内部 NULL 哨兵），随后被那两条指令弹掉。
                            push(instance, frame.get(), instance.singletons().null())?;
                            let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "BC-55：这条指令没有跳转目标",
                            })?;
                            decoder.set_position(target);
                        }
                    }
                }
                "END_FOR" | "POP_ITER" => {
                    // 实测两条都是 −1：前者收耗尽时压的那个占位，后者收迭代器本身
                    release(instance, frame.get().pop()?);
                }
                "GET_YIELD_FROM_ITER" => {
                    // 实测净 0：TOS 是生成器（或协程）就留着，否则换成 `iter(TOS)`
                    let iterable = frame.get().pop()?;
                    // SAFETY: iterable 是刚出栈的存活对象。
                    let ty = unsafe { iterable.as_ref() }.ty();
                    if ty == builtin_type(instance, "generator") || is_iterator_type(instance, ty) {
                        frame.get().push(iterable)?;
                    } else {
                        // **回退走 `iter_value`**（与 `GET_ITER` **同一处实现** ✓）：`iterator_type_for`
                        // 只认内建容器 ✓ ⇒ 先前 `yield from <有 __iter__ 的自定义对象>` 报
                        // `指令 16 …只接线了…的内建迭代器` ✗（真受害者：`ChainMap` 的 `KeysView.__iter__`
                        // 就是 `yield from self._mapping` ✓ ⇒ `list(cm.keys())` 必炸 ✓）。
                        match iter_value(instance, iterable) {
                            Ok(iterator) => {
                                // 迭代器可能**就是**入参（`iter(迭代器) is 它自己`）⇒ 别释放
                                if iterator != iterable {
                                    release(instance, iterable);
                                }
                                frame.get().push(iterator)?;
                            }
                            Err(error) => {
                                release(instance, iterable);
                                return Err(error);
                            }
                        }
                    }
                }
                "GET_AITER" => {
                    // 实测净 0：异步生成器原样就是 async iterator；其余走 `__aiter__`；
                    // 都没有 ⇒ 实测 `TypeError: 'async for' requires an object with __aiter__ method, got int`
                    let value = frame.get().pop()?;
                    // SAFETY: value 是帧值栈上的存活对象。
                    let ty = unsafe { value.as_ref() }.ty();
                    // **本层的 `async def` 编成生成器**（已登记的近似）⇒ `async for` 也得认它 ✓
                    // （第 306 轮）：否则 `async for` 一到运行期就报
                    // `'async for' requires an object with __aiter__ method, got generator` ✗。
                    if ty == builtin_type(instance, "generator") {
                        push(instance, frame.get(), value)?;
                        release(instance, value);
                        return Ok(Step::Continue);
                    }
                    if ty == builtin_type(instance, "async_generator") {
                        push(instance, frame.get(), value)?;
                        release(instance, value);
                    } else {
                        // SAFETY: value 是存活对象。
                        let name = unsafe { ty.as_ref() }.name();
                        match attribute_lookup(instance, value, "__aiter__") {
                            Ok(Attribute::Method { function, this }) => {
                                let mut arguments: Vec<NonNull<Header>> = Vec::new();
                                // SAFETY: this 由类型字典与调用方持有，这里新增一份交给调用。
                                unsafe { instance.incref_object(this.as_ptr()) };
                                arguments.push(this);
                                release(instance, value);
                                let iterator = call_callable(
                                    instance,
                                    function,
                                    None,
                                    arguments,
                                    Vec::new(),
                                    opcode_number,
                                )?;
                                push(instance, frame.get(), iterator)?;
                                release(instance, iterator);
                            }
                            _ => {
                                release(instance, value);
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    &format!(
                                        "'async for' requires an object with __aiter__ method, got {name}"
                                    ),
                                ));
                            }
                        }
                    }
                }
                "GET_ANEXT" => {
                    // 实测净 +1：在 async iterator **之上**压一个 awaitable。
                    // 异步生成器就压它自己（本层"await 它 ＝ 推进一次"）；其余走 `__anext__`。
                    let iterator = frame.get().peek()?;
                    // SAFETY: iterator 在帧值栈上，存活。
                    let ty = unsafe { iterator.as_ref() }.ty();
                    // **本层的 `async def` 编成生成器**（已登记的近似）⇒ 异步迭代时把它自己当 awaitable
                    // 压上去（与 `async_generator` 同一条路 ✓，第 306 轮）—— 否则 `async for` 报
                    // `'async for' requires an object with __anext__ method, got generator` ✗。
                    if ty == builtin_type(instance, "generator") {
                        // SAFETY: iterator 由帧值栈持有，这里新增一份交给新压上的那一格。
                        unsafe { instance.incref_object(iterator.as_ptr()) };
                        push(instance, frame.get(), iterator)?;
                        return Ok(Step::Continue);
                    }
                    match attribute_lookup(instance, iterator, "__anext__") {
                        // 类型字典里的函数（取方法）
                        Ok(Attribute::Method { function, this }) => {
                            let mut arguments: Vec<NonNull<Header>> = Vec::new();
                            // SAFETY: this 由类型字典与调用方持有。
                            unsafe { instance.incref_object(this.as_ptr()) };
                            arguments.push(this);
                            let awaitable = call_callable(
                                instance,
                                function,
                                None,
                                arguments,
                                Vec::new(),
                                opcode_number,
                            )?;
                            push(instance, frame.get(), awaitable)?;
                            release(instance, awaitable);
                        }
                        // `getattr` 槽交出的**绑定方法**（异步生成器的 `__anext__` 走这条）
                        Ok(Attribute::Owned(bound)) => {
                            let awaitable =
                                call_callable(instance, bound, None, Vec::new(), Vec::new(), opcode_number)?;
                            release(instance, bound);
                            push(instance, frame.get(), awaitable)?;
                            release(instance, awaitable);
                        }
                        _ => {
                            // SAFETY: 类型身份未知，取名字用。
                            let name = unsafe { ty.as_ref() }.name();
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                &format!("'async for' requires an object with __anext__ method, got {name}"),
                            ));
                        }
                    }
                }
                "END_ASYNC_FOR" => {
                    // 实测净 −2。栈是 `[async iterator, 异常]`（异常在 TOS）：
                    // `StopAsyncIteration` ⇒ 丢掉异常与迭代器、跳到循环之后；
                    // 其余 ⇒ 原样重抛（把异常交回派发器）。
                    let exception = frame.get().pop()?;
                    // SAFETY: exception 是存活对象。
                    let ty = unsafe { exception.as_ref() }.ty();
                    let stop_async_iteration = instance
                        .type_named("StopAsyncIteration")
                        .expect("异常层次在引导期已登记");
                    if instance.is_subtype(ty, stop_async_iteration) {
                        release(instance, exception);
                        // 迭代器那一格也丢掉
                        release(instance, frame.get().pop()?);
                        if let Some(target) = instruction.jump_target() {
                            decoder.set_position(target);
                        }
                    } else {
                        release(instance, frame.get().pop()?);
                        return Err(ExecError::Raised { exception });
                    }
                }
                "CLEANUP_THROW" => {
                    // 实测净 −1。参照实现用它收拾"`throw`／`close` 穿过当前帧"时的异常：
                    // `StopIteration` ⇒ 换成它的**值**往下走；其余 ⇒ 原样留下（继续往派发器去）。
                    let exception = frame.get().pop()?;
                    // SAFETY: exception 是存活对象。
                    let ty = unsafe { exception.as_ref() }.ty();
                    let stop_iteration = instance
                        .type_named("StopIteration")
                        .expect("异常层次在引导期已登记");
                    if instance.is_subtype(ty, stop_iteration) {
                        // SAFETY: 类型身份已确认。
                        let object = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };
                        let value = object.args().first().copied();
                        match value {
                            Some(value) => {
                                // SAFETY: 值由异常对象持有，新增一份交给值栈。
                                unsafe { instance.incref_object(value.as_ptr()) };
                                release(instance, exception);
                                push(instance, frame.get(), value)?;
                                release(instance, value);
                            }
                            None => {
                                release(instance, exception);
                                push(instance, frame.get(), instance.singletons().none())?;
                            }
                        }
                    } else {
                        push(instance, frame.get(), exception)?;
                        release(instance, exception);
                    }
                }
                "GET_AWAITABLE" => {
                    // 实测：净 0（弹一个、压一个）。协程（以及 `CO_ITERABLE_COROUTINE` 标记的
                    // 生成器）**原样**就是 awaitable；其余对象走 `__await__`；
                    // 都没有 ⇒ 实测 `TypeError: 'int' object can't be awaited`。
                    let value = frame.get().pop()?;
                    // SAFETY: value 是帧值栈上的存活对象。
                    let ty = unsafe { value.as_ref() }.ty();
                    let is_coroutine = ty == builtin_type(instance, "coroutine");
                    let is_async_generator = ty == builtin_type(instance, "async_generator");
                    // `async_generator.__anext__()` 交出的 awaitable：它**就是** awaitable
                    let is_asend = ty == builtin_type(instance, "async_generator_asend");
                    let is_generator = ty == builtin_type(instance, "generator");
                    let iterable_coroutine = is_generator && {
                        // SAFETY: 类型身份已确认。
                        let object = unsafe { &*value.as_ptr().cast::<GeneratorObject>() };
                        let frame_header = object.frame();
                        // SAFETY: 帧由生成器持有，存活。
                        let generator_frame = unsafe { &*frame_header.as_ptr().cast::<Frame>() };
                        match generator_frame.code() {
                            // SAFETY: code 由帧持有，存活。
                            Some(code) => {
                                unsafe { code.cast::<CodeObject>().as_ref() }.flags() & 0x100 != 0
                            }
                            None => false,
                        }
                    };
                    // **注意**：异步生成器**不在**这里——实测 `await agen` ⇒
                    // `TypeError: 'async_generator' object can't be awaited`（它要经 `__anext__()`
                    // 交出的 awaitable）。第一版我图省事让它"await 一次推进一格"，被实测打回。
                    let _ = is_async_generator;
                    // **本层的 `async def` 编成生成器**（已登记的近似）⇒ 生成器也算 awaitable ✓
                    //（第 307 轮）：否则 `async with` 的 `GET_AWAITABLE` 一跑就报
                    // `TypeError: 'generator' object can't be awaited` ✗。参照只认带
                    // `CO_ITERABLE_COROUTINE` 标记的生成器 ✓ —— 本层生成的生成器没有那个标记 ✓，
                    // 这里按近似一并认下 ✓。
                    if is_coroutine || is_asend || iterable_coroutine || is_generator {
                        push(instance, frame.get(), value)?;
                        release(instance, value);
                    } else {
                        // SAFETY: value 是存活对象。
                        let name = unsafe { ty.as_ref() }.name();
                        match attribute_lookup(instance, value, "__await__") {
                            Ok(Attribute::Method { function, this }) => {
                                let mut arguments: Vec<NonNull<Header>> = Vec::new();
                                // SAFETY: this 由调用方与类型字典持有，这里新增一份交给调用。
                                unsafe { instance.incref_object(this.as_ptr()) };
                                arguments.push(this);
                                release(instance, value);
                                let iterator =
                                    call_callable(instance, function, None, arguments, Vec::new(), opcode_number)?;
                                push(instance, frame.get(), iterator)?;
                                release(instance, iterator);
                            }
                            _ => {
                                release(instance, value);
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    &format!("'{name}' object can't be awaited"),
                                ));
                            }
                        }
                    }
                }
                "SEND" => {
                    // 实测：`SEND delta` 净 0 —— 栈是 `[接收者, 送进去的值]`，
                    // 让出就压"让出的值"并**往下走**（下一条通常是 `YIELD_VALUE` 把它再让出去），
                    // 跑完就压"接收者的返回值"并**跳转 delta**（跳到 `END_SEND`）。
                    let sent = frame.get().pop()?;
                    let receiver = frame.get().peek()?;
                    let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BC-55：SEND 没有跳转目标",
                    })?;
                    // SAFETY: receiver 在帧值栈上，存活。
                    let receiver_type = unsafe { receiver.as_ref() }.ty();
                    // 生成器与协程走**同一条**恢复路径（载荷同形）；协程的 `throw`／`close` 也一样
                    let is_generator = receiver_type == builtin_type(instance, "generator");
                    let is_coroutine = receiver_type == builtin_type(instance, "coroutine");
                    let is_async_generator = receiver_type == builtin_type(instance, "async_generator");
                    let is_asend = receiver_type == builtin_type(instance, "async_generator_asend");
                    if is_asend {
                        // `await agen.__anext__()`：推进**底层**异步生成器一次。
                        // 送进去的值以包装对象里记着的为准（`__anext__()` 是 `None`）。
                        // SAFETY: 类型身份已确认。
                        let asend = unsafe { &*receiver.as_ptr().cast::<AsendObject>() };
                        let inner = asend.generator();
                        let carried = asend.sent();
                        release(instance, sent);
                        match resume_generator(instance, inner, carried)? {
                            GeneratorOutcome::Yielded(value) => {
                                // **一步完成**：`await asend` 的语义就是"推进一次并把值交出来"
                                // （实测 `asend.send(None)` ⇒ `StopIteration(值)`），所以这里走
                                // "耗尽"那一支——压值并跳到 `END_SEND`，**不**让出去。
                                frame.get().push(value)?;
                                decoder.set_position(target);
                            }
                            GeneratorOutcome::Returned(value) => {
                                frame.get().push(value)?;
                                decoder.set_position(target);
                            }
                        }
                        return Ok(Step::Continue);
                    }
                    if !is_generator && !is_coroutine && !is_async_generator {
                        // 普通迭代器：参照实现的语义是"取下一个"（`yield from [1, 2]` 就走这条）。
                        // 送进去的值对没有 `send` 的对象没有去处——本层只接受 `None`（如实报其余）。
                        let sent_is_none =
                            unsafe { sent.as_ref() }.ty() == instance.singletons().none_type();
                        if !sent_is_none {
                            release(instance, sent);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "SEND 送非 None 给普通迭代器（参照实现走 `send`／`next` 协议）",
                            });
                        }
                        release(instance, sent);
                        match advance_iterator(instance, receiver, opcode_number)? {
                            Some(item) => frame.get().push(item)?,
                            None => {
                                // 耗尽：压"迭代器的返回值"——普通迭代器没有返回值，压 `None`
                                // （`yield from` 的 `END_SEND` 会把接收者收掉、留下这一格）
                                push(instance, frame.get(), instance.singletons().none())?;
                                decoder.set_position(target);
                            }
                        }
                        return Ok(Step::Continue);
                    }
                    // SAFETY: 类型身份已确认。
                    let generator = unsafe { &*receiver.as_ptr().cast::<GeneratorObject>() };
                    if generator.finished() {
                        // 已经跑完：送进去的值没用上，直接走"耗尽"那一路
                        release(instance, sent);
                        push(instance, frame.get(), instance.singletons().none())?;
                        decoder.set_position(target);
                        return Ok(Step::Continue);
                    }
                    // 复用"恢复生成器"的同一段逻辑（`send`／`__next__` 走的是它）
                    match resume_generator(instance, receiver, Some(sent))? {
                        GeneratorOutcome::Yielded(value) => {
                            // 让出的值是**新引用**，裸 `Frame::push` 正好接手
                            frame.get().push(value)?;
                        }
                        GeneratorOutcome::Returned(value) => {
                            frame.get().push(value)?;
                            decoder.set_position(target);
                        }
                    }
                }
                "END_SEND" => {
                    // 净 −1，但**去掉的是 TOS1**：`SEND` 耗尽时栈是 `[接收者, 结果]`，
                    // `END_SEND` 丢掉接收者、把结果留在栈顶（第一版我按"弹 TOS"写，
                    // 结果把结果丢了自己留下接收者——驱动器拿到的是生成器）。
                    let result = frame.get().pop()?;
                    release(instance, frame.get().pop()?);
                    frame.get().push(result)?;
                }
        _ => unreachable!("step_iterators 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
