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

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::decode::{parse_exception_table, DecodeError, Decoder};
use crate::frame::{Frame, FrameError};
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::opcode;
use crate::refcount::{Owned, PyRef};
use crate::builtin_objects::{
    AttributeObject, BoolObject, ExceptionObject, GeneratorObject, IteratorObject, DictObject, FloatObject, FunctionObject, IntObject, ListObject, SetObject, StrObject,
    TupleObject,
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
    /// 指令接线了，但这个形态／类型还没接线（协议槽位、大整数、容器……）。
    Unsupported { opcode: u8, what: &'static str },
    /// 读到未绑定的局部槽（CPython 的 `UnboundLocalError` 时机）。
    UnboundLocal { slot: usize },
    /// 结果超出本层能表示的范围：需要大整数对象。
    IntOutOfRange { value: i64 },

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

/// 按名字取一个已注册的内建类型（`TS-41` 的表是层次的出处）。
fn builtin_type(instance: &Instance, name: &str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| panic!("TS-41：{name} 应当已注册"))
}

/// 整数载荷（int 与 bool 两种布局分开读，`TS-40`）。
fn integer_payload(instance: &Instance, raw: NonNull<Header>) -> Option<i64> {
    // SAFETY: 调用方保证 raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.int_type() {
        // SAFETY: 类型身份已确认。
        return Some(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 同上。
        return Some(i64::from(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value));
    }
    None
}

/// 数值载荷（`int`／`bool`／`float`）。
fn numeric_payload(instance: &Instance, raw: NonNull<Header>) -> Option<f64> {
    if let Some(value) = integer_payload(instance, raw) {
        return Some(value as f64);
    }
    // SAFETY: 调用方保证 raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty == builtin_type(instance, "float") {
        // SAFETY: 类型身份已确认。
        return Some(unsafe { &*raw.as_ptr().cast::<FloatObject>() }.value());
    }
    None
}

/// **值相等**（*临时*：只管道 `None`／`bool`／`int`／`float`／`str`）。
///
/// 参照实现的 `==` 走 `__eq__` 槽位（随类型系统接线）；本层先按载荷比，
/// 但**必须**保留 `TS-40` 的可观察后果（`True == 1`、`1 == 1.0` 为真）——
/// 否则 `{1: 'a', True: 'b'}` 这类字面量会多出一个键，属于对拍里的新差异。
fn values_equal(instance: &Instance, left: NonNull<Header>, right: NonNull<Header>) -> bool {
    if left == right {
        return true;
    }
    let (left_int, right_int) = (
        integer_payload(instance, left),
        integer_payload(instance, right),
    );
    if let (Some(a), Some(b)) = (left_int, right_int) {
        return a == b;
    }
    let (left_number, right_number) = (
        numeric_payload(instance, left),
        numeric_payload(instance, right),
    );
    if let (Some(a), Some(b)) = (left_number, right_number) {
        // *临时*：整数与浮点比时按 f64 走（超大整数与浮点混用时会有精度话题，随协议槽位收口）
        return a == b;
    }
    let str_type = instance.singletons().str_type();
    // SAFETY: 两个都是存活对象。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    if left_type == str_type && right_type == str_type {
        // SAFETY: 类型身份已确认。
        let (left_text, right_text) = unsafe {
            (
                &*left.as_ptr().cast::<StrObject>(),
                &*right.as_ptr().cast::<StrObject>(),
            )
        };
        return left_text.value() == right_text.value();
    }
    false
}

/// 取出"可解包元素"（**新引用**的列表）。
///
/// *临时*：只管道 `tuple`／`list`／`str`（其余可迭代对象随迭代器族接线）。
/// 返回的每一项都是**新引用**——调用方要么压栈、要么释放。
fn sequence_items(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<Vec<NonNull<Header>>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let owned = |value: NonNull<Header>| {
        // SAFETY: value 是容器持有的存活对象。
        unsafe { instance.incref_object(value.as_ptr()) };
        value
    };

    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<TupleObject>() }
            .items()
            .iter()
            .copied()
            .map(owned)
            .collect());
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<ListObject>() }
            .items()
            .into_iter()
            .map(owned)
            .collect());
    }
    let str_type = instance.singletons().str_type();
    if ty == str_type {
        // SAFETY: 同上。
        let text = unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned();
        // 逐字符造新的 `str` 对象（*临时*：参照实现会intern 单字符，属实现观测面）
        return Ok(text
            .chars()
            .map(|character| {
                let object = instance.alloc(StrObject::new(str_type, character.to_string()));
                object.into_raw().cast::<Header>()
            })
            .collect());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "解包只接线了 tuple／list／str（迭代器协议未接线）",
    })
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

