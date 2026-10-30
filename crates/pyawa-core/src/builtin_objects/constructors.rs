//! 本文件是 `builtin_objects` 的 `constructors` 域：`OM-11` 的 `new` 槽：各内建类型的实例化入口。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;



/// `object()`：无属性的裸实例。
pub unsafe fn plain_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "object() takes no arguments"));
    }
    Ok(instance.alloc(PlainObject::new(class)).into_raw().cast::<Header>())
}

/// 用户类（载荷是 [`AttributeObject`]）：空实例，字典惰性建立（`OM-14`）。
///
/// **实参不在这里处理**——参照实现里它们归 `__init__`（调用方拿到实例后再调它），
/// 所以这个槽**必须**收下任意实参、不因"有实参"而拒绝。
pub unsafe fn attribute_new(
    class: NonNull<crate::TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    Ok(
        instance
            .alloc(AttributeObject::new(class, core::cell::RefCell::new(None)))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `int(<字符串>)` 的十进制解析（**只做实测确认过的那一档**；数值本身是任意精度）。
pub(crate) enum Decimal {
    /// 解析成功。
    Value(BigInt),
    /// 参照会报 `ValueError`（非法字面量）。
    NotALiteral,
    /// 参照**接受**但本层没接线（非 ASCII 数字）⇒ 必须如实报未实现，**不许**冒充 `ValueError`。
    NotWired,
}

pub(crate) fn parse_decimal(text: &str) -> Decimal {
    let trimmed = text.trim_matches(|c: char| c.is_ascii_whitespace());
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if digits.is_empty() {
        return Decimal::NotALiteral;
    }
    let mut cleaned = String::with_capacity(digits.len() + 1);
    if negative {
        cleaned.push('-');
    }
    let mut seen_digit = false;
    for character in digits.chars() {
        if character == '_' {
            continue; // 实测：`'1_2'` 参照接受
        }
        match character.to_digit(10) {
            Some(digit) if character.is_ascii() => {
                seen_digit = true;
                cleaned.push(char::from(b'0' + digit as u8));
            }
            // 非 ASCII 数字（参照接受）⇒ 未接线；真正的非法字符 ⇒ 参照报 ValueError
            Some(_) => return Decimal::NotWired,
            None => return Decimal::NotALiteral,
        }
    }
    if !seen_digit {
        return Decimal::NotALiteral;
    }
    match BigInt::from_decimal(&cleaned) {
        Some(value) => Decimal::Value(value),
        None => Decimal::NotALiteral,
    }
}

/// 取 `bool` **单例**并给调用方一份引用（`OM-23`）。
pub(crate) fn singleton_bool(instance: &Instance, value: bool) -> NonNull<Header> {
    let flag = instance.singletons().boolean(value);
    // SAFETY: 单例由实例持有。
    unsafe { instance.incref_object(flag.as_ptr()) };
    flag
}

/// `bool()`：零参 ⇒ `False`；一个实参 ⇒ 真值；多参 ⇒ 照实测报 `TypeError`。
pub unsafe fn bool_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // 实测：`bool()` ⇒ `False`；`bool(1, 2)` ⇒ `TypeError: bool expected at most 1 argument, got 2`；
    // `bool(x)` ⇒ `x` 的真值 —— 走核心**同一份**真值判定（`truthiness_public`），不另写一套 ✓
    match args {
        [] => Ok(singleton_bool(instance, false)),
        [only] => {
            let truth = crate::executor::truthiness_public(instance, *only, 0)?;
            Ok(singleton_bool(instance, truth))
        }
        _ => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("bool expected at most 1 argument, got {}", args.len()),
        )),
    }
}


