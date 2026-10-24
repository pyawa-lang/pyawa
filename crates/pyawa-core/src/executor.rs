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
//! 整数的**值域**：**任意精度**（`TS-45`／`P1-11` 已落地）——单例表只决定"内联还是分配"，
//! **不是**值域。历史上这里有"结果必须落在单例区间内"的说法，已随 `P1-11` 作废。

pub(crate) use core::cell::{Cell, RefCell};
pub(crate) use core::ptr::NonNull;

pub(crate) use crate::code::CodeObject;
pub(crate) use crate::bigint::IntValue;
pub(crate) use crate::decode::{parse_exception_table, DecodeError, Decoder, Instruction};
pub(crate) use crate::frame::{Frame, FrameError};
pub use crate::executor::subscript::*;
pub use crate::executor::call::*;
pub use crate::executor::arithmetic::*;
pub use crate::executor::attribute::*;
pub use crate::executor::import::*;
pub(crate) use crate::executor::message::*;
pub use crate::executor::values::*;
pub(crate) use crate::executor::ctrls::*;
pub use crate::executor::runtime::*;
pub(crate) use crate::executor::format::*;
pub(crate) use crate::executor::iter_advance::*;
pub use crate::executor::iter::*;
pub use crate::executor::protocol::*;
pub(crate) use crate::header::Header;
pub(crate) use crate::instance::Instance;
pub(crate) use crate::type_object::TypeObject;
pub(crate) use crate::opcode;
pub(crate) use crate::refcount::{Owned, PyRef};
pub(crate) use crate::builtin_objects::{
    AsendObject, BoolObject, ExceptionObject,
    GeneratorObject, MethodObject, DictObject, FloatObject, FunctionObject,
    IntObject, ListObject, SetObject, StrObject, TupleObject,
};
pub(crate) use crate::singleton::{SMALL_INT_MAX, SMALL_INT_MIN};
pub(crate) use crate::value::Value;

/// `execute` 的两种收尾（生成器要把"让出"与"返回"分开）。
pub enum ExecOutcome<'a> {
    /// 正常返回一个值。
    Returned(Value<'a>),
    /// **让出**一个值（生成器挂起：值栈已在帧的恢复点里，`BC-47`）。
    Yielded(NonNull<Header>),
}

/// 单条指令的结果。
enum Step<'a> {
    Continue,
    Return(Value<'a>),
    Yield(NonNull<Header>),
}

/// 执行失败的形态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecError {
    /// 解码失败（`BC-32`…`BC-36`）。
    Decode(DecodeError),
    /// 帧操作失败（`BC-43`：值栈越界等）。
    Frame(FrameError),
    /// 指令还没接线（`BC-49` 的全表随 M2 补齐）。
    NotImplemented { opcode: u8 },
    /// 本实例被请求中断（`AB-5`①：宿主 `pa_interrupt` ⇒ 执行类函数返回 `PA_ERR_INTERRUPT`）。
    Interrupted,
    /// 指令接线了，但这个形态／类型还没接线（协议槽位、大整数、容器……）。
    Unsupported { opcode: u8, what: &'static str },
    /// 读到未绑定的局部槽（CPython 的 `UnboundLocalError` 时机）。
    UnboundLocal { slot: usize },
    /// 抛出了一个 Python 异常（**异常对象由实例的 `pending_exception` 保活**）。
    ///
    /// `BC-60` ②：异常状态按实例存；这里只带一个借用的裸引用。
    Raised { exception: NonNull<Header> },
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
        let payload = unsafe { &*raw.as_ptr().cast::<IntObject>() }.value.clone();
        if let Some(value) = payload.to_i64() {
            if (SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
                release(instance, raw); // 单例由实例持有，交回我们这份即可
                return Value::small_int(value);
            }
        }
    }
    // SAFETY: 我们持有 raw 的那份新引用，转交给守卫。
    Value::Object(unsafe { PyRef::from_raw(raw, instance) })
}

/// 把一批**新引用**交出去造一个容器对象并压栈。
fn push_container<T: crate::header::PyObject>(
    instance: &Instance,
    frame: &Frame,
    object: T,
) -> Result<(), ExecError> {
    let owned = instance.alloc(object);
    frame.push(owned.into_raw().cast::<Header>())?;
    Ok(())
}

/// 迭代器类型的名字（**照探测表取**；`str` 的迭代器在这台机器上叫 `str_ascii_iterator`）。
pub(crate) const ITERATOR_TYPE_NAMES: [&str; 30] = [
    // **`reversed`** ✓（第 727 轮）：通用反向迭代器（序列协议 ✓）。
    "reversed",
    "tuple_iterator",
    "list_iterator",
    "str_ascii_iterator",
    "bytes_iterator",
    // **`bytearray` 的迭代器** ✓（第 644 轮）：它一直是**真迭代器类型**（`IteratorObject` 槽 ✓），
    // 只是**名字表里漏了** ✗ ⇒ `list(bytearray(…))` 落进"没有 `__next__`"那条协议回退 ✓
    //（实测报"这个对象既不是内建迭代器，也没有 `__next__`" ✗）。
    "bytearray_iterator",
    // **`reversed(list)` 的迭代器** ✓（第 227 轮）：`_collections_abc.py:75` 要 `type(iter(reversed([])))` ✓。
    "list_reverseiterator",
    // **`range(<超出 i64 的上限>)`** ✓（第 228 轮）：`_collections_abc.py:77` 要它 ✓。
    "longrange_iterator",
    // **`range()` 的常规迭代器** ✓（第 228 轮）：参照的名字 ✓。
    "range_iterator",
    // **`zip()` 的迭代器** ✓（第 229 轮）：参照的名字也是 `zip` ✓。
    "zip",
    "dict_keyiterator",
    "set_iterator",
    // `itertools` 的（Pyawa 专有类型，`SPEC-c-modules.md` §5.2.6）
    "count",
    "repeat",
    "islice",
    "chain",
    "takewhile",
    "dropwhile",
    "filterfalse",
    "accumulate",
    "starmap",
    "cycle",
    "pairwise",
    "batched",
    "zip_longest",
    "compress",
    "combinations",
    "combinations_with_replacement",
    "permutations",
    "product",
];

/// 属性查找的结果。
pub(crate) enum Attribute {
    /// 一个**新引用**（`OM-11` 的 `getattr` 槽交出来的，直接压栈即可）。
    Owned(NonNull<Header>),
    /// 一个普通值（**借用**的裸引用）。
    Value(NonNull<Header>),
    /// 类型字典里查到的是函数 ⇒ 取方法：函数 ＋ 要绑的 `self`（都是**借用**）。
    Method {
        /// 函数对象（由类型字典持有）。
        function: NonNull<Header>,
        /// 要绑上去的实例。
        this: NonNull<Header>,
    },
}

