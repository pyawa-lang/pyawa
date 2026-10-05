//! **`call` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `call_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`attribute_lookup`、`bind_arguments`、`builtin_type`、`execute`、`function_closure`、`function_defaults`、`is_type_object`、`own_code`、`push`、`raise_builtin`、`release`、`value_into_raw` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{attribute_lookup, bind_arguments, builtin_type, execute, function_closure, function_defaults, own_code, raise_builtin, release, value_into_raw};
use crate::executor::Attribute;
use crate::builtin_objects::BuiltinFunctionObject;
use core::cell::Cell;
use crate::code::CodeObject;
use crate::executor::ExecError;
use crate::executor::ExecOutcome;
use crate::frame::Frame;
use crate::builtin_objects::FunctionObject;
use crate::builtin_objects::GeneratorObject;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::MethodObject;
use core::ptr::NonNull;
use crate::type_object::TypeObject;


/// 按**属性通道**（`TS-44`）在对象上找一个 dunder 并调用它（找不到就什么也不做）。
///
/// 用于类创建钩子（`__init_subclass__`）一类"有就调、没有就算了"的钩子。
pub(crate) fn call_dunder_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<(), ExecError> {
    let found = match attribute_lookup(instance, object, name) {
        Ok(found) => found,
        Err(_) => return Ok(()),
    };
    let (callable, this) = match found {
        Attribute::Method { function, this } => (function, this),
        Attribute::Value(method) | Attribute::Owned(method) => (method, object),
    };
    let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        call_args.push(*argument);
    }
    let result = call_callable(instance, callable, Some(this), call_args, Vec::new(), 0)?;
    release(instance, result);
    Ok(())
}

/// 按值调用一个可调用对象（`AB-24` 的宿主交接面与 `pa_call` 用）。
///
/// 实参是**借用视图**；成功返回**新引用**。异常经 [`ExecError::Raised`] 上抛。
pub fn call_value(
    instance: &Instance,
    callable: NonNull<Header>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut owned_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        owned_args.push(*argument);
    }
    let mut owned_kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(kwargs.len());
    for (key, value) in kwargs {
        // SAFETY: 同上。
        unsafe {
            instance.incref_object(key.as_ptr());
            instance.incref_object(value.as_ptr());
        }
        owned_kwargs.push((*key, *value));
    }
    call_callable(instance, callable, None, owned_args, owned_kwargs, 0)
}

/// 按**属性通道**在对象上找一个方法并调用（`TS-44`）：找不到返回 `Ok(None)`，
/// 找到就返回调用的结果（**新引用**）。
pub(crate) fn call_object_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<Option<NonNull<Header>>, ExecError> {
    let found = match attribute_lookup(instance, object, name) {
        Ok(found) => found,
        Err(_) => return Ok(None),
    };
    let (callable, this) = match found {
        Attribute::Method { function, this } => (function, this),
        Attribute::Value(method) | Attribute::Owned(method) => (method, object),
    };
    let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        call_args.push(*argument);
    }
    let result = call_callable(instance, callable, Some(this), call_args, Vec::new(), 0)?;
    Ok(Some(result))
}

