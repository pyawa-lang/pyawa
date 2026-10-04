//! `builtins` 模块的**纯计算面**（`PLAN` §9.4 第 4 条）。
//!
//! 契约（`CM-4` 的"从 Python 看到的 API 与语义"）写在 `docs/SPEC-c-modules.md` §5.2.2。
//! 只做**不需要能力域、也不需要输出通道**的那些内建：需要平台（`print` 之类）的留给
//! `P3-14`／待裁口径。
//!
//! 本模块**只用安全函数**定义原生：核心的 `NativeFn` 是 `unsafe fn` 指针，而**安全 `fn`
//! 可以强转成它**（同一 ABI）⇒ stdlib 在 `#![forbid(unsafe_code)]` 下也能落地原生函数。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

/// 模块名（`builtins`）。
pub const NAME: &str = "builtins";

/// 本模块落地的内建函数名（按名字排序；测试与合约核对用）。
pub const IMPLEMENTED: &[&str] = &[
    "abs", "all", "any", "bin", "bool", "callable", "chr", "dict", "float", "getattr", "hasattr",
    "globals", "hex", "int", "isinstance", "issubclass", "iter", "len", "list", "max", "min", "next", "oct",
    "ord", "range", "repr",
    "set", "setattr", "sorted", "str", "sum", "tuple", "type",
];