/// **`super` 的属性查找** ✓（第 233 轮）：在 `__self__` 的 MRO 上、**定义类之后**找 ✓。
///
/// **`__new__` 的 `__self__` 是类对象**（task-6 ✓）：参照的 `super.__getattribute__` 用**该类自己的
/// `tp_mro`**（`PyType_Check(obj) ⇒ ((PyTypeObject *)obj)->tp_mro`），**不是**元类的 MRO ✗ ——
/// 先前一律 `type_of(__self__)` ⇒ `__new__` 里的零参 `super().__new__(cls)` 会去 `[type, object]` 上找
/// （拿到 `type.__new__` 后按类当命名空间调 ⇒ 报"至少要 3 个实参"✗，模块级类也一样）。
///
/// 函数 ⇒ 绑到 `__self__`（与实例方法同款 ✓）；其余 ⇒ 原样给（**如实说** ✗：描述符的 `__get__` 随后补 ✓）。
pub(crate) fn super_lookup(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Option<Attribute>, ExecError> {
    // SAFETY: object 是存活的 super 对象（载荷是 `AttributeObject` ✓）。
    let attrs = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::AttributeObject>() };
    let Some(dict) = attrs.attributes() else {
        return Ok(None);
    };
    let Some(thisclass) = instance.dict_get(dict, "__thisclass__") else {
        return Ok(None);
    };
    let Some(this) = instance.dict_get(dict, "__self__") else {
        return Ok(None);
    };
    // **`__new__` 的 `__self__` 是类对象**（类体里的 `__new__` 被包成 staticmethod ⇒ 第一个实参是类）：
    // 参照的 `super.__getattribute__` 对类对象用它的**自己的 `tp_mro`** ✓
    //（`PyType_Check(obj) ⇒ ((PyTypeObject *)obj)->tp_mro`），否则用 `type(__self__)` 的 MRO ——
    // 本层先只对 `__new__` 放开 ✗：`__init_subclass__` 这类**隐式 classmethod** 的类对象 `super()`
    // 若也改走自己的 MRO，会命中 `generic_init_subclass_native` 的简化条件（`params.is_empty()` 一律报
    // `Cannot inherit from plain Generic`）⇒ `import typing` 当场红 ✗ —— 那一格（`builtin/typing.rs`）
    // 不在本任务写域，待它按参照的 `Generic in cls.__bases__`／`__orig_bases__` 口径对齐后再放开。
    let this_type = if name == "__new__" && instance.is_type_object(this) {
        this.cast::<TypeObject>()
    } else {
        instance.type_of(this)
    };
    let stop = thisclass.cast::<TypeObject>();
    // SAFETY: this_type 由注册表持有。
    let mro = unsafe { this_type.as_ref() }.mro();
    // **定义类不在被查的 MRO 里** ✓ ⇒ **整条 MRO 都算数** ✓（第 234 轮实测的形态 ✓）：
    // `ABCMeta.__new__` 里的 `super()` ⇒ `super(ABCMeta, ABCMeta)` ✓ —— `ABCMeta` 是**元类自己** ✓，
    // 它**不在** `type(ABCMeta).__mro__`（＝`[type, object]`）里 ✗ ⇒ 若仍要求"跳过定义类" ⇒
    // 永远跳不过去 ⇒ 报 `'super' object has no attribute '__new__'` ✗。
    let stop_in_mro = mro.iter().any(|entry| *entry == stop);
    let mut after = !stop_in_mro;
    for entry in mro {
        if after {
            let entry_name = unsafe { entry.as_ref() }.name();
            let resolved_new = instance.type_lookup(entry, name);
            let object_default_new = instance
                .type_named("object")
                .and_then(|ty| instance.type_lookup(ty, "__new__"));
            // 判据（第 608 轮修 ✓）：**该条目自己有 `new` 槽** ✓ 且（查不到 ✓ 或查到的就是 `object` 层默认 ✓）
            // ⇒ 用**该条目**的槽 ✓；**排除 `object`／`type`** ✗（对它们桥接会波及元类建类 ✓ —— 第 607 轮那条红的来源 ✓）。
            if name == "__new__"
                && entry_name != "object"
                && entry_name != "type"
                && unsafe { entry.as_ref() }.slots().new.is_some()
                && (resolved_new.is_none() || resolved_new == object_default_new)
            {
                if let Some(bridge) = crate::builtin_objects::new_bridge_function(instance) {
                    return Ok(Some(Attribute::Method {
                        function: bridge,
                        this: entry.cast::<Header>(),
                    }));
                }
            }
            if let Some((owner, found)) = instance.type_lookup_owner(entry, name) {
                // **只认"本条目自己定义的"** ✓（task-24 尾件真 bug 修 ✗）：`type_lookup(entry, name)` 会把
                // **该条目祖先**（尤其 `object` 的默认实现 `__init__`／`__new__`）提前拉进来 ✗ ——
                // 于是 `super()` 沿 MRO 走到**第一个中间基类**就命中 `object` 的默认 ⇒ **跳过真正的下一层** ✗。
                // 实测 `class _SpecialGenericAlias(_NotIterable, _BaseGenericAlias)` 的
                // `super().__init__(origin, inst=inst, name=name)` 落到了 `object.__init__` ⇒ `_name` 没设 ⇒
                // `AttributeError: _name` ✓（`Lib/typing.py` 的 `ByteString = _DeprecatedGenericAlias(...)` 就卡这 ✓）。
                if owner != entry {
                    continue;
                }
                // **两族都要绑** ✓（第 106 轮真 bug 修 ✗）：先前只认 `function` ✗ —— **原生方法**
                // （`builtin_function_or_method` ✓，如 `dict.__init__` ✓）被原样交出 ⇒ 调用时没有
                // `self` ✗（`Lib/enum.py` 的 `EnumDict.__init__` 里那句 `super().__init__()` 同型 ✓）。
                let found_type = instance.type_of(found);
                // **兜底：`__new__` 落到"默认实现"时，去 `self`（是类 ✓）自己的 MRO 里找内建条目** ✓
                //（第 611 轮 ✓）。为什么需要 ✗：`super(N, cls).__new__(cls, value)` 要找到 **`int` 的 `new` 槽** ✓，
                // 而主路按 `type_of(this)`（＝`type` ✗）走 ⇒ 只会拿到 `type.__new__`／`object.__new__` ✗
                //（实测报"`type.__new__` 至少要 3 个实参" ✓，`re/_constants.py:70` 同型 ✓）。
                // **为什么只认"内建条目"** ✓：Python 定义的类（`has_generic_allocation()` ✓，如 `ABCMeta` ✓）
                // 不该在这里被桥接 ✗ —— 那正是第 607/608 轮把 `meta_path_shapes` 打红的那条路 ✓。
                if name == "__new__"
                    && (Some(found)
                        == instance
                            .type_named("object")
                            .and_then(|ty| instance.type_lookup(ty, "__new__"))
                        || Some(found)
                            == instance
                                .type_named("type")
                                .and_then(|ty| instance.type_lookup(ty, "__new__")))
                    && instance.is_type_object(this)
                {
                    // SAFETY: this 是存活类对象，由注册表持有。
                    for candidate in unsafe { this.cast::<TypeObject>().as_ref() }.mro() {
                        // SAFETY: candidate 由注册表持有。
                        let candidate_ref = unsafe { candidate.as_ref() };
                        if candidate_ref.name() == "object" || candidate_ref.name() == "type" {
                            continue;
                        }
                        if candidate_ref.has_generic_allocation() {
                            continue;
                        }
                        if candidate_ref.slots().new.is_none() {
                            continue;
                        }
                        if let Some(bridge) = crate::builtin_objects::new_bridge_function(instance) {
                            return Ok(Some(Attribute::Method {
                                function: bridge,
                                this: candidate.cast::<Header>(),
                            }));
                        }
                    }
                }
                // **`staticmethod`／`classmethod` 是描述符** ✓（task-6 连带修 ✗）：`__self__` 是类对象时，
                // 上面按"该类自己的 MRO"查（task-6）会走到**父类 Python 定义**的 `__new__`／classmethod
                // ⇒ 类字典里存的是**包装对象** ✓，参照的 `super().__getattribute__` 走 `__get__` 解包 ✓
                //（先前一律按元类 MRO 查、走不到这一格 ⇒ 漏着没暴露；真 `_collections_abc` 的
                // `_DeprecateByteStringMeta.__new__` 当场报 `'staticmethod' object is not callable` ✗）。
                if Some(found_type) == instance.type_named("staticmethod") {
                    // SAFETY: 类型身份刚确认 ⇒ 载荷是 `StaticMethodObject`。
                    let inner = unsafe {
                        &*found.as_ptr().cast::<crate::builtin_objects::StaticMethodObject>()
                    }
                    .function();
                    // `staticmethod.__get__` 不绑 `self` ⇒ 交**裸函数**（`LOAD_ATTR` 的方法位会补 NULL 槽）。
                    return Ok(Some(Attribute::Value(inner)));
                }
                if Some(found_type) == instance.type_named("classmethod") {
                    // SAFETY: 类型身份刚确认 ⇒ 载荷是 `ClassMethodObject`。
                    let inner = unsafe {
                        &*found.as_ptr().cast::<crate::builtin_objects::ClassMethodObject>()
                    }
                    .function();
                    // 参照口径：`classmethod.__get__` 绑**类**（实例语境绑 `type(self)`）。
                    let bound = if instance.is_type_object(this) {
                        this
                    } else {
                        instance.type_of(this).cast::<Header>()
                    };
                    return Ok(Some(Attribute::Method {
                        function: inner,
                        this: bound,
                    }));
                }
                if found_type == builtin_type(instance, "function")
                    || found_type == builtin_type(instance, "builtin_function_or_method")
                {
                    return Ok(Some(Attribute::Method {
                        function: found,
                        this,
                    }));
                }
                return Ok(Some(Attribute::Value(found)));
            }
        }
        if entry == stop {
            after = true;
        }
    }
    Ok(None)
}

/// **PEP 562 的模块级 `__getattr__`**（task-24 尾件 ✓）：`from M import name` 与
/// `M.name` 在名字缺失时都要问**模块自己的** `__getattr__` ✓ —— 它在**模块命名空间**里 ✓，
/// **不在**模块类型的 MRO 上 ✗（所以 `attribute_lookup_with_getattr` 那条通道够不着 ✓）。
/// 成功给**新引用**的值 ✓、失败（包括钩子自己抛 `AttributeError` ✓）给 `None` ✓。
///
/// 动因：`Lib/typing.py:3827` 的 `def __getattr__(attr)` 懒建 `Match`／`Pattern` ✓
/// ⇒ 先前 `from typing import Match`（`importlib.metadata` 那 8 个模块 ✓）报
/// `cannot import name 'Match' from 'typing'` ✗。
pub(crate) fn module_getattr_missing(
    instance: &Instance,
    module: NonNull<Header>,
    name: &str,
) -> Option<NonNull<Header>> {
    // **只对模块生效** ✓：用户实例的 `__getattr__` 走**类型 MRO** 那条路 ✓（实例字典里同名键
    // 不该被当钩子调 ✓）。
    if instance.type_name(instance.type_of(module)) != "module" {
        return None;
    }
    let namespace = crate::mounted_instance_dict(instance, module)?;
    let hook = instance.dict_get(namespace, "__getattr__")?;
    let name_object = instance.new_str(name);
    // `call_value` 自己给实参新增引用 ✓ ⇒ 我们这份用完即还 ✓。
    let outcome = crate::executor::call::call_value(instance, hook, &[name_object], &[]);
    instance.release(name_object);
    outcome.ok()
}