/// 把下标归一成 0 起的位置（负数从末尾数；越界返回 `None`）。
fn normalize_index(index: i64, length: usize) -> Option<usize> {
    let normalized = if index < 0 { index + length as i64 } else { index };
    if normalized < 0 || normalized >= length as i64 {
        return None;
    }
    Some(normalized as usize)
}

/// 下标**读**（`BINARY_OP` ＋ `NB_SUBSCR`，3.14 无 `BINARY_SUBSCR`）。返回**新引用**。
fn subscript_get(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: container 与 key 都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if container_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
        })?;
        let position = match normalize_index(index, object.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "tuple index out of range")),
        };
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: value 由容器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if container_type == builtin_type(instance, "list") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
        })?;
        let position = match normalize_index(index, object.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "list index out of range")),
        };
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        // `KeyError` 的 `args` 就是那个键（实测：`KeyError('nope')`），不是一条消息
        let position = match object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
        {
            Some(position) => position,
            None => {
                // SAFETY: key 是帧值栈上的存活对象。
                unsafe { instance.incref_object(key.as_ptr()) };
                let exception = new_exception_with_args(
                    instance,
                    exception_type(instance, "KeyError"),
                    vec![key],
                );
                return Err(raise(instance, exception));
            }
        };
        let (_, value) = object.entry(position).expect("刚查到的位置");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    let str_type = instance.singletons().str_type();
    if container_type == str_type {
        // SAFETY: 同上。
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value().to_owned();
        let characters: Vec<char> = text.chars().collect();
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
        })?;
        let position = match normalize_index(index, characters.len()) {
            Some(position) => position,
            None => {
                return Err(raise_builtin(instance, "IndexError", "string index out of range"))
            }
        };
        let object = instance.alloc(StrObject::new(str_type, characters[position].to_string()));
        return Ok(object.into_raw().cast::<Header>());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标只接线了 tuple／list／dict／str",
    })
}

/// 下标**写**（`STORE_SUBSCR`；`value` 是**新引用**，无论成败都会被接手）。
fn subscript_set(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // SAFETY: 三个都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if container_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let index = match integer_payload(instance, key) {
            Some(index) => index,
            None => {
                release(instance, value);
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
                });
            }
        };
        let length = object.len();
        let position = match normalize_index(index, length) {
            Some(position) => position,
            None => {
                release(instance, value);
                return Err(raise_builtin(instance, "IndexError", "list index out of range"));
            }
        };
        if let Some(old) = object.replace(position, value) {
            release(instance, old);
        }
        return Ok(());
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key));
        match position {
            Some(slot) => {
                if let Some(old) = object.replace_value(slot, value) {
                    release(instance, old);
                }
                // 键已在表里：新键那份引用交回去
                release(instance, key);
            }
            None => object.insert_raw(key, value),
        }
        return Ok(());
    }
    release(instance, value);
    if container_type == builtin_type(instance, "tuple") {
        return Err(ExecError::Unsupported {
            opcode,
            what: "tuple 不支持下标赋值（不可变）",
        });
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标赋值只接线了 list／dict",
    })
}

/// 下标**删**（`DELETE_SUBSCR`）。
fn subscript_del(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // SAFETY: 两个都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if container_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数",
        })?;
        let length = object.len();
        let position = match normalize_index(index, length) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "list index out of range")),
        };
        if let Some(removed) = object.remove(position) {
            release(instance, removed);
        }
        return Ok(());
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = match object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
        {
            Some(position) => position,
            None => {
                // SAFETY: key 是帧值栈上的存活对象。
                unsafe { instance.incref_object(key.as_ptr()) };
                let exception = new_exception_with_args(
                    instance,
                    exception_type(instance, "KeyError"),
                    vec![key],
                );
                return Err(raise(instance, exception));
            }
        };
        if let Some((removed_key, removed_value)) = object.remove(position) {
            release(instance, removed_key);
            release(instance, removed_value);
        }
        return Ok(());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标删除只接线了 list／dict",
    })
}

/// 迭代器类型的名字（**照探测表取**；`str` 的迭代器在这台机器上叫 `str_ascii_iterator`）。
const ITERATOR_TYPE_NAMES: [&str; 5] = [
    "tuple_iterator",
    "list_iterator",
    "str_ascii_iterator",
    "dict_keyiterator",
    "set_iterator",
];