/// 建 `builtins` 模块的命名空间（**新引用** 的 `dict`）。
///
/// 内容：上表那些内建函数 ＋ **`__build_class__`**（由核心在引导期建好，`OM-14`）＋
/// `__name__`。**没有** `print`（要输出通道，口径待裁）、也没有需要能力域的那些。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let natives: &[(&str, pyawa_core::NativeFn)] = &[
        ("abs", abs_native as pyawa_core::NativeFn),
        // **构造器一族**（第 130 轮）：`_bootstrap.py` 起手就缺 `list` ✓；都按 CPython 的最小面接线 ✓
        ("bool", bool_native as pyawa_core::NativeFn),
        ("dict", dict_native as pyawa_core::NativeFn),
        ("float", float_native as pyawa_core::NativeFn),
        ("getattr", getattr_native as pyawa_core::NativeFn),
        ("hasattr", hasattr_native as pyawa_core::NativeFn),
        ("setattr", setattr_native as pyawa_core::NativeFn),
        ("int", int_native as pyawa_core::NativeFn),
        ("list", list_native as pyawa_core::NativeFn),
        ("set", set_native as pyawa_core::NativeFn),
        ("str", str_native as pyawa_core::NativeFn),
        ("tuple", tuple_native as pyawa_core::NativeFn),
        ("type", type_native as pyawa_core::NativeFn),
        ("all", all_native as pyawa_core::NativeFn),
        ("any", any_native as pyawa_core::NativeFn),
        ("bin", bin_native as pyawa_core::NativeFn),
        ("callable", callable_native as pyawa_core::NativeFn),
        ("chr", chr_native as pyawa_core::NativeFn),
        ("hex", hex_native as pyawa_core::NativeFn),
        ("isinstance", isinstance_native as pyawa_core::NativeFn),
        ("issubclass", issubclass_native as pyawa_core::NativeFn),
        ("len", len_native as pyawa_core::NativeFn),
        // **`next`**（第 142 轮）：`_bootstrap.py` 与语料都要它 ✓ ⇒ 复用执行器的 `advance` ✓
        ("next", next_native as pyawa_core::NativeFn),
        ("globals", globals_native as pyawa_core::NativeFn),
        ("iter", iter_native as pyawa_core::NativeFn),
        ("max", max_native as pyawa_core::NativeFn),
        ("min", min_native as pyawa_core::NativeFn),
        ("oct", oct_native as pyawa_core::NativeFn),
        ("ord", ord_native as pyawa_core::NativeFn),
        // **`print`**（`CM-26` 的硬边界：走 `sys.stdout` ⇒ `_io` ⇒ `fs` 域 ✓，**禁止**临时 sink ✓）
        ("print", print_native as pyawa_core::NativeFn),
        ("range", range_native as pyawa_core::NativeFn),
        ("pow", pow_native as pyawa_core::NativeFn),
        ("range", range_native as pyawa_core::NativeFn),
        ("repr", repr_native as pyawa_core::NativeFn),
        ("round", round_native as pyawa_core::NativeFn),
        ("divmod", divmod_native as pyawa_core::NativeFn),
        ("sorted", sorted_native as pyawa_core::NativeFn),
        ("sum", sum_native as pyawa_core::NativeFn),
    ];
    for (name, handler) in natives {
        let function = make_native(instance, name, *handler);
        instance.dict_set(namespace, name, function);
    }
    // **`object`**（第 132 轮）：参照里它是所有类的根 ✓（`class X(object)` 是很常见的写法 ✓，
    // `_bootstrap.py` 里就有 ✓）。类型对象在引导期已登记 ✓ ⇒ 按名字取出来放进名字空间 ✓。
    // 注意：`int`／`str` 等**内建类型名**本层目前是 native 函数（不是类型对象 ✗）⇒
    // `class X(int)` 与 `int.from_bytes` 这类用法要等"内建类型化"那一轮 ✓（已登记 ✓）。
    if let Some(object_type) = instance.type_named("object") {
        instance.dict_set(namespace, "object", object_type.cast());
    }
    // **`slice`**（第 148 轮）：类型对象**早已登记** ✓（`P1-10`／`slice_new` ✓）⇒ 与 `object` 同一
    // 手法：按名字取出来放进名字空间 ✓（此前 `slice(1, 3)` 报 `NameError` ✗）。
    // **异常类型进名字空间**（第 163 轮）：这些类型**早就建好了** ✓（`type_named(\"ValueError\")` 有 ✓），
    //   但**从没放进内建名字空间** ✗ ⇒ 实测 `ValueError` ⇒ `NameError: name 'ValueError' is not defined` ✓
    //   ⇒ 于是 `try: raise ValueError(...) / except ValueError:` 这类**最基础的**异常匹配也走不通 ✗
    //   （`Lib/` 里每一处 `try/except` 回退都靠它 ✓）。
    //   名字取自 `TS-41` 的探测表 ✓（**一处真相** ✓，不另抄一份 ✗）；判定用 `BaseException` 子类 ✓。
    //   **先 `retain` 再交给字典** ✓ —— `dict_set` 是「接管一份引用」的规矩 ✓（第 161 轮的堆损坏就是这么来的 ✗）。
    if let Some(base_exception) = instance.type_named("BaseException") {
        for entry in pyawa_core::builtin_types::BUILTIN_TYPES {
            let Some(ty) = instance.type_named(entry.name) else {
                continue;
            };
            if !instance.is_subtype(ty, base_exception) {
                continue;
            }
            instance.retain(ty.cast());
            instance.dict_set(namespace, entry.name, ty.cast());
        }
    }
    // **`classmethod`**（第 158 轮）：与 `slice`／`object` 同一手法 ✓（类型在引导期已登记 ✓）。
    if let Some(classmethod_type) = instance.type_named("classmethod") {
        instance.dict_set(namespace, "classmethod", classmethod_type.cast());
    }
    if let Some(slice_type) = instance.type_named("slice") {
        instance.dict_set(namespace, "slice", slice_type.cast());
    }
    // **形态类／描述符类型**（第 152 轮）：这三个类型**早就在探测表里** ✓（`builtin_types.rs`
    // 有 `property`／`staticmethod`／`classmethod` ✓）；本层先按名字把它们放进名字空间 ✓
    //（与 `object`／`slice` 同一手法 ✓）；**描述符协议（`__get__`）未接** ✗ ⇒
    // `@property` 这类**还不能真正生效** ✗（已如实登记 ✓）。
    for descriptor_name in ["property", "staticmethod", "classmethod"] {
        if let Some(ty) = instance.type_named(descriptor_name) {
            // **先 `retain` 再交给字典** ✓（第 161 轮定位到的真因 ✓）：`dict_set` 走的是
            //   "**接管**一份引用"的规矩 ✓ ⇒ 直接传类型对象会让**注册表与字典都以为自己持有**
            //   同一份引用 ✗ ⇒ 双双释放 ⇒ 实测 100% 可复现的 `corrupted size vs. prev_size` ✓
            //   （`object`／`slice` 那两处之所以没事 ✓，是因为它们的引用计数另有来源 ✓）。
            instance.retain(ty.cast());
            instance.dict_set(namespace, descriptor_name, ty.cast());
        }
    }
    // `__build_class__`：核心在引导期已经建好（`OM-14`），这里原样放进 `builtins`
    if let Some(build_class) = instance.build_class() {
        instance.dict_set(namespace, "__build_class__", build_class);
    }
    // **内建类型化（第 181 轮，用户裁定 A ✓）**：`int` 这个名字应当是**类型对象** ✓（不是 native ✗）——
    // 这样 `int.__name__`／`int.__dict__`／`isinstance(x, int)`／`class X(int)` 才成立 ✓。
    // 构造逻辑本来就在 `int` 类型的 `new` 槽里 ✓ ⇒ 这里只**改指** ✓（**先 `retain` 再交给字典** ✓）。
    // 名字与**类型对象**一一对应地改指 ✓ —— 逐个来 ✓：**先只接已经验过构造槽的** ✓
    // （`int` 的 `new` 槽本就在 ✓；`type` 就是元类型 ✓，调用它即建类 ✓）。
    // 其余（`list`／`dict`／…）等各自的构造槽核过再改 ✗ —— 免得把 `list(...)` 这类构造弄坏 ✓。
    // **只留 `int`** ✗：`type` 改指元类型会让 `type()` 无参调用**变宽** ✗（参照抛 `cannot create
    // 'type' instances` ✓）——元类型的 **call 槽**得先按参照语义接好 ✓ 再改指 ✓。
    // **一次一个名字** ✓（第 182 轮节奏 ✓ —— 第 181 轮一次改五个当场红 ✗）：本轮只加 `dict` ✓。
    // `dict` 的类型对象已接 `dict_new` ✓、还带 `getattr` **方法面** ✓（`fromkeys` 就挂那儿 ✓）。
    // `type` 也改指**元类型** ✓（第 183 轮：劫持"调用类"的毛病已修 ✓ ——
    // `call_callable` 现在只在**元类型自身**被调用时走它的 call 槽 ✓，`C(...)` 一律走实例化 ✓）。
    // **逐个改指** ✓（第 184 轮：一次一个名字 ＋ 每次跑闸门 ✓ —— 第 181 轮一次五个当场红 ✗）。
    for name in ["int", "dict", "type", "list", "tuple", "set", "str", "float", "bool", "bytes", "slice", "object"] {
    // **`str` 已改指** ✓（第 185 轮：`str_new` 的构造槽补齐了 ✓ —— 第 184 轮退回的原因 ✓）。
        if let Some(ty) = instance.type_named(name) {
            instance.retain(ty.cast());
            instance.dict_set(namespace, name, ty.cast());
        }
    }
    // **类级方法进类型字典** ✓（第 182 轮）：`dict` 的类型对象已惰性挂上命名空间 ✓ ⇒ 把 `fromkeys` 放进去 ✓。
    if let Some(dict_type) = instance.type_named("dict") {
        if let Some(type_namespace) = instance.type_namespace(dict_type.cast()) {
            let method = make_native(instance, "fromkeys", dict_fromkeys_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "fromkeys", method);
        }
    }
    // **`function.__code__`／`__globals__` 进 `function` 的类型字典** ✓（第 214 轮）：
    // `Lib/types.py` 要 `type(FunctionType.__code__)` ✓ 与 `type(FunctionType.__globals__)` ✓
    // —— 类级取法先前取不到 ✗（与 `str.join` **同款** ✓）。
    if let Some(function_type) = instance.type_named("function") {
        if let Some(type_namespace) = instance.type_namespace(function_type.cast()) {
            let code_method = make_native(instance, "__code__", pyawa_core::function_code_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__code__", code_method);
            let globals_method = make_native(instance, "__globals__", pyawa_core::function_globals_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__globals__", globals_method);
        }
    }

    // **`str.join` 进 `str` 的类型字典** ✓（第 212 轮）：`Lib/types.py:52` 是 `type(str.join)` ✓ ——
    // 方法面只挂在**类型的 `getattr` 槽**上 ✗ ⇒ 类级取法（`str.join`）先前取不到 ✗。
    // **一次一个名字** ✓（第 181 轮的教训 ✓）：本轮只加 `join` ✓。
    if let Some(str_type) = instance.type_named("str") {
        if let Some(type_namespace) = instance.type_namespace(str_type.cast()) {
            if let Some(handler) = pyawa_core::str_method_native("join") {
                let method = make_native(instance, "join", handler);
                instance.dict_set(type_namespace, "join", method);
            }
        }
    }

    // **`object.__init__` 进 `object` 的命名空间** ✓（第 210 轮）。
    if let Some(object_type) = instance.type_named("object") {
        if let Some(type_namespace) = instance.type_namespace(object_type.cast()) {
            let method = make_native(instance, "__init__", pyawa_core::object_init_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__init__", method);
            // **`__str__`／`__repr__`** ✓（第 210 轮）：默认 `<X object at 0x…>` ✓（`Lib/types.py` 要 ✓）。
            let str_method = make_native(instance, "__str__", pyawa_core::object_text_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__str__", str_method);
            let repr_method = make_native(instance, "__repr__", pyawa_core::object_text_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__repr__", repr_method);
        }
    }
    // **`Ellipsis`／`NotImplemented` 两个名字** ✓（第 215 轮）：`Lib/types.py` 要 `type(Ellipsis)` ✓
    // 与 `type(NotImplemented)` ✓（`EllipsisType`／`NotImplementedType` ✓）。
    // `None`／`True`／`False` 走**解析字面量** ✓，不在这一档 ✓。
    instance.dict_set(namespace, "Ellipsis", instance.singletons().ellipsis());
    instance.dict_set(namespace, "NotImplemented", instance.not_implemented());

    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}

/// **`print`**（最小面）：`print(*args)` —— 实参**必须已经是 `str`** ✓（其余形态待 `str()` 落地 ✗）。
///
/// 目的地是**组合根装好的那个 `sys.stdout` 对象**（内建名字空间里的 `__stdout__` ✓）——
/// 字节经 `_io` 的文本层走 `fs` 域的 `write` 槽 ✓（`CM-26`：**不设临时 sink** ✓）。
fn print_native(
    instance: &Instance,
    _self_object: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let builtins = instance.builtins().ok_or(ExecError::Unsupported {
        opcode: 0,
        what: "`print` 需要内建名字空间（组合根装配）",
    })?;
    // 身份检查：`sys.stdout` 那个对象必须在（目的地与它是同一个 ✓）
    let _stdout = instance
        .dict_get(builtins, "__stdout__")
        .ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`sys.stdout` 尚未装配（`print` ⇒ `sys.stdout` ⇒ `_io` ⇒ `fs`）",
        })?;
    // 句柄与 `stdout` 对象**同源**（组合根一起装 ✓）：对象是身份、句柄是落点 ✓
    let handle_object = instance
        .dict_get(builtins, "__stdout_handle__")
        .ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`sys.stdout` 的句柄尚未装配（`print` ⇒ `sys.stdout` ⇒ `_io` ⇒ `fs`）",
        })?;
    let handle = instance
        .int_value(handle_object)
        .ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`__stdout_handle__` 不是整数",
        })? as u64;

    let mut bytes: Vec<u8> = Vec::new();
    for (index, argument) in args.iter().enumerate() {
        if index > 0 {
            bytes.push(b' ');
        }
        let text = instance.text_of(*argument).ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`print` 目前只接受 `str` 实参（`str()` 落地前，如实拒绝 ✓）",
        })?;
        bytes.extend_from_slice(text.as_bytes());
    }
    bytes.push(b'\n');
    crate::_io_module::write_bytes(instance, handle, &bytes)?;
    // **必须 `retain`**（返回给 VM 的是**新引用** ✓）：少了它，调用方会释放单例 ⇒
    // 双重释放 ⇒ 进程退出时报 `tcache_thread_shutdown(): unaligned tcache chunk detected` ✗
    // （第 94 轮实测：`print` 少了这一句 ✗）
    Ok(instance.retain(instance.singletons().none()))
}

