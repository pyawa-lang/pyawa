//! `array` 模块（**task-18 ✓**）：`import multiprocessing` 的下一道 C 模块闸
//! （task-16 链实测：循环导入修完后 `multiprocessing` 一族 24 格卡 `No module named 'array'`）。
//!
//! 侦察（本轮实测 ✓）：`array` 在链上**导入期只被"取类型属性"**，不构造 ——
//! `multiprocessing/managers.py:50` 的 `reduction.register(array.array, reduce_array)` 读一次
//! `array.array`；`multiprocessing/reduction.py:140`／`dummy/__init__.py:23` 只是 `import array`
//! （`array.array(...)` 都在**函数体**里＝运行期）。所以最小面＝**类型存在**；但类型若完全是空壳，
//! `managers.py` 的 `register` 与运行期 `reduce_array`（`a.typecode`／`a.tobytes()`）就没有真语义 ✓
//! ⇒ 本轮按"**薄载荷真做**"落地：**空数组**可以真构造，`typecode`／`itemsize`／`len`／`repr`／
//! `tobytes`／`tolist` 对空数组**语义照参照** ✓。
//!
//! ## 真面（值全部照参照实测 ✓：本机 `python3` 3.14.4／Linux）
//!
//! - `typecodes` ＝ `"bBuwhHiIlLqQfd"`（**字符串**，3.14 起含 `w`）；`ArrayType is array` ✓；
//! - `array(typecode)`：合法 typecode（14 个）⇒ 空数组；`typecode`／`itemsize` 是**真值**
//!   （itemsize 表：`b/B=1`、`u/w=4`、`h/H=2`、`i/I=4`、`l/L=8`、`q/Q=8`、`f=4`、`d=8`）；
//! - 构造校验文案照参照：`array() takes at least 1 argument (0 given)`／`takes at most 2 arguments`／
//!   `array.array() takes no keyword arguments`／`array() argument 1 must be a unicode character, not X`
//!   （长度≠1 的 str ⇒ `not a string of length 2`）／`ValueError: bad typecode (must be b, B, u, w, h, H,
//!   i, I, l, L, q, Q, f or d)`／初始值非可迭代 ⇒ `'X' object is not iterable`；
//! - `repr(array('i'))` ⇒ `array('i')`（`str` 同形）、`len` ⇒ 0、`tobytes()` ⇒ `b''`、`tolist()` ⇒ `[]`。
//!
//! ## 如实边界（硬 ✓：**禁止假数据**）
//!
//! **只做空数组**：元素存储未接线 —— 带**非空**初始值构造、以及 `append`／`extend`／`frombytes`／
//! `tobytes` 之外的元素级方法**一律如实报 `NotImplementedError`**（消息含"未接线"）或**不登记**
//! （缺名 = 语义少接线，如实记；参照 21 个方法里本轮登记 5 个：`__init__`／`__repr__`／`__len__`／
//! `tobytes`／`tolist`）。因此 `tobytes()` 恒为 `b''`、`tolist()` 恒为 `[]` —— 这不是编数据：
//! **本对象的长度恒为 0**，空数组的这两个返回就是参照真值 ✓。
//!
//! **已知偏差（如实记）**：`typecode`／`itemsize` 是**实例属性**（参照是只读属性 ⇒ 本层可写）；
//! `'u'` 的 DeprecationWarning 不打（本层无 warnings 通道）；`buffer_info` 等其余方法未登记。
//!
//! 本模块**不碰平台**（`CX-4`：`pyawa-stdlib/src` 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`array`）。
pub const NAME: &str = "array";

/// 模块 `__doc__`（**照参照逐字抄** ✓）。
pub const DOC: &str = "This module defines an object type which can efficiently represent\nan array of basic values: characters, integers, floating-point\nnumbers.  Arrays are sequence types and behave very much like lists,\nexcept that the type of objects stored in them is constrained.\n";

/// 合法 typecode 的集合（**照参照真值** ✓；3.14 起含 `w`）。
const TYPECODES: &str = "bBuwhHiIlLqQfd";

/// `typecode → itemsize`（**照参照本机实测** ✓）。
const ITEMSIZES: &[(char, i64)] = &[
    ('b', 1),
    ('B', 1),
    ('u', 4),
    ('w', 4),
    ('h', 2),
    ('H', 2),
    ('i', 4),
    ('I', 4),
    ('l', 8),
    ('L', 8),
    ('q', 8),
    ('Q', 8),
    ('f', 4),
    ('d', 8),
];

/// 非法 typecode 的报错文案（**照参照逐字** ✓，注意含 `w`）。
const BAD_TYPECODE: &str = "bad typecode (must be b, B, u, w, h, H, i, I, l, L, q, Q, f or d)";

/// 建 `array` 的命名空间（**新引用**的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    if let Ok(class) = build_array_class(instance) {
        instance.dict_set(namespace, "array", class);
        // 参照实测：`array.ArrayType is array.array` ✓（同一对象）
        instance.dict_set(namespace, "ArrayType", class);
    }
    instance.dict_set(namespace, "typecodes", instance.new_str(TYPECODES));
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}