/// 一个对象是不是本层接线的迭代器。
fn is_iterator_type(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    ITERATOR_TYPE_NAMES
        .iter()
        .any(|name| instance.type_named(name) == Some(ty))
}

/// 被迭代对象的元素个数。
fn iterable_length(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<usize, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<TupleObject>() }.len());
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<ListObject>() }.len());
    }
    if ty == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<DictObject>() }.len());
    }
    if ty == builtin_type(instance, "set") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<SetObject>() }.len());
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().chars().count());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "只接线了 tuple／list／dict／set／str 的迭代（__iter__ 协议未接线）",
    })
}

/// 取被迭代对象的第 `index` 个元素（**新引用**；`str` 会造一个单字符 `str`）。
fn iterable_item(
    instance: &Instance,
    raw: NonNull<Header>,
    index: usize,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();

    let owned = |value: NonNull<Header>| {
        // SAFETY: value 由容器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        value
    };

    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<TupleObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<ListObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "dict") {
        // SAFETY: 同上。字典迭代的是**键**（与参照实现一致）
        let value = unsafe { &*raw.as_ptr().cast::<DictObject>() }
            .entry(index)
            .map(|(key, _)| key);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "set") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<SetObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 同上。
        let text = unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned();
        let character = text.chars().nth(index).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        })?;
        let object = instance.alloc(StrObject::new(
            instance.singletons().str_type(),
            character.to_string(),
        ));
        return Ok(object.into_raw().cast::<Header>());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "只接线了 tuple／list／dict／set／str 的迭代",
    })
}

/// 该对象该用哪个迭代器类型（名字照探测表）。
fn iterator_type_for(
    instance: &Instance,
    raw: NonNull<Header>,
) -> Result<NonNull<TypeObject>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let name = if ty == builtin_type(instance, "tuple") {
        "tuple_iterator"
    } else if ty == builtin_type(instance, "list") {
        "list_iterator"
    } else if ty == builtin_type(instance, "dict") {
        "dict_keyiterator"
    } else if ty == builtin_type(instance, "set") {
        "set_iterator"
    } else if ty == instance.singletons().str_type() {
        "str_ascii_iterator"
    } else {
        return Err(ExecError::Unsupported {
            opcode: opcode_of("GET_ITER"),
            what: "只接线了 tuple／list／dict／set／str 的迭代（__iter__ 协议未接线）",
        });
    };
    Ok(builtin_type(instance, name))
}