/// **经 `fs` 域把一个文件读成文本**（`IM-15`：I/O 一律走能力域 ✓，本层不碰平台 ✓ `CX-4`）。
pub(crate) fn read_file_through_fs(instance: &Instance, path: &[u8]) -> Option<String> {
    let handle = instance
        .fs_open(path, pyawa_capabilities::fs::open_flag::RDONLY, 0)
        .ok()?;
    let mut bytes: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match instance.fs_read(handle, &mut buffer) {
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(_) => {
                let _ = instance.fs_close(handle);
                return None;
            }
        }
    }
    let _ = instance.fs_close(handle);
    if crate::diag::flag("PYAWA_TRACE_IMPORT") {
        // **诊断** ✓（第 200 轮）：读出多少字节 ✓（好与磁盘上的大小对照 ✗）。
        let path_text = String::from_utf8_lossy(path);
        eprintln!("[读文件] {path_text} ⇒ {} 字节", bytes.len());
    }
    String::from_utf8(bytes).ok()
}

/// **`__build_class__`**：用一个"局部变量是映射"的帧跑**类体**。
///
/// `body` 是类体函数（`MAKE_FUNCTION` 造出来的那个），`namespace` 是类命名空间。
pub(crate) fn run_class_body(
    instance: &Instance,
    body: NonNull<Header>,
    namespace: NonNull<Header>,
) -> Result<(), ExecError> {
    // 类体函数的 code object（`OM-11`：函数持有它）
    // SAFETY: body 由调用方保证存活。
    let body_ref = unsafe { &*body.as_ptr().cast::<FunctionObject>() };
    let code = body_ref.code();
    let globals = body_ref.globals();
    // **类体函数的闭包** ✓（task-24 轮 2 步① ✓）：`def outer(x): class C: y = x` 里
    // 类体读 `x` 走 `LOAD_DEREF` ⇒ 帧必须装上这份 cell ✓（与函数调用同口径 ✓）。
    let closure = function_closure(instance, body);
    let frame = crate::classes::class_body_frame(instance, code, namespace, globals, &closure);
    let frame = crate::Owned::new(frame, instance);
    match execute(instance, &frame)? {
        ExecOutcome::Returned(value) => {
            // 类体正常的收尾：`LOAD_CONST None; RETURN_VALUE`
            let raw = value_into_raw(instance, value);
            release(instance, raw);
            Ok(())
        }
        ExecOutcome::Yielded(_) => Err(ExecError::Unsupported {
            opcode: 0,
            what: "类体不该让出",
        }),
    }
}

/// **`truthiness` 的公开入口**（`TS-40` 的真值口径）。
///
/// stdlib 里需要"按 Python 口径判真值"的模块（`operator.truth`／`not_` 一类）用它 ——
/// **不要**在 stdlib 里另写一份真值规则（那是两处真相，`AGENTS.md` 禁止）。
/// `opcode` 只用于报错时指明来源（照内部那份的用法传即可）。
pub fn truthiness_public(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<bool, ExecError> {
    truthiness(instance, raw, opcode)
}

/// 序列重复的安全上限（**如实报 `MemoryError`** 而不是硬扛 ✓）：字符／字节数与元素数分开算 ✓。
pub(crate) const MAX_REPEAT_BYTES: usize = 1 << 26;
pub(crate) const MAX_REPEAT_ITEMS: usize = 1 << 24;

/// **一元运算的公开入口**（`operator.neg`／`pos`／`abs`／`invert`）。
///
/// `symbol` 取 `"-"`／`"+"`／`"abs"`／`"~"`；非整数按**参照实测**的消息报
/// `TypeError: bad operand type for unary -: 'str'`。
pub fn unary_public(
    instance: &Instance,
    operand: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    if let Some(value) = instance.int_of(operand) {
        // `-`／`+`／`abs`／`~` 全走任意精度（`TS-45`）
        let wide = value.to_bigint();
        let result = match symbol {
            "-" => wide.neg(),
            "+" => wide,
            "abs" => wide.abs(),
            "~" => wide.invert(),
            _ => {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "unary_public 收到了没见过的一元运算符",
                })
            }
        };
        return Ok(instance.new_int_value(IntValue::from_big(result)));
    }
    // **浮点的一元面**（第 318 轮）：`-1.5`／`+1.5`／`abs(-1.5)` —— 先前落到最后那条
    // `bad operand type for unary -: 'float'` ✗（`Lib/` 里浮点遍地都是 ✓）。
    // SAFETY: operand 由调用方保证存活。
    if unsafe { operand.as_ref() }.ty() == builtin_type(instance, "float") {
        // SAFETY: 类型身份刚确认 ⇒ `FloatObject` 载荷。
        let value = unsafe { &*operand.as_ptr().cast::<FloatObject>() }.value();
        let result = match symbol {
            "-" => -value,
            "+" => value,
            "abs" => value.abs(),
            _ => {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "unary_public 收到了没见过的一元运算符（浮点）",
                })
            }
        };
        return Ok(instance.new_float(result));
    }
    // **实例的 dunder 面**（第 219 轮；`MS-19`：能力缺口必须修 ✓）：`-obj` ⇒ `__neg__` ✓、
    // `+obj` ⇒ `__pos__` ✓、`abs(obj)` ⇒ `__abs__` ✓、`~obj` ⇒ `__invert__` ✓。
    // 先前实例**一律**落到最后那句 `TypeError` ✗ —— 实测 `Lib/datetime` 族
    // （`bad operand type for unary -: 'timedelta'` ✓）就是这么被挡住的 ✗。
    let dunder = match symbol {
        "-" => "__neg__",
        "+" => "__pos__",
        "abs" => "__abs__",
        "~" => "__invert__",
        _ => "",
    };
    if !dunder.is_empty() {
        if let Ok(Attribute::Method { function, this }) =
            crate::executor::attribute::attribute_lookup(instance, operand, dunder)
        {
            // SAFETY: 两者分别由类型字典／实例持有，存活。
            unsafe {
                instance.incref_object(function.as_ptr());
                instance.incref_object(this.as_ptr());
            }
            let bound = instance.alloc(crate::builtin_objects::MethodObject::new(
                builtin_type(instance, "method"),
                function,
                this,
            ));
            let bound = bound.into_raw().cast::<Header>();
            let outcome = crate::executor::call::call_value(instance, bound, &[], &[]);
            release(instance, bound);
            return outcome;
        }
    }
    let name = instance.type_name(instance.type_of(operand));
    let shown = if symbol == "abs" { "abs()" } else { symbol };
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("bad operand type for unary {shown}: '{name}'"),
    ))
}

/// **集合的四个运算符**（第 102 轮）：`&`（交）／`|`（并）／`-`（差）／`^`（对称差）⇒ **新 `set`** ✓。
///
/// 认元素用 `values_equal`（**值相等** ✓，与 `in` 同一口径 ✓）；`new_set` 接手所有权 ⇒ 每个元素
/// 都要先 `incref` 一份 ✓（`OM-16` ✓）。
pub(crate) fn set_operation(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
) -> NonNull<Header> {
    // SAFETY: 调用方刚核过两边都是 set 族，载荷就是 `SetObject` ✓。
    let left_set = unsafe { &*left.as_ptr().cast::<SetObject>() };
    // SAFETY: 同上。
    let right_set = unsafe { &*right.as_ptr().cast::<SetObject>() };
    let in_right = |value: NonNull<Header>| {
        right_set
            .position_of(|item| values_equal(instance, item, value))
            .is_some()
    };
    let in_left = |value: NonNull<Header>| {
        left_set
            .position_of(|item| values_equal(instance, item, value))
            .is_some()
    };
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let push = |value: NonNull<Header>, items: &mut Vec<NonNull<Header>>| {
        // SAFETY: 值的存活由两侧集合保证；`new_set` 接手那一份。
        unsafe { instance.incref_object(value.as_ptr()) };
        items.push(value);
    };
    match symbol {
        "|" => {
            for index in 0..left_set.len() {
                if let Some(item) = left_set.item(index) {
                    push(item, &mut items);
                }
            }
            for index in 0..right_set.len() {
                if let Some(item) = right_set.item(index) {
                    if !in_left(item) {
                        push(item, &mut items);
                    }
                }
            }
        }
        "&" => {
            for index in 0..left_set.len() {
                if let Some(item) = left_set.item(index) {
                    if in_right(item) {
                        push(item, &mut items);
                    }
                }
            }
        }
        "-" => {
            for index in 0..left_set.len() {
                if let Some(item) = left_set.item(index) {
                    if !in_right(item) {
                        push(item, &mut items);
                    }
                }
            }
        }
        _ => {
            // `^`（对称差）
            for index in 0..left_set.len() {
                if let Some(item) = left_set.item(index) {
                    if !in_right(item) {
                        push(item, &mut items);
                    }
                }
            }
            for index in 0..right_set.len() {
                if let Some(item) = right_set.item(index) {
                    if !in_left(item) {
                        push(item, &mut items);
                    }
                }
            }
        }
    }
    instance.new_set(items)
}