/// 建 `array` 类（照 `_random.Random`／`_struct.Struct` 的手法：`build_class_from_parts` ✓）。
fn build_array_class(instance: &Instance) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let base = instance
        .type_named("object")
        .expect("object 在引导期已登记")
        .cast::<Header>();
    let class_namespace = instance.new_dict();
    for (name, handler) in [
        ("__init__", array_init_native as NativeFn),
        ("__repr__", array_repr_native as NativeFn),
        ("__len__", array_len_native as NativeFn),
        ("tobytes", array_tobytes_native as NativeFn),
        ("tolist", array_tolist_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    pyawa_core::build_class_from_parts(
        instance,
        "array".to_owned(),
        vec![base],
        class_namespace,
        None,
    )
}

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

fn this_of<'a>(
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Option<(NonNull<Header>, &'a [NonNull<Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

fn type_error(instance: &Instance, text: &str) -> pyawa_core::ExecError {
    instance.raise_builtin_error("TypeError", text)
}

/// `array(typecode [, initializer])`（**只做空数组**，见模块头 ✓）。
fn array_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some((this, rest)) = this_of(bound, args) else {
        return Err(type_error(instance, "array.__init__ 缺少 self"));
    };
    if !kwargs.is_empty() {
        return Err(type_error(instance, "array.array() takes no keyword arguments"));
    }
    if rest.is_empty() {
        return Err(type_error(instance, "array() takes at least 1 argument (0 given)"));
    }
    if rest.len() > 2 {
        return Err(type_error(
            instance,
            &format!("array() takes at most 2 arguments ({} given)", rest.len()),
        ));
    }
    // ---- typecode 校验（文案照参照 ✓）----
    let Some(text) = instance.text_of(rest[0]) else {
        // 参照对 `None` 的文案是 `not None`（不是 `NoneType`）——照实测 ✓。
        let type_name = if rest[0] == instance.singletons().none() {
            "None".to_owned()
        } else {
            instance.type_name(instance.type_of(rest[0]))
        };
        return Err(type_error(
            instance,
            &format!("array() argument 1 must be a unicode character, not {type_name}"),
        ));
    };
    let characters = text.chars().count();
    if characters != 1 {
        return Err(type_error(
            instance,
            &format!(
                "array() argument 1 must be a unicode character, not a string of length {characters}"
            ),
        ));
    }
    let typecode = text.chars().next().expect("恰好一个字符");
    let Some((_, itemsize)) = ITEMSIZES.iter().find(|(candidate, _)| *candidate == typecode) else {
        return Err(instance.raise_builtin_error("ValueError", BAD_TYPECODE));
    };
    // ---- 初始值：只收"空"（元素存储未接线 ✓，禁止假数据）----
    if rest.len() == 2 {
        match instance.length_of(rest[1]) {
            Some(0) => {}
            Some(_) => {
                return Err(instance.raise_builtin_error(
                    "NotImplementedError",
                    "`array.array`：元素存储未接线（本轮只落空数组薄载荷；带元素构造随后接）",
                ));
            }
            None => {
                let type_name = instance.type_name(instance.type_of(rest[1]));
                return Err(type_error(
                    instance,
                    &format!("'{type_name}' object is not iterable"),
                ));
            }
        }
    }
    // ---- 真值属性（参照里是只读属性；本层用实例属性，如实记 ✓）----
    let code_text = instance.new_str(&typecode.to_string());
    instance.set_attribute_value(this, "typecode", code_text)?;
    instance.set_attribute_value(this, "itemsize", instance.new_int(*itemsize))?;
    Ok(instance.retain(instance.singletons().none()))
}

/// 取实例上的 `typecode` 单字符串（真值属性 ✓）。
fn typecode_of(instance: &Instance, this: NonNull<Header>) -> Option<char> {
    instance
        .attribute_optional_of(this, "typecode")
        .ok()
        .flatten()
        .and_then(|value| instance.text_of(value))
        .and_then(|text| text.chars().next())
}

/// `repr(array('i'))` ⇒ `array('i')`（参照 ✓）。
fn array_repr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some((this, _)) = this_of(bound, args) else {
        return Err(type_error(instance, "array.__repr__ 缺少 self"));
    };
    let typecode = typecode_of(instance, this).map(|code| code.to_string()).unwrap_or_default();
    Ok(instance.new_str(&format!("array('{typecode}')")))
}

/// `len(array(...))` ⇒ 0（**本层对象长度恒为 0** ✓：元素存储未接线）。
fn array_len_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if this_of(bound, args).is_none() {
        return Err(type_error(instance, "array.__len__ 缺少 self"));
    }
    Ok(instance.new_int(0))
}

/// `array.tobytes()` ⇒ `b''`（空数组的参照真值 ✓）。
fn array_tobytes_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if this_of(bound, args).is_none() {
        return Err(type_error(instance, "array.tobytes 缺少 self"));
    }
    Ok(instance.new_bytes(&[]))
}

/// `array.tolist()` ⇒ `[]`（空数组的参照真值 ✓）。
fn array_tolist_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if this_of(bound, args).is_none() {
        return Err(type_error(instance, "array.tolist 缺少 self"));
    }
    Ok(instance.new_list(Vec::new()))
}