/// 属性查找的结果。
enum Attribute {
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

/// 对象的属性字典（只有带 [`crate::HAS_INSTANCE_DICT`] 的实例才有）。
fn instance_attributes(_instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    // SAFETY: object 是存活对象。
    let ty = unsafe { object.as_ref() }.ty();
    // SAFETY: ty 由注册表持有。
    if unsafe { ty.as_ref() }.type_flags() & crate::HAS_INSTANCE_DICT == 0 {
        return None;
    }
    // SAFETY: 标志位保证载荷就是 AttributeObject。
    unsafe { &*object.as_ptr().cast::<AttributeObject>() }.attributes()
}

/// 往实例的属性字典里写一项（`value` 是**新引用**，由字典接手；旧值被释放）。
fn instance_attribute_set(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    let mapping = match instance_attributes(instance, object) {
        Some(mapping) => mapping,
        None => {
            // SAFETY: 上面确认过这个类型带实例字典。
            let created = instance
                .alloc(DictObject::new(
                    builtin_type(instance, "dict"),
                    RefCell::new(Vec::new()),
                ))
                .into_raw()
                .cast::<Header>();
            // SAFETY: object 是存活对象，且标志位保证载荷就是 AttributeObject。
            let previous = unsafe { &*object.as_ptr().cast::<AttributeObject>() }
                .set_attributes(Some(created));
            if let Some(previous) = previous {
                release(instance, previous);
            }
            created
        }
    };
    // SAFETY: mapping 由对象或本函数持有。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .into_iter()
        .position(|(existing, _)| str_matches_public(instance, existing, name));
    match position {
        Some(slot) => {
            if let Some(old) = dict.replace_value(slot, value) {
                release(instance, old);
            }
        }
        None => {
            let key = instance
                .alloc(StrObject::new(instance.singletons().str_type(), name.to_owned()))
                .into_raw()
                .cast::<Header>();
            dict.insert_raw(key, value);
        }
    }
    let _ = opcode;
    Ok(())
}

/// 一个 `str` 对象的内容是否等于给定的 Rust 字符串。
fn str_matches_public(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 是存活对象。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}

/// `LOAD_ATTR` 一族的查找顺序（本层口径，写在注释里以免以后漂）：
///
/// ① 实例字典（**非数据描述符**会被它遮住：函数就是非数据描述符，故实例属性优先）
/// ② 类型字典（沿 MRO）：查到**函数**就是取方法，查到别的值就原样返回
/// ③ 都没有 ⇒ [`ExecError::AttributeNotFound`]
fn attribute_lookup(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Attribute, ExecError> {
    // ① 类型自己的 `getattr` 槽（`OM-11`）——**内建类型的属性通道**，不许旁路
    // SAFETY: object 是存活对象。
    let object_type = unsafe { object.as_ref() }.ty();
    // SAFETY: object_type 由注册表持有。
    if let Some(slot) = unsafe { object_type.as_ref() }.slots().getattr {
        // SAFETY: 槽位由类型提供，契约见 `GetAttrFn`。
        if let Some(found) = unsafe { slot(object.as_ptr(), name, instance) } {
            return Ok(Attribute::Owned(found));
        }
    }

    // ② 实例字典
    if let Some(mapping) = instance_attributes(instance, object) {
        // SAFETY: mapping 是属性字典（dict）。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let found = dict
            .entries()
            .into_iter()
            .find(|(key, _)| str_matches_public(instance, *key, name));
        if let Some((_, value)) = found {
            return Ok(Attribute::Value(value));
        }
    }

    // ③ 类型字典沿 MRO
    if let Some(found) = instance.type_lookup(object_type, name) {
        // SAFETY: found 由类型字典持有。
        if unsafe { found.as_ref() }.ty() == builtin_type(instance, "function") {
            return Ok(Attribute::Method {
                function: found,
                this: object,
            });
        }
        return Ok(Attribute::Value(found));
    }

    // 实测消息：`'int' object has no attribute 'nope'`（类型名取自对象的类型）
    // SAFETY: object 是存活对象。
    let type_name = unsafe { object.as_ref().ty().as_ref() }.name();
    Err(raise_builtin(
        instance,
        "AttributeError",
        &format!("'{type_name}' object has no attribute '{name}'"),
    ))
}

/// 删掉实例属性字典里的一项（`DELETE_ATTR`；参照实现只删实例属性，不碰类型）。
fn instance_attribute_delete(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<(), ExecError> {
    let missing = || {
        // SAFETY: object 是存活对象。
        let type_name = unsafe { object.as_ref().ty().as_ref() }.name();
        raise_builtin(
            instance,
            "AttributeError",
            &format!("'{type_name}' object has no attribute '{name}'"),
        )
    };
    let mapping = instance_attributes(instance, object).ok_or_else(missing)?;
    // SAFETY: mapping 是属性字典（dict）。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .into_iter()
        .position(|(key, _)| str_matches_public(instance, key, name))
        .ok_or_else(missing)?;
    if let Some((key, value)) = dict.remove(position) {
        release(instance, key);
        release(instance, value);
    }
    Ok(())
}

/// 取一个已注册的**异常类**（`TS-41` 的表里那棵树）。
fn exception_type(instance: &Instance, name: &str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| panic!("TS-41：异常类 {name} 应当已注册"))
}

/// 这个类型是不是异常类（MRO 里有 `BaseException`）。
fn is_exception_type(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    instance.is_subtype(ty, exception_type(instance, "BaseException"))
}

/// 造一个异常实例（`args` 只有一个 `str` 消息）——**新引用**。
fn new_exception(instance: &Instance, ty: NonNull<TypeObject>, message: &str) -> NonNull<Header> {
    let text = instance
        .alloc(StrObject::new(
            instance.singletons().str_type(),
            message.to_owned(),
        ))
        .into_raw()
        .cast::<Header>();
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(vec![text]),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
    ));
    object.into_raw().cast::<Header>()
}

/// 造一个异常实例，`args` 用给定的那批**新引用**（异常对象接手）。
fn new_exception_with_args(
    instance: &Instance,
    ty: NonNull<TypeObject>,
    args: Vec<NonNull<Header>>,
) -> NonNull<Header> {
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
    ));
    object.into_raw().cast::<Header>()
}