/// 造一个原生可调用对象（**新引用**）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记（OM-13）");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        // `TypeObject::name` 要 `&'static str`：内建函数名是常量，泄漏一份即可
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 参数个数检查：不够就给统一的 `TypeError`（消息照参照实现的口径写）。
/// `dict.fromkeys(iterable, value=None)`（第 182 轮）：**先占位** ✗ —— 实现（遍历可迭代对象）随后补 ✓。
///
/// 为什么先占位：`Lib/types.py` 只需**取到**这个名字 ✓（它做 `type(dict.__dict__['fromkeys'])` ✓），
/// 而**类级方法**必须在**类型字典**里 ✓（实例方法面 `dict_getattr` 看不到它 ✗）。调用时**如实报未接线** ✓，
/// 不静默给错值 ✗。
// **`dict.fromkeys`** ✓：真实实现在 **core**（要看容器内部 ✓）⇒ 这里只转发 ✓（**一处真相** ✓）。
fn dict_fromkeys_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **本 crate `forbid(unsafe_code)`** ✗ ⇒ core 那个跨 crate 的入口是 **`safe fn`** ✓（第 184 轮 ✓）。
    pyawa_core::dict_fromkeys_native(instance, bound, args, kwargs)
}

fn need_args(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    count: usize,
) -> Result<(), ExecError> {
    if args.len() < count {
        let message = format!(
            "{name}() takes at least {count} argument{} ({} given)",
            if count == 1 { "" } else { "s" },
            args.len()
        );
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    Ok(())
}

/// 类型名（诊断消息里用）。
fn type_name(instance: &Instance, object: NonNull<Header>) -> String {
    instance.type_name(instance.type_of(object))
}

/// `abs(x)`：整数给整数、浮点给浮点；别的 `TypeError`。
///
/// 实测：`abs(True)` ⇒ `1`（**int**，不是 bool）；`abs("x")` ⇒
/// `bad operand type for abs(): 'str'`。
fn abs_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "abs", args, 1)?;
    let value = args[0];
    if let Some(integer) = instance.int_value(value) {
        return Ok(instance.new_int(integer.abs()));
    }
    if let Some(number) = instance.float_value(value) {
        return Ok(instance.new_float(number.abs()));
    }
    let message = format!("bad operand type for abs(): '{}'", type_name(instance, value));
    Err(instance.raise_builtin_error("TypeError", &message))
}