/// `dict()`：空字典。
pub unsafe fn dict_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **实参归 `__init__` 管** ✓（第 99 轮照参照定 ✓）：`dict.__new__` **只负责建空映射** ✓，
    // 填内容是 `dict.__init__` 的事 ✓（子类可以自己重写 `__init__` 用自己的实参 ✓ ——
    // `Lib/enum.py` 的 `EnumDict.__init__(self, cls_name=None)` 正是这样 ✓，
    // 而类的实例化会先调 `new` 再调 `__init__` ✓）。先前这里"有实参就报未接线" ✗
    // ⇒ `EnumDict(cls)` 直接失败 ✓。
    let _ = args;
    Ok(
        instance
            .alloc(DictObject::new(class, core::cell::RefCell::new(Vec::new())))
            .into_raw()
            .cast::<Header>(),
    )
}

/// `tuple()`：空元组。
pub unsafe fn tuple_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **子类布局** ✓（第 728 轮）：`class P(tuple)` 的实例要**真的是 `P`** ✓ ——
    // 载荷同构（`TupleObject` ✓），只是**类型槽位**用请求的类 ✓（照 `make_str` 的口径 ✓）；
    // `class` 就是 `tuple` 本身时走老路（含 `tuple() is ()` 的单例 ✓）。
    let tuple_type = instance.type_named("tuple").unwrap_or(class);
    // **`tuple()`**：**OM-23** 要求给**空元组单例**（`tuple() is ()` 必须为真 ✓）。
    let Some(source) = args.first() else {
        if class == tuple_type {
            return Ok(instance.new_tuple(Vec::new()));
        }
        return Ok(instance
            .alloc(TupleObject::new(class, Vec::new()))
            .into_raw()
            .cast::<Header>());
    };
    // **`tuple(可迭代)`**（第 184 轮，与 `list_new` 同款 ✓）：容器走快路 ✓，其余走
    // **`iter_object` ＋ `advance_iterator`**（**一处真相** ✓）。
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut borrowed = true;
    let source_ty = instance.type_name(instance.type_of(*source));
    match source_ty.as_str() {
        "list" => items = unsafe { &*source.as_ptr().cast::<ListObject>() }.items().to_vec(),
        "tuple" => items = unsafe { &*source.as_ptr().cast::<TupleObject>() }.items().to_vec(),
        "set" | "frozenset" => {
            items = unsafe { &*source.as_ptr().cast::<SetObject>() }.items().to_vec()
        }
        "dict" => {
            items = unsafe { &*source.as_ptr().cast::<DictObject>() }
                .entries()
                .into_iter()
                .map(|(key, _)| key)
                .collect()
        }
        _ => {
            borrowed = false;
            let iterator = instance.iter_object(*source)?;
            loop {
                match instance.advance_iterator(iterator)? {
                    Some(item) => items.push(item),
                    None => break,
                }
            }
            unsafe { instance.release_object(iterator.as_ptr()) };
        }
    }
    if borrowed {
        for item in &items {
            unsafe { instance.incref_object(item.as_ptr()) };
        }
    }
    if class == tuple_type {
        return Ok(instance.new_tuple(items));
    }
    // 子类：**按请求的类**分配 ✓（`type(p) is P` ✓）。
    Ok(instance
        .alloc(TupleObject::new(class, items))
        .into_raw()
        .cast::<Header>())
}

/// **`tuple.__new__(cls, iterable=())`** ✓（第 728 轮）：属性通道上的入口 ✓ ——
/// 先前 `tuple.__new__` 落到 `object.__new__` ✗（`Lib/collections/__init__.py` 的 namedtuple
/// 生成类第一句就是 `_tuple_new(cls, (…))` ✓，`dis.py`／`enum`／`CodecInfo` 全压在这格 ✓）。
pub unsafe fn tuple_new_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`tuple.__new__` 是 staticmethod 语义** ✓：`tuple.__new__(cls, iterable)` 的 `cls` 是**第一个显式实参** ✓。
    // **两条派发形态都要认** ✓（task-14 尾巴 ✓）：
    // - 属性通道绑成方法 ⇒ `cls` 在 `bound`（`tuple.__new__(P, it)` 时 `bound`＝`tuple` 而 `args[0]`＝`P` ✓）；
    // - 构造分派（`call.rs` 的覆写支）⇒ `cls` 也可能在 `bound` 或 `args[0]` ✓。
    // 判据 ✓：`bound` 是**类型对象**且（`args` 空 **或** `args[0]` **不是**类型对象）⇒ 用 `bound` 当 `cls` ✓；
    // 否则用 `args[0]` ✓（显式 `tuple.__new__(cls, …)` 永远是后者 ✓）。
    let (class_header, rest) = match (bound, args.split_first()) {
        (Some(this), Some((first, _)))
            if instance.is_type_object(this) && !instance.is_type_object(*first) =>
        {
            (this, args)
        }
        (Some(this), None) if instance.is_type_object(this) => (this, &[][..]),
        (_, Some((first, remaining))) => (*first, remaining),
        (Some(this), None) => (this, &[][..]),
        (None, None) => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "tuple.__new__(): not enough arguments",
            ))
        }
    };
    let Some(class) = instance.as_type(class_header) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "tuple.__new__(X): X is not a type object",
        ));
    };
    unsafe { tuple_new(class, rest, instance) }
}