/// **通用比较**（`TS-40`）：`int`／`bool`／`str` **按值**比较，其余类型报**参照实测**的
/// `TypeError`（`'<' not supported between instances of 'int' and 'str'`）。
///
/// `operator` 模块的比较族与 `COMPARE_OP` **共用同一份实现**（两处各写一份就是两处真相）。
/// `symbol` 取 `"<"`／`"<="`／`"=="`／`"!="`／`">"`／`">="`（照 `cmp_op` 的名字）。
///
/// **浮点尚未接线**（本层浮点类型还在未落地清单里）⇒ 遇到浮点按"别的类型"处理（报实测消息形）。
/// **取集合元素** ✓（第 205 轮）：`set` 与 `frozenset` **同一载荷** ✓（第 236 轮 ✓）⇒ 两边都认 ✓。
pub(crate) fn set_items_of(instance: &Instance, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
    let ty = instance.type_of(object);
    let is_set = Some(ty) == instance.type_named("set") || Some(ty) == instance.type_named("frozenset");
    if !is_set {
        return None;
    }
    // SAFETY: 类型身份刚确认 ⇒ 载荷就是 `SetObject` ✓。
    Some(unsafe { &*object.as_ptr().cast::<crate::builtin_objects::SetObject>() }.items().to_vec())
}


// ---- `BC-23` 的边界检查（`TS-10`…`TS-13`） ----

/// 渲染一个标签给消息用：类型对象给名字、二元组给 `外[内]`、
/// **union 标签**给 `Union[成员, 成员]`、其余给 `Any`。
///
/// union 标签形态（`TS-46`／`BC-24`）：元组首项是字符串 `"Union"`、其后是成员标签
/// `("Union", 标签, 标签, …)`——成员递归渲染（类型对象／`"Any"`／嵌套 union／`(外, 内)`）；
/// 首项是**类型对象**的二元素元组**不是** union，仍走 `外[内]`（区分逻辑在
/// `protocol::boundary_accepts`）。
pub(crate) fn render_label(instance: &Instance, label: NonNull<Header>) -> String {
    // SAFETY: label 由常量表持有，存活。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type == builtin_type(instance, "str") {
        // SAFETY: 类型身份刚确认。
        return unsafe { &*label.as_ptr().cast::<StrObject>() }.value().to_owned();
    }
    if is_type_object(instance, label_type) {
        return type_name_of(instance, label.cast::<TypeObject>());
    }
    if label_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份刚确认。
        let parts = unsafe { &*label.as_ptr().cast::<TupleObject>() };
        // `TS-46`／`BC-24`：union 标签 ⇒ `Union[成员, 成员]`（成员用同一函数递归渲染）。
        if let Some(head) = parts.item(0) {
            // SAFETY: 元组元素存活。
            let head_is_union_marker = unsafe { head.as_ref() }.ty() == builtin_type(instance, "str")
                // SAFETY: 类型身份刚确认。
                && unsafe { &*head.as_ptr().cast::<StrObject>() }.value() == "Union";
            if head_is_union_marker {
                let members = (1..parts.len())
                    .filter_map(|index| parts.item(index))
                    .map(|member| render_label(instance, member))
                    .collect::<Vec<_>>();
                return format!("Union[{}]", members.join(", "));
            }
        }
        if parts.len() == 2 {
            return format!(
                "{}[{}]",
                render_label(instance, parts.item(0).expect("下标在范围内")),
                render_label(instance, parts.item(1).expect("下标在范围内"))
            );
        }
    }
    "Any".to_owned()
}

/// 类型对象的 `__name__`。
pub(crate) fn type_name_of(instance: &Instance, ty: NonNull<TypeObject>) -> String {
    let _ = instance;
    // SAFETY: ty 由注册表持有。
    unsafe { ty.as_ref() }.name().to_owned()
}

// ---- **`PEP 604`**：`X | None` 与 union 再 `|`（`NB_OR` 的类型/union 语境） ----

/// `NB_OR` 一侧操作数的**类型/union 语境**视图（参照 3.14 实测口径）。
enum OrOperand {
    /// 类型对象 ⇒ 单成员。
    Type(NonNull<Header>),
    /// `types.UnionType` 实例 ⇒ 其 `__args__` 成员（按书写顺序）。
    Union(Vec<NonNull<Header>>),
    /// `None`。**只有**另一边是类型/union 时才当 `NoneType` 用；`None | None` 仍是 `TypeError`。
    NoneValue,
    /// 其余值（`5` 一类）。只在与 union 再 `|` 时成为成员。
    Other(NonNull<Header>),
}

/// 是 `types.UnionType` 实例 ⇒ 取 `__args__` 的成员表（**借用**，不增减引用）；否则 `None`。
fn union_args_of(instance: &Instance, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
    let union_type = instance.type_named("UnionType")?;
    // SAFETY: object 是存活对象；类型身份下面刚比过。
    if unsafe { object.as_ref() }.ty() != union_type {
        return None;
    }
    let mapping = mounted_instance_dict(instance, object)?;
    let args = instance.dict_get(mapping, "__args__")?;
    if instance.type_of(args) != builtin_type(instance, "tuple") {
        return None;
    }
    // SAFETY: 上面刚确认过是 `tuple`。
    let tuple = unsafe { &*args.as_ptr().cast::<TupleObject>() };
    Some((0..tuple.len()).filter_map(|index| tuple.item(index)).collect())
}

/// 一侧操作数的视图（`None` 先认，因为它既不是类型对象也不是 union）。
fn or_operand(instance: &Instance, object: NonNull<Header>) -> OrOperand {
    if object == instance.singletons().none() {
        return OrOperand::NoneValue;
    }
    if instance.is_type_object(object) {
        return OrOperand::Type(object);
    }
    match union_args_of(instance, object) {
        Some(members) => OrOperand::Union(members),
        None => OrOperand::Other(object),
    }
}

/// 往 union 成员表里加一项：**去重**（参照对成员按 `==` 去重：`int | int` ⇒ `int`，
/// `(int | int) | str` ⇒ `int | str`）+ 自己持一份引用。
fn push_union_member(
    instance: &Instance,
    members: &mut Vec<NonNull<Header>>,
    member: NonNull<Header>,
) {
    if members
        .iter()
        .any(|existing| values_equal_public(instance, *existing, member))
    {
        return;
    }
    instance.retain(member);
    members.push(member);
}

/// 造一个 `types.UnionType` 实例：成员表按给定顺序进 `__args__`（成员是**新引用**，由本函数接手）。
fn new_union_of(instance: &Instance, members: Vec<NonNull<Header>>) -> NonNull<Header> {
    let union_type = instance
        .type_named("UnionType")
        .unwrap_or_else(|| instance.new_attribute_type("UnionType"));
    let object = instance
        .alloc(crate::builtin_objects::AttributeObject::new(
            union_type,
            RefCell::new(Some(instance.new_dict())),
        ))
        .into_raw()
        .cast::<Header>();
    let args = instance.new_tuple(members);
    // `set_attribute_value` 收**借用** ⇒ 交出去的元组引用随后还掉（字典自己持一份）。
    let _ = instance.set_attribute_value(object, "__args__", args);
    release(instance, args);
    object
}

/// `NB_OR` 的 **`PEP 604`** 分支：`X | None`／`None | X`／union 再 `|`。
///
/// 返回 `None` ⇒ 这一对不该按类型/union 处理（落回算术：`1 | 2`、`int | 5` 的 `TypeError` 照旧）。
/// 参照实测（3.14.4）的规则与顺序：
/// * 类型／union 参与的 `|` ⇒ **扁平** union（`int | str | None` 的 `__args__` 是三项）；
/// * `None` 只在与类型/union 同侧时当 `NoneType` 类用；`None | None` 不是 union；
/// * union 再 `|` 会摊平并**去重**；`(int | str) | 5` 这类 union ＋ 任意值也成成员（参照行为）。
fn pep604_union_or(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
) -> Option<NonNull<Header>> {
    let left_operand = or_operand(instance, left);
    let right_operand = or_operand(instance, right);
    // 先行排除"没有一方是类型/union"与"裸类型/None 对任意值"的组合 ⇒ 那些必须落回算术报 TypeError。
    match (&left_operand, &right_operand) {
        (OrOperand::Type(_), OrOperand::Other(_))
        | (OrOperand::NoneValue, OrOperand::NoneValue)
        | (OrOperand::NoneValue, OrOperand::Other(_))
        | (OrOperand::Other(_), OrOperand::Type(_))
        | (OrOperand::Other(_), OrOperand::NoneValue)
        | (OrOperand::Other(_), OrOperand::Other(_)) => return None,
        _ => {}
    }
    let mut members: Vec<NonNull<Header>> = Vec::new();
    for operand in [left_operand, right_operand] {
        match operand {
            OrOperand::NoneValue => {
                // `None` ⇒ `NoneType` 那个**类**（参照 `__args__` 里就是这个类）。
                let none_type = instance.type_value(instance.singletons().none_type());
                push_union_member(instance, &mut members, none_type);
                release(instance, none_type);
            }
            OrOperand::Type(label) => push_union_member(instance, &mut members, label),
            OrOperand::Union(items) => {
                for item in items {
                    push_union_member(instance, &mut members, item);
                }
            }
            OrOperand::Other(value) => push_union_member(instance, &mut members, value),
        }
    }
    // 去重后只剩一个成员 ⇒ 就是那个类型本身（参照：`int | int` 给 `int`）。
    if members.len() == 1 {
        return members.pop();
    }
    Some(new_union_of(instance, members))
}