/// `len(x)`：`str`／`list`／`tuple`／`dict`／`set`；别的 `TypeError`。
///
/// 实测：`len(5)` ⇒ `object of type 'int' has no len()`。
fn len_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "len", args, 1)?;
    let value = args[0];
    match instance.length_of(value) {
        Some(length) => Ok(instance.new_int(length as i64)),
        None => {
            let message = format!(
                "object of type '{}' has no len()",
                type_name(instance, value)
            );
            Err(instance.raise_builtin_error("TypeError", &message))
        }
    }
}

/// `ord(s)`：单字符字符串 → 码点。
///
/// 实测：`ord("ab")` ⇒ `ord() expected a character, but string of length 2 found`。
fn ord_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "ord", args, 1)?;
    let value = args[0];
    let text = match instance.text_value(value) {
        Some(text) => text,
        None => {
            let message = format!(
                "ord() expected string of length 1, but {} found",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    // 长度按**码点**（`TS-22`）；本层 `str` 存 UTF-8 ⇒ 先按字符取
    let mut characters = text.chars();
    let first = characters.next();
    if first.is_none() || characters.next().is_some() {
        // 实测消息里的长度也是**字符数**
        let message = format!(
            "ord() expected a character, but string of length {} found",
            text.chars().count()
        );
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    Ok(instance.new_int(i64::from(first.expect("上面确认过有字符") as u32)))
}

/// `chr(i)`：码点 → 单字符字符串。
///
/// 实测：`chr(-1)`／`chr(0x110000)` ⇒ `chr() arg not in range(0x110000)`；
/// `chr(1.0)` ⇒ `'float' object cannot be interpreted as an integer`。
fn chr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "chr", args, 1)?;
    let value = args[0];
    let code = match instance.int_value(value) {
        Some(code) => code,
        None => {
            let message = format!(
                "'{}' object cannot be interpreted as an integer",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    let character = match u32::try_from(code).ok().and_then(char::from_u32) {
        Some(character) => character,
        None => {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "chr() arg not in range(0x110000)",
            ))
        }
    };
    Ok(instance.new_str(&character.to_string()))
}

/// `bin`／`oct`／`hex` 的共同实现（`base` 是 2／8／16，前缀照参照实现）。
fn radix_native(
    instance: &Instance,
    name: &str,
    base: u32,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, name, args, 1)?;
    let value = args[0];
    let integer = match instance.int_value(value) {
        Some(integer) => integer,
        None => {
            let message = format!(
                "'{}' object cannot be interpreted as an integer",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    let sign = if integer < 0 { "-" } else { "" };
    let magnitude = integer.unsigned_abs();
    let digits = match base {
        2 => format!("{magnitude:b}"),
        8 => format!("{magnitude:o}"),
        _ => format!("{magnitude:x}"),
    };
    let prefix = match base {
        2 => "0b",
        8 => "0o",
        _ => "0x",
    };
    Ok(instance.new_str(&format!("{sign}{prefix}{digits}")))
}

/// `bin(x)`：`bin(-5)` ⇒ `'-0b101'`；`bin(True)` ⇒ `'0b1'`。
fn bin_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "bin", 2, args)
}

/// `oct(x)`：`oct(8)` ⇒ `'0o10'`。
fn oct_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "oct", 8, args)
}

/// `hex(x)`：`hex(1.5)` ⇒ `'float' object cannot be interpreted as an integer`。
fn hex_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "hex", 16, args)
}

/// `callable(x)`：**有 `call` 槽就返回 `True`**（`OM-11`），否则 `False`。
fn callable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "callable", args, 1)?;
    let value = args[0];
    Ok(instance.new_bool(instance.is_callable(value)))
}

