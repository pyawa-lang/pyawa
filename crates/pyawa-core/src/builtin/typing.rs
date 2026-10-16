//! **`_typing` 的最小可用面**（task-24 轮 1 ✓）：`TypeVar`／`NoDefault`（`NoDefaultType`）／`_idfunc`
//! —— 参照里这一档是 **C 模块** ✗（`Lib/_typing.py` 不存在 ✓）⇒ 本层落成"core 造类型 ＋
//! stdlib 装配模块"（与 `memoryview` 同款手法 ✓：`alloc_type_raw` ＋ `new` 槽 ＋ `getattr` 槽 ✓）。
//!
//! **口径对参照实测** ✓：
//! - `_typing.TypeVar('T')` ⇒ `__name__='T'` ✓、`__constraints__=()` ✓、`__bound__=None` ✓、
//!   `__covariant__=False` ✓、`__contravariant__=False` ✓、`__default__=_typing.NoDefault` ✓、
//!   `__infer_variance__=False` ✓（`TypeVar('D', default=5)` ⇒ `__default__=5` ✓）；
//! - `_typing.NoDefault` ⇒ `type(...) is _typing.NoDefaultType` ✓、`NoDefaultType()` 回**同一个**单例 ✓；
//! - `_typing._idfunc(5)` ⇒ `5` ✓（恒等函数 ✓）；
//! - `_typing.Generic` ⇒ 可当基类的普通类 ✓（"`Cannot inherit from plain Generic`" 那条在
//!   `typing.py` 的 `__init_subclass__` 里 ✓，归轮 3 ✓）。
//!
//! **如实范围** ✗（轮 1 只到"装得下"）：`__repr__`／`__or__`／`__typing_subst__` 等 `TypeVar` 的
//! 方法面未接 ✓；`Generic` 的 `__class_getitem__`／`__init_subclass__` 参数收集在轮 3 ✓。

use core::ptr::NonNull;

use crate::builtin_objects::{AttributeObject, BuiltinFunctionObject};
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::ExecError;

/// `_typing` 的 `TypeVar` 实例字段（都放**实例字典**里 ✓ —— 不新增载荷类型 ✓）。
const NAME: &str = "_name";
const CONSTRAINTS: &str = "_constraints";
const BOUND: &str = "_bound";
const COVARIANT: &str = "_covariant";
const CONTRAVARIANT: &str = "_contravariant";
const DEFAULT: &str = "_default";
const INFER_VARIANCE: &str = "_infer_variance";
/// **惰性 `bound`／`default` 的 thunk**（PEP 695 的 `[T: int]`／`[T = int]` ✓，task-24 收口 ①）：
/// 首次读 `__bound__`／`__default__` 才调它（`format=1` ✓），随后写进 `BOUND`／`DEFAULT` 缓存 ✓。
const BOUND_THUNK: &str = "_bound_thunk";
const DEFAULT_THUNK: &str = "_default_thunk";
/// 惰性 default 求值后的**缓存**（构造时那份 `DEFAULT` 是 `NoDefault` 标记 ⇒ 不能拿它当"已求值" ✓）。
const DEFAULT_VALUE: &str = "_default_value";

/// **`TypeVar(name, *constraints, bound=None, covariant=False, contravariant=False,
/// infer_variance=False, default=NoDefault)`** ✓（实例化槽 ✓）。
pub fn typevar_new(
    class: NonNull<TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    // CPython 的签名检查顺序：先要 name ✓。
    let Some(name_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "typevar() missing required argument 'name' (pos 1)",
        ));
    };
    let Some(name) = instance.text_of(name_value) else {
        let actual = instance.type_name(unsafe { name_value.as_ref() }.ty());
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("typevar() argument 'name' must be str, not {actual}"),
        ));
    };
    let name = name.to_owned();
    let constraints: Vec<NonNull<Header>> = args[1..]
        .iter()
        .map(|value| instance.retain(*value))
        .collect();
    finish_typevar(
        instance, class, &name, constraints, None, false, false, false, None,
    )
}

