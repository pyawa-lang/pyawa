//! **`object` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `object_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`attribute_new`、`container_receiver` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::container_receiver;

/// `object.__eq__(self, other)`：与 `==` 同一处实现。
pub fn object_eq_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(left), Some(right)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__eq__ expected 2 arguments"));
    };
    Ok(instance.new_bool(crate::executor::values_equal_public(instance, left, *right)))
}
/// `object.__ne__(self, other)`：`__eq__` 取反（参照默认语义）。
pub fn object_ne_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(left), Some(right)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__ne__ expected 2 arguments"));
    };
    Ok(instance.new_bool(!crate::executor::values_equal_public(instance, left, *right)))
}
/// `object.__repr__(self)`：本层已有的默认 repr（`object_repr`）。
pub fn object_repr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, _) = container_receiver(bound, args);
    let Some(object) = receiver else {
        return Err(instance.raise_builtin_error("TypeError", "__repr__ expected 1 argument"));
    };
    let text = instance
        .object_repr(object)
        .unwrap_or_else(|_| "<无法取 repr>".to_owned());
    Ok(instance.new_str(&text))
}
/// `object.__setattr__(self, name, value)`：走 `attribute_write`（写入接管一份新引用 ✓）。
pub fn object_setattr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(object), Some(name), Some(value)) = (receiver, rest.first(), rest.get(1)) else {
        return Err(instance.raise_builtin_error("TypeError", "__setattr__ expected 3 arguments"));
    };
    let Some(text) = instance.text_of(*name).map(|text| text.to_owned()) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    crate::executor::attribute_write(instance, object, &text, *value)?;
    Ok(instance.retain(instance.singletons().none()))
}
/// `object.__getattribute__(self, name)`：走 `attribute_read`。
pub fn object_getattribute_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(object), Some(name)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__getattribute__ expected 2 arguments",
        ));
    };
    let Some(text) = instance.text_of(*name).map(|text| text.to_owned()) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    crate::executor::attribute_read(instance, object, &text)
}
/// **`object.__new__(cls, *args)`** ✓（第 193 轮）：参照里它是**独立的内建** ✓（不是 `type.__new__` ✓）。
///
/// 走**类自己的 `new` 槽** ✓（`attribute_new` 等 ✓ ⇒ **一处真相** ✓）；`args` 按参照的规矩处理 ✓
/// （见下面的"多给了实参"分支 ✓）。
pub fn object_new_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // **类可能落在 `bound` 槽里** ✓（第 193 轮实测 ✓）：参照的类装饰器 `@object.__new__` 发的是
    // `CALL arg=0` ✓，栈上是 `[装饰器, 新类]` ✓ ⇒ 按 CPython 的约定，那个"self／NULL 槽"**就是第一个
    // 位置实参** ✓ ⇒ 本层把它单独交给 `bound` ✓ ⇒ 所以这里**两处都要认** ✓（`args[0]` 或 `bound` ✓）。
    let Some(class_value) = args.first().copied().or(bound) else {
        return Err(instance.raise_builtin_error("TypeError", "object.__new__() 至少要 1 个实参"));
    };
    let Some(class) = instance.as_type(class_value) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "object.__new__() 的参数 1 必须是类型",
        ));
    };
    // 实参个数要**把 `bound` 那一份算进去** ✓。
    let given = args.len() + usize::from(bound.is_some());
    let extra = given > 1 || !kwargs.is_empty();
    if extra {
        // 参照的规矩 ✓：**多给了实参**时，若本类**覆写了 `__init__`** ⇒ 忽略它们 ✓；
        // 否则报 `TypeError: object.__new__() takes exactly one argument …` ✓。
        // `object` 命名空间里那个默认 `__init__` ✓（第 210 轮挂的 ✓）⇒ 与它不同 ⇒ 本类**覆写**了 ✓。
        let default_init = instance
            .type_named("object")
            .and_then(|ty| instance.type_namespace(ty.cast()))
            .and_then(|namespace| instance.dict_get(namespace, "__init__"));
        let init_overridden = instance
            .type_lookup(class, "__init__")
            .is_some_and(|found| Some(found) != default_init);
        if !init_overridden {
            let what = format!(
                "object.__new__() takes exactly one argument (the type to instantiate)，实际给了 {} 个",
                given.saturating_sub(1) + kwargs.len()
            );
            return Err(instance.raise_builtin_error("TypeError", &what));
        }
    }
    let slot = unsafe { class.as_ref() }.slots().new;
    let Some(slot) = slot else {
        let name = unsafe { class.as_ref() }.name().to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("cannot create '{name}' instances"),
        ));
    };
    // SAFETY: 槽位由类型提供（契约见 `NewFn` ✓）；额外实参**不再下传** ✓（参照的 `object.__new__` 本就不接它们 ✓）。
    unsafe { slot(class, &[], instance) }
}
/// **`object.__str__`／`object.__repr__`** ✓（第 210 轮）：默认就是 `<X object at 0x…>` ✓
/// （与 [`Instance::object_repr`] 同形 ✓ —— `Lib/types.py` 会取 `type(object.__str__)` ✓）。
pub fn object_text_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let this = bound.or_else(|| args.first().copied()).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "descriptor '__str__' needs an argument")
    })?;
    // **走"槽位路径"** ✓（`object_repr_native` ✓）——**不能**走 `object_repr` ✗：
    // 那条路会先查属性通道里的 `__repr__` 覆写 ✓ ⇒ 而 `object.__repr__` **就是**那个覆写 ⇒ **自递归** ✗
    //（实测：改之前探针直接**栈溢出** ✓）。
    let text = instance.object_repr_native(this)?;
    Ok(instance.new_str(&text))
}
/// **`object.__init__`** ✓（第 210 轮落地）：`Lib/types.py:50` 的 `type(object.__init__)` 要它 ✓。
pub fn object_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // **口径** ✓：默认实现**什么都不做**、返回 `None` ✓。
    // **未强制**参照的"多给实参就报 `TypeError`"那条细节 ✗ —— 本层实例化会把实例也放进
    // `args` ✓（`bound` 另有其一 ✓），按 `args.len()` 判会把 `C()` 这种无参构造误判成"多给了" ✗
    //（实测 ✓）。⇒ 如实简化 ✓（这条细节以后随调用约定一起对齐 ✓）。
    let _ = (bound, args);
    Ok(instance.retain(instance.singletons().none()))
}
/// **`object.__hash__`**（第 309 轮）：本层按**身份哈希**给一个稳定值 ✓。
///
/// 动因：`Lib/weakref.py:89` 的 `__hash__ = ref.__hash__` —— "在**类型对象**上取
/// `__hash__`" ✗ ⇒ 先前报 `AttributeError: 'type' object has no attribute '__hash__'` ✗
/// （那一族 **31** 个模块 ✓）。
///
/// **如实登记的偏差**：与参照各类型的哈希值**不一致**（本层的字典查键走 `values_equal`
/// 与 `dict_position` ✓，不靠这个值 ✓）；同一对象在同一进程里**恒定** ✓。
pub fn object_hash_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    use core::ptr::NonNull as _NonNull;
    let Some(_object) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__hash__' needs an argument"));
    };
    let _ = _NonNull::<Header>::dangling;
    // 指针右移几位再当成有符号数（去掉低位对齐的规律性 ✓）。
    let value = (_object.as_ptr() as isize) >> 4;
    Ok(instance.new_int(value as i64))
}