/// **数据类型的 `__str__`／`__repr__` 方法面** ✓（第 411 轮，task-22 G3 ✓）：
/// `str.__str__`／`int.__str__` 一族要在**类型字典**里看得见 ✓ —— 真 `enum.py` 的
/// `ReprEnum` 分支靠 `member_type.__str__`／`__repr__` 给 `StrEnum`／`IntEnum` 装成员格式化 ✓
/// （先前 `str.__str__` 落到元类兜底的 `object.__str__` ✗ ⇒ `str(StrEnum成员)` 出 repr 形 `'a'` ✗）。
/// 实现直接走**本对象类型的槽位**（`object_str_native`／`object_repr_native` ✓），不再经覆写通道 ✓。
pub fn data_dunder_str_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(target) = bound.or_else(|| args.first().copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__str__' of 'object' object needs an argument",
        ));
    };
    // **只走 `str` 槽** ✓（不走属性覆写通道 —— 那会调回本原生 ⇒ 无限递归 ✗；
    // 本函数**就是**"数据类型的 `__str__` 实现" ✓）。
    // SAFETY: target 由实参／绑定槽保证存活。
    let ty = unsafe { target.as_ref() }.ty();
    // SAFETY: ty 由注册表持有。
    if let Some(slot) = unsafe { ty.as_ref() }.slots().str {
        // SAFETY: 槽位契约见 `StrFn`。
        return Ok(instance.new_str(&unsafe { slot(target.as_ptr(), instance) }?));
    }
    // 槽位缺失 ⇒ 照 `object.__str__` 的回退：用 `__repr__` 槽（`object_repr_native` 只走槽 ✓）
    Ok(instance.new_str(&instance.object_repr_native(target)?))
}

/// 数据类型的 `__repr__`（同 [`data_dunder_str_native`]）。
pub fn data_dunder_repr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(target) = bound.or_else(|| args.first().copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__repr__' of 'object' object needs an argument",
        ));
    };
    Ok(instance.new_str(&instance.object_repr_native(target)?))
}