/// **未捕获异常**的宿主诊断文本（`pa_errmsg` 用）：`<类型名>: <str(exception)>`。
///
/// 参照的未捕获报错尾行就是 `str(exception)` 的口径（`MS-8` 比到这一行）⇒ 用首实参的裸内容
/// （`Instance::exception_message_of`）会漏掉 `KeyError` 的 repr 层：参照 `KeyError: 'missing'`、
/// 本层先前给 `KeyError: missing`。`str` 槽的 KeyError 口径在 `builtin_objects::exception_str`
/// 里已有（单实参 ⇒ 实参 repr）⇒ 这里直接复用，**不改**那个实现。
///
/// 形状核对照 `Instance::exception_message_of`（先核 `BaseException` 子类型与载荷大小，再读载荷）：
/// 核不过 ⇒ **只给类型名**（绝不硬读）。`str` 渲染失败也只是退化成类型名。
pub fn exception_uncaught_text(instance: &Instance, exception: NonNull<Header>) -> String {
    // SAFETY: exception 由调用方保证存活。
    let ty = unsafe { exception.as_ref() }.ty();
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { ty.as_ref() };
    let type_name = type_object.name();
    let is_exception = instance
        .type_named("BaseException")
        .is_none_or(|base| instance.is_subtype(ty, base));
    if !is_exception
        || type_object.instance_size()
            != core::mem::size_of::<crate::builtin_objects::ExceptionObject>()
    {
        return type_name.to_owned();
    }
    // SAFETY: 形状刚核过（与 `exception_message_of` 同一套检查）。
    match unsafe { crate::builtin_objects::exception_str(exception.as_ptr(), instance) } {
        Ok(text) if !text.is_empty() => format!("{type_name}: {text}"),
        _ => type_name.to_owned(),
    }
}

/// **按对象抛**（`_codecs` 的错误处理器要用：`strict_errors` 就是"原样再抛" ✓）。
///
/// 契约与 [`raise`] 同款：调用方交**一份新引用**（`Err(Raised)` 那一份由派发器接手 ✓），
/// 函数内部再为**实例的待处理异常状态**加一份 ✓。
pub fn raise_object_public(instance: &Instance, exception: NonNull<Header>) -> ExecError {
    raise(instance, exception)
}

pub(crate) fn line_at_offset(code: &CodeObject, offset: usize) -> u32 {
    let mut decoder = crate::decode::Decoder::new(code.code());
    let mut ordinal = 0usize;
    while let Ok(Some(instruction)) = decoder.next_instruction() {
        if instruction.offset == offset {
            // **`BC-4` 扩**后行号也可缺失（合成指令）⇒ 缺失时落到 `firstlineno`（与参照的
            // `PyCode_Addr2Line` 对无行条目一致地"不冒充行号"）
            if let Some((Some(line), _, _, _)) = code.positions().get(ordinal) {
                return *line;
            }
            break;
        }
        ordinal += 1;
    }
    code.firstlineno() as u32
}

// ---- `BC-56` 的消息：**逐条实测**（禁止手写近似文本，见 tests/calls.rs 的记录）----

/// 取一个 `str` 对象的文本（关键字实参的名字要用它）。
fn str_text(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<String, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty != instance.singletons().str_type() {
        return Err(ExecError::Unsupported {
            opcode,
            what: "关键字实参的名字必须是 str",
        });
    }
    // SAFETY: 类型身份已确认。
    Ok(unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned())
}

/// 取函数对象的位置默认值（**借用**，需要时自行 incref）。
/// 函数对象的闭包（**cell 列表**；每项是**借用**的裸引用，不新增引用）。
pub(crate) fn function_closure(instance: &Instance, callable: NonNull<Header>) -> Vec<NonNull<Header>> {
    // SAFETY: 调用方保证 callable 是存活对象；类型身份在调用路径已确认。
    let object = unsafe { &*callable.as_ptr().cast::<FunctionObject>() };
    let _ = instance;
    object.closure()
}

pub(crate) fn function_defaults(function: NonNull<Header>) -> (NonNull<Header>, Vec<NonNull<Header>>, Option<NonNull<Header>>) {
    // SAFETY: function 是帧值栈上的存活对象，且调用方已确认它是 function。
    let object = unsafe { &*function.as_ptr().cast::<FunctionObject>() };
    (object.code(), object.defaults().to_vec(), object.kwdefaults())
}

/// **`BC-56`**：把实参绑进局部槽；返回长度 ＝ `co_nlocals` 的槽数组（**新引用**）。
///
/// 顺序与报错类别都按 `BC-56`：仅位置 → 位置或关键字 → `*args` → 仅关键字 → `**kwargs`；
/// 四类错误各成一个 [`ExecError`]（参照实现的**消息**已实测记录在案，等异常对象接线后再原样产出）。
#[allow(clippy::too_many_arguments)]
/// **槽级追踪** ✓（第 653 轮，门控 `PYAWA_SLOT_TRACE="函数名:槽号"`）：把某函数某槽的**每一次读／写**
/// 按**发生顺序**打出来 ✓（句首是动作 ✓）—— 用来夹"先读后写"这类**控制流分岔** ✓
///（`Lib/re._parser._parse` 的槽 14 就是靠它定位 ✓）。不设这个环境变量时**零开销** ✓（只多一次查表 ✓）。
fn slot_trace(instance: &Instance, code: &crate::CodeObject, slot: usize, action: &str) {
    let Some(wanted) = std::env::var_os("PYAWA_SLOT_TRACE") else {
        return;
    };
    let wanted = wanted.to_string_lossy().into_owned();
    let mut parts = wanted.splitn(2, ':');
    let (Some(name), Some(number)) = (parts.next(), parts.next()) else {
        return;
    };
    if name != code.name() || number.parse::<usize>() != Ok(slot) {
        return;
    }
    eprintln!(
        "[slot] {} name={} 槽={} 站点={} 局部={:?}",
        action,
        code.name(),
        slot,
        instance.current_site(),
        code.varname(slot).unwrap_or("?")
    );
}