/// **`TypeVar(...)` 的关键字通道** ✓（task-24 收口 ① ✓）：装成 `TypeVar.__new__` ✓ ——
/// `type.__call__` 的**覆写路**会把 `(cls, *args, **kwargs)` 原样交给它 ✓（`executor/call.rs` ✓）
/// ⇒ 不必动 `NewFn` 的槽签名 ✓（其余内建类型不受影响 ✓）。
///
/// 报错文案照参照实测 ✓：缺名／名字类型／单个约束／未知关键字／`infer_variance` 与变型并用 ✓。
pub unsafe fn typevar_new_kwargs(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 覆写路的实参表第一项就是 `cls`（`call_callable` 的契约 ✓）。
    let Some(class_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "typevar() missing required argument 'name' (pos 1)",
        ));
    };
    let class = class_value.cast::<TypeObject>();
    let positional = &args[1..];
    let Some(name_value) = positional.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "typevar() missing required argument 'name' (pos 1)",
        ));
    };
    let Some(name_text) = instance.text_of(name_value) else {
        let actual = instance.type_name(unsafe { name_value.as_ref() }.ty());
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("typevar() argument 'name' must be str, not {actual}"),
        ));
    };
    let name = name_text.to_owned();
    let constraints: Vec<NonNull<Header>> = positional[1..]
        .iter()
        .map(|value| instance.retain(*value))
        .collect();
    let mut bound_value: Option<NonNull<Header>> = None;
    let mut covariant = false;
    let mut contravariant = false;
    let mut infer_variance = false;
    let mut default_value: Option<NonNull<Header>> = None;
    let mut flag_error: Option<ExecError> = None;
    for (key, value) in kwargs {
        let Some(key_text) = instance.text_of(*key) else {
            continue;
        };
        match key_text {
            "bound" => bound_value = Some(*value),
            "covariant" => match instance.truthiness_of(*value) {
                Ok(flag) => covariant = flag,
                Err(error) => flag_error = Some(error),
            },
            "contravariant" => match instance.truthiness_of(*value) {
                Ok(flag) => contravariant = flag,
                Err(error) => flag_error = Some(error),
            },
            "infer_variance" => match instance.truthiness_of(*value) {
                Ok(flag) => infer_variance = flag,
                Err(error) => flag_error = Some(error),
            },
            "default" => default_value = Some(*value),
            other => {
                let message = format!("typevar() got an unexpected keyword argument '{other}'");
                for value in constraints {
                    instance.release(value);
                }
                return Err(instance.raise_builtin_error("TypeError", &message));
            }
        }
        if flag_error.is_some() {
            break;
        }
    }
    if let Some(error) = flag_error {
        for value in constraints {
            instance.release(value);
        }
        return Err(error);
    }
    if (covariant || contravariant) && infer_variance {
        for value in constraints {
            instance.release(value);
        }
        return Err(instance.raise_builtin_error(
            "ValueError",
            "Variance cannot be specified with infer_variance.",
        ));
    }
    finish_typevar(
        instance,
        class,
        &name,
        constraints,
        bound_value,
        covariant,
        contravariant,
        infer_variance,
        default_value,
    )
}

