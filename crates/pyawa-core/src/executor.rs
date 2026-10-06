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

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::bigint::IntValue;
use crate::decode::{parse_exception_table, DecodeError, Decoder};
use crate::frame::{Frame, FrameError};
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
pub use crate::executor::iter::*;
pub use crate::executor::protocol::*;
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::opcode;
use crate::refcount::{Owned, PyRef};
use crate::builtin_objects::{
    AsendObject, BoolObject, ExceptionObject,
    GeneratorObject, IteratorObject, MethodObject, DictObject, FloatObject, FunctionObject,
    IntObject, ListObject, SetObject, StrObject, TupleObject,
};
use crate::singleton::{SMALL_INT_MAX, SMALL_INT_MIN};
use crate::value::Value;

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
pub(crate) const ITERATOR_TYPE_NAMES: [&str; 28] = [
    "tuple_iterator",
    "list_iterator",
    "str_ascii_iterator",
    "bytes_iterator",
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

/// **`super` 的属性查找** ✓（第 233 轮）：在 `type(__self__)` 的 MRO 上、**定义类之后**找 ✓。
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
    let this_type = instance.type_of(this);
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
            if let Some(found) = instance.type_lookup(entry, name) {
                // **两族都要绑** ✓（第 106 轮真 bug 修 ✗）：先前只认 `function` ✗ ——
                // **原生方法**（`builtin_function_or_method` ✓，如 `dict.__init__` ✓）被原样交出 ✓
                // ⇒ 调用时一个 `self` 都没有 ✓ ⇒ 原生那侧报 `descriptor needs an argument` ✓
                //（上限榜那一族 **119** 个模块的第一句错 ✓：`Lib/enum.py` 的
                //  `EnumDict.__init__` 里那句 `super().__init__()` ✓，现场实测 `@21` ✓）。
                let found_type = instance.type_of(found);
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
    let frame = crate::classes::class_body_frame(instance, code, namespace, globals);
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

/// 渲染一个标签给消息用：类型对象给名字、二元组给 `外[内]`、其余给 `Any`。
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
        let dict = instance.alloc(DictObject::new(
            builtin_type(instance, "dict"),
            RefCell::new(collected),
        ));
        locals[argcount + kwonly + usize::from(code.has_varargs())] =
            Some(dict.into_raw().cast::<Header>());
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
        let previous = instance.set_current_frame(frame);
        Self { instance, previous }
    }
}