pub(crate) fn bind_arguments(
    instance: &Instance,
    code: &CodeObject,
    args: Vec<NonNull<Header>>,
    kwargs: Vec<(NonNull<Header>, NonNull<Header>)>,
    defaults: &[NonNull<Header>],
    kwdefaults: Option<NonNull<Header>>,
    opcode: u8,
) -> Result<Vec<Option<NonNull<Header>>>, ExecError> {
    let mut locals: Vec<Option<NonNull<Header>>> = vec![None; code.nlocals()];
    let argcount = code.argcount();
    let kwonly = code.kwonlyargcount();
    let mut args = args.into_iter();

    // ① 位置实参填进前 `argcount` 个槽
    let mut given = 0usize;
    for slot in 0..argcount {
        match args.next() {
            Some(value) => {
                locals[slot] = Some(value);
                given += 1;
            }
            None => break,
        }
    }

    // ② 多出来的位置实参：收进 `*args`，否则报错
    let extra: Vec<NonNull<Header>> = args.collect();
    // **`*args` 必须无条件绑定** ✓（第 277 轮真 bug ✗）：先前整块写在 `if !extra.is_empty()` **里面** ✗
    // ⇒ **没有多余位置实参时那一格从不绑** ✗ ⇒ 函数体里一读就是"未绑定局部" ✓（CPython 会绑空元组 ✓）。
    // 实测：`def f(a, *p): return len(p)` 调 `f(1)` ⇒ 参照 `0`、本层报未绑定 ✗；
    // `Lib/posixpath.py` 的 `join(a, *p)` 与 `import site` 都撞在它上面 ✓。
    if code.has_varargs() {
        let varargs_slot = argcount + kwonly;
        // **不许 panic** ✓（task-22 第 3 轮加固 ✗）：`nlocals` 若没把 `*args` 槽算进去 ✗
        // （编译器那一半的 bug ✓），先前这里**直接下标** ⇒ Rust panic ✓
        // （实测 `import concurrent.futures.thread` 就是这么炸的 ✓ —— 报错变成**空消息** ✓）。
        // 这里改成**如实报一条 `TypeError`**（带函数名与槽位数 ✓），把 panic 换成能定位的错 ✓。
        if varargs_slot >= locals.len() {
            let release_all = |values: Vec<NonNull<Header>>| {
                for value in values {
                    release(instance, value);
                }
            };
            release_all(extra);
            for slot in locals.iter_mut().filter_map(Option::take) {
                release(instance, slot);
            }
            let message = format!(
                "内部缺陷：{} 的 `*args` 槽 {} 超出 nlocals {}（编译器没把该槽算进 varnames）",
                code.name(),
                varargs_slot,
                locals.len()
            );
            return Err(raise_builtin(instance, "TypeError", &message));
        }
        // **OM-23**：没有多余实参时这个元组是空的 ⇒ 走单例
        locals[varargs_slot] = Some(instance.new_tuple(extra));
    } else if !extra.is_empty() {
        let release_all = |values: Vec<NonNull<Header>>| {
            for value in values {
                release(instance, value);
            }
        };
        release_all(extra);
        for slot in locals.iter_mut().filter_map(Option::take) {
            release(instance, slot);
        }
        let message = message_too_many(
            code.name(),
            argcount,
            argcount - defaults.len(),
            given + 1,
        );
        return Err(raise_builtin(instance, "TypeError", &message));
    }

    // ③ 关键字实参
    let mut collected: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
    for (name, value) in kwargs {
        let text = match str_text(instance, name, opcode) {
            Ok(text) => text,
            Err(error) => {
                release(instance, name);
                release(instance, value);
                for slot in locals.iter_mut().filter_map(Option::take) {
                    release(instance, slot);
                }
                return Err(error);
            }
        };

        let positional_hit = (0..argcount).find(|slot| code.varname(*slot) == Some(text.as_str()));
        let keyword_hit = (0..kwonly)
            .find(|offset| code.varname(argcount + offset) == Some(text.as_str()))
            .map(|offset| argcount + offset);

        let outcome = if let Some(slot) = positional_hit {
            if slot < code.posonlyargcount() {
                // 仅位置形参不能用关键字传
                release(instance, value);
                release(instance, name);
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &message_positional_only(code.name(), std::slice::from_ref(&text)),
                ))
            } else if locals[slot].is_some() {
                release(instance, value);
                release(instance, name);
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &message_duplicate(code.name(), &text),
                ))
            } else {
                locals[slot] = Some(value);
                release(instance, name);
                Ok(())
            }
        } else if let Some(slot) = keyword_hit {
            if locals[slot].is_some() {
                release(instance, value);
                release(instance, name);
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &message_duplicate(code.name(), &text),
                ))
            } else {
                locals[slot] = Some(value);
                release(instance, name);
                Ok(())
            }
        } else if code.has_varkeywords() {
            collected.push((name, value));
            Ok(())
        } else {
            release(instance, value);
            release(instance, name);
            Err(raise_builtin(
                instance,
                "TypeError",
                &message_unexpected_keyword(code.name(), &text),
            ))
        };

        if let Err(error) = outcome {
            for (key, item) in collected {
                release(instance, key);
                release(instance, item);
            }
            for slot in locals.iter_mut().filter_map(Option::take) {
                release(instance, slot);
            }
            return Err(error);
        }
    }

    // ④ 位置形参的默认值（对齐到**尾部**若干位置参数）；缺的一并报出来（参照实现如此）
    let mut missing_positional: Vec<String> = Vec::new();
    for slot in 0..argcount {
        if locals[slot].is_some() {
            continue;
        }
        let from_end = argcount - slot;
        if from_end <= defaults.len() {
            let value = defaults[defaults.len() - from_end];
            // SAFETY: 默认值由函数对象持有，存活。
            unsafe { instance.incref_object(value.as_ptr()) };
            locals[slot] = Some(value);
        } else {
            missing_positional.push(code.varname(slot).unwrap_or("<unknown>").to_owned());
        }
    }
    if !missing_positional.is_empty() {
        for (key, item) in collected {
            release(instance, key);
            release(instance, item);
        }
        for slot in locals.iter_mut().filter_map(Option::take) {
            release(instance, slot);
        }
        let message = message_missing(code.name(), &missing_positional, false);
        return Err(raise_builtin(instance, "TypeError", &message));
    }

    // ⑤ 仅关键字形参：先看默认值，缺了一并报出来
    let mut missing_keyword_only: Vec<String> = Vec::new();
    for offset in 0..kwonly {
        let slot = argcount + offset;
        if locals[slot].is_some() {
            continue;
        }
        let name = code.varname(slot).unwrap_or("<unknown>").to_owned();
        let mut found = None;
        if let Some(mapping) = kwdefaults {
            // SAFETY: mapping 由函数对象持有。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            let name_object = instance.alloc(StrObject::new(instance.singletons().str_type(), name.clone()));
            let name_raw = name_object.as_ptr().cast::<Header>();
            let position = dict
                .entries()
                .iter()
                .position(|(existing, _)| values_equal(instance, *existing, name_raw));
            if let Some(position) = position {
                found = dict.entry(position).map(|(_, value)| value);
            }
        }
        match found {
            Some(value) => {
                // SAFETY: 默认值由 kwdefaults 持有。
                unsafe { instance.incref_object(value.as_ptr()) };
                locals[slot] = Some(value);
            }
            None => missing_keyword_only.push(name),
        }
    }
    if !missing_keyword_only.is_empty() {
        for (key, item) in collected {
            release(instance, key);
            release(instance, item);
        }
        for slot in locals.iter_mut().filter_map(Option::take) {
            release(instance, slot);
        }
        let message = message_missing(code.name(), &missing_keyword_only, true);
        return Err(raise_builtin(instance, "TypeError", &message));
    }

    // ⑥ `**kwargs`
    if code.has_varkeywords() {
        let slot = argcount + kwonly + usize::from(code.has_varargs());
        // **不许 panic** ✓（与上面 `*args` 那条同款加固 ✓）：槽位越界 ⇒ 如实报一条 `TypeError` ✓。
        if slot >= locals.len() {
            for (key, item) in collected {
                release(instance, key);
                release(instance, item);
            }
            for value in locals.iter_mut().filter_map(Option::take) {
                release(instance, value);
            }
            let message = format!(
                "内部缺陷：{} 的 `**kwargs` 槽 {} 超出 nlocals {}（varnames={:?}）",
                code.name(),
                slot,
                locals.len(),
                code.varnames
            );
            return Err(raise_builtin(instance, "TypeError", &message));
        }
        let dict = instance.alloc(DictObject::new(
            builtin_type(instance, "dict"),
            RefCell::new(collected),
        ));
        locals[slot] = Some(dict.into_raw().cast::<Header>());
    } else if !collected.is_empty() {
        for (key, item) in collected {
            release(instance, key);
            release(instance, item);
        }
    }

    Ok(locals)
}

/// 为 code object 现取一个 [`Owned`] 守卫（**新增一份引用**）。
pub(crate) fn own_code<'a>(instance: &'a Instance, header: NonNull<Header>) -> Owned<'a, CodeObject> {
    // SAFETY: header 指向本实例的存活 code object；这里新增一份引用交给守卫。
    unsafe { instance.incref_object(header.as_ptr()) };
    Owned::new(header.cast::<CodeObject>(), instance)
}

/// `BC-56` 与调用（`CALL`／`CALL_KW`）。

/// **异常派发**（`BC-60` ①）：按异常表找到处理块，回退值栈到 `depth`、按 `lasti` 压偏移、
/// 压异常实例、跳到入口；没有处理块就把它继续往外抛。
///
/// 两条路径共用它：指令自己报错（`Err(Raised)`）与**恢复时要先抛**（生成器的 `throw`／`close`）。
fn dispatch_raise(
    instance: &Instance,
    frame: &Frame,
    exceptiontable: &[u8],
    offset_bytes: usize,
    exception: NonNull<Header>,
    decoder: &mut Decoder,
) -> Result<(), ExecError> {
    let table = parse_exception_table(exceptiontable).map_err(ExecError::Decode)?;
    let handler = table
        .iter()
        .find(|entry| entry.start <= offset_bytes && offset_bytes < entry.end);
    let Some(entry) = handler else {
        return Err(ExecError::Raised { exception });
    };
    if crate::diag::flag("PYAWA_TRY_DEPTH_DEBUG") {
        eprintln!("[try_depth] 展开 实际栈深={} 记账 depth={}", frame.depth(), entry.depth);
    }
    while frame.depth() > entry.depth {
        release(instance, frame.pop()?);
    }
    // **先把值栈弹到异常表记的深度** ✓（第 164 轮的真 bug ✗）：先前漏了这一步 ⇒ 处理块带着多余的
    //   栈项开跑 ✓ ⇒ 症状有两种：「弹出个 `int`」（陈旧的栈项被当成异常 ✓）与 `StackUnderflow` ✗。
    for value in frame.truncate_stack(entry.depth as usize) {
        release(instance, value);
    }
    if entry.lasti {
        push_int(instance, frame, (offset_bytes / 2) as i64)?;
    }
    push(instance, frame, exception)?;
    decoder.set_position(entry.target / 2);
    Ok(())
}

/// **恢复一个生成器**：把 `sent` 送进挂起的帧，跑到下一次让出或跑完。
///
/// `SEND` 与生成器方法 `send`／`__next__` 共用这一段——栈效应与"跑完"的记账只有一处真相。
/// 返回的两个值都是**新引用**（调用方接手）。
pub(crate) enum GeneratorOutcome {
    /// 又让出了一次（值是**新引用**）。
    Yielded(NonNull<Header>),
    /// 跑完了（返回值是**新引用**；生成器已置"跑完"）。
    Returned(NonNull<Header>),
}