/// `isinstance(x, T)`：`T` 可以是类型，也可以是类型的元组。
///
/// 实测：`isinstance(1, 5)` ⇒
/// `isinstance() arg 2 must be a type, a tuple of types, or a union`。
fn isinstance_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "isinstance", args, 2)?;
    let subject = args[0];
    let matches = type_matches(instance, subject, args[1], "isinstance")?;
    Ok(instance.new_bool(matches))
}

/// `issubclass(T, U)`：`bool` 是 `int` 的子类，反向不是。
///
/// 实测：`issubclass(1, int)` ⇒ `issubclass() arg 1 must be a class`。
fn issubclass_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "issubclass", args, 2)?;
    let subject = args[0];
    // 注意：`issubclass(T, U)` 的**主题类型是 `T` 自己**（不是 `type_of(T)`，那是 `type`）
    let subject_type = match instance.as_type(subject) {
        Some(ty) => ty,
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "issubclass() arg 1 must be a class",
            ))
        }
    };
    let matches = type_matches_against(instance, subject_type, args[1], "issubclass")?;
    Ok(instance.new_bool(matches))
}

/// `isinstance` 的判定：`T` 是类型 ⇒ 直接比；是元组 ⇒ 逐个比。
fn type_matches(
    instance: &Instance,
    subject: NonNull<Header>,
    expected: NonNull<Header>,
    caller: &str,
) -> Result<bool, ExecError> {
    let subject_type = instance.type_of(subject);
    type_matches_against(instance, subject_type, expected, caller)
}

/// 判定"某个类型是不是 `expected` 描述的类型（或之一）"。
fn type_matches_against(
    instance: &Instance,
    subject_type: NonNull<pyawa_core::TypeObject>,
    expected: NonNull<Header>,
    caller: &str,
) -> Result<bool, ExecError> {
    if let Some(target) = instance.as_type(expected) {
        return Ok(instance.is_subtype(subject_type, target));
    }
    if let Some(items) = instance.tuple_items(expected) {
        for item in items {
            if type_matches_against(instance, subject_type, item, caller)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let message = format!("{caller}() arg 2 must be a type, a tuple of types, or a union");
    Err(instance.raise_builtin_error("TypeError", &message))
}

/// `repr(x)`：走 `OM-11` 的 `repr` 槽（`TS-44` 的口径）。
fn repr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "repr", args, 1)?;
    // `OM-11` 扩之后 `repr` 槽能表达失败 ⇒ 如实上抛（如 `TS-45` ①的位数上限）
    let text = instance.object_repr(args[0])?;
    Ok(instance.new_str(&text))
}

/// `iter(object)`（第 142 轮）：走执行器**同一处** `iter_value` ✓（`iter(迭代器)` 返回它自己 ✓）。
fn iter_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "iter", args, 1)?;
    instance.iter_object(args[0])
}

/// `pow(base, exp)`（第 152 轮）：**整数面** ✓（`pow(2, 10)` ✓；三参数（模）随后补 ✗）。
fn pow_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "pow", args, 2)?;
    if args.len() > 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "pow expected at most 3 arguments",
        ));
    }
    let base = instance
        .int_value(args[0])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "pow() 目前只接整数（浮点面随后补）"))?;
    let exponent = instance
        .int_value(args[1])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "pow() 目前只接整数（浮点面随后补）"))?;
    if exponent < 0 {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "pow() 的负指数（返回浮点）尚未接线",
        ));
    }
    let mut result: i64 = 1;
    for _ in 0..exponent {
        result = result.wrapping_mul(base);
    }
    Ok(instance.new_int(result))
}

/// `divmod(a, b)`（第 152 轮）：**整数面** ✓ ⇒ `(a // b, a % b)` ✓（整除**向下取整**，与参照一致 ✓）。
fn divmod_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "divmod", args, 2)?;
    let left = instance
        .int_value(args[0])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "divmod() 目前只接整数（浮点面随后补）"))?;
    let right = instance
        .int_value(args[1])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "divmod() 目前只接整数（浮点面随后补）"))?;
    if right == 0 {
        return Err(instance.raise_builtin_error("ZeroDivisionError", "integer division or modulo by zero"));
    }
    // **向下取整**（Python 口径 ✓）：Rust 的 `/` 是向零取整 ✗ ⇒ 自己算 ✓
    let mut quotient = left / right;
    let mut remainder = left % right;
    if remainder != 0 && (remainder < 0) != (right < 0) {
        quotient -= 1;
        remainder += right;
    }
    let pair = instance.new_tuple(vec![instance.new_int(quotient), instance.new_int(remainder)]);
    Ok(pair)
}

/// `round(number[, ndigits])`（第 152 轮）：**整数面** ✓（`round(7) == 7` ✓；浮点面随后补 ✗）。
fn round_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "round", args, 1)?;
    let number = instance
        .int_value(args[0])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "round() 目前只接整数（浮点面随后补）"))?;
    Ok(instance.new_int(number))
}

/// `range(...)`（第 148 轮）：用现成的两个迭代器拼 ✓（`count(start, step)` ＋ `islice` ✓，
/// **一处真相** ✓）。
///
/// **已知偏离**（如实登记 ✓）：参照里 `range` 是**类型对象**（有 `len`／`in`／下标 ✓），
/// 本层先给**迭代器** ✓ —— `for i in range(n)`／`list(range(n))` 这些最常见用法一致 ✓；
/// **负步长**未接 ✗（`islice` 不支持负步 ✓ ⇒ 如实报错 ✓）。
fn range_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "range", args, 1)?;
    let numbers: Vec<i64> = args
        .iter()
        .take(3)
        .map(|value| instance.int_value(*value))
        .collect::<Option<Vec<i64>>>()
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "range() 的参数要整数"))?;
    let (start, stop, step) = match numbers.as_slice() {
        [stop] => (0, *stop, 1),
        [start, stop] => (*start, *stop, 1),
        [start, stop, step] => (*start, *stop, *step),
        _ => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "range expected at most 3 arguments",
            ))
        }
    };
    if step == 0 {
        return Err(instance.raise_builtin_error("ValueError", "range() arg 3 must not be zero"));
    }
    if step < 0 {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "range() 的负步长尚未接线（islice 不支持负步）",
        ));
    }
    let span = stop - start;
    let count = if span <= 0 { 0 } else { (span + step - 1) / step };
    let inner = instance.new_count_iterator(start, step);
    Ok(instance.new_islice_iterator(inner, 0, count, 1))
}