/// **数据类型自己的 `__new__`**（第 410 轮，task-21 G2 ✓）：`int.__new__(子类, 值)`／
/// `str.__new__(子类, 值)` 一族 ✓。
///
/// 背景 ✗：此前 `builtins` 给 `int/str/float/tuple/bytes/list/dict/set` 全塞**同一个**
/// `object.__new__` 占位（只为 `enum.py` 的 `'__new__' in base.__dict__`）⇒
/// `str.__new__(S, "abc")` 报 `object.__new__() takes exactly one argument` ✗
/// （真 `enum.py` 的 `StrEnum.__new__`／`Lib/http/__init__.py` 的 `HTTPMethod.__new__`
/// 全压在这格 ✓）。
///
/// 语义照参照实测（3.14.4）：
/// * `T.__new__(子类, *args)` ⇒ **子类型**实例（`type(str.__new__(S, "abc")).__name__ == 'S'` ✓）；
/// * `T.__new__(cls)` 一族也照槽位默认（`int.__new__(I)` ⇒ `0`、`str.__new__(S)` ⇒ `''` ✓）；
/// * 类不是 `T` 的子类型 ⇒
///   `TypeError: str.__new__(I): I is not a subtype of str` ✓；
/// * 第一实参不是类型 ⇒ `TypeError: str.__new__(X): X is not a type object (str)` ✓；
/// * 关键字实参 ⇒ `TypeError: str() got an unexpected keyword argument 'bad'` ✓
///   （`T.__new__` 的槽位形态本来就只收位置实参 ✓）。
pub(crate) fn data_new_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    owner: &str,
) -> Result<NonNull<Header>, crate::ExecError> {
    // 派发形态同 [`tuple_new_native`] ✓：属性通道可能把**拥有者类型**放进 `bound` ✓，
    // 构造分派（机器 `__new__` 覆写支）则把 `cls` 放进 `args[0]` ✓ —— 两种都要认 ✓。
    let (class_header, rest) = match (bound, args.split_first()) {
        (Some(this), Some((first, _)))
            if instance.is_type_object(this) && !instance.is_type_object(*first) =>
        {
            (this, args)
        }
        (Some(this), None) if instance.is_type_object(this) => (this, &[][..]),
        (_, Some((first, remaining))) => (*first, remaining),
        (Some(this), None) => (this, &[][..]),
        (None, None) => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("{owner}.__new__(): not enough arguments"),
            ))
        }
    };
    let Some(class) = instance.as_type(class_header) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{owner}.__new__(X): X is not a type object ({owner})"),
        ));
    };
    let owner_type = instance
        .type_named(owner)
        .expect("数据类型的 `__new__` 只在引导期之后挂");
    if !instance.is_subtype(class, owner_type) {
        // SAFETY: class 是注册表持有的类型对象。
        let class_name = unsafe { class.as_ref() }.name().to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{owner}.__new__({class_name}): {class_name} is not a subtype of {owner}"),
        ));
    }
    if let Some((key, _)) = kwargs.first() {
        let key = instance.text_of(*key).unwrap_or("<unknown>").to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{owner}() got an unexpected keyword argument '{key}'"),
        ));
    }
    // 槽位按**请求的类**取 ✓（子类继承数据类型的构造槽 ⇒ 造出的是子类实例 ✓）。
    // SAFETY: class 是存活类型对象。
    let Some(slot) = (unsafe { class.as_ref() }).slots().new else {
        // SAFETY: 同上。
        let class_name = unsafe { class.as_ref() }.name().to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("cannot create '{class_name}' instances"),
        ));
    };
    // SAFETY: 槽位契约见 `NewFn`；实参借用视图 ✓。
    unsafe { slot(class, rest, instance) }
}

/// `str.__new__(cls, …)`（详见 [`data_new_native`]）。
pub unsafe fn str_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "str")
}

/// `int.__new__(cls, …)`。
pub unsafe fn int_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "int")
}

/// `float.__new__(cls, …)`。
pub unsafe fn float_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "float")
}

/// `bytes.__new__(cls, …)`。
pub unsafe fn bytes_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "bytes")
}

/// `list.__new__(cls, …)`。
pub unsafe fn list_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "list")
}

/// `dict.__new__(cls, …)`。
pub unsafe fn dict_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "dict")
}

/// `set.__new__(cls, …)`。
pub unsafe fn set_dunder_new(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    data_new_native(instance, bound, args, kwargs, "set")
}

/// **这是我们自己挂的"默认 `__new__`"吗**（第 410 轮，task-21 G2 ✓）：构造分派
/// （`executor/call.rs`）据此区分"Python 级覆写"与"内建类型自带实现" ✓ ——
/// 按**原生函数指针**比 ✓（先前只与 `object.__new__`／`type.__new__` 比**对象指针** ✗ ⇒
/// 新挂的数据类型实现会被当成"用户覆写"⇒ 先调它再调 `__init__`，语义错 ✗）。
pub fn is_default_new(instance: &Instance, found: NonNull<Header>) -> bool {
    if instance.type_name(instance.type_of(found)) != "builtin_function_or_method" {
        return false;
    }
    // SAFETY: 类型身份刚确认 ⇒ `BuiltinFunctionObject` 载荷。
    let native = unsafe { &*found.as_ptr().cast::<BuiltinFunctionObject>() };
    let handler = native.function();
    let defaults: [NativeFn; 9] = [
        crate::builtin::object::object_new_native as NativeFn,
        type_new_native as NativeFn,
        str_dunder_new as NativeFn,
        int_dunder_new as NativeFn,
        float_dunder_new as NativeFn,
        bytes_dunder_new as NativeFn,
        list_dunder_new as NativeFn,
        dict_dunder_new as NativeFn,
        set_dunder_new as NativeFn,
    ];
    defaults.iter().any(|candidate| core::ptr::fn_addr_eq(*candidate, handler))
}