pub(crate) fn resume_generator(
    instance: &Instance,
    generator: NonNull<Header>,
    sent: Option<NonNull<Header>>,
) -> Result<GeneratorOutcome, ExecError> {
    // SAFETY: 调用方保证 generator 是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    let frame_header = object.frame();
    let generator_frame = Owned::new(
        // SAFETY: frame_header 由生成器持有，这里新增一份引用交给守卫。
        {
            unsafe { instance.incref_object(frame_header.as_ptr()) };
            frame_header.cast::<Frame>()
        },
        instance,
    );
    if generator_frame.get().is_suspended() {
        generator_frame.get().resume()?;
    }
    // "送进去的值"要落在恢复后的值栈顶（`yield` 表达式的值）；`push` 会新增一份引用，
    // 所以送出的那份随后要还（调用方给的是借用的视图或已有引用）。
    match sent {
        Some(value) => {
            push(instance, generator_frame.get(), value)?;
        }
        None => {
            let none = instance.singletons().none();
            push(instance, generator_frame.get(), none)?;
        }
    }
    match execute(instance, &generator_frame) {
        Ok(ExecOutcome::Yielded(value)) => Ok(GeneratorOutcome::Yielded(value)),
        Ok(ExecOutcome::Returned(value)) => {
            object.mark_finished();
            let raw = value_into_raw(instance, value);
            // **异步生成器**跑完不是"返回值"，而是 `StopAsyncIteration`（`async for` 靠它收尾）
            if instance.type_name(unsafe { generator.as_ref() }.ty()) == "async_generator" {
                release(instance, raw);
                return Err(async_generator_exhausted(instance));
            }
            Ok(GeneratorOutcome::Returned(raw))
        }
        Err(error) => {
            // 让出点之后出错 ⇒ 生成器就此作废（参照实现同：之后再取就是耗尽）
            object.mark_finished();
            // **协程**里逃出来的 `StopIteration` 要变成 `RuntimeError`。实测两句话都在，
            // 词由**被驱动的对象种类**决定：协程 ⇒ `coroutine raised StopIteration`，
            // `CO_ITERABLE_COROUTINE`（0x100）生成器 ⇒ `generator raised StopIteration`。
            // 转换放在这里而不是只靠 `INTRINSIC_STOPITERATION_ERROR`，是因为**这里知道种类**
            // （那条 intrinsic 在栈上只看到异常对象）。普通生成器的 `yield from` 不走这条：
            // 那里 `StopIteration` 是**返回值**机制。
            if let ExecError::Raised { exception } = &error {
                // SAFETY: exception 是存活对象。
                let ty = unsafe { exception.as_ref() }.ty();
                let stop_iteration = exception_type(instance, "StopIteration");
                // SAFETY: generator 是本实例里存活的对象。
                let is_coroutine =
                    unsafe { generator.as_ref() }.ty() == builtin_type(instance, "coroutine");
                let code_flags = {
                    let frame_header = object.frame();
                    // SAFETY: 帧由生成器持有，存活。
                    let generator_frame = unsafe { &*frame_header.as_ptr().cast::<Frame>() };
                    match generator_frame.code() {
                        // SAFETY: code 由帧持有，存活。
                        Some(code) => unsafe { code.cast::<CodeObject>().as_ref() }.flags(),
                        None => 0,
                    }
                };
                let iterable_coroutine = code_flags & 0x100 != 0;
                if instance.is_subtype(ty, stop_iteration) && (is_coroutine || iterable_coroutine) {
                    let word = if is_coroutine { "coroutine" } else { "generator" };
                    let message = format!("{word} raised StopIteration");
                    // SAFETY: 错误里那份引用在此消费。
                    unsafe { instance.release_object(exception.as_ptr()) };
                    return Err(raise_builtin(instance, "RuntimeError", &message));
                }
            }
            Err(error)
        }
    }
}

/// 异步生成器耗尽时抛的东西（`async for` 的结束信号）。
pub(crate) fn async_generator_exhausted(instance: &Instance) -> ExecError {
    let exception = crate::builtin_objects::exception_instance(
        instance,
        "StopAsyncIteration",
        Vec::new(),
    );
    raise(instance, exception)
}

/// **恢复生成器并立刻抛一个异常**（`throw`／`close`）：异常放进帧的"待抛"格，
/// 由 `execute` 按**本帧的**异常表派发（生成器体里的 `try/except` 因此能接住）。
pub(crate) fn resume_generator_with_raise(
    instance: &Instance,
    generator: NonNull<Header>,
    exception: NonNull<Header>,
) -> Result<GeneratorOutcome, ExecError> {
    // SAFETY: 调用方保证 generator 是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    let frame_header = object.frame();
    let generator_frame = Owned::new(
        // SAFETY: frame_header 由生成器持有，这里新增一份引用交给守卫。
        {
            unsafe { instance.incref_object(frame_header.as_ptr()) };
            frame_header.cast::<Frame>()
        },
        instance,
    );
    if generator_frame.get().is_suspended() {
        generator_frame.get().resume()?;
    }
    // 帧接手的是**新引用**（`Frame::clear` 会释放它）
    // SAFETY: exception 由调用方保证存活。
    unsafe { instance.incref_object(exception.as_ptr()) };
    if let Some(previous) = generator_frame.get().set_pending_raise(Some(exception)) {
        // SAFETY: 被顶下来的那份由帧交出。
        unsafe { instance.release_object(previous.as_ptr()) };
    }
    match execute(instance, &generator_frame) {
        Ok(ExecOutcome::Yielded(value)) => Ok(GeneratorOutcome::Yielded(value)),
        Ok(ExecOutcome::Returned(value)) => {
            object.mark_finished();
            Ok(GeneratorOutcome::Returned(value_into_raw(instance, value)))
        }
        Err(error) => {
            object.mark_finished();
            Err(error)
        }
    }
}

/// 跑一段 code object，直到 `RETURN_VALUE`。
///
/// **BC-42**：指令指针沿途写回帧（码元单位），因此挂起／恢复有据可依。
/// **当前全局映射的 RAII 守卫**（第 156 轮）：`Drop` 时恢复上一格 ✓ ⇒ `execute` 里**任何**提前返回
/// （含 `?`）都安全 ✓（生成器挂起返回时也算「本帧不活跃」✓，恢复正是对的 ✓）。
/// **当前帧的 RAII 守卫** ✓（第 230 轮）：与全局映射那只**同款** ✓ —— 进帧时公布、出帧时还原 ✓。
struct CurrentFrameGuard<'a> {
    instance: &'a Instance,
    previous: Option<NonNull<Header>>,
}

impl<'a> CurrentFrameGuard<'a> {
    fn install(instance: &'a Instance, frame: Option<NonNull<Header>>) -> Self {
        // **调用栈**（第 617 轮 ✓）：`sys._getframemodulename(1)` 要回到调用者那一帧 ✓。
        if let Some(entered) = frame {
            crate::builtin_objects::push_frame(entered);
        }
        let previous = instance.set_current_frame(frame);
        Self { instance, previous }
    }
}

impl Drop for CurrentFrameGuard<'_> {
    fn drop(&mut self) {
        if self.instance.current_frame().is_some() {
            crate::builtin_objects::pop_frame();
        }
        self.instance.set_current_frame(self.previous);
    }
}

struct CurrentGlobalsGuard<'a> {
    instance: &'a Instance,
    previous: Option<NonNull<Header>>,
}

impl<'a> CurrentGlobalsGuard<'a> {
    fn install(instance: &'a Instance, globals: Option<NonNull<Header>>) -> Self {
        let previous = instance.set_current_globals(globals);
        Self { instance, previous }
    }
}

impl Drop for CurrentGlobalsGuard<'_> {
    fn drop(&mut self) {
        self.instance.set_current_globals(self.previous);
    }
}

/// 读 **cell 槽第 `slot` 格**那个 cell 的值（**新引用**交给调用方）——`LOAD_DEREF` 与本轮新增的
/// `LOAD_FROM_DICT_OR_DEREF` 的兜底路**共用这一处真相**（第 267 轮的诊断在此保留 ✓）。
fn load_deref_value(
    instance: &Instance,
    frame: &Frame,
    code: &CodeObject,
    slot: usize,
) -> Result<NonNull<Header>, ExecError> {
    let cell = match frame.cell(slot) {
        Ok(Some(cell)) => cell,
        Ok(None) => {
            // **诊断升级 ＋ 更接近参照** ✓（第 267 轮）：参照在同样情形给 `NameError` ✓
            // ⇒ 把 **cell 名字**与**作用域**一起报出来 ✓（先前只说"cell 是空的" ✗ ⇒ 无从下手 ✓）。
            let name = localsplus_name(code, slot);
            let message = format!(
                "cannot access free variable '{name}' where it is not associated with a value yet（作用域 {} ✓ 指令 {} ✓）",
                code.name(),
                frame.instruction_pointer()
            );
            return Err(instance.raise_builtin_error("NameError", &message));
        }
        Err(error) => return Err(ExecError::Frame(error)),
    };
    // SAFETY: cell 由帧的 cell 槽持有，存活。
    let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
    let Some(value) = object.value() else {
        let name = localsplus_name(code, slot);
        let message = format!(
            "cannot access free variable '{name}' where it is not associated with a value yet（作用域 {} ✓ 指令 {} ✓）",
            code.name(),
            frame.instruction_pointer()
        );
        return Err(instance.raise_builtin_error("NameError", &message));
    };
    // SAFETY: 值由 cell 持有，存活。
    unsafe { instance.incref_object(value.as_ptr()) };
    Ok(value)
}

