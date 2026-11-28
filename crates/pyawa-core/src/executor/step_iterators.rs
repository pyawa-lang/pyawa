//! `executor` 的 `step_iterators` 域：迭代器／异步迭代协议。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use crate::HotId;
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
                    if ty == instance.hot_type(HotId::Generator).expect("generator 已登记") {
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
                    if ty == instance.hot_type(HotId::Generator).expect("generator 已登记") || is_iterator_type(instance, ty) {
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
                    if ty == instance.hot_type(HotId::Generator).expect("generator 已登记") {
                        push(instance, frame.get(), value)?;
                        release(instance, value);
                        return Ok(Step::Continue);
                    }
                    if ty == instance.hot_type(HotId::AsyncGenerator).expect("async_generator 已登记") {
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
                            // **宿主 getattr 槽交出的已绑定方法**（`AB-73`/`AB-75`）：直接调用。
                            Ok(Attribute::Owned(bound)) => {
                                let iterator =
                                    call_callable(instance, bound, None, Vec::new(), Vec::new(), opcode_number)?;
                                release(instance, bound);
                                push(instance, frame.get(), iterator)?;
                                release(instance, iterator);
                            }
                            Ok(Attribute::Value(method)) => {
                                // SAFETY: value 由调用方持有（本帧栈刚弹出，close 前不释放）。
                                unsafe { instance.incref_object(value.as_ptr()) };
                                let iterator = call_callable(
                                    instance,
                                    method,
                                    Some(value),
                                    Vec::new(),
                                    Vec::new(),
                                    opcode_number,
                                );
                                // SAFETY: 刚新增的那份引用。
                                unsafe { instance.release_object(value.as_ptr()) };
                                release(instance, value);
                                push(instance, frame.get(), iterator?)?;
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
                    if ty == instance.hot_type(HotId::Generator).expect("generator 已登记") {
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
                    if crate::diag::flag_site!("PYAWA_AF_DEBUG") {
                        let mut names: Vec<String> = Vec::new();
                        for value in frame.get().stack_snapshot() {
                            // SAFETY: 值栈里的项都存活。
                            let ty = unsafe { value.as_ref() }.ty();
                            names.push(instance.type_name(ty).to_owned());
                        }
                        eprintln!(
                            "[end-async-for] depth={} off={} target={:?} stack={names:?}",
                            frame.get().depth(),
                            instruction.offset,
                            instruction.jump_target()
                        );
                    }
                    // 实测净 −2。栈是 `[async iterator, 异常]`（异常在 TOS）：
                    // `StopAsyncIteration` ⇒ 丢掉异常与迭代器、跳到循环之后；
                    // 其余 ⇒ 原样重抛（把异常交回派发器）。
                    let exception = frame.get().pop()?;
                    // SAFETY: exception 是存活对象。
                    let ty = unsafe { exception.as_ref() }.ty();
                    let stop_async_iteration = instance.hot_type(HotId::StopAsyncIteration)
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
                        return Err(ExecError::Raised {
                            exception: crate::raise_guard::RaiseRef::from_owned(instance, exception),
                        });
                    }
                }
                "CLEANUP_THROW" => {
                    // 实测净 −1。参照实现用它收拾"`throw`／`close` 穿过当前帧"时的异常：
                    // `StopIteration` ⇒ 换成它的**值**往下走；其余 ⇒ 原样留下（继续往派发器去）。
                    let exception = frame.get().pop()?;
                    // SAFETY: exception 是存活对象。
                    let ty = unsafe { exception.as_ref() }.ty();
                    let stop_iteration = instance.hot_type(HotId::StopIteration)
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
                    let is_coroutine = ty == instance.hot_type(HotId::Coroutine).expect("coroutine 已登记");
                    let is_async_generator = ty == instance.hot_type(HotId::AsyncGenerator).expect("async_generator 已登记");
                    // `async_generator.__anext__()` 交出的 awaitable：它**就是** awaitable
                    let is_asend = ty == instance.hot_type(HotId::Asend).expect("async_generator_asend 已登记");
                    let is_generator = ty == instance.hot_type(HotId::Generator).expect("generator 已登记");
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
                        // **S1.1（`AB-88`）**：宿主 awaitable —— 先问宿主 `await_fn`，拿到 token
                        // 后把对象本身作为 awaitable 留在帧上；SEND 第一次让出 token，第二次收值完成。
                        match instance.host_object_await(value) {
                            Ok(Some(token)) => {
                                let Some(current) = instance.current_frame() else {
                                    release(instance, value);
                                    return Err(raise_builtin(
                                        instance,
                                        "RuntimeError",
                                        "宿主 await 缺少当前帧（S1.1）",
                                    ));
                                };
                                instance.begin_host_await(current, value, token);
                                push(instance, frame.get(), value)?;
                                release(instance, value);
                                return Ok(Step::Continue);
                            }
                            Ok(None) => {}
                            Err(error) => {
                                release(instance, value);
                                return Err(error);
                            }
                        }
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
                    // **S1.1（`AB-88`/`AB-90`）**：宿主 awaitable 的 SEND 两阶段；
                    // 状态按**当前帧**索引 ⇒ 同一宿主对象被多个协程 await 也不串态。
                    if let Some(current) = instance.current_frame() {
                        if let Some((object_key, token, yielded)) = instance.host_await_state(current) {
                            if object_key != receiver.as_ptr() as usize {
                                return Err(raise_builtin(
                                    instance,
                                    "RuntimeError",
                                    "宿主 await 状态与接收者不一致（S1.1）",
                                ));
                            }
                            if !yielded {
                                instance.mark_host_await_yielded(current);
                                release(instance, sent);
                                // token 是 u64：用十进制整数对象承载，避免 `as i64` 截断。
                                let token_text = token.to_string();
                                let token_value = instance
                                    .new_int_from_decimal(&token_text)
                                    .expect("u64 token 的十进制串一定可解析");
                                frame.get().push(token_value)?;
                                return Ok(Step::Continue);
                            }
                            instance.finish_host_await(current);
                            frame.get().push(sent)?;
                            decoder.set_position(target);
                            return Ok(Step::Continue);
                        }
                    }
                    // SAFETY: receiver 在帧值栈上，存活。
                    let receiver_type = unsafe { receiver.as_ref() }.ty();
                    // 生成器与协程走**同一条**恢复路径（载荷同形）；协程的 `throw`／`close` 也一样
                    let is_generator = receiver_type == instance.hot_type(HotId::Generator).expect("generator 已登记");
                    let is_coroutine = receiver_type == instance.hot_type(HotId::Coroutine).expect("coroutine 已登记");
                    let is_async_generator = receiver_type == instance.hot_type(HotId::AsyncGenerator).expect("async_generator 已登记");
                    let is_asend = receiver_type == instance.hot_type(HotId::Asend).expect("async_generator_asend 已登记");
                    if is_asend {
                        // `await agen.__anext__()`：推进**底层**异步生成器一次。
                        // 送进去的值以包装对象里记着的为准（`__anext__()` 是 `None`）。
                        // SAFETY: 类型身份已确认。
                        let asend = unsafe { &*receiver.as_ptr().cast::<AsendObject>() };
                        let inner = asend.generator();
                        let carried = asend.sent();
                        let mode = asend.mode.get();
                        release(instance, sent);
                        // **`aclose()`**（round 233）：把 `GeneratorExit` 抛进底层并**推进到底**，
                        // 吞掉 `GeneratorExit`／`StopAsyncIteration`，`await` 结果是 `None` ✓。
                        if mode == 1 {
                            let generator_exit_type = instance.hot_type(HotId::GeneratorExit)
                                .expect("GeneratorExit 在引导期已登记");
                            let exception = crate::executor::format::new_exception(
                                instance,
                                generator_exit_type,
                                "",
                            );
                            let mut outcome = resume_generator_with_raise(instance, inner, exception);
                            loop {
                                match outcome {
                                    Ok(GeneratorOutcome::Yielded(value)) => {
                                        release(instance, value);
                                        let exception = crate::executor::format::new_exception(
                                            instance,
                                            generator_exit_type,
                                            "",
                                        );
                                        outcome =
                                            resume_generator_with_raise(instance, inner, exception);
                                    }
                                    Ok(GeneratorOutcome::Returned(value)) => {
                                        release(instance, value);
                                        break;
                                    }
                                    Err(ExecError::Raised { exception }) => {
                                        // SAFETY: exception 是存活对象。
                                        let ty = unsafe { exception.get().as_ref() }.ty();
                                        let is_swallowed = instance.hot_type(HotId::GeneratorExit)
                                            .is_some_and(|base| instance.is_subtype(ty, base))
                                            || instance.hot_type(HotId::StopAsyncIteration)
                                                .is_some_and(|base| instance.is_subtype(ty, base));
                                        if is_swallowed {
                                            exception.discard();
                                            break;
                                        }
                                        return Err(ExecError::Raised { exception });
                                    }
                                    Err(other) => return Err(other),
                                }
                            }
                            push(instance, frame.get(), instance.singletons().none())?;
                            decoder.set_position(target);
                            return Ok(Step::Continue);
                        }
                        // **`athrow(exc)`**（round 233）：把 `carried` 里的异常抛进底层 ✓；
                        // 让出的值照 `asend` 的"一步完成"口径交出去 ✓。
                        if mode == 2 {
                            let exception = carried.ok_or(()).map_err(|_| {
                                instance.raise_builtin_error("TypeError", "athrow 缺少异常对象")
                            })?;
                            match resume_generator_with_raise(instance, inner, exception)? {
                                GeneratorOutcome::Yielded(value) => {
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
                        // **中间 await 的让出要"内循环"消化**（round 244）：异步生成器体里的
                        // `await` 也走 `YIELD_VALUE`（`SEND; YIELD_VALUE 1; RESUME 3; …` ✓）——
                        // 那些让出**不是**生成器的值 ✗，要原样再驱动下去；只有**生成器语句的 `yield`**
                        // 才是"`await asend` 完成"的那一刻 ✓（`test_asyncgen` 的
                        // `test_async_gen_iteration_01`：`await awaitable()` 让出的 `('result',)`
                        // 先前被当成值交出去 ⇒ 列表里混进 `('result',)` ✗）。
                        // 异步生成器内部若正挂起在**宿主 await** 上：内层让出的 token
                        // 必须交给外层宿主 loop（不能当普通中间 await 吞掉）；外层下一次
                        // SEND 送进来的值要传给内层，而不是被 asend 忽略。
                        let inner_frame = unsafe {
                            (&*inner.as_ptr().cast::<GeneratorObject>()).frame()
                        };
                        let inner_host_wait = instance.host_await_state(inner_frame).is_some();
                        let mut carried_value = if inner_host_wait {
                            Some(sent)
                        } else {
                            release(instance, sent);
                            carried
                        };
                        loop {
                            match resume_generator(instance, inner, carried_value)? {
                                GeneratorOutcome::Yielded(value) => {
                                    let inner_frame_now = unsafe {
                                        (&*inner.as_ptr().cast::<GeneratorObject>()).frame()
                                    };
                                    if instance.host_await_state(inner_frame_now).is_some() {
                                        // 宿主 await 的 token：作为真正让出冒泡给最外层驱动者
                                        // （不能跳到 END_SEND 把它当成 asend 的结果）。
                                        frame.get().push(value)?;
                                        break;
                                    }
                                    if generator_suspended_inside_await(inner) {
                                        // 中间让出：丢掉让出值（外层的驱动者会看到 `None` 以外的
                                        // 值后再送 `None` —— 本层的 `async for`／`run_until_complete`
                                        // 都送 `None` ✓，如实按 `None` 继续 ✓）。
                                        release(instance, value);
                                        carried_value = Some(instance.singletons().none());
                                        continue;
                                    }
                                    // **一步完成**：`await asend` 的语义就是"推进一次并把值交出来"
                                    // （实测 `asend.send(None)` ⇒ `StopIteration(值)`），所以这里走
                                    // "耗尽"那一支——压值并跳到 `END_SEND`，**不**让出去。
                                    frame.get().push(value)?;
                                    decoder.set_position(target);
                                    break;
                                }
                                GeneratorOutcome::Returned(value) => {
                                    frame.get().push(value)?;
                                    decoder.set_position(target);
                                    break;
                                }
                            }
                        }
                        return Ok(Step::Continue);
                    }
                    if !is_generator && !is_coroutine && !is_async_generator {
                        // 普通迭代器：参照实现的语义是"取下一个"（`yield from [1, 2]` 就走这条）。
                        let sent_is_none =
                            unsafe { sent.as_ref() }.ty() == instance.singletons().none_type();
                        if sent_is_none {
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
                        // **A-2a（PEP 380 的"子迭代器 send"）**：送非 None 时参照走
                        // `receiver.send(sent)`（没有 `send` 属性 ⇒ `AttributeError`）；返回的值
                        // 就是本次让出值，`StopIteration` 的 `value` 就是 `SEND` 的返回值。
                        // 先前这里一律报 `Unsupported` ✗ —— canonical 循环修好后送非 None 的值
                        // 会真的流到这里（`yield from [1, 2]` 后 `send(5)` ⇒ 参照 `AttributeError`）。
                        let method = match attribute_optional(instance, receiver, "send") {
                            Ok(Some(method)) => method,
                            Ok(None) => {
                                let receiver_type = unsafe { receiver.as_ref() }.ty();
                                let name = instance.type_name(receiver_type);
                                release(instance, sent);
                                return Err(raise_builtin(
                                    instance,
                                    "AttributeError",
                                    &format!("'{name}' object has no attribute 'send'"),
                                ));
                            }
                            Err(error) => {
                                release(instance, sent);
                                return Err(error);
                            }
                        };
                        let outcome = call_value(instance, method, &[sent], &[]);
                        release(instance, method);
                        release(instance, sent);
                        match outcome {
                            Ok(value) => {
                                // 子迭代器 send 返回的让出值 ⇒ push 后继续（下一条 `YIELD_VALUE`）。
                                frame.get().push(value)?;
                            }
                            Err(ExecError::Raised { exception }) => {
                                // SAFETY: exception 是存活对象。
                                let raised = unsafe { exception.get().as_ref() }.ty();
                                let stop_iteration = instance
                                    .hot_type(HotId::StopIteration)
                                    .expect("StopIteration 已登记");
                                if instance.is_subtype(raised, stop_iteration) {
                                    // `StopIteration(value)` ⇒ 压 value（没有实参 ⇒ `None`），跳 END_SEND。
                                    // SAFETY: 类型已确认 ⇒ 载荷是 `ExceptionObject`。
                                    let exception_object = unsafe {
                                        &*exception.get().as_ptr().cast::<ExceptionObject>()
                                    };
                                    let value = match exception_object.args().first().copied() {
                                        Some(value) => {
                                            // SAFETY: 值由异常对象持有；这里新增一份交给帧。
                                            unsafe { instance.incref_object(value.as_ptr()) };
                                            value
                                        }
                                        None => instance.singletons().none(),
                                    };
                                    exception.discard();
                                    frame.get().push(value)?;
                                    decoder.set_position(target);
                                } else {
                                    return Err(ExecError::Raised { exception });
                                }
                            }
                            Err(other) => return Err(other),
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
                    // **A-2a**：`yield from` 的委托链改**显式推进**（内联进 `execute_stack` 的
                    // 委托帧），不再 `resume_generator → execute` 递归（深链会吃 D1 原生预算）。
                    // 只有**精确 generator** 走这条；`coroutine`／`async_generator`／`asend` 的
                    // SEND 分支原样保留（await／async for 的递归不受影响）。
                    if is_generator {
                        // 重入守卫（与 `resume_generator` 同一文案：`ValueError: … already executing`）。
                        if generator.running() {
                            release(instance, sent);
                            let word = instance.type_name(unsafe { receiver.as_ref() }.ty());
                            return Err(raise_builtin(
                                instance,
                                "ValueError",
                                &format!("{word} already executing"),
                            ));
                        }
                        // 委托帧计入**语义**递归额度（`recursion_limit`；与 CPython `SEND_GEN` 里
                        // `py_recursion_remaining--` 同口径）。**不碰** D1／`MAX_NATIVE_DEPTH`。
                        if instance.enter_call().is_err() {
                            release(instance, sent);
                            return Err(instance.raise_builtin_error(
                                "RecursionError",
                                "maximum recursion depth exceeded",
                            ));
                        }
                        // `running` 在委托帧出栈时由 `FrameSlot::finish_delegate` 清掉；
                        // 生成器引用新增一份交给委托帧（出栈时归还）。
                        generator.set_running(true);
                        // SAFETY: receiver 在父帧值栈上，存活；新增一份交给委托帧。
                        unsafe { instance.incref_object(receiver.as_ptr()) };
                        return Err(ExecError::DeferredGeneratorSend {
                            generator: receiver,
                            sent,
                            return_target: target,
                        });
                    }
                    // 协程／异步生成器：沿用原"恢复生成器"的同一段逻辑（`send`／`__next__` 同一条）。
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

/// **生成器是不是停在 `await` 的 `SEND` 循环里**（round 244）：帧的指令指针指向
/// `SEND; YIELD_VALUE 1; RESUME 3` 那条回边的 `RESUME` 处时，前一条是 `YIELD_VALUE`、
/// 再往前（跳过 `SEND` 的那格内联缓存）是 `SEND` ⇒ 这次让出是**中间 await**，不是
/// 生成器语句的 `yield` ✓（判据实测：`async def g(): await x` 与 `async def g(): yield 1`
/// 的让出位点同形，唯一区别是前者前面有 `SEND` ✓）。
pub(crate) fn generator_suspended_inside_await(generator: NonNull<Header>) -> bool {
    // SAFETY: 调用方保证 generator 是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<crate::builtin_objects::GeneratorObject>() };
    let frame_header = object.frame();
    // SAFETY: 帧由生成器持有，存活。
    let frame = unsafe { &*frame_header.as_ptr().cast::<crate::frame::Frame>() };
    let Some(code_header) = frame.code() else {
        return false;
    };
    // SAFETY: code 由帧持有，存活。
    let code = unsafe { &*code_header.as_ptr().cast::<crate::CodeObject>() };
    let bytes = code.code();
    let ip = frame.instruction_pointer();
    // 让出后恢复点＝`RESUME`（1 码元）；前一条 `YIELD_VALUE`（1 码元）；再前是 `SEND`（2 码元）。
    if ip < 3 {
        return false;
    }
    let opcode_at = |unit: usize| -> u16 {
        if unit * 2 + 1 < bytes.len() {
            u16::from(bytes[unit * 2])
        } else {
            0
        }
    };
    // `RESUME`(128) 前是 `YIELD_VALUE`(120)；再往前跳过 `SEND`(106) 的内联缓存（1 格）。
    opcode_at(ip) == 128 && opcode_at(ip - 1) == 120 && opcode_at(ip - 3) == 106
}