/// `next(iterator[, default])`（第 142 轮）：走执行器**同一处** `advance` ✓（内建迭代器 ＋
/// `__next__` 协议 ✓）；耗尽时有 `default` 给 `default` ✓，没有就抛 `StopIteration` ✓。
fn next_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "next", args, 1)?;
    match instance.advance_iterator(args[0])? {
        Some(item) => Ok(item),
        None => match args.get(1) {
            Some(default) => Ok(*default),
            None => Err(instance.raise_builtin_error("StopIteration", "")),
        },
    }
}

/// `list([iterable])`（第 130 轮）：空表或把可迭代项收进来 ✓。
fn list_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "list", args, 0)?;
    let items = match args.first() {
        Some(iterable) => instance
            .iterable_items(*iterable)
            .ok_or_else(|| instance.raise_builtin_error("TypeError", "object is not iterable"))?,
        None => Vec::new(),
    };
    Ok(instance.new_list(items))
}

/// `tuple([iterable])`。
fn tuple_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "tuple", args, 0)?;
    let items = match args.first() {
        Some(iterable) => instance
            .iterable_items(*iterable)
            .ok_or_else(|| instance.raise_builtin_error("TypeError", "object is not iterable"))?,
        None => Vec::new(),
    };
    Ok(instance.new_tuple(items))
}

/// `set([iterable])`。
fn set_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "set", args, 0)?;
    let items = match args.first() {
        Some(iterable) => instance
            .iterable_items(*iterable)
            .ok_or_else(|| instance.raise_builtin_error("TypeError", "object is not iterable"))?,
        None => Vec::new(),
    };
    Ok(instance.new_set(items))
}

/// `dict()`（**最小面**：只接无参 ✓；从映射／键值对建表随后补 ✗）。
fn dict_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "dict() 目前只接无参（从映射／键值对建表随后补）",
        ));
    }
    Ok(instance.new_dict())
}

/// `str([object])`：字符串原样返回 ✓，其余走 `repr`（最小面 ✓）。
fn str_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "str", args, 0)?;
    let Some(value) = args.first() else {
        return Ok(instance.new_str(""));
    };
    if instance.type_of(*value) == instance.singletons().str_type() {
        return Ok(instance.retain(*value));
    }
    let text = instance.object_repr(*value)?;
    Ok(instance.new_str(&text))
}

/// `int([value])`：整数原样 ✓、字符串按十进制解析 ✓（最小面 ✓）。
fn int_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "int", args, 0)?;
    let Some(value) = args.first() else {
        return Ok(instance.new_int(0));
    };
    if let Some(number) = instance.int_value(*value) {
        return Ok(instance.new_int(number));
    }
    if let Some(text) = instance.text_of(*value) {
        return match text.trim().parse::<i64>() {
            Ok(number) => Ok(instance.new_int(number)),
            Err(_) => Err(instance.raise_builtin_error(
                "ValueError",
                &format!("invalid literal for int(): {text}"),
            )),
        };
    }
    if let Some(number) = instance.float_value(*value) {
        return Ok(instance.new_int(number as i64));
    }
    Err(instance.raise_builtin_error("TypeError", "int() 只接整数／字符串／浮点（最小面）"))
}

/// `float([value])`：浮点原样 ✓、整数与字符串转换 ✓（最小面 ✓）。
fn float_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "float", args, 0)?;
    let Some(value) = args.first() else {
        return Ok(instance.new_float(0.0));
    };
    if let Some(number) = instance.float_value(*value) {
        return Ok(instance.new_float(number));
    }
    if let Some(number) = instance.int_value(*value) {
        return Ok(instance.new_float(number as f64));
    }
    if let Some(text) = instance.text_of(*value) {
        return match text.trim().parse::<f64>() {
            Ok(number) => Ok(instance.new_float(number)),
            Err(_) => Err(instance.raise_builtin_error(
                "ValueError",
                &format!("could not convert string to float: {text}"),
            )),
        };
    }
    Err(instance.raise_builtin_error("TypeError", "float() 只接浮点／整数／字符串（最小面）"))
}

/// `bool([value])`：真假表 ✓（`bool_value` 就是引擎的口径 ✓）。
fn bool_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "bool", args, 0)?;
    let value = match args.first() {
        // **走执行器的通用真假判定**（第 131 轮）：`bool_value` 只认 bool／None ✗
        Some(value) => instance.truthiness_of(*value)?,
        None => false,
    };
    Ok(instance.retain(instance.singletons().boolean(value)))
}

/// `type(object)`：返回它的类型对象 ✓。
fn type_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "type", args, 1)?;
    // **返回既有对象必须 `retain`**（第 131 轮：不 retain ⇒ 调用方释放后 double free ✗）
    Ok(instance.retain(instance.type_of(args[0]).cast()))
}