pub fn execute<'a>(
    instance: &'a Instance,
    frame: &Owned<'a, Frame>,
) -> Result<ExecOutcome<'a>, ExecError> {
    let code_header = frame.get().code().expect("BC-42：帧必须持有 code object");
    // SAFETY: 帧持有一份对 code object 的引用（BC-42），因此它在帧存活期间有效；
    // 帧由本函数的调用方持有。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    // **把本帧的全局映射挂到实例上**（第 156 轮）：内建 `globals()` 取它 ✓；守卫 `Drop` 时恢复 ✓。
    // **模块帧的 `globals` 那格本来就是 `None`** ✗（`BC-57`：模块体没有单独的一层，此时就是它的
    // **命名空间** ✓）⇒ 取 `globals`，没有就用 `namespace` ✓ —— 函数帧两格都有 ✓。
    let _globals_guard = CurrentGlobalsGuard::install(
        instance,
        frame.get().globals().or_else(|| frame.get().namespace()),
    );
    // **公布当前帧** ✓（第 230 轮）：`sys._getframe()` 与 `frame.f_locals` 都取它 ✓。
    // SAFETY: 帧由调用方持有，本函数运行期间存活 ✓。
    let frame_header: NonNull<Header> = frame.as_ptr().cast::<Header>();
    let _frame_guard = CurrentFrameGuard::install(instance, Some(frame_header));

    // **BC-47**：挂起的帧（生成器／await）从**恢复点**接着跑——值栈与 ip 都在恢复点里。
    // 新帧的 ip 是 0，所以"一律按帧的 ip 起步"这一条对两种情况都成立。
    if frame.get().is_suspended() {
        frame.get().resume()?;
    }
    // **`throw`／`close`**：恢复点上有"待抛异常"就先按本帧的异常表派发它
    // （所以生成器体里的 `try/except` 能接住，与参照实现"抛在挂起点"一致）。
    let mut forced_raise = frame.get().take_pending_raise();
    let mut decoder = Decoder::new(code.code());
    decoder.set_position(frame.get().instruction_pointer());
    while let Some(instruction) = decoder.next_instruction()? {
        if let Some(exception) = forced_raise.take() {
            dispatch_raise(
                instance,
                frame.get(),
                code.exceptiontable(),
                frame.get().instruction_pointer() * 2,
                exception,
                &mut decoder,
            )?;
            continue;
        }
        // **`AB-5`①**：宿主请求中断后就地停手（每条指令查一次，按实例存，`CX-3`）。
        if instance.interrupted() {
            return Err(ExecError::Interrupted);
        }
        let opcode_number = instruction.opcode;
        frame.get().set_instruction_pointer(instruction.offset);
        let oparg = instruction.oparg as usize;

        // BC-50：一律按名字分派，**禁止**依赖具体编号
        let Some(name) = opcode::opname(u16::from(opcode_number)) else {
            return Err(ExecError::NotImplemented { opcode: opcode_number });
        };

        // 把"一条指令"的执行包进闭包：这样异常能被这里接住并派发到处理块（BC-60 ①）。
        // 闭包返回 `Option<Value>`：`Some` 表示这条指令结束了整个执行（`RETURN_VALUE`）。
        let outcome = (|| -> Result<Step<'a>, ExecError> {
        match name {
            "RESUME" | "NOP" | "POP_TOP" | "NOT_TAKEN" | "LOAD_SMALL_INT" | "CACHE" => return step_misc::step_misc(instance, frame, oparg, name),
            "CHECK_BOUNDARY_IN" | "CHECK_BOUNDARY_OUT" | "CHECK_ASSIGNMENT" => return step_checks::step_checks(instance, frame, oparg, opcode_number, name),
            "LOAD_COMMON_CONSTANT" | "LOAD_NAME" | "LOAD_GLOBAL" | "STORE_GLOBAL" | "DELETE_GLOBAL" | "STORE_NAME" | "DELETE_NAME" => return step_names::step_names(instance, frame, code, oparg, opcode_number, name),
            "COPY_FREE_VARS" | "MAKE_CELL" | "LOAD_LOCALS" | "STORE_DEREF" | "LOAD_DEREF" | "LOAD_FROM_DICT_OR_DEREF" | "LOAD_FROM_DICT_OR_GLOBALS" | "LOAD_CONST" | "LOAD_FAST" | "LOAD_FAST_CHECK" | "LOAD_FAST_BORROW" | "STORE_FAST" | "DELETE_FAST" | "LOAD_FAST_AND_CLEAR" | "STORE_FAST_LOAD_FAST" | "LOAD_FAST_LOAD_FAST" | "LOAD_FAST_BORROW_LOAD_FAST_BORROW" | "STORE_FAST_STORE_FAST" => return step_locals::step_locals(instance, frame, code, oparg, opcode_number, name),
            "TO_BOOL" | "UNARY_NOT" | "UNARY_NEGATIVE" | "UNARY_INVERT" | "CONTAINS_OP" | "IS_OP" | "COMPARE_OP" | "BINARY_OP" => return step_operators::step_operators(instance, frame, oparg, opcode_number, name),
            "JUMP_FORWARD" | "JUMP_BACKWARD" | "JUMP_BACKWARD_NO_INTERRUPT" | "POP_JUMP_IF_TRUE" | "POP_JUMP_IF_FALSE" | "POP_JUMP_IF_NONE" | "POP_JUMP_IF_NOT_NONE" => return step_jumps::step_jumps(instance, frame, &mut decoder, &instruction, opcode_number, name),
            "BUILD_TUPLE" | "BUILD_LIST" | "BUILD_SET" | "BUILD_SLICE" | "BINARY_SLICE" | "BUILD_MAP" | "BUILD_STRING" | "UNPACK_SEQUENCE" | "UNPACK_EX" | "LIST_APPEND" | "SET_ADD" | "MAP_ADD" | "LIST_EXTEND" | "SET_UPDATE" | "GET_LEN" | "STORE_SUBSCR" | "STORE_SLICE" | "DELETE_SUBSCR" => return step_containers::step_containers(instance, frame, code, oparg, opcode_number, name),
            "GET_ITER" | "FOR_ITER" | "END_FOR" | "POP_ITER" | "GET_YIELD_FROM_ITER" | "GET_AITER" | "GET_ANEXT" | "END_ASYNC_FOR" | "CLEANUP_THROW" | "GET_AWAITABLE" | "SEND" | "END_SEND" => return step_iterators::step_iterators(instance, frame, &mut decoder, &instruction, opcode_number, name),
            "DICT_MERGE" | "DICT_UPDATE" | "CALL_FUNCTION_EX" | "CALL_INTRINSIC_1" | "CALL_INTRINSIC_2" | "LOAD_BUILD_CLASS" | "LOAD_SPECIAL" | "PUSH_NULL" | "MAKE_FUNCTION" | "SET_FUNCTION_ATTRIBUTE" | "CALL" | "CALL_KW" => return step_calls::step_calls(instance, frame, oparg, opcode_number, name),
            "MATCH_SEQUENCE" | "MATCH_MAPPING" | "MATCH_KEYS" | "MATCH_CLASS" => return step_match::step_match(instance, frame, oparg, opcode_number, name),
            "FORMAT_SIMPLE" | "CONVERT_VALUE" | "FORMAT_WITH_SPEC" | "SWAP" | "COPY" => return step_format::step_format(instance, frame, oparg, opcode_number, name),
            "PUSH_EXC_INFO" | "POP_EXCEPT" | "CHECK_EXC_MATCH" | "RERAISE" | "WITH_EXCEPT_START" | "RAISE_VARARGS" => return step_exceptions::step_exceptions(instance, frame, oparg, opcode_number, name),
            "IMPORT_NAME" | "IMPORT_FROM" => return step_imports::step_imports(instance, frame, code, oparg, opcode_number, name),
            "LOAD_ATTR" | "STORE_ATTR" | "DELETE_ATTR" => return step_attributes::step_attributes(instance, frame, code, oparg, opcode_number, name),
            "RETURN_VALUE" | "YIELD_VALUE" | "RETURN_GENERATOR" => return step_generators::step_generators(instance, frame, code, &instruction, name),
            _ => return Err(ExecError::NotImplemented { opcode: opcode_number }),
        }
        })();

        match outcome {
            Ok(Step::Return(value)) => return Ok(ExecOutcome::Returned(value)),
            Ok(Step::Yield(value)) => return Ok(ExecOutcome::Yielded(value)),
            Ok(Step::Continue) => {}
            Err(ExecError::Raised { exception }) => {
                // BC-60 ①：按异常表回退值栈到 `depth`、按 `lasti` 压最后一条指令偏移、
                // 压异常实例、跳到处理块入口（实测：入口就是 `PUSH_EXC_INFO` 那条指令）。
                dispatch_raise(
                    instance,
                    frame.get(),
                    code.exceptiontable(),
                    instruction.offset * 2,
                    exception,
                    &mut decoder,
                )?;
            }
            Err(other) => return Err(other),
        }
    }
    Err(ExecError::FellOffEnd)
}
pub mod subscript;
pub mod call;
pub mod arithmetic;
pub mod attribute;
pub mod import;
pub mod message;
pub mod values;
pub mod ctrls;
pub mod runtime;
pub mod format;
pub mod iter;
pub mod iter_advance;
pub mod protocol;
pub mod step_misc;
pub mod step_checks;
pub mod step_names;
pub mod step_locals;
pub mod step_operators;
pub mod step_jumps;
pub mod step_containers;
pub mod step_iterators;
pub mod step_calls;
pub mod step_match;
pub mod step_format;
pub mod step_exceptions;
pub mod step_imports;
pub mod step_attributes;
pub mod step_generators;