/// 抛一个异常：记在实例上（借它保活）并交出错误（`BC-60` ②）。
fn raise(instance: &Instance, exception: NonNull<Header>) -> ExecError {
    if let Some(previous) = instance.set_pending_exception(Some(exception)) {
        release(instance, previous);
    }
    ExecError::Raised { exception }
}

/// 抛一个内建异常（带消息）。
fn raise_builtin(instance: &Instance, name: &str, message: &str) -> ExecError {
    let exception = new_exception(instance, exception_type(instance, name), message);
    raise(instance, exception)
}

// ---- `BC-56` 的消息：**逐条实测**（禁止手写近似文本，见 tests/calls.rs 的记录）----

fn message_too_many(name: &str, accepted: usize, required: usize, given: usize) -> String {
    if required < accepted {
        format!(
            "{name}() takes from {required} to {accepted} positional arguments but {given} were given"
        )
    } else if accepted == 1 {
        format!("{name}() takes 1 positional argument but {given} were given")
    } else {
        format!("{name}() takes {accepted} positional arguments but {given} were given")
    }
}

fn message_missing(name: &str, missing: &[String], keyword_only: bool) -> String {
    let kind = if keyword_only {
        "keyword-only"
    } else {
        "positional"
    };
    if missing.len() == 1 {
        return format!(
            "{name}() missing 1 required {kind} argument: '{}'",
            missing[0]
        );
    }
    let quoted: Vec<String> = missing.iter().map(|item| format!("'{item}'")).collect();
    let head = quoted[..quoted.len() - 1].join(", ");
    let last = quoted.last().cloned().unwrap_or_default();
    // 实测：两个是 `'a' and 'b'`（无逗号），三个及以上是 `'a', 'b', and 'c'`（有逗号）
    let conjunction = if quoted.len() == 2 { " and " } else { ", and " };
    format!(
        "{name}() missing {} required {kind} arguments: {head}{conjunction}{last}",
        missing.len()
    )
}

fn message_duplicate(name: &str, argument: &str) -> String {
    format!("{name}() got multiple values for argument '{argument}'")
}

fn message_unexpected_keyword(name: &str, argument: &str) -> String {
    format!("{name}() got an unexpected keyword argument '{argument}'")
}

fn message_positional_only(name: &str, arguments: &[String]) -> String {
    format!(
        "{name}() got some positional-only arguments passed as keyword arguments: '{}'",
        arguments.join(", ")
    )
}

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
fn function_defaults(function: NonNull<Header>) -> (NonNull<Header>, Vec<NonNull<Header>>, Option<NonNull<Header>>) {
    // SAFETY: function 是帧值栈上的存活对象，且调用方已确认它是 function。
    let object = unsafe { &*function.as_ptr().cast::<FunctionObject>() };
    (object.code(), object.defaults().to_vec(), object.kwdefaults())
}

