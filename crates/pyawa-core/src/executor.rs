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

use core::cell::RefCell;
use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::decode::{DecodeError, Decoder};
use crate::frame::{Frame, FrameError};
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::opcode;
use crate::refcount::{Owned, PyRef};
use crate::builtin_objects::{
    BoolObject, DictObject, FloatObject, FunctionObject, IntObject, ListObject, SetObject, StrObject,
    TupleObject,
};
use crate::singleton::{SMALL_INT_MAX, SMALL_INT_MIN};
use crate::value::Value;

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
    /// 解包时元素个数不符（参照实现报 `ValueError`；异常对象尚未接线，这里先如实报错）。
    WrongUnpackCount { expected: usize, found: usize },
    /// 下标越界（参照实现报 `IndexError`；异常对象尚未接线）。
    IndexOutOfRange { index: i64, length: usize },
    /// 字典里没有这个键（参照实现报 `KeyError`；异常对象尚未接线）。
    KeyNotFound,
    /// `BC-56`：位置实参多于形参，且函数不收 `*args`。
    TooManyArguments { given: usize, accepted: usize },
    /// `BC-56`：必填形参没拿到实参。
    MissingArgument { name: String },
    /// `BC-56`：同一个形参被位置与关键字各给了一次。
    DuplicateArgument { name: String },
    /// `BC-56`：关键字不是任何形参的名字，且函数不收 `**kwargs`。
    UnexpectedKeyword { name: String },
    /// `BC-56`：把仅位置形参当关键字传了。
    PositionalOnlyAsKeyword { name: String },
    /// `BC-56`：仅关键字形参被位置实参填了。
    KeywordOnlyAsPositional { name: String },
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
        let position = normalize_index(index, object.len())
            .ok_or(ExecError::IndexOutOfRange { index, length: object.len() })?;
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
        let position = normalize_index(index, object.len())
            .ok_or(ExecError::IndexOutOfRange { index, length: object.len() })?;
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
            .ok_or(ExecError::KeyNotFound)?;
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
        let position = normalize_index(index, characters.len()).ok_or(
            ExecError::IndexOutOfRange { index, length: characters.len() },
        )?;
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
                return Err(ExecError::IndexOutOfRange { index, length });
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
        let position = normalize_index(index, length)
            .ok_or(ExecError::IndexOutOfRange { index, length })?;
        if let Some(removed) = object.remove(position) {
            release(instance, removed);
        }
        return Ok(());
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
            .ok_or(ExecError::KeyNotFound)?;
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
            return Err(ExecError::TooManyArguments {
                given: given + 1,
                accepted: argcount,
            });
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
                Err(ExecError::PositionalOnlyAsKeyword { name: text })
            } else if locals[slot].is_some() {
                release(instance, value);
                release(instance, name);
                Err(ExecError::DuplicateArgument { name: text })
            } else {
                locals[slot] = Some(value);
                release(instance, name);
                Ok(())
            }
        } else if let Some(slot) = keyword_hit {
            if locals[slot].is_some() {
                release(instance, value);
                release(instance, name);
                Err(ExecError::DuplicateArgument { name: text })
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
            Err(ExecError::UnexpectedKeyword { name: text })
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

    // ④ 位置形参的默认值（对齐到**尾部**若干位置参数）
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
            let name = code.varname(slot).unwrap_or("<unknown>").to_owned();
            for (key, item) in collected {
                release(instance, key);
                release(instance, item);
            }
            for slot in locals.iter_mut().filter_map(Option::take) {
                release(instance, slot);
            }
            return Err(ExecError::MissingArgument { name });
        }
    }

    // ⑤ 仅关键字形参：先看默认值，缺了才报错
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
            None => {
                for (key, item) in collected {
                    release(instance, key);
                    release(instance, item);
                }
                for slot in locals.iter_mut().filter_map(Option::take) {
                    release(instance, slot);
                }
                return Err(ExecError::MissingArgument { name });
            }
        }
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

    let result = execute(instance, &frame)?;
    Ok(value_into_raw(instance, result))
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
                        return Err(ExecError::WrongUnpackCount {
                            expected: oparg,
                            found: items.len(),
                        });
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
                        return Err(ExecError::WrongUnpackCount {
                            expected: before + after,
                            found: items.len(),
                        });
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
                return Ok(value_from_raw(instance, frame.get().pop()?));
            }
            _ => return Err(ExecError::NotImplemented { opcode: opcode_number }),
        }
    }
    Err(ExecError::FellOffEnd)
}