/// **构造一个 `TypeVar` 实例**（槽路与 `__new__` 覆写路**一处真相** ✓）：
/// 约束／bound／变型位／default 全放**实例字典** ✓（不新增载荷类型 ✓；读取见 `typevar_getattr` ✓）。
#[allow(clippy::too_many_arguments)]
fn finish_typevar(
    instance: &Instance,
    class: NonNull<TypeObject>,
    name: &str,
    constraints: Vec<NonNull<Header>>,
    bound: Option<NonNull<Header>>,
    covariant: bool,
    contravariant: bool,
    infer_variance: bool,
    default: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    // **单个约束不合法** ✓（参照：`TypeVar('T', int)` ⇒ `A single constraint is not allowed` ✓）。
    if constraints.len() == 1 {
        for value in constraints {
            instance.release(value);
        }
        return Err(instance.raise_builtin_error(
            "TypeError",
            "A single constraint is not allowed",
        ));
    }
    let object = instance
        .alloc(AttributeObject::new(
            class,
            core::cell::RefCell::new(Some(instance.new_dict())),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: 刚造的就是 `AttributeObject`。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let Some(dict) = view.attributes() else {
        return Err(instance.raise_builtin_error("RuntimeError", "TypeVar 没有实例字典"));
    };
    let name_value = instance.new_str(name);
    instance.dict_set(dict, NAME, name_value);
    instance.release(name_value);
    let constraint_values = instance.new_tuple(constraints);
    instance.dict_set(dict, CONSTRAINTS, constraint_values);
    instance.release(constraint_values);
    if let Some(bound) = bound {
        instance.dict_set(dict, BOUND, bound);
    }
    let covariant_value = instance.new_bool(covariant);
    instance.dict_set(dict, COVARIANT, covariant_value);
    instance.release(covariant_value);
    let contravariant_value = instance.new_bool(contravariant);
    instance.dict_set(dict, CONTRAVARIANT, contravariant_value);
    instance.release(contravariant_value);
    let infer_value = instance.new_bool(infer_variance);
    instance.dict_set(dict, INFER_VARIANCE, infer_value);
    instance.release(infer_value);
    // `default` 是**借用**（调用方持有）⇒ 直接用 ✓；没给才自己造单例（自己那份要归还 ✓）。
    let owned_default = default.is_none();
    let default_value = default.unwrap_or_else(|| default_marker(instance));
    instance.dict_set(dict, DEFAULT, default_value);
    if owned_default {
        instance.release(default_value);
    }
    Ok(object)
}

/// `_typing.NoDefault` 单例（**惰性**挂在 `NoDefaultType` 的类型命名空间里 ✓ —— 与 `memoryview`
/// 的 `__mro_entries__` 同款 ✓）。
pub fn no_default_singleton(instance: &Instance) -> NonNull<Header> {
    let Some(no_default_type) = instance.type_named("NoDefaultType") else {
        return instance.singletons().none();
    };
    if let Some(namespace) = instance.type_namespace(no_default_type.cast()) {
        if let Some(existing) = instance.dict_get(namespace, "__no_default_singleton__") {
            // SAFETY: existing 由命名空间持有，存活；这里交出新的一份。
            unsafe { instance.incref_object(existing.as_ptr()) };
            return existing;
        }
        let object = instance
            .alloc(AttributeObject::new(
                no_default_type,
                core::cell::RefCell::new(Some(instance.new_dict())),
            ))
            .into_raw()
            .cast::<Header>();
        instance.dict_set(namespace, "__no_default_singleton__", object);
        return object;
    }
    instance.singletons().none()
}

fn default_marker(instance: &Instance) -> NonNull<Header> {
    no_default_singleton(instance)
}

/// **`NoDefaultType()` 的实例化槽** ✓：回**同一个**单例 ✓（参照实测 `NoDefaultType() is NoDefault` ✓）。
pub fn no_default_new(
    _class: NonNull<TypeObject>,
    _args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    Ok(no_default_singleton(instance))
}

/// **`TypeVar` 的属性面** ✓（`getattr` 槽；本层 `alloc_type_raw` 型不走 MRO 查字典 ✓）。
pub unsafe fn typevar_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 契约保证 ptr 是本类型的存活对象。
    let object = unsafe { NonNull::new_unchecked(ptr) };
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let dict = view.attributes()?;
    // **`getattr` 槽的契约是"交回新引用"** ✓（见 `exception_getattr` 的写法 ✓）：先前这里直接
    // 把 `dict_get` 的**借用**交出去 ✗ ⇒ 每个读者都"归还"一次 ⇒ 计数被慢慢扣光 ⇒ 悬垂 ✗
    //（实测：构造处一旦按正确口径归还自己那份、`T.__name__` 多读几次就段错误 ✓）。
    let fresh = |value: Option<NonNull<Header>>| -> Option<NonNull<Header>> {
        value.map(|value| {
            // SAFETY: value 由实例字典持有，存活；这里交出新的一份。
            unsafe { instance.incref_object(value.as_ptr()) };
            value
        })
    };
    match name {
        "__name__" => fresh(instance.dict_get(dict, NAME)),
        "__constraints__" => fresh(instance.dict_get(dict, CONSTRAINTS)),
        "__bound__" => {
            if let Some(value) = instance.dict_get(dict, BOUND) {
                // SAFETY: value 由实例字典持有，存活；这里交出新的一份。
                unsafe { instance.incref_object(value.as_ptr()) };
                return Some(value);
            }
            // **PEP 695 的惰性 bound** ✓：`[T: int]` 的 thunk 首次读才求值 ✓（随后缓存 ✓）。
            if let Some(thunk) = instance.dict_get(dict, BOUND_THUNK) {
                if let Some(value) = eval_format_thunk(instance, thunk) {
                    instance.dict_set(dict, BOUND, value);
                    return Some(value);
                }
            }
            let none = instance.singletons().none();
            // SAFETY: none 是单例 ⇒ 交出新的一份 ✓。
            unsafe { instance.incref_object(none.as_ptr()) };
            Some(none)
        }
        "__covariant__" => fresh(instance.dict_get(dict, COVARIANT)),
        "__contravariant__" => fresh(instance.dict_get(dict, CONTRAVARIANT)),
        "__infer_variance__" => fresh(instance.dict_get(dict, INFER_VARIANCE)),
        "__default__" => {
            // **PEP 695 的惰性 default** ✓（`[T = int]` ✓）：先看求值缓存 ✓（构造时那份 `DEFAULT`
            // 是 `NoDefault` 标记 ✓ ⇒ 不能拿它当"已经求值过" ✓）。
            if let Some(value) = instance.dict_get(dict, DEFAULT_VALUE) {
                return fresh(Some(value));
            }
            if let Some(thunk) = instance.dict_get(dict, DEFAULT_THUNK) {
                if let Some(value) = eval_format_thunk(instance, thunk) {
                    instance.dict_set(dict, DEFAULT_VALUE, value);
                    return Some(value);
                }
            }
            fresh(instance.dict_get(dict, DEFAULT))
        }
        "__type_params__" => {
            let empty = instance.new_tuple(Vec::new());
            Some(empty)
        }
        _ => None,
    }
}

/// `TypeAliasType` 实例字段（都放**实例字典** ✓ —— 不新增载荷类型 ✓）。
const ALIAS_NAME: &str = "_alias_name";
const ALIAS_PARAMS: &str = "_alias_type_params";
const ALIAS_THUNK: &str = "_alias_value_thunk";
const ALIAS_VALUE: &str = "_alias_value";

/// **造一个 `TypeAliasType` 实例** ✓（`INTRINSIC_TYPEALIAS` 用 ✓，task-24 收口 ③）：
/// 三个字段（名字／类型参数元组／值 thunk）全进实例字典 ✓；`__value__` **惰性** ✓
/// （读它才调 thunk(1) ✓，结果缓存 ✓ —— 照参照的 PEP 695 别名语义 ✓）。
pub fn typealias_from_parts(
    instance: &Instance,
    name: NonNull<Header>,
    params: NonNull<Header>,
    thunk: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    let Some(alias_type) = instance.type_named("TypeAliasType") else {
        return Err(instance.raise_builtin_error("RuntimeError", "TypeAliasType 还没有登记"));
    };
    let object = instance
        .alloc(AttributeObject::new(
            alias_type,
            core::cell::RefCell::new(Some(instance.new_dict())),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: 刚造的就是 `AttributeObject`。
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let Some(dict) = view.attributes() else {
        return Err(instance.raise_builtin_error("RuntimeError", "TypeAliasType 没有实例字典"));
    };
    instance.dict_set(dict, ALIAS_NAME, name);
    instance.dict_set(dict, ALIAS_PARAMS, params);
    instance.dict_set(dict, ALIAS_THUNK, thunk);
    Ok(object)
}

/// **`TypeAliasType` 的属性面** ✓（`getattr` 槽 ✓）：`__name__`／`__type_params__`／`__value__` ✓；
/// `__value__` 首次读时调 thunk（`format=1` ✓）并**缓存** ✓（口径与 `typevar_getattr` 一致：交回新引用 ✓）。
pub unsafe fn typealias_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 契约保证 ptr 是本类型的存活对象。
    let object = unsafe { NonNull::new_unchecked(ptr) };
    let view = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
    let dict = view.attributes()?;
    let fresh = |value: Option<NonNull<Header>>| -> Option<NonNull<Header>> {
        value.map(|value| {
            // SAFETY: value 由实例字典持有，存活；这里交出新的一份。
            unsafe { instance.incref_object(value.as_ptr()) };
            value
        })
    };
    match name {
        "__name__" => fresh(instance.dict_get(dict, ALIAS_NAME)),
        "__type_params__" => {
            // 非泛型别名编译期传的是 `None`（参照的 `LOAD_CONST None` ✓）⇒ 读出来要像参照一样
            // 给**空元组** ✓（实测 `type X = int` 的 `X.__type_params__ == ()` ✓）。
            match instance.dict_get(dict, ALIAS_PARAMS) {
                Some(params) if params == instance.singletons().none() => {
                    Some(instance.new_tuple(Vec::new()))
                }
                other => fresh(other),
            }
        }
        "__value__" => {
            if let Some(cached) = instance.dict_get(dict, ALIAS_VALUE) {
                return fresh(Some(cached));
            }
            let thunk = instance.dict_get(dict, ALIAS_THUNK)?;
            let format = instance.new_int(1);
            let outcome = crate::executor::call::call_value(instance, thunk, &[format], &[]);
            instance.release(format);
            let value = outcome.ok()?;
            // 缓存一份（`dict_set` **借用** ⇒ 自己那份仍归调用方 ✓）。
            instance.dict_set(dict, ALIAS_VALUE, value);
            Some(value)
        }
        _ => None,
    }
}

/// 调一个"格式 thunk"（`format=1` ✓）—— bound／default／别名的惰性求值共用 ✓；失败给 `None`
/// （`getattr` 槽只能回 `Option` ✓）。
fn eval_format_thunk(instance: &Instance, thunk: NonNull<Header>) -> Option<NonNull<Header>> {
    let format = instance.new_int(1);
    let outcome = crate::executor::call::call_value(instance, thunk, &[format], &[]);
    instance.release(format);
    outcome.ok()
}

/// **`INTRINSIC_TYPEVAR_WITH_BOUND`（`CALL_INTRINSIC_2` 的 oparg 2）** ✓（task-24 收口 ①）：
/// 造一个 TypeVar（名字来自 TOS-1 ✓），并把 `bound` 的 **thunk** 挂上（惰性 ✓）。
pub fn typevar_with_bound(
    instance: &Instance,
    class: NonNull<TypeObject>,
    name: NonNull<Header>,
    thunk: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    let Some(name_text) = instance.text_of(name) else {
        return Err(instance.raise_builtin_error("TypeError", "typevar() argument 'name' must be str"));
    };
    let name = name_text.to_owned();
    let typevar = finish_typevar(instance, class, &name, Vec::new(), None, false, false, false, None)?;
    // SAFETY: typevar 刚造出来、存活。
    let view = unsafe { &*typevar.as_ptr().cast::<AttributeObject>() };
    if let Some(dict) = view.attributes() {
        instance.dict_set(dict, BOUND_THUNK, thunk);
    }
    Ok(typevar)
}

/// **`INTRINSIC_SET_TYPEPARAM_DEFAULT`（oparg 5）** ✓：给类型参数挂 `default` 的 thunk（惰性 ✓），
/// 并把那个类型参数**原样交回** ✓（参照的 `COPY 1` 紧跟其后 ✓）。
pub fn typevar_set_default(
    instance: &Instance,
    typevar: NonNull<Header>,
    thunk: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: typevar 是调用方保证存活的对象。
    let view = unsafe { &*typevar.as_ptr().cast::<AttributeObject>() };
    let Some(dict) = view.attributes() else {
        return Err(instance.raise_builtin_error("RuntimeError", "TypeVar 没有实例字典"));
    };
    instance.dict_set(dict, DEFAULT_THUNK, thunk);
    Ok(instance.retain(typevar))
}

/// **`NoDefaultType` 的属性面** ✓（最小面：`repr` 由 `str`／`repr` 槽兜 ✓）。
pub unsafe fn no_default_getattr(
    _ptr: *mut Header,
    _name: &str,
    _instance: &Instance,
) -> Option<NonNull<Header>> {
    None
}

/// **`Generic.__init_subclass__`** ✓（task-24 步② 前置 ✓）：照参照口径把类的类型参数收进
/// `__parameters__` ✓ ——
/// ① **PEP 695**（`class C[T]` ✓）：类体序言写的 `__type_params__` 直接用 ✓；
/// ② **下标形态**（`class C(Generic[T])` ✓）：类创建时记下的 `__orig_bases__` 里，凡带 `__args__`
///    的（我们的 `GenericAlias` ✓）把参数收上来 ✓；
/// ③ 两者都没有 ⇒ 照参照报 **`Cannot inherit from plain Generic`** ✓。
pub fn generic_init_subclass_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(cls) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__init_subclass__ 没有绑定新类",
        ));
    };
    let Some(namespace) = instance.type_namespace(cls) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "Cannot inherit from plain Generic",
        ));
    };
    let mut params: Vec<NonNull<Header>> = Vec::new();
    if let Some(type_params) = instance.dict_get(namespace, "__type_params__") {
        params.extend(instance.tuple_items(type_params).unwrap_or_default());
    } else if let Some(original) = instance.dict_get(namespace, "__orig_bases__") {
        for base in instance.tuple_items(original).unwrap_or_default() {
            if let Ok(Some(args)) = instance.attribute_optional_of(base, "__args__") {
                // **`__args__` 可能不是元组** ✓（如实：本层的 `GenericAlias` 单参下标存的是**那个参数本身** ✓
                // —— 参照恒为元组 ✓）：元组就摊开 ✓，否则当成一个参数 ✓。
                match instance.tuple_items(args) {
                    Some(items) => params.extend(items),
                    None => params.push(args),
                }
            }
        }
    }
    if params.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "Cannot inherit from plain Generic",
        ));
    }
    let tuple = instance.new_tuple(
        params
            .into_iter()
            .map(|value| instance.retain(value))
            .collect(),
    );
    instance.dict_set(namespace, "__parameters__", tuple);
    instance.release(tuple);
    Ok(instance.retain(instance.singletons().none()))
}