impl Drop for CurrentFrameGuard<'_> {
    fn drop(&mut self) {
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
            "RESUME" | "NOP" => {}
            // `BC-23`：边界检查的两条**专有**指令（`TS-10`…`TS-13`）
            "CHECK_BOUNDARY_IN" | "CHECK_BOUNDARY_OUT" => {
                boundary_check(instance, frame.get(), oparg as u8, opcode_number)?;
            }
            "LOAD_COMMON_CONSTANT" => {
                // `BC-57`＋`SPEC-bytecode.md`：oparg 索引**固定表**（实测 `dis._common_constants`：
                // 0 `AssertionError`／1 `NotImplementedError`／2 `tuple`／3 `all`／4 `any`），
                // **不是** `co_consts`／`co_names`。前三个是**类型对象**，后两个取 builtins 里的函数。
                let value = match oparg {
                    0 => instance.type_value(
                        instance
                            .type_named("AssertionError")
                            .expect("AssertionError 在内建表里"),
                    ),
                    1 => instance.type_value(
                        instance
                            .type_named("NotImplementedError")
                            .expect("NotImplementedError 在内建表里"),
                    ),
                    2 => instance.type_value(
                        instance.type_named("tuple").expect("tuple 在内建表里"),
                    ),
                    3 | 4 => {
                        let name = if oparg == 3 { "all" } else { "any" };
                        match instance
                            .builtins()
                            .and_then(|builtins| lookup_in_mapping(instance, builtins, name))
                        {
                            Some(found) => {
                                // SAFETY: found 由 builtins 持有，存活。
                                unsafe { instance.incref_object(found.as_ptr()) };
                                found
                            }
                            None => {
                                return Err(raise_builtin(
                                    instance,
                                    "NameError",
                                    &format!("name '{name}' is not defined"),
                                ))
                            }
                        }
                    }
                    _ => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "LOAD_COMMON_CONSTANT 的索引超出固定表（0…4）",
                        })
                    }
                };
                push(instance, frame.get(), value)?;
                release(instance, value);
            }
            // **cell 族**（`BC-45`）：cell 是独立对象（`CellObject`），帧的 cell 槽存"哪个 cell"。
            // 类体那条路径（`__classdict__`）与将来的闭包都用它。
            "COPY_FREE_VARS" => {
                // **3.11+ 是 no-op**：自由变量在建帧阶段由函数的闭包装入（`Frame::install_closure`）。
                // 保留这条 arm 是为了**不报 Unsupported**（如实表达"语义已由建帧承担"）。
                let _ = oparg;
            }
            "MAKE_CELL" => {
                // 净 0：把 **cell 槽**第 `oparg` 格换成一个新 cell；初值取**同号局部槽**（若有）
                let slot = oparg as usize;
                // **把同号局部槽的值"搬"进新 cell**（第 334 轮真 bug 修 ✗）：先前用 `raw_local`
                // **借用** ✗ ⇒ 同一份引用**两边都算持有** ✓（局部数组一份、cell 一份 ✓）⇒
                // 帧收尾释放局部那份 ＋ `cell_clear` 释放 cell 那份 ⇒ **同一份引用被减两次** ✗
                // —— 实测：`import threading` ⇒ `对已释放对象 decref：类型 list` ✓，
                // 回溯落点正是 `Header::decref ← cell::cell_clear` ✓（上限榜上那一族 73 个模块 ✓）。
                // 现在把局部那份**取走**（`set_local(slot, None)` 返回旧值 ✓）⇒ 所有权只剩一份 ✓。
                // 类体的 `nlocals` 是 0 ⇒ 这一步给 `None` ✓（与先前一致 ✓）。
                let initial = frame.get().set_local(slot, None).unwrap_or(None);
                let cell_type = instance
                    .type_named("cell")
                    .expect("引导期已登记 cell 类型");
                let cell = instance
                    .alloc(crate::cell::CellObject::new(cell_type, RefCell::new(initial)))
                    .into_raw()
                    .cast::<Header>();
                match frame.get().set_cell(slot, Some(cell)) {
                    Ok(Some(old)) => release(instance, old),
                    Ok(None) => {}
                    Err(error) => {
                        release(instance, cell);
                        return Err(ExecError::Frame(error));
                    }
                }
            }
            "LOAD_LOCALS" => {
                // 净 +1：压**本帧的命名空间映射**（类体的 `LOAD_LOCALS` 就是取那个 dict）。
                // 函数帧没有独立命名空间 ⇒ 如实报未接线。
                match frame.get().namespace() {
                    Some(mapping) => {
                        // SAFETY: mapping 由帧持有，存活。
                        unsafe { instance.incref_object(mapping.as_ptr()) };
                        push(instance, frame.get(), mapping)?;
                        release(instance, mapping);
                    }
                    None => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "`LOAD_LOCALS` 只接线了带命名空间的帧（类体）",
                        })
                    }
                }
            }
            "STORE_DEREF" => {
                // 净 −1：把 TOS 存进 cell 槽第 `oparg` 格那个 cell（cell 接手一份引用）
                let value = frame.get().pop()?;
                let slot = oparg as usize;
                let cell = match frame.get().cell(slot) {
                    Ok(cell) => cell,
                    Err(error) => {
                        release(instance, value);
                        return Err(ExecError::Frame(error));
                    }
                };
                let Some(cell) = cell else {
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "`STORE_DEREF` 的 cell 槽是空的（`MAKE_CELL` 没跑过）",
                    });
                };
                // SAFETY: cell 由帧的 cell 槽持有，存活。
                let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                // **`replace` 接管 `value` 那份引用** ✓（见 `CellObject::replace` 的契约 ✓）——
                // 这里**不得**再释放一次 ✗（第 208 轮真 bug 修复 ✗：先前多放一次 ⇒ cell 里留着
                // **没被记账的指针** ✗ ⇒ 值被提前释放 ⇒ cell 还指着它 ⇒ **释放后重用** ⇒ 堆损坏 ✓）。
                // 线索来自"**每次释放前扫全图看还有谁指着它**"那把尺子 ✓（引用者全是 `cell` ✓）。
                if let Some(old) = object.replace(Some(value)) {
                    release(instance, old);
                }
            }
            "LOAD_DEREF" => {
                // 净 +1：压 cell 槽第 `oparg` 格那个 cell 的值
                let cell = match frame.get().cell(oparg as usize) {
                    Ok(Some(cell)) => cell,
                    Ok(None) => {
                        // **诊断升级 ＋ 更接近参照** ✓（第 267 轮）：参照在同样情形给 `NameError` ✓
                        // ⇒ 把 **cell 名字**与**作用域**一起报出来 ✓（先前只说"cell 是空的" ✗ ⇒ 无从下手 ✓）。
                        let frame_ref = frame.get();
                        let (name, scope) = match frame_ref.code() {
                            Some(header) => {
                                let code = unsafe { &*header.as_ptr().cast::<crate::CodeObject>() };
                                (
                                    localsplus_name(code, oparg as usize),
                                    code.name().to_owned(),
                                )
                            }
                            None => ("?".to_owned(), "?".to_owned()),
                        };
                        let message = format!(
                            "cannot access free variable '{name}' where it is not associated with a value yet（作用域 {scope} ✓ 指令 {} ✓）",
                            frame_ref.instruction_pointer()
                        );
                        return Err(instance.raise_builtin_error("NameError", &message));
                    }
                    Err(error) => return Err(ExecError::Frame(error)),
                };
                // SAFETY: cell 由帧的 cell 槽持有，存活。
                let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                let Some(value) = object.value() else {
                    // **诊断升级 ＋ 更接近参照** ✓（第 267 轮）：参照在同样情形给 `NameError` ✓ ⇒
                    // 把 **cell 名字**与**作用域**一起报出来 ✓（先前只说“还是空的” ✗ ⇒ 无从下手 ✓）。
                    let frame_ref = frame.get();
                    let (name, scope) = match frame_ref.code() {
                        Some(header) => {
                            let code = unsafe { &*header.as_ptr().cast::<crate::CodeObject>() };
                            (localsplus_name(code, oparg as usize), code.name().to_owned())
                        }
                        None => ("?".to_owned(), "?".to_owned()),
                    };
                    let message = format!(
                        "cannot access free variable '{name}' where it is not associated with a value yet（作用域 {scope} ✓ 指令 {} ✓）",
                        frame_ref.instruction_pointer()
                    );
                    return Err(instance.raise_builtin_error("NameError", &message));
                };
                // SAFETY: 值由 cell 持有，存活。
                unsafe { instance.incref_object(value.as_ptr()) };
                push(instance, frame.get(), value)?;
                release(instance, value);
            }
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
            // `LOAD_FAST_BORROW` 是 3.14 的借用形态：语义与 `LOAD_FAST` 相同（栈上不留新引用）。
            // 本层的值栈一律持有引用，故照常新增一份——**可观察语义一致**，只是少了那点优化。
            "LOAD_FAST" | "LOAD_FAST_CHECK" | "LOAD_FAST_BORROW" => {
                                match frame.get().local(oparg) {
                    Ok(Some(raw)) => push(instance, frame.get(), raw)?,
                    Ok(None) => return Err(unbound_local_error(instance, frame.get(), oparg)),
                    // **cell 在参照实现里也是"快速局部槽"**：类体的 `__classdict__` 只有 cell 槽
                    // （`nlocals` 是 0），而参照收尾用的是 `LOAD_FAST_BORROW 0` 读那个 cell
                    // ⇒ 局部槽越界时回落到**同号 cell 槽**（`BC-45` 的独立 cell 槽模型下的兼容）。
                    Err(crate::FrameError::SlotOutOfRange { .. }) => {
                        let cell = match frame.get().cell(oparg) {
                            Ok(Some(cell)) => cell,
                            _ => return Err(unbound_local_error(instance, frame.get(), oparg)),
                        };
                        // SAFETY: cell 由帧的 cell 槽持有，存活。
                        let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                        let Some(value) = object.value() else {
                            return Err(unbound_local_error(instance, frame.get(), oparg));
                        };
                        // SAFETY: 值由 cell 持有，存活。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        push(instance, frame.get(), value)?;
                        release(instance, value);
                    }
                    Err(error) => return Err(ExecError::Frame(error)),
                }
            }
            "STORE_FAST" => {
                let value = frame.get().pop()?;
                // **NULL 哨兵＝"未绑定"**（第 234 轮）：推导式的 `LOAD_FAST_AND_CLEAR` 在外层同名局部
                // **本来就没有**时会压那个哨兵，收尾的 `STORE_FAST` 要把它还原成"清空槽"而不是存一个
                // NULL 对象（否则推导式之后那个名字会变成 NULL 而不是 `NameError`）
                let restored = if value == instance.singletons().null() {
                    None
                } else {
                    Some(value)
                };
                                if let Some(old) = frame.get().set_local(oparg, restored)? {
                    release(instance, old);
                }
                // **`STORE_FAST` 的槽号诊断** ✓（第 305 轮，门控 `PYAWA_STORE_FAST_DEBUG=1`）：
                // 与融合加载读到的槽号对照 ✓ ⇒ 定"写入槽 ≠ 读取槽"✓（第 304 轮的铁证 ✓）。
                if crate::diag::flag("PYAWA_STORE_FAST_DEBUG") {
                    if let Some(v) = restored {
                        // SAFETY: v 由本帧刚写入的槽持有，存活。
                        let tn = unsafe { (&*v.as_ptr()).ty().as_ref() }.name().to_owned();
                        eprintln!("[store_fast] slot={oparg} type={tn}");
                    }
                }
            }
            "DELETE_FAST" => {
                match frame.get().set_local(oparg, None)? {
                Some(old) => release(instance, old),
                None => return Err(unbound_local_error(instance, frame.get(), oparg)),
                }
            }
            "POP_TOP" => {
                if crate::diag::flag("PYAWA_POP_DEBUG") {
                    eprintln!("[pop_top] 弹前深={} site={}", frame.get().depth(), instance.current_site());
                }
                release(instance, frame.get().pop()?)
            }
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
                let symbol = if name == "UNARY_NEGATIVE" { "-" } else { "~" };
                let result = unary_public(instance, value, symbol, opcode_number);
                release(instance, value);
                frame.get().push(result?)?;
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
            "BUILD_TUPLE" | "BUILD_LIST" | "BUILD_SET" => {
                // oparg 是元素个数；压栈顺序就是元素顺序（先压的在前）
                let mut items = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    items.push(frame.get().pop()?);
                }
                items.reverse();

                match name {
                    "BUILD_TUPLE" => {
                        // **OM-23**：空元组是单例 ⇒ 必须走 `new_tuple`（`items` 为空时它给单例）
                        let tuple = instance.new_tuple(items);
                        frame.get().push(tuple)?;
                    }
                    "BUILD_LIST" => push_container(
                        instance,
                        frame.get(),
                        ListObject::new(builtin_type(instance, "list"), RefCell::new(items)),
                    )?,
                    _ => {
                        // set：按**值相等**查重，保留**先出现**的那个（与参照实现一致）
                        let set = instance.alloc(SetObject::new(
                            builtin_type(instance, "set"),
                            RefCell::new(Vec::new()),
                        ));
                        for item in items {
                            let duplicate = set
                                .get()
                                .items()
                                .iter()
                                .any(|existing| values_equal(instance, *existing, item));
                            if duplicate {
                                release(instance, item);
                            } else {
                                set.get().insert_raw(item);
                            }
                        }
                        frame.get().push(set.into_raw().cast::<Header>())?;
                    }
                }
            }
            "BUILD_SLICE" => {
                // `a[b:c:d]`（三段，至少一段非常量时参照发这条）：栈序是 lower, upper[, step]
                if !(2..=3).contains(&oparg) {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BUILD_SLICE 的 oparg 只能是 2 或 3（参照实测）",
                    });
                }
                let mut arguments = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    arguments.push(frame.get().pop()?);
                }
                arguments.reverse();
                let slice = build_slice(instance, &arguments, opcode_number)?;
                frame.get().push(slice)?;
            }
            "BINARY_SLICE" => {
                // `a[b:c]`（两段，至少一段非常量时参照发这条）：栈序是 container, lower, upper
                let upper = frame.get().pop()?;
                let lower = frame.get().pop()?;
                let container = frame.get().pop()?;
                let slice = build_slice(instance, &[lower, upper], opcode_number)?;
                let result = subscript_get(instance, container, slice, opcode_number);
                release(instance, container);
                release(instance, slice);
                frame.get().push(result?)?;
            }
            "BUILD_MAP" => {
                // 压栈顺序是 key1 value1 key2 value2 …（实测），弹出后反转成对
                let mut items = Vec::with_capacity(oparg * 2);
                for _ in 0..oparg * 2 {
                    items.push(frame.get().pop()?);
                }
                items.reverse();

                let dict = instance.alloc(DictObject::new(
                    builtin_type(instance, "dict"),
                    RefCell::new(Vec::new()),
                ));
                for pair in items.chunks(2) {
                    let (key, value) = (pair[0], pair[1]);
                    // 键按**值相等**查重：命中则**保留先出现的键**、替换值
                    let position = dict
                        .get()
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key));
                    match position {
                        Some(slot) => {
                            if let Some(old) = dict.get().replace_value(slot, value) {
                                release(instance, old);
                            }
                            release(instance, key);
                        }
                        None => dict.get().insert_raw(key, value),
                    }
                }
                frame.get().push(dict.into_raw().cast::<Header>())?;
            }
            "BUILD_STRING" => {
                let mut parts = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    parts.push(frame.get().pop()?);
                }
                parts.reverse();

                let str_type = instance.singletons().str_type();
                let mut text = String::new();
                let mut wrong_type = false;
                for part in parts {
                    // SAFETY: part 是刚出栈的存活对象。
                    if unsafe { part.as_ref() }.ty() == str_type {
                        // SAFETY: 类型身份已确认。
                        text.push_str(unsafe { &*part.as_ptr().cast::<StrObject>() }.value());
                    } else {
                        wrong_type = true;
                    }
                    release(instance, part);
                }
                if wrong_type {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BUILD_STRING 只接线了 str",
                    });
                }
                if text.is_empty() {
                    // OM-23：空串走单例
                    push(instance, frame.get(), instance.singletons().empty_str())?;
                } else {
                    push_container(instance, frame.get(), StrObject::new(str_type, text))?;
                }
            }
            "UNPACK_SEQUENCE" | "UNPACK_EX" => {
                let raw = frame.get().pop()?;
                if crate::diag::flag("PYAWA_UNPACK_DEBUG") {
                    // SAFETY: raw 由本帧值栈持有，存活。
                    let tn = unsafe { (&*raw.as_ptr()).ty().as_ref() }.name().to_owned();
                    eprintln!("[unpack_site] opcode={opcode_number} raw_type={tn} site={}", instance.current_site());
                }
                let items = sequence_items(instance, raw, opcode_number);
                release(instance, raw);
                let items = items?;

                if name == "UNPACK_SEQUENCE" {
                    if items.len() != oparg {
                        for item in items.iter().copied() {
                            release(instance, item);
                        }
                        let message = if items.len() < oparg {
                            format!(
                                "not enough values to unpack (expected {oparg}, got {})",
                                items.len()
                            )
                        } else {
                            format!(
                                "too many values to unpack (expected {oparg})"
                            )
                        };
                        return Err(raise_builtin(instance, "ValueError", &message));
                    }
                    // 参照实现把元素**从右往左**压栈 ⇒ 最左边的目标拿到 TOS
                    for item in items.into_iter().rev() {
                        frame.get().push(item)?;
                    }
                } else {
                    // BC-38：UNPACK_EX 的 oparg ＝ 前者个数 ｜ 后者个数 << 8
                    let before = oparg & 0xFF;
                    let after = oparg >> 8;
                    if items.len() < before + after {
                        for item in items.iter().copied() {
                            release(instance, item);
                        }
                        let message = format!(
                            "not enough values to unpack (expected at least {}, got {})",
                            before + after,
                            items.len()
                        );
                        return Err(raise_builtin(instance, "ValueError", &message));
                    }
                    let total = items.len();
                    let middle = items[before..total - after].to_vec();
                    let middle_list = instance.alloc(ListObject::new(
                        builtin_type(instance, "list"),
                        RefCell::new(middle),
                    ));
                    let middle_raw = middle_list.into_raw().cast::<Header>();

                    for item in items[total - after..].iter().rev() {
                        frame.get().push(*item)?;
                    }
                    frame.get().push(middle_raw)?;
                    for item in items[..before].iter().rev() {
                        frame.get().push(*item)?;
                    }
                }
            }
            "LIST_APPEND" | "SET_ADD" => {
                // **第 235 轮实测修正**：真正的栈里 `FOR_ITER` **不弹迭代器**，迭代器夹在容器与
                // 元素之间 ⇒ `[保存值, 容器, 迭代器, 元素]`。参照的 `PEEK(oparg)` 是"从新栈顶数
                // 第 oparg 个"（`PEEK(1)` 才是 TOS）⇒ 弹出值之后容器在 `peek_from_top(oparg)`
                //（此前写成 `oparg - 1`，取到的是迭代器 ⇒ 报"容器在栈上的位置或类型不符"）
                if oparg == 0 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LIST_APPEND／SET_ADD 的 oparg 至少为 1",
                    });
                }
                let value = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg)?;
                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if name == "LIST_APPEND" && ty == builtin_type(instance, "list") {
                    // SAFETY: 类型身份已确认。
                    unsafe { &*container.as_ptr().cast::<ListObject>() }.append(value);
                } else if name == "SET_ADD" && ty == builtin_type(instance, "set") {
                    // SAFETY: 同上。
                    let set = unsafe { &*container.as_ptr().cast::<SetObject>() };
                    let duplicate = set
                        .items()
                        .iter()
                        .any(|existing| values_equal(instance, *existing, value));
                    if duplicate {
                        release(instance, value);
                    } else {
                        set.insert_raw(value);
                    }
                } else {
                    release(instance, value);
                    // **诊断**：把真实栈形状带进报错（此前只报"位置或类型不符"，看不出取错了哪一格）
                    let mut kinds: Vec<String> = Vec::new();
                    for index in 1..=5 {
                        kinds.push(match frame.get().peek_from_top(index) {
                            Ok(raw) => {
                                let raw_ty = unsafe { raw.as_ref() }.ty();
                                let mut label = "其它".to_owned();
                                for candidate in ["list", "tuple", "set", "dict", "int", "str"] {
                                    if raw_ty == builtin_type(instance, candidate) {
                                        label = candidate.to_owned();
                                    }
                                }
                                if raw == instance.singletons().none() {
                                    label = "None".to_owned();
                                }
                                if raw == instance.singletons().null() {
                                    label = "NULL".to_owned();
                                }
                                label
                            }
                            Err(_) => "越界".to_owned(),
                        });
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: Box::leak(
                            format!(
                                "{name} 的容器位置／类型不符（oparg {oparg}，栈顶往下 {kinds:?}）"
                            )
                            .into_boxed_str(),
                        ),
                    });
                }
            }
            "MAP_ADD" => {
                // **第 235 轮实测修正**：同 `LIST_APPEND`——`[保存值, 容器, 迭代器, 键, 值]`，
                // 地址从新栈顶数：弹出键值之后容器在 `peek_from_top(oparg)`（此前写成 `oparg - 1`）。
                if oparg == 0 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAP_ADD 的 oparg 至少为 1",
                    });
                }
                let value = frame.get().pop()?;
                let key = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg)?;
                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if ty != builtin_type(instance, "dict") {
                    release(instance, key);
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: Box::leak(
                            format!("MAP_ADD 的容器在栈上的位置或类型不符（opcode {opcode_number}）")
                                .into_boxed_str(),
                        ),
                    });
                }
                // SAFETY: 类型身份已确认。
                let dict = unsafe { &*container.as_ptr().cast::<DictObject>() };
                let position = dict
                    .entries()
                    .iter()
                    .position(|(existing, _)| values_equal(instance, *existing, key));
                match position {
                    Some(slot) => {
                        if let Some(old) = dict.replace_value(slot, value) {
                            release(instance, old);
                        }
                        release(instance, key);
                    }
                    None => dict.insert_raw(key, value),
                }
            }
            "LIST_EXTEND" | "SET_UPDATE" => {
                // 实测（`[*a, *b]`）：容器在 PEEK(oparg + 1)、源是 TOS ⇒ 弹出源之后容器在 PEEK(oparg)
                let source = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg)?;
                let items = sequence_items(instance, source, opcode_number);
                release(instance, source);
                let items = items?;

                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if name == "LIST_EXTEND" && ty == builtin_type(instance, "list") {
                    // SAFETY: 类型身份已确认。
                    unsafe { &*container.as_ptr().cast::<ListObject>() }.extend(items);
                } else if name == "SET_UPDATE" && ty == builtin_type(instance, "set") {
                    // SAFETY: 同上。
                    let set = unsafe { &*container.as_ptr().cast::<SetObject>() };
                    for item in items {
                        let duplicate = set
                            .items()
                            .iter()
                            .any(|existing| values_equal(instance, *existing, item));
                        if duplicate {
                            release(instance, item);
                        } else {
                            set.insert_raw(item);
                        }
                    }
                } else {
                    for item in items {
                        release(instance, item);
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "容器在栈上的位置或类型不符（指令名见下）",
                    });
                }
            }
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
                if !is_iterator_type(instance, ty) {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "FOR_ITER 的对象不是本层接线的迭代器",
                    });
                }
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
                    match iterator_type_for(instance, iterable) {
                        Ok(iterator_type) => {
                            let iterator =
                                instance.alloc(IteratorObject::new(iterator_type, iterable, Cell::new(0)));
                            frame.get().push(iterator.into_raw().cast::<Header>())?;
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
            "NOT_TAKEN" => {
                // §10 三分类②：参照实现**会发**这条（跟在 `POP_JUMP_*` 之后），
                // 但它是给专门化解释器用的提示；VM **必须容受**它（净 0，什么也不做）。
            }
            "LOAD_FAST_AND_CLEAR" => {
                // **推导式的"变量不外泄"**（第 234 轮实测）：把该局部**原值**压栈（本来没绑定就压
                // NULL 哨兵），随即**清空**这个槽；推导式收尾的 `STORE_FAST` 再把它还原
                let saved = frame.get().local(oparg)?;
                match saved {
                    Some(raw) => push(instance, frame.get(), raw)?,
                    None => push(instance, frame.get(), instance.singletons().null())?,
                }
                if let Some(old) = frame.get().set_local(oparg, None)? {
                    release(instance, old);
                }
            }
            "STORE_FAST_LOAD_FAST" => {
                // 净 0：`oparg` 打包两个局部槽——**高 4 位收 TOS**、低 4 位**再压回**（实测
                // `STORE_FAST_LOAD_FAST 0 (x, x)`：先存 `x` 再把同一个槽压回来）
                let value = frame.get().pop()?;
                let store_slot = oparg >> 4;
                let load_slot = oparg & 0x0F;
                if frame.get().set_local(store_slot, Some(value)).is_err() {
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "STORE_FAST_LOAD_FAST 的槽位越界",
                    });
                }
                let loaded = frame.get().local(load_slot)?.ok_or(ExecError::UnboundLocal {
                    slot: load_slot,
                })?;
                push(instance, frame.get(), loaded)?;
            }
            "LOAD_FAST_LOAD_FAST" | "LOAD_FAST_BORROW_LOAD_FAST_BORROW" => {
                // 实测净 +2：`oparg` 打包两个局部槽，**高 4 位先压**（`dis` 的 argrepr 就是
                // "(第一个, 第二个)"；`LOAD_FAST_BORROW_LOAD_FAST_BORROW 1 (a, b)` 里 a＝0、b＝1）
                let first = oparg >> 4;
                let second = oparg & 0x0F;
                let left = frame.get().local(first)?.ok_or(ExecError::UnboundLocal {
                    slot: first,
                })?;
                push(instance, frame.get(), left)?;
                let right = frame.get().local(second)?.ok_or(ExecError::UnboundLocal {
                    slot: second,
                })?;
                push(instance, frame.get(), right)?;
                // **融合加载的槽号诊断** ✓（第 304 轮，门控 `PYAWA_FUSED_LOAD_DEBUG=1`）：第 302 轮
                // 已证"错值是这一句自己压的" ✗ ⇒ 这里看它**到底从哪两个槽取的** ✓。
                if crate::diag::flag("PYAWA_FUSED_LOAD_DEBUG") {
                    // SAFETY: 两个值分别由对应帧槽持有，存活。
                    let lt = unsafe { (&*left.as_ptr()).ty().as_ref() }.name().to_owned();
                    let rt = unsafe { (&*right.as_ptr()).ty().as_ref() }.name().to_owned();
                    eprintln!(
                        "[fused_load] oparg={oparg} first={first} second={second} left={lt} right={rt}"
                    );
                }
            }
            "DICT_MERGE" | "DICT_UPDATE" => {
                // 实测净 −1：把 TOS 那个字典并进 TOS1，然后弹掉 TOS。
                // `DICT_UPDATE` 覆盖同名键；`DICT_MERGE` 遇到同名键要报错——那条消息在参照实现里
                // 带着**函数的 qualname**（实测：`__main__.demo() got multiple values for keyword
                // argument 'a'`），而此刻调用者还在栈下好几层，本层取不到，所以如实报未接线。
                let source = frame.get().pop()?;
                let destination = frame.get().peek()?;
                // SAFETY: 两个都在帧值栈上，存活。
                let source_type = unsafe { source.as_ref() }.ty();
                let destination_type = unsafe { destination.as_ref() }.ty();
                if source_type != builtin_type(instance, "dict")
                    || destination_type != builtin_type(instance, "dict")
                {
                    release(instance, source);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "DICT_MERGE／DICT_UPDATE 只接线了 dict（映射协议随后补）",
                    });
                }
                // SAFETY: 类型身份已确认。
                let source_entries = unsafe { &*source.as_ptr().cast::<DictObject>() }.entries();
                // SAFETY: 同上。
                let destination_dict = unsafe { &*destination.as_ptr().cast::<DictObject>() };
                let is_merge = name == "DICT_MERGE";
                for (key, value) in source_entries {
                    let position = destination_dict
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key));
                    if let Some(existing_position) = position {
                        if is_merge {
                            release(instance, source);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "DICT_MERGE 的同名键错误要函数的 qualname（参照实现的消息带它）",
                            });
                        }
                        let (old_key, old_value) = destination_dict
                            .remove(existing_position)
                            .expect("刚查到的位置");
                        release(instance, old_key);
                        release(instance, old_value);
                    }
                    // SAFETY: 键值由源字典持有，这里各新增一份引用交给目标字典。
                    unsafe {
                        instance.incref_object(key.as_ptr());
                        instance.incref_object(value.as_ptr());
                    }
                    destination_dict.insert_raw(key, value);
                }
                release(instance, source);
            }
            "CALL_FUNCTION_EX" => {
                // 实测净 −3；栈自下而上是 `[可调用, self|NULL, 实参 tuple, 关键字 dict|NULL]`
                // （`f(*a)` 的发射里第二个 `PUSH_NULL` 就是"没有关键字"那一格）。
                let keyword_source = frame.get().pop()?;
                let argument_source = frame.get().pop()?;
                let self_or_null = frame.get().pop()?;
                let callable = frame.get().pop()?;
                // SAFETY: 都在帧值栈上（刚出栈），存活。
                let null = instance.singletons().null();
                let bound_self = if self_or_null == null {
                    None
                } else {
                    // SAFETY: 同上。
                    unsafe { instance.incref_object(self_or_null.as_ptr()) };
                    Some(self_or_null)
                };
                release(instance, self_or_null);

                // SAFETY: 类型身份检查在下面。
                let argument_type = unsafe { argument_source.as_ref() }.ty();
                // **实参可以是任意可迭代** ✓（第 149 轮实测参照：`f(*[1, 2])`／`f(*(i for i in (1, 2)))`
                //   都行 ✓）⇒ `tuple` 走快路 ✓，其余按**迭代协议**摊开 ✓（`iterable_items` 是
                //   **一处真相** ✓，且它返回的是**借用** ⇒ 每项先 `retain` ✓ —— 第 145 轮的教训 ✓）。
                let mut args: Vec<NonNull<Header>> = Vec::new();
                // **位置实参为空时参照传的是 `NULL`**（第 149 轮：`f(1, **kw)` ⇒ `LOAD f; LOAD_CONST 1;
                //   BUILD_MAP 1; …; CALL_FUNCTION_EX` ✓，其中实参那格是 `NULL` ✗ 不是空元组 ✓）
                // ⇒ 先认下这一种，再走后面的通用路 ✓。
                if argument_source == null {
                    // 空实参 ⇒ 什么也不用做 ✓（`args` 已是空表 ✓）
                } else if argument_type == builtin_type(instance, "tuple") {
                    // SAFETY: 类型身份已确认。
                    let arguments = unsafe { &*argument_source.as_ptr().cast::<TupleObject>() };
                    args.reserve(arguments.len());
                    for index in 0..arguments.len() {
                        let value = arguments.item(index).expect("下标在范围内");
                        // SAFETY: 元素由元组持有。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        args.push(value);
                    }
                } else if let Some(items) = instance.iterable_items(argument_source) {
                    args.reserve(items.len());
                    for item in items {
                        instance.retain(item);
                        args.push(item);
                    }
                } else {
                    release(instance, callable);
                    release(instance, argument_source);
                    release(instance, keyword_source);
                    if let Some(bound) = bound_self {
                        release(instance, bound);
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "argument after * must be an iterable",
                    });
                }
                release(instance, argument_source);

                let mut kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
                if keyword_source != null {
                    // SAFETY: 类型身份检查在下面。
                    let keyword_type = unsafe { keyword_source.as_ref() }.ty();
                    if keyword_type != builtin_type(instance, "dict") {
                        for value in args {
                            release(instance, value);
                        }
                        release(instance, callable);
                        release(instance, keyword_source);
                        if let Some(bound) = bound_self {
                            release(instance, bound);
                        }
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "CALL_FUNCTION_EX 的关键字必须是 dict（编译器保证）",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let mapping = unsafe { &*keyword_source.as_ptr().cast::<DictObject>() };
                    for (key, value) in mapping.entries() {
                        // SAFETY: 键值由字典持有。
                        unsafe {
                            instance.incref_object(key.as_ptr());
                            instance.incref_object(value.as_ptr());
                        }
                        kwargs.push((key, value));
                    }
                }
                release(instance, keyword_source);

                let result =
                    call_callable(instance, callable, bound_self, args, kwargs, opcode_number)?;
                frame.get().push(result)?;
            }
            "CALL_INTRINSIC_1" => {
                // 实测净 0（就地把 TOS 换掉）；oparg 是**内建表的编号**，按名字分派（`BC-50`）。
                let intrinsic = crate::opcode_metadata::INTRINSIC1_DESCS
                    .get(oparg)
                    .copied()
                    .unwrap_or("INTRINSIC_1_INVALID");
                match intrinsic {
                    "INTRINSIC_UNARY_POSITIVE" => {
                        // `+x`：本层只接整数／布尔（真协议 `__pos__` 随后补）——原地不动即可
                        let value = frame.get().peek()?;
                        // SAFETY: value 在帧值栈上，存活。
                        let ty = unsafe { value.as_ref() }.ty();
                        // **浮点也要过**（第 318 轮）：`+1.5` 参照给 `1.5` ✓ —— 先前只接
                        // 整数／布尔 ✗ ⇒ `f(+1.5)` 报"只接线了整数／布尔" ✗。
                        if ty != instance.singletons().int_type()
                            && ty != instance.singletons().bool_type()
                            && ty != builtin_type(instance, "float")
                        {
                            // **实例走 `__pos__`**（第 220 轮；`MS-19`：能力缺口必须修 ✓）：
                            // 参照对**没有** `__pos__` 的对象报
                            // `TypeError: bad operand type for unary +: 'C'` ✓；先前这里一律报
                            // "只接线了整数／布尔／浮点"（`Unsupported` ✗）—— 连错误面都对不上 ✗。
                            // 交给 `unary_public` 的同一条 dunder 路 ✓（第 219 轮已经铺好 ✓）。
                            let operand = frame.get().pop()?;
                            let outcome = unary_public(instance, operand, "+", opcode_number);
                            release(instance, operand);
                            let result = outcome?;
                            push(instance, frame.get(), result)?;
                            release(instance, result);
                        }
                    }
                    "INTRINSIC_IMPORT_STAR" => {
                        // `from <模块> import *`：把模块的**公开**名字写进当前命名空间 ✓
                        // （最小面：`__all__` 还没接 ✗ ⇒ 只取不以下划线开头的名字 ✓，与参照的默认口径同 ✓）
                        // **不动栈**：参照里随后的 `POP_TOP` 才把模块弹掉 ✓（`IMPORT_STAR` 净 0 ✓）
                        let module = frame.get().peek()?;
                        let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "`import *` 需要命名空间帧（模块／类体）",
                        })?;
                        let Some(attributes) = mounted_instance_dict(instance, module) else {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "`import *` 的对象没有属性字典",
                            });
                        };
                        // SAFETY: attributes 由模块对象持有，存活。
                        let entries = unsafe { &*attributes.as_ptr().cast::<DictObject>() }.entries();
                        for (key, _) in entries {
                            let Some(name) = instance.text_of(key).map(|text| text.to_owned())
                            else {
                                continue;
                            };
                            if name.starts_with('_') {
                                continue;
                            }
                            let Some(value) = instance.dict_get(attributes, &name) else {
                                continue;
                            };
                            // **交一份新引用**：`dict_set` 会接管它（`OM-16` ✓）
                            // SAFETY: value 由模块的属性字典持有，存活。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            instance.dict_set(namespace, &name, value);
                        }
                    }
                    "INTRINSIC_LIST_TO_TUPLE" => {
                        // `(*[1, 2],)`：把 TOS 的列表换成元组（元素各持一份引用）
                        let value = frame.get().pop()?;
                        // SAFETY: value 是刚出栈的存活对象。
                        let ty = unsafe { value.as_ref() }.ty();
                        if ty != builtin_type(instance, "list") {
                            release(instance, value);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "INTRINSIC_LIST_TO_TUPLE 的操作数必须是 list",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        let list = unsafe { &*value.as_ptr().cast::<ListObject>() };
                        let items = list.items();
                        let mut moved: Vec<NonNull<Header>> = Vec::with_capacity(items.len());
                        for item in items {
                            // SAFETY: 元素由列表持有，这里各新增一份引用交给元组。
                            unsafe { instance.incref_object(item.as_ptr()) };
                            moved.push(item);
                        }
                        release(instance, value);
                        let tuple = instance.new_tuple(moved);
                        frame.get().push(tuple)?;
                    }
                    "INTRINSIC_STOPITERATION_ERROR" => {
                        // 生成器里漏出来的 `StopIteration` 要转成 `RuntimeError`
                        // （实测原话：`generator raised StopIteration`）——用于生成器异常表那条收尾路径。
                        let value = frame.get().peek()?;
                        // SAFETY: value 在帧值栈上，存活。
                        let ty = unsafe { value.as_ref() }.ty();
                        let stop_iteration = instance
                            .type_named("StopIteration")
                            .expect("StopIteration 在异常层次里");
                        if instance.is_subtype(ty, stop_iteration) {
                            release(instance, frame.get().pop()?);
                            let exception = new_exception(
                                instance,
                                exception_type(instance, "RuntimeError"),
                                "generator raised StopIteration",
                            );
                            frame.get().push(exception)?;
                        }
                        // 不是 `StopIteration` 就原样留着（净 0）
                    }
                    "INTRINSIC_1_INVALID" => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "CALL_INTRINSIC_1 的 oparg 越界（表里没有这一号）",
                        });
                    }
                    other => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: other,
                        });
                    }
                }
            }
            "MATCH_SEQUENCE" => {
                // 净 +1：压"是不是序列"，被测对象留着。实测 `str`／`dict` **不算**序列
                let subject = frame.get().peek()?;
                // SAFETY: subject 在帧值栈上，存活。
                let ty = unsafe { subject.as_ref() }.ty();
                let truth = ty == builtin_type(instance, "list")
                    || ty == builtin_type(instance, "tuple");
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "MATCH_MAPPING" => {
                // 净 +1：本层只有 `dict` 算映射
                let subject = frame.get().peek()?;
                // SAFETY: subject 在帧值栈上，存活。
                let truth = unsafe { subject.as_ref() }.ty() == builtin_type(instance, "dict");
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "MATCH_KEYS" => {
                // 净 +1：栈是 `[被测映射, 键的 tuple]`——**两者都留着**，再压"值的 tuple"；
                // 任一键缺失就压 `None`（实测：缺键 ⇒ 这个 case 不匹配）
                let keys = frame.get().peek()?;
                let subject = frame.get().peek_from_top(2)?;
                // SAFETY: 两个都在帧值栈上，存活。
                let keys_type = unsafe { keys.as_ref() }.ty();
                if keys_type != builtin_type(instance, "tuple") {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MATCH_KEYS 的键必须是 tuple（编译器保证）",
                    });
                }
                // SAFETY: 类型身份已确认。
                let key_items = unsafe { &*keys.as_ptr().cast::<TupleObject>() };
                // SAFETY: subject 是存活对象。
                let subject_type = unsafe { subject.as_ref() }.ty();
                if subject_type != builtin_type(instance, "dict") {
                    let none = instance.singletons().none();
                    push(instance, frame.get(), none)?;
                    return Ok(Step::Continue);
                }
                // SAFETY: 类型身份已确认。
                let mapping = unsafe { &*subject.as_ptr().cast::<DictObject>() };
                let mut values: Vec<NonNull<Header>> = Vec::with_capacity(key_items.len());
                let mut missing = false;
                for index in 0..key_items.len() {
                    let key = key_items.item(index).expect("下标在范围内");
                    match mapping
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key))
                    {
                        Some(position) => {
                            let (_, value) = mapping.entry(position).expect("刚查到的位置");
                            // SAFETY: value 由字典持有，存活。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            values.push(value);
                        }
                        None => {
                            missing = true;
                            break;
                        }
                    }
                }
                if missing {
                    for value in values {
                        release(instance, value);
                    }
                    let none = instance.singletons().none();
                    push(instance, frame.get(), none)?;
                } else {
                    let tuple = instance.new_tuple(values);
                    frame.get().push(tuple)?;
                }
            }
            "MATCH_CLASS" => {
                // 净 −2：栈是 `[被测对象, 类, 关键字名 tuple]`——**被测对象也被吃掉**，
                // 命中就压"取出的属性 tuple"，不命中就压 `None`（实测形状：
                // `MATCH_CLASS n; COPY 1; POP_JUMP_IF_NONE L; UNPACK_SEQUENCE …`）。
                let names = frame.get().pop()?;
                let class_object = frame.get().pop()?;
                let subject = frame.get().pop()?;
                if oparg != 0 {
                    release(instance, names);
                    release(instance, class_object);
                    release(instance, subject);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MATCH_CLASS 只接线了关键字形参（位置形参随后补）",
                    });
                }
                // SAFETY: class_object 是刚出栈的存活对象。
                let class_type = unsafe { class_object.as_ref() }.ty();
                if class_type != builtin_type(instance, "type") {
                    release(instance, names);
                    release(instance, class_object);
                    release(instance, subject);
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        "called match pattern must be a type",
                    ));
                }
                let class = class_object.cast::<TypeObject>();
                // SAFETY: subject 是存活对象。
                let subject_type = unsafe { subject.as_ref() }.ty();
                let matched = instance.is_subtype(subject_type, class);
                release(instance, class_object);
                if !matched {
                    release(instance, names);
                    release(instance, subject);
                    let none = instance.singletons().none();
                    push(instance, frame.get(), none)?;
                    return Ok(Step::Continue);
                }
                // SAFETY: names 是 tuple（编译器保证）。
                let name_items = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                let mut values: Vec<NonNull<Header>> = Vec::with_capacity(name_items.len());
                for index in 0..name_items.len() {
                    let name = name_items.item(index).expect("下标在范围内");
                    // SAFETY: name 由 tuple 持有，存活。
                    let name_type = unsafe { name.as_ref() }.ty();
                    if name_type != instance.singletons().str_type() {
                        for value in values {
                            release(instance, value);
                        }
                        release(instance, names);
                        release(instance, subject);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MATCH_CLASS 的关键字名必须是 str",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let text = unsafe { &*name.as_ptr().cast::<StrObject>() }.value().to_owned();
                    match attribute_lookup(instance, subject, &text) {
                        Ok(Attribute::Owned(raw)) => values.push(raw),
                        Ok(Attribute::Value(raw)) => {
                            // SAFETY: raw 由类型／实例字典持有，存活。
                            unsafe { instance.incref_object(raw.as_ptr()) };
                            values.push(raw);
                        }
                        Ok(Attribute::Method { .. }) => {
                            // 取到的是**方法**（函数 ＋ self 绑定），本层还没有"绑定方法"对象
                            for value in values {
                                release(instance, value);
                            }
                            release(instance, names);
                            release(instance, subject);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "MATCH_CLASS 的属性是方法时要「绑定方法」对象（随后补）",
                            });
                        }
                        Err(error) => {
                            release(instance, names);
                            release(instance, subject);
                            return Err(error);
                        }
                    }
                }
                release(instance, names);
                release(instance, subject);
                let tuple = instance.new_tuple(values);
                frame.get().push(tuple)?;
            }
            "STORE_FAST_STORE_FAST" => {
                // 净 −2：`oparg` 打包两个局部槽——**高 4 位收 TOS**、低 4 位收 TOS1
                // （实测 `STORE_FAST_STORE_FAST 18 (a, b)` 里 a 是 1、b 是 2，而解包把**第一个**元素压在栈顶）
                let first = frame.get().pop()?;
                let second = frame.get().pop()?;
                let low = oparg & 0x0F;
                let high = oparg >> 4;
                if frame.get().set_local(high, Some(first)).is_err()
                    || frame.get().set_local(low, Some(second)).is_err()
                {
                    release(instance, first);
                    release(instance, second);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "STORE_FAST_STORE_FAST 的槽位越界",
                    });
                }
            }
            "LOAD_SMALL_INT" => {
                // 3.14 的新指令：直接把 oparg 当小整数压栈（不走常量表）。实测效果 +1。
                push_int(instance, frame.get(), oparg as i64)?;
            }
            "FORMAT_SIMPLE" => {
                // 净 0：TOS 换成它的 `str()`（3.14 把旧的 `FORMAT_VALUE` 拆成了三条）
                let value = frame.get().pop()?;
                // **直接走 `object_str`** ✓（第 210 轮修正 ✗）：它自己就是"**槽位优先** ＋ 覆写通道 ＋
                // 兜底" ✓ —— 与参照的默认 `object.__format__`（= `str(self)` ✓）同序 ✓。
                // 先前这里"**先查 `__str__` 覆写**" ✗ ⇒ 一旦 `object.__str__` 存在（本轮起 ✓），
                // 每个 MRO 都会命中它 ⇒ 把 `str`／`int` 自带的 `str` 槽带跑 ✗（实测 `f"{x}"` 给 `'1'` ✗）。
                let text = instance.object_str(value)?;
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "CONVERT_VALUE" => {
                // 净 0：`!s`／`!r`／`!a`（实测 oparg 1／2／3）
                let value = frame.get().pop()?;
                // `!s`／`!r`／`!a`（实测 oparg 1／2／3），都走 `OM-11` 的槽位
                let text = match oparg {
                    // **同 `FORMAT_SIMPLE`** ✓（第 210 轮修正 ✗）。
                    1 => instance.object_str(value)?,
                    2 => match dunder_text(instance, value, "__repr__", opcode_number)? {
                        Some(text) => text,
                        None => instance.object_repr(value)?,
                    },
                    3 => {
                        let base = match dunder_text(instance, value, "__repr__", opcode_number)? {
                            Some(text) => text,
                            None => instance.object_repr(value)?,
                        };
                        escape_non_ascii(&base)
                    }
                    _ => {
                        release(instance, value);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "CONVERT_VALUE 的 oparg 只能是 1／2／3",
                        });
                    }
                };
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "FORMAT_WITH_SPEC" => {
                // 实测净 −1：栈是 `[值, 规格]`（规格在 TOS）。
                // 路线：① 类型字典里的 `__format__`（Python 级覆写优先）② 类型的 `format` 槽
                // ③ 都不认 ⇒ 报错（消息**实测**：`unsupported format string passed to X.__format__`）。
                let spec_object = frame.get().pop()?;
                // SAFETY: spec_object 是刚出栈的存活对象。
                let spec_type = unsafe { spec_object.as_ref() }.ty();
                if spec_type != instance.singletons().str_type() {
                    release(instance, spec_object);
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        "format spec must be a str",
                    ));
                }
                // SAFETY: 类型身份已确认。
                let spec_text =
                    unsafe { &*spec_object.as_ptr().cast::<StrObject>() }.value().to_owned();
                release(instance, spec_object);

                let value = frame.get().pop()?;
                // SAFETY: value 是刚出栈的存活对象。
                let value_type = unsafe { value.as_ref() }.ty();
                let class_name = {
                    // SAFETY: 类型名由注册表持有。
                    unsafe { value_type.as_ref() }.name().to_owned()
                };
                // ① **属性通道**（`TS-44`）：类型字典里的 `__format__`，函数与原生可调用对象一视同仁
                // （`OM-11` 的 `getattr` 槽在查到函数时给"函数 ＋ self"，其余给值）。
                match attribute_lookup(instance, value, "__format__") {
                    Ok(Attribute::Method { function, this }) => {
                        let mut args: Vec<NonNull<Header>> = Vec::with_capacity(1);
                        args.push(instance.new_str(&spec_text));
                        let result = call_callable(
                            instance,
                            function,
                            Some(this),
                            args,
                            Vec::new(),
                            opcode_number,
                        )?;
                        release(instance, value);
                        frame.get().push(result)?;
                        return Ok(Step::Continue);
                    }
                    Ok(Attribute::Value(method)) | Ok(Attribute::Owned(method)) => {
                        // 原生可调用对象：self 经 `bound_self` 递进去
                        let mut args: Vec<NonNull<Header>> = Vec::with_capacity(1);
                        args.push(instance.new_str(&spec_text));
                        let result = call_callable(
                            instance,
                            method,
                            Some(value),
                            args,
                            Vec::new(),
                            opcode_number,
                        )?;
                        release(instance, value);
                        frame.get().push(result)?;
                        return Ok(Step::Continue);
                    }
                    Err(_) => {}
                }
                // ② 属性通道查不到 `__format__`：`TS-44` 说语义**只走属性通道**
                // （槽位是"没有 Python 级 dunder 时的原生默认实现"；`object` 那一层给默认，
                // 于是正常对象总能查到）。走到这里说明类型的 MRO 不完整 ⇒ 如实报错。
                let message = format!("unsupported format string passed to {class_name}.__format__");
                return Err(raise_builtin(instance, "TypeError", &message));
            }
            "GET_LEN" => {
                // 实测：+1（不弹原对象）
                let raw = frame.get().peek()?;
                let length = iterable_length(instance, raw, opcode_number)?;
                push_int(instance, frame.get(), length as i64)?;
            }
            "SWAP" => {
                // 参照实现：SWAP(i) 交换 TOS 与 TOS[-i]（净 0）
                frame.get().swap_from_top(oparg)?;
            }
            "COPY" => {
                // 参照实现：COPY(i) 把 TOS[-i] 复制一份压栈（+1）
                let raw = frame.get().peek_from_top(oparg)?;
                push(instance, frame.get(), raw)?;
            }
            "PUSH_EXC_INFO" => {
                // 实测骨架：处理块入口第一条就是它；栈效果 `(new_exc -- prev_exc, new_exc)`
                let exception = frame.get().pop()?;
                let previous = instance.current_exception();
                let previous_owned = match previous {
                    Some(value) => {
                        // SAFETY: value 由实例的异常状态持有，存活。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        value
                    }
                    None => {
                        let none = instance.singletons().none();
                        // SAFETY: 单例由实例持有。
                        unsafe { instance.incref_object(none.as_ptr()) };
                        none
                    }
                };
                // 状态接管这份引用（当前的"正在处理的异常"）
                instance.push_exception(exception);
                frame.get().push(previous_owned)?;
                push(instance, frame.get(), exception)?;
            }
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
            "LOAD_BUILD_CLASS" => {
                // 实测：`LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <类体>; MAKE_FUNCTION; …`
                let Some(build_class) = instance.build_class() else {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "本实例没有 __build_class__（引导期未建？）",
                    });
                };
                push(instance, frame.get(), build_class)?;
            }
            "LOAD_NAME" => {
                // 参照顺序：**局部（命名空间）→ 全局 → 内建**（`BC-57` 的注）。
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "LOAD_NAME 需要命名空间帧（模块／类体）",
                })?;
                match lookup_in_mapping(instance, namespace, &name) {
                    Some(value) => push(instance, frame.get(), value)?,
                    None => {
                        // 第二层：全局（类体帧的全局在 `PUSH_EXC_INFO` 之外的另一格上）
                        let globals = frame.get().globals();
                        let found = globals.and_then(|mapping| lookup_in_mapping(instance, mapping, &name));
                        match found.or_else(|| instance.builtins().and_then(|builtins| lookup_in_mapping(instance, builtins, &name))) {
                            Some(value) => push(instance, frame.get(), value)?,
                            None => {
                                // 实测消息：`name 'Base' is not defined`（参照实现还会附"Did you mean"
                                // 建议，那属于建议机制，已在差异清单 `DIV-6` 里登记）
                                let message = format!("name '{name}' is not defined");
                                return Err(raise_builtin(instance, "NameError", &message));
                            }
                        }
                    }
                }
            }
            "LOAD_GLOBAL" => {
                // `BC-57`：`LOAD_GLOBAL` 像 `LOAD_ATTR` 一样移位（**名字下标 ＝ `oparg >> 1`**），
                // 低位是"调用前先压 `NULL`"（实测：`dis` 的 argrepr 显示 `+ NULL`）。
                let name = code
                    .name_at(oparg >> 1)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                // **低位那把 `NULL` 要压在"值之后"** ✓（第 156 轮抓到的真 bug ✓）：`CALL` 期望
                //   `[可调用, NULL]`（NULL 在**上** ✓ —— 模块级的 `LOAD_NAME; PUSH_NULL` 就是这个形状，
                //   一直能跑 ✓）；先前这里把 NULL 压在**值之前** ✗ ⇒ `CALL` 把 NULL 当可调用 ⇒
                //   `TypeError: 'NULL' object is not callable` ✓（实测：**函数里调用任何内建都崩** ✗，
                //   而 `return 5` 之类的非调用语句都好 ✓ ⇒ 夹具只比编译 ✗、语料又没覆盖 ⇒ 一直没暴露 ✗）。
                // 顺序：**全局 → 内建**（`LOAD_GLOBAL` 不看局部）
                let found = frame
                    .get()
                    .effective_globals()
                    .and_then(|mapping| lookup_in_mapping(instance, mapping, &name))
                    .or_else(|| {
                        instance
                            .builtins()
                            .and_then(|builtins| lookup_in_mapping(instance, builtins, &name))
                    });
                match found {
                    Some(value) => {
                        push(instance, frame.get(), value)?;
                        if oparg & 1 != 0 {
                            let null = instance.singletons().null();
                            push(instance, frame.get(), null)?;
                        }
                    }
                    None => {
                        let message = format!("name '{name}' is not defined");
                        return Err(raise_builtin(instance, "NameError", &message));
                    }
                }
            }
            "STORE_GLOBAL" => {
                // `BC-57`：这两条**不移位**（名字下标就是 `oparg` 本身）
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let mapping = frame.get().effective_globals().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "STORE_GLOBAL 需要全局映射（函数记着定义处的全局）",
                })?;
                let value = frame.get().pop()?;
                // SAFETY: mapping 由帧或函数持有，存活。
                let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                let position = dict
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                if let Some(position) = position {
                    if let Some((old_key, old_value)) = dict.remove(position) {
                        release(instance, old_key);
                        release(instance, old_value);
                    }
                }
                let key = instance.new_str(&name);
                dict.insert_raw(key, value);
            }
            "DELETE_GLOBAL" => {
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let mapping = frame.get().effective_globals().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "DELETE_GLOBAL 需要全局映射",
                })?;
                // SAFETY: mapping 由帧或函数持有，存活。
                let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                let position = dict
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                match position {
                    Some(position) => {
                        if let Some((old_key, old_value)) = dict.remove(position) {
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                    }
                    None => {
                        let message = format!("name '{name}' is not defined");
                        return Err(raise_builtin(instance, "NameError", &message));
                    }
                }
            }
            // **`CACHE`（编号 0）**（第 280 轮 ✓）：参照里它是**无操作填充** ✓（`opcode_metadata.rs`
            // 的 `("CACHE", 0)` ✓ —— 编译器用它做自适应特化的占位 ✓）；我们**未接线** ✗ ⇒ 一旦执行到
            // 就硬报 `指令 0 尚未接线` ✗（第 273 轮实测：装上类体映射协议后，`e_plain.py` 就卡在这条 ✗）。
            // 按参照语义**跳过** ✓。**证据**：该指令在册 ✓、参照语义为无操作 ✓、跳过后闸门保持全绿 ✓
            //（端到端效果要等 enum 那条链的映射协议一起装上才会显形 ✓ —— 本节如实标注 ✓）。
            "CACHE" => {}
            "STORE_NAME" => {
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let namespace = frame.get().namespace().ok_or_else(|| ExecError::Unsupported {
                    opcode: opcode_number,
                    // **把"要存的名字"报出来**（第 339 轮）：先前只有一句"需要命名空间帧" ✗ ⇒
                    // 上限榜上那一族（76 个模块 ✓）完全看不出**是哪个构造**把它带进来的 ✓。
                    // **再把"是哪一段源码"报出来**（第 341 轮）：光有名字还不够 ✓ —— 实测
                    // `functools` 这条报的名字是 `_dict`，而上游源码里**根本没有**这个标识符 ✗
                    // ⇒ 必须指出**位点**才能定位（`co_positions()` 的
                    // `(行起, 行止, 列起, 列止)` ✓）。
                    what: Box::leak({
                        let frame_ref = frame.get();
                        // `code` 就是本 arm 开头那一份（`co_names` 也是从它取的 ✓）。
                        let position = code
                            .positions()
                            .get(frame_ref.instruction_pointer().saturating_sub(1))
                            .copied();
                        format!(
                            "STORE_NAME 需要命名空间帧（模块／类体）；名字 `{name}`；位点 {position:?}；\
                             所在代码对象 `{}`",
                            code.qualname()
                        )
                        .into_boxed_str()
                    }),
                })?;
                let value = frame.get().pop()?;
                // SAFETY: namespace 由帧持有，存活。
                // **映射协议**（第 506 轮真 bug 修 ✗）：命名空间的类型若**自带** `__setitem__`
                //（不是从 `dict` 继承的那份 ✓，典型 `Lib/enum.py` 的 `_EnumDict` ✓），
                // 存名就必须走它 ✓ —— 参照里类体／模块存名是 `PyObject_SetItem` ✓。
                // 先前无条件 `insert_raw` 直接改 `DictObject` 载荷 ✗ ⇒ 覆盖版 `__setitem__` 永不触发 ✗
                // ⇒ `enum` 收不到成员（`TypeError: 'NoneType' object is not iterable` ✓）⇒ 整包 `unittest` 进不来 ✓。
                // 依据：`NEXT.md`（R2 闸门）＋ `PYAWA_NS_DEBUG` 探针（类体赋值从不打 `setitem`）。
                let ns_type = unsafe { namespace.as_ref() }.ty();
                let dict_setitem = instance.type_lookup(builtin_type(instance, "dict"), "__setitem__");
                let overrides_setitem = instance
                    .type_lookup(ns_type, "__setitem__")
                    .is_some_and(|found| Some(found) != dict_setitem);
                if overrides_setitem {
                    let key = instance.new_str(&name);
                    // `call_dunder_method` 内部会给实参**新增**一份 ✓ ⇒ 这两份由我们自己交还 ✓。
                    let outcome = crate::executor::call::call_dunder_method(
                        instance,
                        namespace,
                        "__setitem__",
                        &[key, value],
                    );
                    release(instance, key);
                    release(instance, value);
                    outcome?;
                } else {
                    let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
                    let position = mapping
                        .entries()
                        .iter()
                        .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                    if let Some(position) = position {
                        if let Some((old_key, old_value)) = mapping.remove(position) {
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                    }
                    let key = instance.new_str(&name);
                    mapping.insert_raw(key, value);
                }
            }
            "DELETE_NAME" => {
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "DELETE_NAME 需要命名空间帧（模块／类体）",
                })?;
                // SAFETY: namespace 由帧持有，存活。
                let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
                let position = mapping
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                match position {
                    Some(position) => {
                        if let Some((old_key, old_value)) = mapping.remove(position) {
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                    }
                    None => {
                        let message = format!("name '{name}' is not defined");
                        return Err(raise_builtin(instance, "NameError", &message));
                    }
                }
            }
            "CONTAINS_OP" => {
                // 实测：`x in c` 的栈是 `[x, c]`（容器在 TOS）；`oparg` 0 ＝ `in`、1 ＝ `not in`
                // （`dis` 的 argrepr 就是这两个词）。
                let container = frame.get().pop()?;
                let item = frame.get().pop()?;
                let found = contains(instance, container, item, opcode_number)?;
                let truth = if oparg & 1 != 0 { !found } else { found };
                release(instance, container);
                release(instance, item);
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "LOAD_SPECIAL" => {
                // **`with` 协议的第一步**（3.14 的发射骨架实测）：
                //   `LOAD_FAST_BORROW ctx; COPY; LOAD_SPECIAL __exit__; SWAP 2; SWAP 3;
                //    LOAD_SPECIAL __enter__; CALL 0; …`
                // 净栈效应 **+1**：**弹出对象、压入 (可调用, self)**，`self` 在 TOS
                // ——`CALL` 一贯的栈形状是 `[可调用, NULL|self, 实参…]`（`PUSH_NULL` 排在可调用
                // **之后**），所以这里必须"可调用在下、self 在上"，紧随其后的 `CALL` 才取得对
                // （`__exit__` 那一份留在栈上，给正常出口与异常出口各用一次）。
                let name = crate::opcode::get_special_method_names()
                    .get(oparg as usize)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LOAD_SPECIAL 的下标不在特殊方法表里",
                    })?;
                let object = frame.get().pop()?;
                match attribute_lookup(instance, object, name) {
                    Ok(Attribute::Method { function, this }) => {
                        // 两者都是**借用**：压栈会各自 incref
                        push(instance, frame.get(), function)?;
                        push(instance, frame.get(), this)?;
                        release(instance, object);
                    }
                    Ok(Attribute::Value(value)) => {
                        push(instance, frame.get(), value)?;
                        push(instance, frame.get(), object)?;
                        release(instance, object);
                    }
                    Ok(Attribute::Owned(value)) => {
                        push(instance, frame.get(), value)?;
                        push(instance, frame.get(), object)?;
                        release(instance, object);
                        release(instance, value);
                    }
                    Err(error) => {
                        release(instance, object);
                        return Err(error);
                    }
                }
            }
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

                        // SAFETY: exception 是刚拿到的新引用，存活。
                        let object = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };

                        // 隐式上下文 ＝ 当前正在处理的异常（参照实现始终设，展示与否看抑制位）
                        if let Some(current) = instance.current_exception() {
                            // SAFETY: current 由实例的异常状态持有。
                            unsafe { instance.incref_object(current.as_ptr()) };
                            if let Some(old) = object.set_context(Some(current)) {
                                release(instance, old);
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
                                if let Some(old) = object.set_cause(Some(cause)) {
                                    release(instance, old);
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
                                if let Some(old) = object.set_cause(Some(created)) {
                                    release(instance, old);
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
                            object.set_suppress_context(true);
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
            "PUSH_NULL" => {
                // CALL 的"没有 self"槽位（参照实现在栈上放 NULL 指针，这里放内部哨兵）
                push(instance, frame.get(), instance.singletons().null())?;
            }
            "MAKE_FUNCTION" => {
                // 实测：MAKE_FUNCTION **只**吃 code 对象；默认值随后由 SET_FUNCTION_ATTRIBUTE 挂
                let code_header = frame.get().pop()?;
                // SAFETY: code_header 是本实例的存活对象（由常量表持有）。
                let code_type_ok = unsafe { code_header.as_ref() }.ty();
                let code_object_type = instance
                    .type_named("CodeObject")
                    .map(|ty| ty)
                    .unwrap_or_else(|| builtin_type(instance, "object"));
                if code_type_ok != code_object_type {
                    release(instance, code_header);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAKE_FUNCTION 只接受 code object（闭包与注解随后补）",
                    });
                }
                // **`__globals__`**：函数"记住"定义处的全局映射（`BC-57` 的 `LOAD_GLOBAL` 要它）。
                // 模块体没有单独的一层 ⇒ 取命名空间（`effective_globals`）。
                let captured = frame.get().effective_globals();
                if let Some(mapping) = captured {
                    // SAFETY: 映射由帧持有，函数要自己那份。
                    unsafe { instance.incref_object(mapping.as_ptr()) };
                }
                let object = instance.alloc(FunctionObject::new(
                    builtin_type(instance, "function"),
                    code_header,
                    Vec::new(),
                    None,
                    RefCell::new(captured),
                    RefCell::new(Vec::new()),
                    RefCell::new(None),
            core::cell::RefCell::new(None)));
                frame.get().push(object.into_raw().cast::<Header>())?;
            }
            "SET_FUNCTION_ATTRIBUTE" => {
                // 实测：栈是 [属性值, 函数]，**函数在 TOS**；挂完把函数留在栈上
                let function = frame.get().pop()?;
                let attribute = frame.get().pop()?;
                // SAFETY: function 是帧值栈上的存活对象。
                if unsafe { function.as_ref() }.ty() != builtin_type(instance, "function") {
                    release(instance, attribute);
                    release(instance, function);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "SET_FUNCTION_ATTRIBUTE 只接线了函数对象",
                    });
                }
                // SAFETY: 类型身份已确认。
                let object = unsafe { &mut *function.as_ptr().cast::<FunctionObject>() };
                match oparg {
                    1 => {
                        // defaults：一个 tuple（实测）
                        let items = sequence_items(instance, attribute, opcode_number);
                        release(instance, attribute);
                        object.set_defaults(items?);
                    }
                    2 => {
                        // kwdefaults：一个 dict（实测）
                        if unsafe { attribute.as_ref() }.ty() != builtin_type(instance, "dict") {
                            release(instance, attribute);
                            release(instance, function);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "kwdefaults 必须是 dict",
                            });
                        }
                        if let Some(old) = object.set_kwdefaults(Some(attribute)) {
                            release(instance, old);
                        }
                    }
                    16 => {
                        // **bit4 `annotate`**（3.14 的延迟注解协议，`SPEC-bytecode.md` 的属性位表）：
                        // 值是那个"按 `format` 产出注解字典"的**可调用对象**，挂在函数上
                        if let Some(old) = object.set_annotate(Some(attribute)) {
                            release(instance, old);
                        }
                    }
                    8 => {
                        // **bit3 `closure`**：值是 **cell 元组**（`BUILD_TUPLE n` 造的），
                        // 建帧时装进自由槽（`SPEC-bytecode.md` 的属性位表）
                        let items = sequence_items(instance, attribute, opcode_number);
                        release(instance, attribute);
                        match items {
                            Ok(items) => {
                                for value in object.set_closure(items) {
                                    release(instance, value);
                                }
                            }
                            Err(error) => {
                                release(instance, function);
                                return Err(error);
                            }
                        }
                    }
                    _ => {
                        release(instance, attribute);
                        release(instance, function);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "SET_FUNCTION_ATTRIBUTE 只接线了 defaults(1)／kwdefaults(2)／closure(8)／annotate(16)",
                        });
                    }
                }
                frame.get().push(function)?;
            }
            "IMPORT_NAME" => {
                // 栈：`[level, fromlist]`（编译器先压两项 ✓）；`level > 0` 是**相对导入**（第 278 轮 ✓）。
                let fromlist = frame.get().pop()?;
                let level = frame.get().pop()?;
                let level_value = instance.int_value(level);
                release(instance, level);
                // `fromlist` **留到装载之后再放** ✓（第 279 轮：`from . import 子模块` 那一步要用它 ✓）
                let raw = code
                    .name_at(oparg as usize)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                // **相对导入** ✓（第 278 轮接线）：`level > 0` 时按当前模块的**包上下文**把名字解析成
                // **绝对名** ✓ ⇒ 之后**只走下面这一条路** ✓（不复制第二套查找逻辑 ✓）。
                let full = if level_value.unwrap_or(0) > 0 {
                    resolve_relative_import(
                        instance,
                        frame.get(),
                        &raw,
                        level_value.unwrap_or(0) as usize,
                        opcode_number,
                    )?
                } else {
                    raw
                };
                // `import a.b.c` 交出的是**顶层模块**（随后 `STORE_NAME a` ✓，照参照实测）
                let top = full.split('.').next().unwrap_or(full.as_str()).to_owned();
                let modules = instance.modules().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "模块表未装配（import 的加载器未接：`P3-12`）",
                })?;
                // **`sys.modules` 里已有"整条带点名字"⇒ 直接用它** ✓（第 203 轮真 bug 修复 ✗）：
                // `Lib/os.py:103` 正是 `sys.modules['os.path'] = path` ✓、104 再 `from os.path import …` ✓
                // ⇒ 先前**先**去加载顶层 `os` ✗ —— 而 `os` 是**模块**不是包 ✗ ⇒ 报 `No module named 'os'`-ish ✗
                //（实测最小复现：`sys.modules['demo.sub'] = itertools` 之后 `from demo.sub import count` ✗，
                //  参照成功 ✓）。⇒ 与 CPython 同序 ✓：**先查模块表** ✓。
                let module = match instance.dict_get(modules, &full) {
                    Some(found) => found,
                    None => {
                        let loaded = match instance.dict_get(modules, &top) {
                            Some(found) => found,
                            // **加载器**（`IM-` 最小面）：按 `sys.path` 经 `fs` 域读 `<dir>/<名字>.py` ✓
                            None => load_module(instance, modules, &top, opcode_number)?,
                        };
                        // **带点名字要把整条链都导入**（第 135 轮）：`import a.b` 之后 `a.b` 必须可见 ✓
                        // （参照语义 ✓；`load_module` 会在父包的 `__path__` 里找子模块并挂成属性 ✓）。
                        // 顶层仍然交出（随后 `STORE_NAME a` ✓，与参照实测一致 ✓）。
                        if full != top && instance.dict_get(modules, &full).is_none() {
                            load_module(instance, modules, &full, opcode_number)?;
                        }
                        // **交出去的是叶子还是顶层，取决于 `fromlist`** ✓（第 513 轮真 bug 修 ✗）：
                        // * `from .helper import thing`（fromlist **非空** ✓）⇒ 必须交**叶子**
                        //   （`pkg.sub.helper` ✓）；先前交顶层 ✗ ⇒ 实测报
                        //   `cannot import name 'thing' from 'pkg'` ✗（参照成功 ✓）；
                        // * `import a.b`（fromlist **空** ✓）⇒ 必须交**顶层** ✓（随后 `STORE_NAME a` ✓；
                        //   这是参照 `__import__` 的语义 ✓）—— 对拍用例 `package_import` 正是这一格 ✗。
                        let wants_leaf = full != top
                            && instance.type_name(instance.type_of(fromlist)) == "tuple"
                            && instance.tuple_items(fromlist).is_some_and(|items| !items.is_empty());
                        if wants_leaf {
                            instance.dict_get(modules, &full).unwrap_or(loaded)
                        } else {
                            loaded
                        }
                    }
                };
                // **`fromlist`：把"名字"当子模块载入** ✓（第 279 轮；参照的 `_handle_fromlist` ✓）
                // —— `from . import _bootstrap` 就是靠这一步让 `importlib` 一族 4 个文件过线的 ✓。
                // 模块名取**模块自己的 `__name__`** ✓（照参照 ✓；`sys.modules['os.path'] = posixpath`
                // 那一类里，`full` 与模块真名**可以不同** ✗）。
                let own_name =
                    module_text(instance, module, "__name__").unwrap_or_else(|| full.clone());
                let handled = handle_fromlist(
                    instance,
                    modules,
                    module,
                    fromlist,
                    &own_name,
                    opcode_number,
                );
                release(instance, fromlist);
                handled?;
                // 交出一份**新引用**（`dict_get` 是借出 ✓）
                // SAFETY: module 由模块表持有，活到实例销毁。
                unsafe { instance.incref_object(module.as_ptr()) };
                frame.get().push(module)?;
            }
            "IMPORT_FROM" => {
                let name = code
                    .name_at(oparg as usize)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let module = frame.get().pop()?;
                let found = attribute_lookup(instance, module, &name);
                match found {
                    Ok(Attribute::Owned(value)) => {
                        frame.get().push(module)?;
                        frame.get().push(value)?;
                    }
                    Ok(Attribute::Value(value)) => {
                        frame.get().push(module)?;
                        push(instance, frame.get(), value)?;
                    }
                    Ok(Attribute::Method { function, this }) => {
                        // 模块属性理论上不会是方法 ✗，但**不吞错**：照 `LOAD_ATTR` 的取方法形态绑 ✓
                        // SAFETY: function／this 都还活着（类型字典与调用方持有）。
                        unsafe {
                            instance.incref_object(function.as_ptr());
                            instance.incref_object(this.as_ptr());
                        }
                        let bound = instance.alloc(crate::builtin_objects::MethodObject::new(
                            instance
                                .type_named("method")
                                .expect("`method` 类型已登记"),
                            function,
                            this,
                        ));
                        frame.get().push(module)?;
                        frame.get().push(bound.into_raw().cast::<Header>())?;
                    }
                    Err(error) => {
                        // **`from M import 缺名` 要报 `ImportError`** ✓（第 288 轮）：照参照实测
                        // `cannot import name 'x' from 'm'` ✓ —— 上游 `Lib/io.py:93` 的
                        // `try: from _io import _WindowsConsoleIO / except ImportError: pass` 正是靠它 ✓；
                        // 先前直接抛 `AttributeError` ✗ ⇒ 那个 `try` **接不住** ✗ ⇒ 整个 `io` 导入失败 ✓。
                        let attribute_error = builtin_type(instance, "AttributeError");
                        let is_missing = match &error {
                            ExecError::Raised { exception } => {
                                // SAFETY: 抛出的异常由实例持有，存活。
                                unsafe { exception.as_ref() }.ty() == attribute_error
                            }
                            _ => false,
                        };
                        if is_missing {
                            // 模块名：从模块命名空间里借读 `__name__` ✓（借用 ⇒ 不还引用 ✓）
                            let module_name = crate::executor::module_text(instance, module, "__name__")
                                .unwrap_or_else(|| "?".to_owned());
                            let converted = raise_builtin(
                                instance,
                                "ImportError",
                                &format!("cannot import name '{name}' from '{module_name}'"),
                            );
                            release(instance, module);
                            return Err(converted);
                        }
                        release(instance, module);
                        return Err(error);
                    }
                }
            }
            "LOAD_ATTR" => {
                // 实测：名字下标 ＝ `oparg >> 1`，**低位 ＝ 取方法**（`dis` 的 argrepr 显示 `+ NULL|self`）
                let name = code
                    .name_at(oparg >> 1)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let method_flag = oparg & 1 != 0;
                let object = frame.get().pop()?;
                let found = attribute_lookup(instance, object, &name);
                match found {
                    Ok(Attribute::Owned(value)) => {
                        // 槽位交出的就是新引用 ⇒ 直接压栈，不再 incref
                        frame.get().push(value)?;
                        if method_flag {
                            push(instance, frame.get(), instance.singletons().null())?;
                        }
                    }
                    Ok(Attribute::Value(value)) => {
                        push(instance, frame.get(), value)?;
                        if method_flag {
                            // 取方法形态对非方法值也要补一个 NULL 槽，好让 CALL 统一处理
                            push(instance, frame.get(), instance.singletons().null())?;
                        }
                    }
                    Ok(Attribute::Method { function, this }) => {
                        if method_flag {
                            // 编译器的取方法位：栈上给"函数 ＋ self"，CALL 直接按 [可调用, self] 处理
                            push(instance, frame.get(), function)?;
                            push(instance, frame.get(), this)?;
                        } else {
                            // `obj.method`（**不调用**）：产出一个**绑定方法对象**
                            // SAFETY: function／this 都还活着（由类型字典与调用方持有）。
                            unsafe {
                                instance.incref_object(function.as_ptr());
                                instance.incref_object(this.as_ptr());
                            }
                            let bound = instance.alloc(MethodObject::new(
                                builtin_type(instance, "method"),
                                function,
                                this,
                            ));
                            frame.get().push(bound.into_raw().cast::<Header>())?;
                        }
                    }
                    Err(error) => {
                        release(instance, object);
                        return Err(error);
                    }
                }
                release(instance, object);
            }
            "STORE_ATTR" => {
                // `BC-57`：**只有** `LOAD_GLOBAL`／`LOAD_ATTR`／`LOAD_SUPER_ATTR` 移位——
                // `STORE_ATTR` 的名字下标就是 `oparg` 本身（实测下标 4 的 `obj.epsilon` 给 4）。
                // 栈是 `[值, 对象]`（**对象在 TOS**）
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                // 弹之前先记深度 ✓（用来看"哪条语句留了东西"✗）。
                let depth_before = frame.get().stack.borrow().len();
                // **弹之前的整栈转储** ✓（第 302 轮；独立成块、不动上面那两行 ✓）：看"要弹的两项" ✓。
                if crate::diag::flag("PYAWA_STORE_ATTR_DEBUG") {
                    let items: Vec<String> = frame
                        .get()
                        .stack
                        .borrow()
                        .iter()
                        .map(|item| {
                            // SAFETY: 栈上每项都由本帧持有，存活。
                            unsafe { (&*item.as_ptr()).ty().as_ref() }.name().to_owned()
                        })
                        .collect();
                    eprintln!("[store_attr_stack] depth={} items=[{}]", depth_before, items.join(","));
                }
                let object = frame.get().pop()?;
                let value = frame.get().pop()?;
                // **`STORE_ATTR` 的操作数诊断** ✓（第 294 轮，门控 `PYAWA_STORE_ATTR_DEBUG=1`）：
                // 打印名字与弹出值的**类型名** ✓ —— 用来定位"接收者变成 None/str"的那类问题 ✓（零开销 ✓）。
                if crate::diag::flag("PYAWA_STORE_ATTR_DEBUG") {
                    // SAFETY: 两个值由本帧值栈持有，存活。
                    let oname = unsafe { (&*object.as_ptr()).ty().as_ref() }.name().to_owned();
                    let vname = unsafe { (&*value.as_ptr()).ty().as_ref() }.name().to_owned();
                    // 顺带把**整个值栈**摊开 ✓（第 300 轮：看"栈上到底堆着什么"✗）。
                    let dump: Vec<String> = frame
                        .get()
                        .stack
                        .borrow()
                        .iter()
                        .map(|item| {
                            // SAFETY: 栈上每项都由本帧持有，存活。
                            unsafe { (&*item.as_ptr()).ty().as_ref() }.name().to_owned()
                        })
                        .collect();
                    eprintln!(
                        "[store_attr] name={name} object_type={oname} value_type={vname} depth_before={depth_before} stack=[{}]",
                        dump.join(",")
                    );
                }
                let outcome =
                    instance_attribute_set(instance, object, &name, value, opcode_number);
                release(instance, object);
                outcome?;
            }
            "DELETE_ATTR" => {
                // 实测：名字下标 ＝ `oparg`（**不移位**）；栈是 `[对象]`
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let object = frame.get().pop()?;
                let outcome = instance_attribute_delete(instance, object, &name);
                release(instance, object);
                outcome?;
            }
            "CALL" | "CALL_KW" => {
                // 实测：`[可调用, NULL|self, 位置实参…]`；`CALL_KW` 另把**关键字名元组**放在 TOS
                let names = if name == "CALL_KW" {
                    Some(frame.get().pop()?)
                } else {
                    None
                };
                let keyword_count = match names {
                    Some(names) => {
                        // SAFETY: names 是帧值栈上的存活对象。
                        if unsafe { names.as_ref() }.ty() != builtin_type(instance, "tuple") {
                            release(instance, names);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "CALL_KW 的关键字名表必须是 tuple",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        unsafe { &*names.as_ptr().cast::<TupleObject>() }.len()
                    }
                    None => 0,
                };

                let mut keywords = Vec::with_capacity(keyword_count);
                for _ in 0..keyword_count {
                    keywords.push(frame.get().pop()?);
                }
                keywords.reverse();

                let positional_count = oparg.saturating_sub(keyword_count);
                let mut args = Vec::with_capacity(positional_count);
                for _ in 0..positional_count {
                    args.push(frame.get().pop()?);
                }
                args.reverse();

                let self_or_null = frame.get().pop()?;
                let callable = frame.get().pop()?;

                let bound_self = if self_or_null == instance.singletons().null() {
                    release(instance, self_or_null);
                    None
                } else {
                    Some(self_or_null)
                };

                let mut kwargs = Vec::with_capacity(keyword_count);
                if let Some(names) = names {
                    // SAFETY: 上面确认过它是 tuple。
                    let table = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                    for (index, value) in keywords.into_iter().enumerate() {
                        let key = match table.item(index) {
                            Some(key) => key,
                            None => {
                                release(instance, value);
                                release(instance, names);
                                release(instance, callable);
                                for value in args {
                                    release(instance, value);
                                }
                                if let Some(self_object) = bound_self {
                                    release(instance, self_object);
                                }
                                for (key, value) in kwargs {
                                    release(instance, key);
                                    release(instance, value);
                                }
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "CALL_KW 的关键字个数与名表长度不符",
                                });
                            }
                        };
                        // SAFETY: key 由元组持有，存活。
                        unsafe { instance.incref_object(key.as_ptr()) };
                        kwargs.push((key, value));
                    }
                    release(instance, names);
                }

                let result = call_callable(
                    instance,
                    callable,
                    bound_self,
                    args,
                    kwargs,
                    opcode_number,
                );
                release(instance, callable);
                frame.get().push(result?)?;
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
                // **通用比较**（`TS-40`）：`int`／`bool`／`str` 按值；其余报实测的 `TypeError`。
                // 整数路径与从前一致（同一套 `partial_cmp`），只是不再把非整数当成"未接线"。

                // `BC-39`／`BC-58`：cmp 下标 ＝ `oparg >> 5`；bit 4（`& 16`）是 `bool(...)` 标志，
                // 低 4 位是参照实现的编译期信息（`dis` 不读、本层**不解释**但**必须容受**）
                let operator = opcode::get_cmp_op().get(oparg >> 5).copied().ok_or(
                    ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "COMPARE_OP 的 cmp 下标（oparg >> 5）超出 cmp_op 的六元组（BC-39／BC-58）",
                    },
                )?;
                let truth = compare_public(instance, left, right, operator, opcode_number);
                release(instance, left);
                release(instance, right);
                let raw = instance.singletons().boolean(truth?);
                push(instance, frame.get(), raw)?;
            }
            "BINARY_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;

                // BC-39：oparg 对应 `get_nb_ops()` 的顺序；BC-50：名字从表里取，不写死
                let name = opcode::get_nb_ops()
                    .get(oparg)
                    .map(|(name, _)| *name)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BINARY_OP 的 oparg 超出 get_nb_ops() 的范围（BC-39）",
                    })?;

                if name == "NB_SUBSCR" {
                    // 下标读：`[容器, 键]`（实测）
                    let result = subscript_get(instance, left, right, opcode_number);
                    release(instance, left);
                    release(instance, right);
                    frame.get().push(result?)?;
                } else {
                    // **一处真相**：二元运算走公开入口（`P1-11` 的任意精度核心 ＋ 参照实测的消息），
                    // 不再在这里用 `i64` ＋ "结果必须落在单例区间"那套旧假设
                    let result = match name {
                        // `+` 走 concat：`str`／`bytes`／`list`／`tuple` 拼接，其余落到算术
                        "NB_ADD" => concat_public(instance, left, right, opcode_number),
                        "NB_SUBTRACT" => arithmetic_public(instance, left, right, "-", opcode_number),
                        "NB_MULTIPLY" => arithmetic_public(instance, left, right, "*", opcode_number),
                        "NB_FLOOR_DIVIDE" => {
                            arithmetic_public(instance, left, right, "//", opcode_number)
                        }
                        "NB_REMAINDER" => arithmetic_public(instance, left, right, "%", opcode_number),
                        // 真除法：结果是 **float**（`arithmetic_public` 的 `/` 分支）
                        "NB_TRUE_DIVIDE" => {
                            arithmetic_public(instance, left, right, "/", opcode_number)
                        }
                        "NB_POWER" => arithmetic_public(instance, left, right, "**", opcode_number),
                        "NB_AND" => arithmetic_public(instance, left, right, "&", opcode_number),
                        "NB_OR" if instance.is_type_object(left) && instance.is_type_object(right) =>
                            // **类型的 `|`** ✓（第 214 轮）：`int | str` ⇒ 联合类型 ✓（`Lib/types.py` 的 `UnionType` ✓）。
                            Ok(instance.new_union_type(left, right)),
                        "NB_OR" => arithmetic_public(instance, left, right, "|", opcode_number),
                        "NB_XOR" => arithmetic_public(instance, left, right, "^", opcode_number),
                        // `@`（矩阵乘）：`arithmetic_public` 对 `@` 一律如实报参照实测的 `TypeError`
                        "NB_MATRIX_MULTIPLY" => {
                            arithmetic_public(instance, left, right, "@", opcode_number)
                        }
                        "NB_LSHIFT" => arithmetic_public(instance, left, right, "<<", opcode_number),
                        "NB_RSHIFT" => arithmetic_public(instance, left, right, ">>", opcode_number),
                        // **增强赋值**（`+=` 一族，`NB_INPLACE_*`）：不可变类型（`int`／`bool`／
                        // `float`／`str`／`bytes`／`tuple`）的"就地"就是基运算 + 重新绑定 ⇒ 直接
                        // 落基运算；**可变容器**里 `list` 的 `+=` 是**就地 extend**（别名可见，
                        // 实测）⇒ 单独走；`set`／`dict` 的就地运算**尚未接线**（如实报，不悄悄
                        // 换成"重新绑定"——那会与参照的可观察行为不同）
                        "NB_INPLACE_ADD" => inplace_add(instance, left, right, opcode_number),
                        "NB_INPLACE_SUBTRACT" => {
                            inplace_arithmetic(instance, left, right, "-", opcode_number)
                        }
                        "NB_INPLACE_MULTIPLY" => {
                            inplace_arithmetic(instance, left, right, "*", opcode_number)
                        }
                        "NB_INPLACE_TRUE_DIVIDE" => {
                            inplace_arithmetic(instance, left, right, "/", opcode_number)
                        }
                        "NB_INPLACE_FLOOR_DIVIDE" => {
                            inplace_arithmetic(instance, left, right, "//", opcode_number)
                        }
                        "NB_INPLACE_REMAINDER" => {
                            inplace_arithmetic(instance, left, right, "%", opcode_number)
                        }
                        "NB_INPLACE_POWER" => {
                            inplace_arithmetic(instance, left, right, "**", opcode_number)
                        }
                        "NB_INPLACE_LSHIFT" => {
                            inplace_arithmetic(instance, left, right, "<<", opcode_number)
                        }
                        "NB_INPLACE_RSHIFT" => {
                            inplace_arithmetic(instance, left, right, ">>", opcode_number)
                        }
                        "NB_INPLACE_AND" => {
                            inplace_arithmetic(instance, left, right, "&", opcode_number)
                        }
                        "NB_INPLACE_XOR" => {
                            inplace_arithmetic(instance, left, right, "^", opcode_number)
                        }
                        "NB_INPLACE_OR" => {
                            inplace_arithmetic(instance, left, right, "|", opcode_number)
                        }
                        "NB_INPLACE_MATRIX_MULTIPLY" => {
                            inplace_arithmetic(instance, left, right, "@", opcode_number)
                        }
                        _ => Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "该 NB_* 运算尚未接线（矩阵乘 `@`／`@=` 随后补）",
                        }),
                    };
                    release(instance, left);
                    release(instance, right);
                    frame.get().push(result?)?;
                }
            }
            "STORE_SUBSCR" => {
                // 实测：`[值, 容器, 键]`，键在 TOS
                let key = frame.get().pop()?;
                let container = frame.get().pop()?;
                let value = frame.get().pop()?;
                let outcome = subscript_set(instance, container, key, value, opcode_number);
                release(instance, container);
                release(instance, key);
                outcome?;
            }
            "DELETE_SUBSCR" => {
                // 实测：`[容器, 键]`
                let key = frame.get().pop()?;
                let container = frame.get().pop()?;
                let outcome = subscript_del(instance, container, key, opcode_number);
                release(instance, container);
                release(instance, key);
                outcome?;
            }
            "RETURN_VALUE" => {
                let returned = frame.get().pop()?;
                // **返回现场诊断** ✓（第 349 轮，门控 `PYAWA_RETURN_DEBUG=1`）：只看 `_find_new_` ✓，
                // 用来分辨"没走到 return"✗ 与"调用机制把返回值丢了"✗（第 348 轮的设计 ✓）。
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
            _ => return Err(ExecError::NotImplemented { opcode: opcode_number }),
        }
        Ok(Step::Continue)
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
pub mod protocol;