/// 调用一个可调用对象（本片只有函数对象）。
/// 调用一个可调用对象（**新引用**返回值）。
///
/// **`bound_self` 的所有权契约**：它是**借用**——调用方持有那份引用，本函数不释放它。
/// 需要长期持有（进实参表、进生成的实例）的路径各自 `incref`。
pub(crate) fn call_callable(
    instance: &Instance,
    callable: NonNull<Header>,
    bound_self: Option<NonNull<Header>>,
    args: Vec<NonNull<Header>>,
    kwargs: Vec<(NonNull<Header>, NonNull<Header>)>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: callable 是帧值栈上的存活对象。
    let ty = unsafe { callable.as_ref() }.ty();
    // 本层接线的可调用：函数、**类型对象**（`OM-11` 的 `new` 槽）与**绑定方法**。
    // 内建可调用对象（`builtin_function_or_method`）随后补。
    // `OM-11` 的 `call` 槽：类型自带调用语义（宿主函数一类走这条）
    // SAFETY: callable 是存活对象。
    // **类调用（`C(...)`）与元类型自身调用（`type(x)`）要分开** ✓（第 183 轮实证 ✓）：
    // `C` 的**类型**是元类型 ✓ ⇒ 若照抄元类型的 call 槽 ✗，`C(...)` 会被**劫走** ✗
    //（实测：13 条语料当场红 ✗，`TypeError: cannot create 'type' instances` ✓）。
    // 所以：**只有元类型自己**（self-typed ✓）被调用时才走 call 槽 ✓；其余**一切类**一律走**实例化** ✓。
    let callable_is_metatype = instance.is_type_object(callable)
        && unsafe { callable.as_ref() }.ty().as_ptr() == callable.as_ptr().cast::<TypeObject>();
    let call_slot = if instance.is_type_object(callable) && !callable_is_metatype {
        None
    } else {
        unsafe { ty.as_ref() }.slots().call
    };
    if let Some(slot) = call_slot {
        // SAFETY: 槽位契约见 `CallFn`（借用视图 ＋ 新引用返回值）。
        let result = unsafe { slot(callable.as_ptr(), bound_self, &args, &kwargs, instance) };
        for argument in args {
            release(instance, argument);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        return result;
    }

    let callable_type_ok = ty == builtin_type(instance, "function")
        || instance.is_type_object(callable)
        || ty == builtin_type(instance, "method")
        || ty == builtin_type(instance, "builtin_function_or_method");
    if !callable_type_ok {
        for value in args {
            release(instance, value);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        // 实测：不可调用的对象被调用 ⇒ `TypeError: '<类型名>' object is not callable`
        // （此前报的是 VM 级的 `Unsupported`，属"没有实测口径就当没实现"；现在照参照报）
        // SAFETY: callable 是存活对象。
        let name = instance.type_name(unsafe { callable.as_ref() }.ty());
        return Err(raise_builtin(
            instance,
            "TypeError",
            &format!("'{name}' object is not callable"),
        ));
    }

    // **类型对象被调用**（`list()`／`ValueError("x")`）：走类型自己的 `new` 槽（`OM-11`／`OM-14`），
    // 然后按 `OM-14` 找 `__init__`（Python 子类的覆写就落在那里）。
    // SAFETY: callable 是存活对象。
    // **`is_type_object` 才是对的判据** ✓（第 269 轮真 bug ✗）：先前要求"元类型**恰好是** `type`" ✗
    // ⇒ 元类型是 **Python 类**（`ABCMeta` 一族 ✓）的类被当成**不可调用** ✗（实测 `Lib/os.py` 的
    // `_Environ(...)` ⇒ `TypeError: 'ABCMeta' object is not callable` ✗）。一切**类对象**都该走实例化 ✓。
    if instance.is_type_object(callable) {
        // **`CALL` 的 `self` 槽：对"类调用"是第一个位置实参** ✓（第 279 轮真 bug 修 ✗）。
        // 参照的**装饰器**写法 `@property\ndef g(self): …` 产的是 `LOAD_NAME property; <函数>; CALL 0`
        // ✓ —— 那里**没有** `PUSH_NULL` ✓，函数落在 `self` 槽上 ✓，参照按 `property(g)` 解析 ✓
        //（实测 `dis` ✓）。先前这一支**丢掉** `bound_self` ✗ ⇒ `property(fget)` 的 `fget` 永远是 `None` ✗
        // ⇒ `@property` 描述的属性统统坏掉 ✗（与 `CHANGELOG` 里 `D.__new__(cls, a)` 报"缺 1 个实参"同源 ✓）。
        let mut args = args;
        if let Some(self_object) = bound_self {
            // 契约：`bound_self` 是**借用**（调用方持有）⇒ 为实参表新增一份 ✓
            // SAFETY: self_object 由调用方保证存活。
            unsafe { instance.incref_object(self_object.as_ptr()) };
            args.insert(0, self_object);
        }
        let class = callable.cast::<TypeObject>();
        // SAFETY: class 由注册表持有。
        let new_slot = unsafe { class.as_ref() }.slots().new;
        // SAFETY: 类型名由注册表持有，存活。
        let class_name = unsafe { class.as_ref() }.name().to_owned();
        let Some(new_slot) = new_slot else {
            let message = format!("cannot create '{class_name}' instances");
            for argument in args {
                release(instance, argument);
            }
            for (key, value) in kwargs {
                release(instance, key);
                release(instance, value);
            }
            return Err(raise_builtin(instance, "TypeError", &message));
        };
        // SAFETY: 槽位由类型提供，契约见 `NewFn`。
        let created = match unsafe { new_slot(class, &args, instance) } {
            Ok(created) => created,
            // `OM-11` 扩（裁决）：失败由**槽位**给原因，这里**直接透传**（不再由调用点猜
            // "cannot create '<类名>' instances" 那句话）
            Err(error) => {
                for argument in args {
                    release(instance, argument);
                }
                for (key, value) in kwargs {
                    release(instance, key);
                    release(instance, value);
                }
                return Err(error);
            }
        };
        // **`__new__` 分派**（`OM-14` 的"子类分派槽位"里 Python 侧那一半）。
        //
        // 实测口径：
        // - `__new__` 只可能在**类字典**里（`object` 不带默认 `__new__`，故查到的一定是覆写）
        // - 它拿到 `(cls, *args, **kwargs)`；返回值**不是**本类实例时 `__init__` **不**被调用
        //   （实测：`__new__` 返回 `42` 时 `B()` 就是 `42`）
        // - 返回值是本类（或子类）实例时照常调 `__init__`，且 `__init__` **仍拿到原实参**
        //
        // 所有权：`call_callable` 是**转移**语义（它消耗实参表），所以给 `__new__` 的那一份
        // 要自己新增；原引用留给 `__init__`，没走到 `__init__` 就归还。
        // **`type.__new__` 不算** ✗（第 190 轮真 bug 修复 ✓）：上面那行注释写的口径是
        // "`__new__` 只可能在**类字典**里" ✓，但自从 `type` 的命名空间里挂上 `__new__`（第 179 轮 ✓）
        // 之后，这里的查找会**翻到"元类型那一层"** ✗ ⇒ 于是**任何** `C()` 都变成
        // `type.__new__(C)` ✓（**1 个实参** ✗）⇒ 而 `type.__new__` 要 ≥3 个 ⇒ 报
        // "实际 0 个" 一类的怪错 ✓（实测：`import os` 就撞它 ✓）。
        // ⇒ 与第 179 轮元类那条同款处理 ✓：**是我们挂的那个就跳过** ✓，走默认实例化 ✓。
        // **`type.__new__` 与 `object.__new__` 都不算** ✗（第 193 轮补上后一半 ✓）：参照的实例化走的是
        // **类型自己的 `new` 槽** ✓（`list.__new__` 是**它自己**的 ✓，不是 `object.__new__` ✓）⇒
        // 这两个"我们自己挂的"默认实现只应作为**属性**存在 ✓（`@object.__new__` 那类用法 ✓），
        // **不参与**实例化分派 ✓；否则内建类型一被构造就撞"多给了实参" ✗（实测十多条语料当场变红 ✓）。
        let ours_new = instance
            .type_named("type")
            .and_then(|ty| instance.type_lookup(ty, "__new__"));
        let object_new = instance
            .type_named("object")
            .and_then(|ty| instance.type_lookup(ty, "__new__"));
        if let Some(constructor) = instance
            .type_lookup(class, "__new__")
            .filter(|found| Some(*found) != ours_new && Some(*found) != object_new)
        {
            let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len() + 1);
            // SAFETY: constructor 由类型字典持有；class 在注册表里；实参由调用方保证存活。
            unsafe {
                instance.incref_object(constructor.as_ptr());
                instance.incref_object(class.cast::<Header>().as_ptr());
            }
            call_args.push(class.cast::<Header>());
            for argument in args.iter().copied() {
                // SAFETY: 同上。
                unsafe { instance.incref_object(argument.as_ptr()) };
                call_args.push(argument);
            }
            let mut constructor_kwargs: Vec<(NonNull<Header>, NonNull<Header>)> =
                Vec::with_capacity(kwargs.len());
            for (key, value) in kwargs.iter().copied() {
                // SAFETY: 同上。
                unsafe {
                    instance.incref_object(key.as_ptr());
                    instance.incref_object(value.as_ptr());
                }
                constructor_kwargs.push((key, value));
            }
            let created =
                call_callable(instance, constructor, None, call_args, constructor_kwargs, opcode)?;
            // SAFETY: created 是新引用，存活。
            let is_instance = instance.is_subtype(unsafe { created.as_ref() }.ty(), class);
            let initializer = if is_instance {
                instance.type_lookup(class, "__init__")
            } else {
                None
            };
            match initializer {
                Some(initializer) => {
                    let mut init_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len() + 1);
                    // SAFETY: initializer 由类型字典持有；created 是新引用；实参仍归本函数。
                    unsafe {
                        instance.incref_object(initializer.as_ptr());
                        instance.incref_object(created.as_ptr());
                    }
                    init_args.push(created);
                    // 原实参与关键字实参转交给 `__init__`
                    init_args.extend(args.iter().copied());
                    let result = call_callable(instance, initializer, None, init_args, kwargs, opcode)?;
                    release(instance, result);
                }
                None => {
                    // 没有 `__init__`：把调用方那份实参归还
                    for argument in args.iter().copied() {
                        release(instance, argument);
                    }
                    for (key, value) in kwargs.iter().copied() {
                        release(instance, key);
                        release(instance, value);
                    }
                }
            }
            return Ok(created);
        }

        // `__init__`（`OM-14`：子类覆写要生效）。找到就"实例在先、实参在后"地调它。
        // **默认的 `object.__init__` 不算"有 `__init__`"**（第 310 轮）：它是本轮才挂上去的
        // （为了"在类型对象上取 dunder" ✓）⇒ 若不排除，`class C: pass` 的 `C(1)` 就会走"调
        // `__init__`"这条路 ⇒ **不再**报 `C() takes no arguments` ✗（`type_call` 那道测试当场变红 ✓）。
        let default_init = instance
            .type_named("object")
            .and_then(|object| instance.type_lookup(object, "__init__"));
        let initializer = instance
            .type_lookup(class, "__init__")
            .filter(|found| Some(*found) != default_init);
        if let Some(initializer) = initializer {
            let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len() + 1);
            // SAFETY: initializer 由类型字典持有，存活；这里新增一份引用交给调用。
            unsafe { instance.incref_object(initializer.as_ptr()) };
            // SAFETY: created 是刚拿到的新引用；调用方那份由 `call_callable` 的返回交出，
            // 这里额外加一份给实参表。
            unsafe { instance.incref_object(created.as_ptr()) };
            call_args.push(created);
            call_args.extend(args.iter().copied());
            let result = call_callable(
                instance,
                initializer,
                None,
                call_args,
                kwargs,
                opcode,
            )?;
            // `__init__` 必须返回 None（`T`／参照实现如此）；返回值丢掉那份引用
            release(instance, result);
            // initializer 的那份引用由 `call_callable` 接手（它内部会按需释放）
        } else {
            // 没有 `__init__`，且走的是**通用分配**（`attribute_new`）时，带实参创建就是错的
            // （实测 `Empty(1)` ⇒ `Empty() takes no arguments`）。
            //
            // 判据必须限定在通用分配上：内建类型（`ValueError('x')` 一类）的 `new` 槽是自己的
            // 实现、本来就能吃实参，不该被这条规则误伤。用**类型标志**而不是比较函数指针
            // （`rustc` 明说函数地址不保证唯一）。
            // SAFETY: class 由注册表持有。
            let generic_allocation = unsafe { class.as_ref() }.has_generic_allocation();
            if generic_allocation && (!args.is_empty() || !kwargs.is_empty()) {
                for argument in args {
                    release(instance, argument);
                }
                for (key, value) in kwargs {
                    release(instance, key);
                    release(instance, value);
                }
                release(instance, created);
                let message = format!("{class_name}() takes no arguments");
                return Err(raise_builtin(instance, "TypeError", &message));
            }
            for argument in args {
                release(instance, argument);
            }
            for (key, value) in kwargs {
                release(instance, key);
                release(instance, value);
            }
        }
        return Ok(created);
    }

    // **先剥绑定方法**：剥出来的可能是函数（下面按"绑定位置参数"调），也可能是**原生**
    // （走原生分支、self 当 `bound` 递进去）。顺序很要紧——原生的
    // `BuiltinFunctionObject` 与 `FunctionObject` 布局不同，先当函数读会读到错位的内存
    // （症状是"misaligned pointer dereference"）。
    // SAFETY: callable 是存活对象。
    let callable_type = unsafe { callable.as_ref() }.ty();
    let (callable, bound_self) = if callable_type == builtin_type(instance, "method") {
        // SAFETY: 类型身份已确认。
        let method = unsafe { &*callable.as_ptr().cast::<MethodObject>() };
        // 函数与实例都由该方法对象持有、存活；`bound_self` 是**借用**（见本函数开头的契约）
        (method.function(), Some(method.this()))
    } else {
        (callable, bound_self)
    };

    // **原生可调用对象**（`AB-24`：宿主函数与内建函数的落点）：实参以**借用视图**递进去，
    // 返回值是**新引用**。绑定方法形态在这里剥掉绑定并当第一个位置实参。
    // SAFETY: callable 是存活对象。
    if unsafe { callable.as_ref() }.ty() == builtin_type(instance, "builtin_function_or_method") {
        // SAFETY: 类型身份已确认。
        let native = unsafe { &*callable.as_ptr().cast::<BuiltinFunctionObject>() };
        let function = native.function();
        let bound = match bound_self {
            Some(self_object) => Some(self_object),
            None => None,
        };
        // SAFETY: 签名契约见 `NativeFn`（借用视图 ＋ 新引用返回值）。
        let result = unsafe { function(instance, bound, &args, &kwargs) };
        // 借用视图：实参的引用仍归本函数，调用完要按约归还
        for argument in args {
            release(instance, argument);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        // `bound` 是**借用**：不在这里释放（调用方持有；契约见本函数开头）
        return result;
    }

    let (code_header, defaults, kwdefaults) = function_defaults(callable);
    // **`__globals__`**：函数帧的全局映射取自函数自己（`BC-57`）；`MAKE_FUNCTION` 时捕获。
    let function_globals = {
        // SAFETY: callable 是存活对象。
        let is_function = unsafe { callable.as_ref() }.ty() == builtin_type(instance, "function");
        if is_function {
            // SAFETY: 类型身份已确认。
            unsafe { &*callable.as_ptr().cast::<FunctionObject>() }.globals()
        } else {
            None
        }
    };
    // SAFETY: 函数持有一份对 code object 的引用，故它在函数存活期间有效。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    let mut args = args;
    if let Some(self_object) = bound_self {
        // 契约：`bound_self` 是**借用**（调用方持有那份引用）；实参表由 `bind_arguments` 接手，
        // 故这里先为它新增一份。
        // SAFETY: self_object 由调用方保证存活。
        unsafe { instance.incref_object(self_object.as_ptr()) };
        args.insert(0, self_object);
    }

    let locals = bind_arguments(instance, code, args, kwargs, &defaults, kwdefaults, opcode)?;

    // **帧类型是"内部"类型** ✓（不进探测表 ✓）⇒ 这里必须用 `type_named` ✗（用 `builtin_type` 会 panic ✓）。
    let frame_type = instance
        .type_named("frame")
        .expect("引导期已登记 frame 类型（内部类型 ✓）");
    let frame = instance.alloc(Frame::for_code(frame_type, &own_code(instance, code_header)));
    if let Some(mapping) = function_globals {
        // 帧接手的是**新引用**（`Frame::clear` 会释放它）
        // SAFETY: 映射由函数持有，存活。
        unsafe { instance.incref_object(mapping.as_ptr()) };
        frame.get().set_globals(mapping);
    }
    for (slot, value) in locals.into_iter().enumerate() {
        if let Some(value) = value {
            let _ = frame.get().set_local(slot, Some(value))?;
        }
    }
    // **建帧装闭包**（`CPython` 3.11+ 的时机）：第 i 个自由槽 ← 闭包元组第 i 项（cell 对象）
    for cell in function_closure(instance, callable) {
        // SAFETY: cell 由函数的闭包持有，存活；帧要自己那份引用。
        unsafe { instance.incref_object(cell.as_ptr()) };
        let _ = frame.get().install_closure(&[cell]);
        // SAFETY: 上面那份新增引用已交给帧（`install_closure` 接手）。
    }

    // **生成器／协程函数**（`CO_GENERATOR` ＝ 32、`CO_COROUTINE` ＝ 128，都实测过）：
    // `CALL` **不**跑函数体，而是把挂起的帧包成对应的对象交出去
    // （实测骨架：函数体第一条是 `RETURN_GENERATOR`，恢复时才从 `POP_TOP` 继续）。
    // 两者的载荷同形（一个挂起的帧 ＋ 标志），只是类型不同：`repr` 的词、以及协程**不是迭代器**。
    let wrapped_type = if code.flags() & 0x20 != 0 {
        Some("generator")
    } else if code.flags() & 0x80 != 0 {
        Some("coroutine")
    } else if code.flags() & 0x200 != 0 {
        // `CO_ASYNC_GENERATOR`（实测 0x200，`async def` ＋ `yield`）
        Some("async_generator")
    } else {
        None
    };
    if let Some(type_name) = wrapped_type {
        frame.get().suspend()?;
        // 生成器要**自己持有一份帧的引用**（`GeneratorObject` 的 traverse／clear 会释放它）——
        // 漏了这一份，`call_callable` 一返回帧就被释放，生成器拿到的是悬垂指针。
        // SAFETY: frame 由本函数持有，这里新增一份引用交给生成器。
        unsafe { instance.incref_object(frame.as_ptr().cast::<Header>().as_ptr()) };
        let generator = instance.alloc(GeneratorObject::new(
            builtin_type(instance, type_name),
            frame.as_ptr().cast::<Header>(),
            Cell::new(false),
            Cell::new(false),
        ));
        return Ok(generator.into_raw().cast::<Header>());
    }

    // **调用深度记账**（第 319 轮）：本层的"调用"就是这里的 **Rust 递归** ✗ ⇒ 不设限的话
    // Python 层的深递归会顶穿**原生栈**（对拍测试线程的栈更小 ⇒ 那族 `-11` × 29 就是这么来的 ✓）。
    // 记账点选在**真正要跑函数体**之前 ✓（生成器那条**不算深度**：它只是把挂起的帧交出去 ✓）；
    // 出错路径与正常路径都要配对 `leave_call` ✓。
    if instance.enter_call().is_err() {
        return Err(instance.raise_builtin_error(
            "RecursionError",
            "maximum recursion depth exceeded",
        ));
    }
    let outcome = execute(instance, &frame);
    instance.leave_call();
    match outcome? {
        ExecOutcome::Returned(value) => Ok(value_into_raw(instance, value)),
        ExecOutcome::Yielded(_) => Err(ExecError::Unsupported {
            opcode,
            what: "非生成器函数不该让出（码元被改坏了？）",
        }),
    }
}