/// **`_typing._idfunc(x)`** ✓：恒等函数 ✓（参照 `_idfunc(5) == 5` ✓）。
pub fn idfunc_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_idfunc() takes 1 positional argument but 0 were given",
        ));
    };
    Ok(instance.retain(value))
}

/// 造 `_typing` 用的两个类型（在引导期调用一次 ✓；与 `memoryview`／`bytearray` 同款 ✓）。
pub fn build_typing_types(instance: &Instance) -> (NonNull<TypeObject>, NonNull<TypeObject>) {
    let typevar_type = instance.alloc_type_raw(
        "TypeVar",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots()
            .with_new(typevar_new)
            .with_getattr(typevar_getattr),
    );
    let no_default_type = instance.alloc_type_raw(
        "NoDefaultType",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots()
            .with_new(no_default_new)
            .with_getattr(no_default_getattr),
    );
    // **`TypeAliasType`** ✓（task-24 收口 ③）：PEP 695 别名实例的类型 ✓ —— 只有 `getattr` 槽 ✓
    // （`__name__`／`__type_params__`／惰性的 `__value__` ✓）；*不*给 `new` 槽 ✓（用户不直接构造 ✓）。
    let _alias_type = instance.alloc_type_raw(
        "TypeAliasType",
        core::mem::size_of::<AttributeObject>(),
        AttributeObject::slots().with_getattr(typealias_getattr),
    );
    (typevar_type, no_default_type)
}

/// `_typing` 名字表里那些"**只占位**"的名字 ✓（轮 1 如实：先用普通类型把名字立住 ✓，
/// 语义在轮 3 的 `Lib/typing.py` 那条链上补 ✓）。
pub const PLACEHOLDER_NAMES: &[&str] = &[
    "TypeVarTuple",
    "ParamSpec",
    "ParamSpecArgs",
    "ParamSpecKwargs",
    "TypeAliasType",
    "Union",
];

/// 造一个占位类型 ✓（名字是类型 ✓，参照里它们也都是可调用的类／特殊对象 ✓）。
pub fn placeholder_type(instance: &Instance, name: &'static str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| instance.new_attribute_type(name))
}

/// 供 stdlib 模块装配用：把 `_idfunc` 包成 `builtin_function_or_method` ✓。
pub fn make_function(instance: &Instance, name: &'static str, handler: crate::NativeFn) -> NonNull<Header> {
    let function_type = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        function_type,
        name,
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}