/// `str()`：空串（走 `OM-23` 的单例）。

/// 造一个字符串实例，**类型用调用方给的类** ✓（第 612 轮 ✓，与 `int` 的 `make_int` 同型 ✓）：
/// `class S(str)` 的 `S("ab")` 要造出 **`S` 的实例** ✓（否则 `type(s) is S` 为假 ✗、属性写到 `str` 上 ✗）。
pub(crate) fn make_str(class: NonNull<crate::TypeObject>, instance: &Instance, text: &str) -> NonNull<Header> {
    if class == instance.singletons().str_type() {
        return instance.new_str(text);
    }
    let object = instance.alloc(StrObject::new(class, text.to_owned()));
    object.into_raw().cast::<Header>()
}

/// 造一个 **WTF-8** 字符串实例（P3-38 S3a：`str(bytes, encoding, errors)` 的解码结果可能含
/// 孤立代理 ⇒ 不能走 `make_str(&str)` ✓）；类型仍用调用方给的类 ✓。
pub(crate) fn make_str_wtf8(
    class: NonNull<crate::TypeObject>,
    instance: &Instance,
    bytes: &[u8],
) -> NonNull<Header> {
    if class == instance.singletons().str_type() {
        return instance
            .new_str_wtf8(bytes)
            .expect("解码产物必须是合法 WTF-8");
    }
    let object = StrObject::new_wtf8(class, bytes.to_vec())
        .expect("解码产物必须是合法 WTF-8");
    instance.alloc(object).into_raw().cast::<Header>()
}