/// `getattr(object, name[, default])` ✓（对象字典查名 ✓）。
fn getattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "getattr", args, 2)?;
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    // **走真正的属性通道** ✗（不是"把对象当字典查" ✗ —— 那对非字典是 UB ✓）
    match instance.attribute_optional_of(args[0], name)? {
        Some(found) => return Ok(found),
        None => {}
    }
    if let Some(default) = args.get(2) {
        return Ok(instance.retain(*default));
    }
    Err(instance.raise_builtin_error(
        "AttributeError",
        &format!("object has no attribute '{name}'"),
    ))
}

/// `hasattr(object, name)` ✓。
fn hasattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "hasattr", args, 2)?;
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    let found = instance.attribute_optional_of(args[0], name)?.is_some();
    Ok(instance.retain(instance.singletons().boolean(found)))
}

/// `globals()`（第 156 轮）：**正在执行的那一帧的全局映射** ✓（`Frame` 的 `globals` 那格 ✓ ——
/// `BC-57` 已保证函数帧取的是函数的 `__globals__` ✓ ⇒ 模块级与函数里都对 ✓）。
/// 没有正在执行的帧（不该发生 ✓）⇒ 如实报错 ✗，不用空字典冒充 ✓。
fn globals_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    match instance.current_globals() {
        Some(globals) => Ok(instance.retain(globals)),
        None => Err(instance.raise_builtin_error(
            "NotImplementedError",
            "globals() 需要正在执行的帧（执行器还没挂上当前帧）",
        )),
    }
}

/// `setattr(object, name, value)`（第 148 轮）：走 `STORE_ATTR` 同一条路 ✓（**一处真相** ✓）。
fn setattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "setattr", args, 3)?;
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    instance.set_attribute_value(args[0], name, args[2])?;
    Ok(instance.retain(instance.singletons().none()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_lists_what_it_implements() {
        let instance = Instance::new();
        let namespace = build(&instance);
        for name in IMPLEMENTED {
            assert!(
                instance.dict_get(namespace, name).is_some(),
                "{name} 应当在 builtins 命名空间里"
            );
        }
        // `__build_class__` 来自核心（OM-14）
        assert!(instance.dict_get(namespace, "__build_class__").is_some());
        // 需要输出通道的 `print` 不在这里（口径待裁，见 §5.2.2）
        assert!(instance.dict_get(namespace, "print").is_some());
    }
}

// ---- `min`／`max`／`sorted`（`§9.4` 第 4 条的纯计算面；`CM-4` 的合约见 `SPEC-c-modules.md` §5.2.2）----

/// 取一个关键字实参（`None` 与"没给"不同 ⇒ 返回 `Option<Option<...>>`）。
fn keyword(
    instance: &Instance,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    name: &str,
) -> Result<Option<NonNull<Header>>, ExecError> {
    for (key, value) in kwargs {
        if instance.text_value(*key).as_deref() == Some(name) {
            return Ok(Some(*value));
        }
    }
    Ok(None)
}

/// 关键字里有没有这个名字（不需要值）。
fn has_keyword(kwargs: &[(NonNull<Header>, NonNull<Header>)], instance: &Instance, name: &str) -> bool {
    kwargs
        .iter()
        .any(|(key, _)| instance.text_value(*key).as_deref() == Some(name))
}

/// 拒绝不认识的关键字（实测消息：`min() got an unexpected keyword argument 'reverse'`）。
fn reject_unknown_keywords(
    instance: &Instance,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    allowed: &[&str],
    function: &str,
) -> Result<(), ExecError> {
    for (key, _) in kwargs {
        let name = instance.text_value(*key).unwrap_or_default();
        if !allowed.contains(&name.as_str()) {
            let message = format!("{function}() got an unexpected keyword argument '{name}'");
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    }
    Ok(())
}

/// `min`／`max` 的取值列表：**单实参** ⇒ 迭代它（实测 `'int' object is not iterable`）；
/// **多实参** ⇒ 实参本身就是候选。
fn candidate_items(
    instance: &Instance,
    args: &[NonNull<Header>],
    function: &str,
) -> Result<Vec<NonNull<Header>>, ExecError> {
    if args.is_empty() {
        let message = format!("{function} expected at least 1 argument, got 0");
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    if args.len() == 1 {
        return match instance.iterable_items(args[0]) {
            Some(items) => Ok(items),
            None => {
                let message = format!("'{}' object is not iterable", type_name(instance, args[0]));
                Err(instance.raise_builtin_error("TypeError", &message))
            }
        };
    }
    Ok(args.iter().map(|item| instance.retain(*item)).collect())
}

/// 求 `key(值)`；没给 `key` 时就是值本身（都返回**新引用**，由调用方归还）。
fn decorated_key(
    instance: &Instance,
    value: NonNull<Header>,
    key: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    match key {
        // `call_value` **接手**实参表 ⇒ 先为它新增一份（`retain`），调用方那份仍归自己
        Some(callable) => pyawa_core::call_value(instance, callable, &[instance.retain(value)], &[]),
        None => Ok(instance.retain(value)),
    }
}

/// `min`／`max` 的公共实现：`want` 是"这个序才算更优"。
fn extremum(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
    want: core::cmp::Ordering,
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &["key", "default"], function)?;
    let key = keyword(instance, kwargs, "key")?;
    // `key=None` 与"没给"同义（实测 `sorted([1], key=None)` 正常）
    let key = match key {
        Some(value) if instance.type_of(value) == instance.singletons().none_type() => None,
        other => other,
    };
    let default_given = has_keyword(kwargs, instance, "default");
    let default = keyword(instance, kwargs, "default")?;
    let items = candidate_items(instance, args, function)?;
    if items.is_empty() {
        if default_given {
            for item in &items {
                instance.release(*item);
            }
            return Ok(instance.retain(default.expect("上面确认过给了")));
        }
        let message = format!("{function}() iterable argument is empty");
        return Err(instance.raise_builtin_error("ValueError", &message));
    }
    let mut best = items[0];
    let mut best_key = decorated_key(instance, best, key)?;
    for item in items.iter().skip(1) {
        let item_key = decorated_key(instance, *item, key)?;
        match instance.order_of(item_key, best_key) {
            Some(ordering) if ordering == want => {
                instance.release(best_key);
                best = *item;
                best_key = item_key;
            }
            Some(_) => instance.release(item_key),
            None => {
                // 实测消息把**正在比的那个**放前面：`min(1, 'a')` ⇒ `'str' and 'int'`
                let message = format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    type_name(instance, item_key),
                    type_name(instance, best_key)
                );
                instance.release(item_key);
                instance.release(best_key);
                for item in &items {
                    instance.release(*item);
                }
                return Err(instance.raise_builtin_error("TypeError", &message));
            }
        }
    }
    let result = instance.retain(best);
    instance.release(best_key);
    for item in &items {
        instance.release(*item);
    }
    Ok(result)
}

/// `min(*args, key=None, default=…)`。
fn min_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    extremum(instance, args, kwargs, "min", core::cmp::Ordering::Less)
}