/// **`BC-56`**：把实参绑进局部槽；返回长度 ＝ `co_nlocals` 的槽数组（**新引用**）。
///
/// 顺序与报错类别都按 `BC-56`：仅位置 → 位置或关键字 → `*args` → 仅关键字 → `**kwargs`；
/// 四类错误各成一个 [`ExecError`]（参照实现的**消息**已实测记录在案，等异常对象接线后再原样产出）。
#[allow(clippy::too_many_arguments)]
fn bind_arguments(
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
    if !extra.is_empty() {
        if !code.has_varargs() {
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
        let varargs_slot = argcount + kwonly;
        let tuple = instance.alloc(TupleObject::new(builtin_type(instance, "tuple"), extra));
        locals[varargs_slot] = Some(tuple.into_raw().cast::<Header>());
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

/// 调用一个可调用对象（本片只有函数对象）。
fn call_callable(
    instance: &Instance,
    callable: NonNull<Header>,
    bound_self: Option<NonNull<Header>>,
    args: Vec<NonNull<Header>>,
    kwargs: Vec<(NonNull<Header>, NonNull<Header>)>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: callable 是帧值栈上的存活对象。
    let ty = unsafe { callable.as_ref() }.ty();
    if ty != builtin_type(instance, "function") {
        for value in args {
            release(instance, value);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        return Err(ExecError::Unsupported {
            opcode,
            what: "只接线了函数对象（内建可调用与类随后补）",
        });
    }

    let (code_header, defaults, kwdefaults) = function_defaults(callable);
    // SAFETY: 函数持有一份对 code object 的引用，故它在函数存活期间有效。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    let mut args = args;
    if let Some(self_object) = bound_self {
        args.insert(0, self_object);
    }

    let locals = bind_arguments(instance, code, args, kwargs, &defaults, kwdefaults, opcode)?;

    let frame_type = builtin_type(instance, "Frame");
    let frame = instance.alloc(Frame::for_code(frame_type, &own_code(instance, code_header)));
    for (slot, value) in locals.into_iter().enumerate() {
        if let Some(value) = value {
            let _ = frame.get().set_local(slot, Some(value))?;
        }
    }

    // **生成器函数**（`CO_GENERATOR`，实测 32）：`CALL` **不**跑函数体，而是把挂起的帧
    // 包成生成器交出去（实测骨架：函数体第一条是 `RETURN_GENERATOR`，恢复时才从 `POP_TOP` 继续）。
    if code.flags() & 0x20 != 0 {
        frame.get().suspend()?;
        // 生成器要**自己持有一份帧的引用**（`GeneratorObject` 的 traverse／clear 会释放它）——
        // 漏了这一份，`call_callable` 一返回帧就被释放，生成器拿到的是悬垂指针。
        // SAFETY: frame 由本函数持有，这里新增一份引用交给生成器。
        unsafe { instance.incref_object(frame.as_ptr().cast::<Header>().as_ptr()) };
        let generator = instance.alloc(GeneratorObject::new(
            builtin_type(instance, "generator"),
            frame.as_ptr().cast::<Header>(),
            Cell::new(false),
        ));
        return Ok(generator.into_raw().cast::<Header>());
    }

    match execute(instance, &frame)? {
        ExecOutcome::Returned(value) => Ok(value_into_raw(instance, value)),
        ExecOutcome::Yielded(_) => Err(ExecError::Unsupported {
            opcode,
            what: "非生成器函数不该让出（码元被改坏了？）",
        }),
    }
}

/// 为 code object 现取一个 [`Owned`] 守卫（**新增一份引用**）。
fn own_code<'a>(instance: &'a Instance, header: NonNull<Header>) -> Owned<'a, CodeObject> {
    // SAFETY: header 指向本实例的存活 code object；这里新增一份引用交给守卫。
    unsafe { instance.incref_object(header.as_ptr()) };
    Owned::new(header.cast::<CodeObject>(), instance)
}

/// 把 [`Value`] 变成帧值栈要的**新引用**（内联的那几种换算成它们对应的单例）。
fn value_into_raw(instance: &Instance, value: Value<'_>) -> NonNull<Header> {
    let (raw, needs_reference) = match value {
        Value::None => (instance.singletons().none(), true),
        Value::Bool(flag) => (instance.singletons().boolean(flag), true),
        Value::Int(number) => (
            instance
                .singletons()
                .small_int(number)
                .expect("内联整数一定落在单例区间（OM-39）"),
            true,
        ),
        Value::Object(reference) => (reference.into_raw(), false),
    };
    if needs_reference {
        // SAFETY: 单例由实例持有，存活。
        unsafe { instance.incref_object(raw.as_ptr()) };
    }
    raw
}

/// `BC-56` 与调用（`CALL`／`CALL_KW`）。

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
pub fn execute<'a>(
    instance: &'a Instance,
    frame: &Owned<'a, Frame>,
) -> Result<ExecOutcome<'a>, ExecError> {
    let code_header = frame.get().code().expect("BC-42：帧必须持有 code object");
    // SAFETY: 帧持有一份对 code object 的引用（BC-42），因此它在帧存活期间有效；
    // 帧由本函数的调用方持有。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    // **BC-47**：挂起的帧（生成器／await）从**恢复点**接着跑——值栈与 ip 都在恢复点里。
    // 新帧的 ip 是 0，所以"一律按帧的 ip 起步"这一条对两种情况都成立。
    if frame.get().is_suspended() {
        frame.get().resume()?;
    }
    let mut decoder = Decoder::new(code.code());
    decoder.set_position(frame.get().instruction_pointer());
    while let Some(instruction) = decoder.next_instruction()? {
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
            "BUILD_TUPLE" | "BUILD_LIST" | "BUILD_SET" => {
                // oparg 是元素个数；压栈顺序就是元素顺序（先压的在前）
                let mut items = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    items.push(frame.get().pop()?);
                }
                items.reverse();

                match name {
                    "BUILD_TUPLE" => push_container(
                        instance,
                        frame.get(),
                        TupleObject::new(builtin_type(instance, "tuple"), items),
                    )?,
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
                // 实测：容器在 PEEK(oparg)——`PEEK` **把指令自己的操作数也算进去**（值就是 PEEK(1)）；
                // 所以弹出值之后，容器在 `oparg - 1`。
                if oparg == 0 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LIST_APPEND／SET_ADD 的 oparg 至少为 1",
                    });
                }
                let value = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg - 1)?;
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
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "容器在栈上的位置或类型不符",
                    });
                }
            }
            "MAP_ADD" => {
                // 实测：`[.., 容器, 键, 值]`，oparg 指**容器**（PEEK 含自身操作数）⇒
                // 容器在 PEEK(oparg) = 弹出键值之后的 `oparg - 2`……实测 dict 推导式里 oparg ＝ 2、
                // 容器在 PEEK(3)，故弹出两个操作数后容器在 `oparg - 1`。
                if oparg < 2 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAP_ADD 的 oparg 至少为 2",
                    });
                }
                let value = frame.get().pop()?;
                let key = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg - 1)?;
                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if ty != builtin_type(instance, "dict") {
                    release(instance, key);
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAP_ADD 的容器在栈上的位置或类型不符",
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
                        what: "容器在栈上的位置或类型不符",
                    });
                }
            }
            "GET_ITER" => {
                // 实测：GET_ITER 净 0（弹被迭代对象、压迭代器）
                let iterable = frame.get().pop()?;
                // 生成器是**它自己的迭代器**（参照实现：`GET_ITER` 对迭代器返回它自己）
                // SAFETY: iterable 是刚出栈的存活对象。
                if unsafe { iterable.as_ref() }.ty() == builtin_type(instance, "generator") {
                    // **注意**：这里是**裸的** `Frame::push`（收"新引用"由帧接手），
                    // 不是上面的助手 —— 出栈那份直接交给帧，**不能**再释放一次。
                    frame.get().push(iterable)?;
                    return Ok(Step::Continue);
                }
                match iterator_type_for(instance, iterable) {
                    Ok(ty) => {
                        let iterator = instance.alloc(IteratorObject::new(ty, iterable, Cell::new(0)));
                        frame.get().push(iterator.into_raw().cast::<Header>())?;
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
                // SAFETY: 类型身份已确认是 IteratorObject 的某个类型。
                let object = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
                let target = object.target();
                let index = object.index();
                let length = iterable_length(instance, target, opcode_number)?;

                if index < length {
                    let item = iterable_item(instance, target, index, opcode_number)?;
                    object.advance();
                    frame.get().push(item)?;
                } else {
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
                if receiver_type != builtin_type(instance, "generator") {
                    release(instance, sent);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "SEND 只接线了生成器（`yield from` 的常见形态）；普通迭代器的 SEND 随后补",
                    });
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
                let frame_header = generator.frame();
                let generator_frame = Owned::new(
                    // SAFETY: frame_header 由生成器持有，这里新增一份引用。
                    {
                        unsafe { instance.incref_object(frame_header.as_ptr()) };
                        frame_header.cast::<Frame>()
                    },
                    instance,
                );
                if generator_frame.get().is_suspended() {
                    generator_frame.get().resume()?;
                }
                // 先把"送进去的值"交出去（`push` 会新增一份引用，所以随后要还自己那份）
                push(instance, generator_frame.get(), sent)?;
                release(instance, sent);
                match execute(instance, &generator_frame) {
                    Ok(ExecOutcome::Yielded(value)) => {
                        // 让出的值是**新引用**，裸 `Frame::push` 正好接手
                        frame.get().push(value)?;
                    }
                    Ok(ExecOutcome::Returned(value)) => {
                        generator.mark_finished();
                        let raw = value_into_raw(instance, value);
                        frame.get().push(raw)?;
                        decoder.set_position(target);
                    }
                    Err(error) => return Err(error),
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
                push_small_int(instance, frame.get(), oparg as i64)?;
            }
            "FORMAT_SIMPLE" => {
                // 净 0：TOS 换成它的 `str()`（3.14 把旧的 `FORMAT_VALUE` 拆成了三条）
                let value = frame.get().pop()?;
                let text = match instance.object_str(value) {
                    Some(text) => text,
                    None => {
                        release(instance, value);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "FORMAT_SIMPLE 只接线了 None／bool／int／str 的 str()（协议槽位随后补）",
                        });
                    }
                };
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "CONVERT_VALUE" => {
                // 净 0：`!s`／`!r`／`!a`（实测 oparg 1／2／3）
                let value = frame.get().pop()?;
                let text = match oparg {
                    1 => instance.object_str(value),
                    2 => instance.object_repr(value),
                    3 => instance.object_ascii(value),
                    _ => None,
                };
                let Some(text) = text else {
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "CONVERT_VALUE 只接线了 None／bool／int／str",
                    });
                };
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "FORMAT_WITH_SPEC" => {
                return Err(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "FORMAT_WITH_SPEC 要 `__format__`（含对齐／宽度／精度），随后补",
                });
            }
            "GET_LEN" => {
                // 实测：+1（不弹原对象）
                let raw = frame.get().peek()?;
                let length = iterable_length(instance, raw, opcode_number)?;
                push_small_int(instance, frame.get(), length as i64)?;
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
                let exception = frame.get().peek()?;
                // SAFETY: class_object 是刚出栈的存活对象。
                let class_type = unsafe { class_object.as_ref() }.ty();
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
                    // SAFETY: exception 在帧值栈上，存活。
                    let exception_type = unsafe { exception.as_ref() }.ty();
                    let matched = instance.is_subtype(exception_type, class);
                    release(instance, class_object);
                    matched
                } else {
                    // 类是 tuple（`except (A, B)`）的情形随后补
                    release(instance, class_object);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "CHECK_EXC_MATCH 只接线了单个异常类（tuple 形态随后补）",
                    });
                };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "RERAISE" => {
                // 实测：`RERAISE n` 先弹 `n` 个额外值（通常是 lasti），再抛 TOS
                for _ in 0..oparg {
                    release(instance, frame.get().pop()?);
                }
                let exception = frame.get().pop()?;
                return Err(raise(instance, exception));
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
                let object = instance.alloc(FunctionObject::new(
                    builtin_type(instance, "function"),
                    code_header,
                    Vec::new(),
                    None,
                ));
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
                    _ => {
                        release(instance, attribute);
                        release(instance, function);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "SET_FUNCTION_ATTRIBUTE 只接线了 defaults(1)／kwdefaults(2)；closure(8)／annotate(16) 随后",
                        });
                    }
                }
                frame.get().push(function)?;
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
                            push(instance, frame.get(), function)?;
                            push(instance, frame.get(), this)?;
                        } else {
                            release(instance, object);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "取绑定方法要 method 类型（TS-42 把它排在后面的阶梯）",
                            });
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
                let object = frame.get().pop()?;
                let value = frame.get().pop()?;
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
                let left_value = as_int(instance, left, opcode_number);
                let right_value = as_int(instance, right, opcode_number);
                release(instance, left);
                release(instance, right);
                let (left_value, right_value) = (left_value?, right_value?);

                // `BC-39`／`BC-58`：cmp 下标 ＝ `oparg >> 5`；bit 4（`& 16`）是 `bool(...)` 标志，
                // 低 4 位是参照实现的编译期信息（`dis` 不读、本层**不解释**但**必须容受**）
                let operator = opcode::get_cmp_op().get(oparg >> 5).copied().ok_or(
                    ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "COMPARE_OP 的 cmp 下标（oparg >> 5）超出 cmp_op 的六元组（BC-39／BC-58）",
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
                    let left_value = as_int(instance, left, opcode_number);
                    let right_value = as_int(instance, right, opcode_number);
                    release(instance, left);
                    release(instance, right);
                    let (left_value, right_value) = (left_value?, right_value?);
                    let result = binary_op(name, left_value, right_value)?;
                    push_int_result(instance, frame.get(), result)?;
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
                return Ok(Step::Return(value_from_raw(instance, frame.get().pop()?)));
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
                let offset_bytes = instruction.offset * 2;
                let table = parse_exception_table(code.exceptiontable()).map_err(ExecError::Decode)?;
                let handler = table
                    .iter()
                    .find(|entry| entry.start <= offset_bytes && offset_bytes < entry.end);
                let Some(entry) = handler else {
                    return Err(ExecError::Raised { exception });
                };
                while frame.get().depth() > entry.depth {
                    release(instance, frame.get().pop()?);
                }
                if entry.lasti {
                    push_small_int(instance, frame.get(), (offset_bytes / 2) as i64)?;
                }
                push(instance, frame.get(), exception)?;
                decoder.set_position(entry.target / 2);
            }
            Err(other) => return Err(other),
        }
    }
    Err(ExecError::FellOffEnd)
}