pub unsafe fn str_new(
    _class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    // **`str(x)` 的参照口径**（第 185 轮实测 ✓）：`str()` ⇒ `''` ✓；本来就是 `str` ⇒ **原样给回** ✓（并 `retain` ✓）；
    // 其余走 `str()` 那一套 ✓（`str([1, 2])` ⇒ `'[1, 2]'` ✓、`str(123)` ⇒ `'123'` ✓、`str(None)` ⇒ `'None'` ✓）。
    let Some(value) = args.first() else {
        return Ok(make_str(_class, instance, ""));
    };
    // **`str(object, encoding[, errors])`** ✓（task-17 第 2 轮真 bug 修 ✗）：先前**只看第一个实参** ✗
    // ⇒ 多给的 encoding 被**整个忽略** ✓（实测 `str(b'abc', 'latin1')` 回 `"b'abc'"` ✗，参照回 `'abc'` ✓）。
    // 这条正是 `Lib/re/_parser.py:235` 的 `str(string, 'latin1')`（**bytes 模式**的 tokenizer ✓）⇒
    // bytes 正则的解析器一直在读 **`repr(bytes)`** ✗（`rb'(?s)a'` 因此报
    // `global flags not at the start of the expression` ✗）。**一处真相** ✓：解码仍走
    // `Instance::decode_with_encoding_and_errors` ✓（`bytes.decode` 同一条 ✓）；`errors` 的
    // strict／ignore／replace 三条真接（task-9 ✓）。
    if let Some(encoding_object) = args.get(1) {
        let Some(encoding) = instance.text_value(*encoding_object) else {
            let name = instance.type_name(instance.type_of(*encoding_object));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("str() argument 'encoding' must be str, not {name}"),
            ));
        };
        // **`errors` 真接**（task-9 ✓）：先前只做类型检查、值被**整个忽略** ✗
        // ⇒ `str(b'\xe9', 'ascii', 'replace')` 与参照不一致。现在把值传下去（与
        // `bytes.decode` 同一条 `decode_with_encoding_and_errors` ✓ **一处真相**）。
        let mut errors = "strict".to_owned();
        if let Some(errors_object) = args.get(2) {
            match instance.text_value(*errors_object) {
                Some(text) => errors = text,
                None => {
                    let name = instance.type_name(instance.type_of(*errors_object));
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        &format!("str() argument 'errors' must be str, not {name}"),
                    ));
                }
            }
        }
        let Some(bytes) = instance.bytes_value(*value) else {
            // 参照对 `str` **单列**一条文案 ✓（不带类型名 ✓）。
            let message = if instance.type_of(*value) == instance.singletons().str_type() {
                "decoding str is not supported".to_owned()
            } else {
                let name = instance.type_name(instance.type_of(*value));
                format!("decoding to str: need a bytes-like object, {name} found")
            };
            return Err(instance.raise_builtin_error("TypeError", &message));
        };
        let decoded = instance.decode_with_encoding_and_errors(bytes, &encoding, &errors)?;
        // **P3-38 S3a**：解码结果可能是 WTF-8（`surrogateescape`）⇒ 走 `wtf8_of`（先前
        // `text_value` 对 WTF-8 表示会取不到／panic ✗）。
        let raw = instance.wtf8_of(decoded).map(<[u8]>::to_vec).unwrap_or_default();
        // SAFETY: `decode_with_encoding_and_errors` 交回的是**新引用** ✓ ⇒ 取完字节把这**一份**还掉 ✓。
        unsafe { instance.release_object(decoded.as_ptr()) };
        return Ok(make_str_wtf8(_class, instance, &raw));
    }
    // **"本来就是 `str` ⇒ 原样给回"只对 `str` 本身成立** ✓（第 612 轮修 ✗）：`class S(str)` 的 `S("ab")`
    // 必须造出 **`S` 的实例** ✓ —— 先前这条对子类也生效 ✗ ⇒ 返回的是普通 `str` ✗（`type(s) is S` 为假 ✓）。
    if instance.type_of(*value) == instance.singletons().str_type()
        && _class == instance.singletons().str_type()
    {
        return Ok(instance.retain(*value));
    }
    // **`class S(str)` 的 `S(<str>)`**（P3-38 S3a）：值本来就是 str（可能含孤立代理）⇒ 内容原样、
    // 只换类 ✓（否则落到 `object_str` 的 `.value()` ⇒ 对 WTF-8 表示会 panic ✗）。
    if instance.is_str_object(*value) {
        let bytes = instance.wtf8_of(*value).map(<[u8]>::to_vec).unwrap_or_default();
        return Ok(make_str_wtf8(_class, instance, &bytes));
    }
    // **走 `str()` 那一套** ✓（第 211 轮真 bug 修复 ✗：先前用的是 `object_repr` ✗ ⇒
    // 等于把 `str(x)` 实现成 `repr(x)` ✓ ⇒ 用户自定义的 `__str__` 被**整个忽略** ✗、
    // 异常消息也变成 `"ValueError('v')"` ✗（第 203 轮实测到的那条 ✓）——**同一因** ✓）。
    let text = instance.object_str(*value)?;
    Ok(make_str(_class, instance, &text))
}

/// 异常类：`ValueError("x")` —— **实参进 `args`**（借用视图，这里自己 incref）。
///
/// 这是 `raise ValueError("x")` 能跑通的那一半：编译器发的是"调用类 ＋ `RAISE_VARARGS 1`"。
pub unsafe fn exception_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let mut stored: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        stored.push(*argument);
    }
    let object = instance.alloc(ExceptionObject::new(
        class,
        core::cell::RefCell::new(stored),
        core::cell::RefCell::new(None),
        core::cell::RefCell::new(None),
        core::cell::Cell::new(false),
        core::cell::RefCell::new(None),
    ));
    Ok(object.into_raw().cast::<Header>())
}