/// `max(*args, key=None, default=…)`。
fn max_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    extremum(instance, args, kwargs, "max", core::cmp::Ordering::Greater)
}

/// `sorted(iterable, /, *, key=None, reverse=False)`：稳定排序，结果是一个**新列表**。
fn sorted_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &["key", "reverse"], "sort")?;
    if args.len() != 1 {
        let message = format!("sorted expected 1 argument, got {}", args.len());
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    let key = keyword(instance, kwargs, "key")?;
    let key = match key {
        Some(value) if instance.type_of(value) == instance.singletons().none_type() => None,
        other => other,
    };
    let reverse = match keyword(instance, kwargs, "reverse")? {
        Some(value) => instance.bool_value(value).unwrap_or(false),
        None => false,
    };
    let items = match instance.iterable_items(args[0]) {
        Some(items) => items,
        None => {
            let message = format!("'{}' object is not iterable", type_name(instance, args[0]));
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    // 装饰：把每个元素与它的 key 配对（key 只算**一次**，与参照实现一致）
    let mut decorated: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(items.len());
    for item in &items {
        let item_key = decorated_key(instance, *item, key)?;
        decorated.push((*item, item_key));
    }
    let mut failure: Option<String> = None;
    decorated.sort_by(|left, right| match instance.order_of(left.1, right.1) {
        Some(ordering) => ordering,
        None => {
            failure.get_or_insert_with(|| {
                format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    type_name(instance, left.1),
                    type_name(instance, right.1)
                )
            });
            core::cmp::Ordering::Equal
        }
    });
    if let Some(message) = failure {
        for (_, item_key) in &decorated {
            instance.release(*item_key);
        }
        for item in &items {
            instance.release(*item);
        }
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    // `reverse=True`：参照实现是**排完再反转**（等值元素保持原序）
    if reverse {
        decorated.reverse();
    }
    let result = instance.new_list(
        decorated.iter().map(|(item, _)| instance.retain(*item)).collect(),
    );
    for (_, item_key) in &decorated {
        instance.release(*item_key);
    }
    for item in &items {
        instance.release(*item);
    }
    Ok(result)
}

// ---- `sum`／`all`／`any`（上一条把迭代入口铺好之后就能做；合约见 `SPEC-c-modules.md` §5.2.2）----

/// 取一个可迭代对象摊成一批**新引用**；不是可迭代的就报实测那条 `TypeError`。
fn items_of(instance: &Instance, object: NonNull<Header>) -> Result<Vec<NonNull<Header>>, ExecError> {
    match instance.iterable_items(object) {
        Some(items) => Ok(items),
        None => {
            let message = format!("'{}' object is not iterable", type_name(instance, object));
            Err(instance.raise_builtin_error("TypeError", &message))
        }
    }
}

/// `sum(iterable, /, start=0)`：从 `start` 起累加（数值塔内），比不了就报参照实现那条。
fn sum_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &[], "sum")?;
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sum() takes at least 1 positional argument (0 given)",
        ));
    }
    let mut total = match args.get(1) {
        Some(start) => instance.retain(*start),
        None => instance.new_int(0),
    };
    for item in items_of(instance, args[0])? {
        let next = instance.add_values(total, item);
        instance.release(item);
        match next {
            Some(value) => {
                instance.release(total);
                total = value;
            }
            None => {
                let message = format!(
                    "unsupported operand type(s) for +: '{}' and '{}'",
                    type_name(instance, total),
                    type_name(instance, item)
                );
                instance.release(total);
                return Err(instance.raise_builtin_error("TypeError", &message));
            }
        }
    }
    Ok(total)
}

/// `all(iterable)`：空 ⇒ `True`。
fn all_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    truth_reducer(instance, args, kwargs, "all", true)
}

/// `any(iterable)`：空 ⇒ `False`。
fn any_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    truth_reducer(instance, args, kwargs, "any", false)
}

/// `all`／`any` 的公共实现：`empty` 是空可迭代时的结果。
fn truth_reducer(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
    empty: bool,
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &[], function)?;
    if args.len() != 1 {
        // 实测：`all() takes exactly one argument (0 given)`
        let message = format!("{function}() takes exactly one argument ({} given)", args.len());
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    let mut result = empty;
    for item in items_of(instance, args[0])? {
        let truth = instance.truth_of(item);
        instance.release(item);
        // `any` 见到真就定；`all` 见到假就定
        if truth != empty {
            result = truth;
            break;
        }
    }
    Ok(instance.new_bool(result))
}
