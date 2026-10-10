//! **`attribute` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `attribute_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`advance`、`builtin_type`、`exception_type`、`instance_attribute_set`、`instance_attributes`、`is_type_object`、`mounted_instance_dict`、`raise_builtin`、`release`、`str_matches_public`、`super_lookup` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{builtin_type, exception_type, instance_attribute_set, instance_attributes, mounted_instance_dict, raise_builtin, release, str_matches_public, super_lookup};
use crate::executor::Attribute;
use crate::builtin_objects::DictObject;
use crate::executor::ExecError;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::MethodObject;
use core::ptr::NonNull;


/// `type.mro()` ✓（第 722 轮）：交回**列表** ✓（参照口径 ✓；`__mro__` 那个属性给的是**元组** ✓）。
fn type_mro_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(object) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "mro() 缺少 self"));
    };
    // SAFETY: 绑定契约保证 `object` 是类型对象。
    let items: Vec<NonNull<Header>> = unsafe { object.cast::<crate::TypeObject>().as_ref() }
        .mro()
        .into_iter()
        .map(|base| base.cast::<Header>())
        .collect();
    Ok(instance.new_list(items))
}


/// `LOAD_ATTR` 一族的查找顺序（本层口径，写在注释里以免以后漂）：
///
/// ① 实例字典（**非数据描述符**会被它遮住：函数就是非数据描述符，故实例属性优先）
/// ② 类型字典（沿 MRO）：查到**函数**就是取方法，查到别的值就原样返回
/// ③ 都没有 ⇒ [`ExecError::AttributeNotFound`]
/// **`LOAD_ATTR` 的协议口径** ✓（第 712 轮）：先走属性通道 ✓，找不到再问类型的 `__getattr__` ✓
///（参照的规矩 ✓ —— 与内建 `getattr(obj, 名)` **一处真相** ✓）。回退拿到的值包成 `Attribute::Owned` ✓。
pub(crate) fn attribute_lookup_with_getattr(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Attribute, ExecError> {
    match attribute_lookup(instance, object, name) {
        Ok(found) => Ok(found),
        Err(error) => {
            // 守卫 ①：找 `__getattr__` 自身时不再回退 ✓（免得自递归 ✓）。
            if name == "__getattr__" {
                return Err(error);
            }
            // 守卫 ②：**类型 MRO 上真的没有 `__getattr__`** 就直接把**原始错误**抛回去 ✓ ——
            // 不能直接去 `attribute_lookup` ✗：那对**内建类型**会冒出它**自己**的
            // `'coroutine' object has no attribute '__getattr__'` ✗，把本该报的 `__next__` 顶掉 ✓
            //（实测 `crates/pyawa-core/tests/coroutines.rs:243` 就是这么红的 ✓）。
            if instance
                .type_lookup(unsafe { object.as_ref() }.ty(), "__getattr__")
                .is_none()
            {
                return Err(error);
            }
            match attribute_lookup(instance, object, "__getattr__") {
                Ok(Attribute::Method { function, this }) => {
                    let name_object = instance.new_str(name);
                    let found = crate::executor::call::call_callable(
                        instance,
                        function,
                        Some(this),
                        vec![name_object],
                        Vec::new(),
                        0,
                    )?;
                    Ok(Attribute::Owned(found))
                }
                _ => Err(error),
            }
        }
    }
}

pub(crate) fn attribute_lookup(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Attribute, ExecError> {
    // ① 类型自己的 `getattr` 槽（`OM-11`）——**内建类型的属性通道**，不许旁路
    // SAFETY: object 是存活对象。
    let object_type = unsafe { object.as_ref() }.ty();

    // ①.0 **`f.__annotations__`** 要**调用** `__annotate__`（有异常通道）⇒ 放在槽之前单独处理
    if name == "__annotations__" && object_type == builtin_type(instance, "function") {
        return crate::builtin::function::function_annotations(instance, object.as_ptr())
            .map(Attribute::Owned);
    }
    // ①.0.5 **内建类型的严格子类：自有字典优先于"继承来的" `getattr` 槽** ✓（第 271 轮 ✓，一处真相 ✓）：
    // `class D(dict)` 的 `D.__setitem__` 在**子类类型字典** ✓，而 `dict` 的 `getattr` 槽被继承 ✗
    // ⇒ 先前①先跑 ⇒ 实例拿到**基类 native** ✗（第 269 轮实测：`<bound method builtin_function_or_method of {}>` ✗，
    // 参照 `<bound method D.__setitem__ of {}>` ✓）。同一根卡住 `Lib/enum.py` 的 `EnumDict.__setitem__` ✗。
    //
    // **只对内建类型的严格子类生效** ✓（第 270 轮那一版放太宽 ✗ ⇒ 被 ③ `the_corpus_has_no_new_divergences`
    // 拦下 ✗）：普通类**一律走原路** ✓，既有 dunder 语义不受影响 ✓。
    if !instance.is_type_object(object) {
        let in_builtin_family = [
            "dict", "list", "tuple", "set", "frozenset", "deque", "int", "str", "float", "bytes",
            "bytearray", "bool",
        ]
        .iter()
        .any(|name| {
            instance.type_named(name).is_some_and(|base| {
                object_type != base && instance.is_subtype(object_type, base)
            })
        });
        if in_builtin_family {
            if let Some((owner, value)) = instance.type_lookup_owner(object_type, name) {
                if owner == object_type {
                    let value_type = unsafe { value.as_ref() }.ty();
                    let callable_like = Some(value_type) == instance.type_named("function")
                        || Some(value_type) == instance.type_named("builtin_function_or_method");
                    if callable_like {
                        return Ok(Attribute::Method {
                            function: value,
                            this: object,
                        });
                    }
                    return Ok(Attribute::Owned(instance.retain(value)));
                }
            }
        }
    }

    // SAFETY: object_type 由注册表持有。
    if let Some(slot) = unsafe { object_type.as_ref() }.slots().getattr {
        // SAFETY: 槽位由类型提供，契约见 `GetAttrFn`。
        if let Some(found) = unsafe { slot(object.as_ptr(), name, instance) } {
            return Ok(Attribute::Owned(found));
        }
    }

    // ①.1 **`super` 的查表** ✓（第 233 轮）：在 `type(__self__)` 的 MRO 上、**跳过定义类**之后找 ✓
    //（`ABCMeta.__new__` 里的 `super().__new__(…)` 正是这一支 ✓）。
    // **注意**：这里要与**类型对象**比 ✗ —— `builtin_type(instance, …)` 取的是**命名空间**里那个名字 ✓，
    // 而 `super` 这个名字**绑的是 native** ✓ ⇒ 拿它比会**永远不等** ✗（本轮实测踩到 ✓）。
    if Some(object_type) == instance.type_named("super") {
        if let Some(found) = super_lookup(instance, object, name)? {
            return Ok(found);
        }
    }

    // ①.2 **类型对象的 `__name__`／`__qualname__`**（第 133 轮）：参照里 `X.__name__` 是 `"X"` ✓
    //   （`_bootstrap.py` 的 `_object_name` 就用它 ✓）。函数对象那半边早有（`function_getattr` ✓）。
    // **`type.__mro__`** ✓（第 244 轮）：参照里它是 **tuple** ✓（`EnumType` 建类时要读它 ✓）。
    // MRO 在**类型注册时**就算好并存在类型对象里（`instance/registry.rs` 的 `set_bases` ✓）
    // ⇒ 这里只是把它取出来做成元组 ✓（**不要**用 `builtin_types.rs` 的静态 `mro` ✗，那是内建表 ✓）。
    if name == "__mro__" && instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象，且由注册表持有。
        let items: Vec<NonNull<Header>> = unsafe { object.cast::<crate::TypeObject>().as_ref() }
            .mro()
            .into_iter()
            .map(|base| base.cast::<Header>())
            .collect();
        return Ok(Attribute::Owned(instance.new_tuple(items)));
    }
    // **`type.__bases__`** ✓（第 722 轮）：参照里是**直接基类的 tuple** ✓ ——
    // `Lib/abc.py:171` 的 `for scls in cls.__bases__` 与 `Lib/email/_policybase.py:110/113`
    // 的 `cls.__bases__[0]` 都要它 ✗ ⇒ 先前 `'ABCMeta' object has no attribute '__bases__'`
    // 把 `email._policybase` 那一族（message／parser／policy／mime.* 十来个 ✓）压在下面 ✓。
    if name == "__bases__" && instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象，且由注册表持有。
        let items: Vec<NonNull<Header>> = unsafe { object.cast::<crate::TypeObject>().as_ref() }
            .bases()
            .into_iter()
            .map(|base| base.cast::<Header>())
            .collect();
        return Ok(Attribute::Owned(instance.new_tuple(items)));
    }
    // **`__class__`**（第 332 轮）：参照里**类型对象**的 `__class__` 是 `type` ✓
    // （`enum.py` 的 `_find_new_` 读它 ✓ ⇒ 那一族 118 个模块卡在这 ✗）。
    if name == "__class__" && instance.is_type_object(object) {
        if let Some(ty) = instance.type_named("type") {
            return Ok(Attribute::Owned(ty.cast::<Header>()));
        }
    }
    if (name == "__name__" || name == "__qualname__") && instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象。
        let info = unsafe { &*object.as_ptr().cast::<crate::TypeObject>() };
        return Ok(Attribute::Owned(instance.new_str(info.name())));
    }
    // **内建类型的 `__module__`**（第 310 轮）：`object.__module__` 参照给 `'builtins'` ✓ ——
    // 用户类的 `__module__` 由类体自己写进命名空间 ✓（走上面那条通用查找 ✓），内建类型没有 ✗
    // ⇒ 在这里兜底 ✓（**只兜类型对象** ✗：实例上 `(1).__module__` 参照是 `AttributeError` ✓）；
    // 实例那条由"查它自己的类"自然覆盖 ✓（`Lib/collections/__init__.py` 那一族 **31** 个模块卡在这 ✓）。
    if name == "__module__" && instance.is_type_object(object) {
        // 类型字典里自己写了就用它 ✓（用户类 ✓）；否则给 `builtins` ✓。
        if let Some(own) = instance.type_lookup(object.cast::<crate::TypeObject>(), "__module__") {
            return Ok(Attribute::Owned(instance.retain(own)));
        }
        return Ok(Attribute::Owned(instance.new_str("builtins")));
    }
    // ①.5 **`__dict__`**（实测：实例上它就是**那个字典本身**——同一个对象、透过它加属性立刻可见；
    // 没有实例字典的类型则落到最后那条 `AttributeError`，实测形如
    // `'S' object has no attribute '__dict__'`）。
    if name == "__dict__" {
        // **类型对象的 `__dict__`** ✓（第 181 轮，内建类型化 A）：用户类与内建类型都该有 ✓ ——
        //   `types.py` 的 `type(type.__dict__)`／`dict.__dict__['fromkeys']` 正是靠它 ✓。
        //   CPython 给的是 **mappingproxy** ✓，本层给**那个命名空间本身** ✗ ⇒ **已登记的偏差** ✓。
        // **用类型表里的 `type`** ✓（`builtin_type` 取的是**命名空间**里那个名字 ✗ —— 它可能是 native ✗）。
        let object_is_type = Some(unsafe { object.as_ref() }.ty()) == instance.type_named("type");
        if object_is_type {
            // 命名空间字典的**惰性挂载**在 [`crate::Instance::type_namespace`]（**一处真相** ✓）。
            if let Some(namespace) = instance.type_namespace(object) {
                // SAFETY: namespace 是存活对象；类型自己持一份，这里给调用方**再加一份** ✓。
                unsafe { instance.incref_object(namespace.as_ptr()) };
                return Ok(Attribute::Owned(namespace));
            }
        }
        if let Some(mapping) = mounted_instance_dict(instance, object) {
            // SAFETY: mapping 是存活对象，这里新增一份交给调用方。
            unsafe { instance.incref_object(mapping.as_ptr()) };
            return Ok(Attribute::Owned(mapping));
        }
    }

    // ② 实例字典
    if let Some(mapping) = instance_attributes(instance, object) {
        // **只有真的是 `dict` 才能按 `DictObject` 取项** ✓（第 185 轮实证 ✓）：`instance_attributes` 也可能给出
        // **内联属性对象**（布局不同 ✗）⇒ 照 `DictObject` 强转会**未对齐指针** ⇒ 直接 abort ✗
        //（实测就是 `executor.rs` 那行的 misaligned panic ✗）。不是 dict 就跳过这一支 ✓。
        let is_dict = instance.type_name(instance.type_of(mapping)) == "dict";
        let found = if is_dict {
            // SAFETY: 上面刚确认 mapping 的类型是 dict。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            dict.entries()
                .into_iter()
                .find(|(key, _)| str_matches_public(instance, *key, name))
        } else {
            None
        };
        if let Some((_, value)) = found {
            // **类对象不走这条** ✗（第 235 轮真 bug 修复 ✓）：类自己的名字空间归 **③** 管 ✓ ——
            // 那里会走**描述符协议** ✓。先前这里**直接给原值** ✗ ⇒ 只要**类级**取一个描述符就拿到
            // **描述符对象本身** ✗（`class R: __get__…` ＋ `class E: r = R()` ⇒ `E.r` 给 `<R object …>` ✗，
            // 参照给 `__get__(None, E)` 的结果 ✓；而 `E().r` 走另一条路 ✓、一直是好的 ✓）。
            if !instance.is_type_object(object) {
                return Ok(Attribute::Value(value));
            }
        }
    }

    // ③ 类型字典沿 MRO
    // **对象自己就是类型对象时，要查"它自己"的字典与 MRO** ✓（第 159 轮真 bug ✓）：
    //   `class C: x = 2` 之后 `C.x` 先前报 `AttributeError: 'type' object has no attribute 'x'` ✗
    //   —— 因为这里查的是对象**类型**的字典（对 `C` 来说是 `type` ✗），而不是 `C` 自己的名字空间 ✗。
    //   `Lib/` 里「类名.属性」遍地都是 ✓ ⇒ 这条必须对 ✓。
    let lookup_type = if instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象 ⇒ 头部就在同一地址上。
        Some(object.cast::<crate::TypeObject>())
    } else {
        Some(object_type)
    };
    if let Some(found) = lookup_type.and_then(|ty| instance.type_lookup(ty, name)) {
        // SAFETY: found 由类型字典持有。
        let found_ty = unsafe { found.as_ref() }.ty();
        if found_ty == builtin_type(instance, "function") {
            // **类访问 ⇒ 不绑定** ✗（第 192 轮真 bug 修复 ✓）：参照实测 `D.deco`（沿**类自己的 MRO**
            // 取到的普通函数 ✓）给的是 `<function D.deco at …>` ✓ —— **不绑定** ✓；本层先前一律绑成
            // `<bound method D.deco of <class 'D'>>` ✗ ⇒ 于是 `@D.deco` 那条**类装饰器**把宿主 `D`
            // 当成新类传了 ✗（`Lib/genericpath.py:194` 的 `@object.__new__` 也栽在这一步 ✓）。
            //
            // **注意** ✓：**元类那一层**（`Base.hello()` ✓）**仍要绑** ✓ —— 那里"类"是**元类型的实例** ✓，
            // 与这里"沿自己的 MRO 取**类属性**"是两码事 ✓（元类那条走的是 `attribute_lookup` 里
            // 第 ①.1 段的专用分支 ✓，不经此处 ✓）。
            if instance.is_type_object(object) {
                return Ok(Attribute::Value(found));
            }
            return Ok(Attribute::Method {
                function: found,
                this: object,
            });
        }
        // **`staticmethod`／`classmethod` 的取用**（第 303 轮修 `P3-25` ✗）：本层这两个包装对象
        // **自带 `__get__`** ✗（描述符协议那一格还没接 ✓）⇒ 先前落到最后那条
        // `Attribute::Value(found)` ⇒ `Q.s` 拿到的是**包装对象本身** ✗ ⇒ 调用报
        // `'staticmethod' object is not callable` ✗（参照正常 ✓）。这里按参照语义直接拆开 ✓：
        // - `staticmethod` ⇒ 交回**被包的函数** ✓（类访问与实例访问一样 ✓）；
        // - `classmethod` ⇒ 走内部"绑定方法"形态 ✓，`this` ＝ **那个类** ✓
        //   （`Q.c()` ⇒ `c(Q)` ✓，等价于参照的 `classmethod.__get__` ✓；
        //    第 298 轮 `__prepare__` 那处手工取 `ClassMethodObject::function` 的绕行可留可换 ✓）。
        if Some(found_ty) == instance.type_named("staticmethod") {
            // SAFETY: 类型身份刚确认 ⇒ `StaticMethodObject` 载荷；借出的函数由它持有。
            let inner =
                unsafe { &*found.as_ptr().cast::<crate::builtin_objects::StaticMethodObject>() }
                    .function();
            // SAFETY: 借用要变成调用方那份。
            unsafe { instance.incref_object(inner.as_ptr()) };
            return Ok(Attribute::Value(inner));
        }
        if Some(found_ty) == instance.type_named("classmethod") {
            // SAFETY: 类型身份刚确认 ⇒ `ClassMethodObject` 载荷。
            let inner =
                unsafe { &*found.as_ptr().cast::<crate::builtin_objects::ClassMethodObject>() }
                    .function();
            // `Attribute::Method.this` 与 `object` 同型 ⇒ 直接交（`target` 就是那两个之一 ✓）。
            return Ok(Attribute::Method {
                function: inner,
                this: if instance.is_type_object(object) {
                    object
                } else {
                    object_type.cast::<Header>()
                },
            });
        }
        // **绑定方法（`method`）的 dunder 转发给它的函数**（第 312 轮）：`C().m.__module__`
        // 参照给的是**函数**的那个值 ✓ ⇒ 这里把 `__module__`／`__qualname__`／`__name__` 一类
        // 直接转给 `MethodObject.function` ✓（`Lib/_collections_abc.py` 一族要它 ✓）。
        if instance.type_of(object) == builtin_type(instance, "method")
            && matches!(name, "__module__" | "__qualname__" | "__name__" | "__doc__" | "__code__" | "__defaults__")
        {
            // SAFETY: 类型身份刚确认 ⇒ `MethodObject` 载荷。
            let method = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::MethodObject>() };
            let found = instance.attribute_optional_of(method.function(), name)?;
            let Some(value) = found else {
                return Err(instance.raise_builtin_error(
                    "AttributeError",
                    &format!("'method' object has no attribute '{name}'"),
                ));
            };
            return Ok(Attribute::Owned(value));
        }
        // **类型字典里的原生方法在实例上要绑定**（第 309 轮）：`C().__hash__()` 先前拿到的是
        // **未绑定**的原生 ✗ ⇒ 调用报 `descriptor '__hash__' needs an argument` ✗。
        // 与函数那条同款 ✓（类型访问仍不绑定 ✓ —— `C.__hash__` 给的就是未绑定的那个 ✓）。
        if found_ty == builtin_type(instance, "builtin_function_or_method")
            && !instance.is_type_object(object)
        {
            return Ok(Attribute::Method {
                function: found,
                this: object,
            });
        }
        // **描述符协议 `__get__`** ✓（第 192 轮）：类型字典里找到的东西若**自带 `__get__`** ⇒ **调它** ✓
        //   （`C().x` ⇒ `__get__(实例, C)` ✓；`C.x` ⇒ `__get__(None, C)` ✓）。
        //   **函数不走这里** ✗（上面那支已处理绑定 ✓）；`builtin_function_or_method` 同理 ✗
        //   —— `Lib/` 里方法遍地都是 ✓，别把它们的绑定路径抢了 ✗。
        if found_ty != builtin_type(instance, "builtin_function_or_method") {
            if let Some(get) = instance.type_lookup(found_ty, "__get__") {
                // `self` 实参：实例给**实例本身** ✓；类型对象给 `None` ✓（参照口径 ✓）。
                let this = if instance.is_type_object(object) {
                    instance.retain(instance.singletons().none())
                } else {
                    instance.retain(object)
                };
                let owner = lookup_type.expect("上面判过 lookup_type 非空");
                // SAFETY: owner 是类型对象（上面的分支保证 ✓）。
                let owner_object = owner.cast::<Header>();
                instance.retain(owner_object);
                let result = crate::executor::call_callable(
                    instance,
                    get,
                    Some(found),
                    vec![this, owner_object],
                    Vec::new(),
                    0,
                )?;
                return Ok(Attribute::Value(result));
            }
        }
        return Ok(Attribute::Value(found));
    }

    // **`type.mro()`** ✓（第 722 轮）：参照里类对象有 `mro()`（返回**列表** ✓ —— 与 `__mro__`
    // 那个元组属性不同 ✓）。`Lib/email/_policybase.py:113` 的 `base.mro()` 与 `Lib/abc.py`
    // 一族都要它 ✗ ⇒ 先前 `'ABCMeta' object has no attribute 'mro'` 把 email 一族压在下面 ✓。
    if name == "mro" && instance.is_type_object(object) {
        let method_type = instance
            .type_named("builtin_function_or_method")
            .expect("builtin_function_or_method 在引导期已登记");
        let native = instance
            .alloc(crate::builtin_objects::BuiltinFunctionObject::new(
                method_type,
                "mro",
                core::cell::Cell::new(type_mro_native),
            ))
            .into_raw()
            .cast::<Header>();
        // SAFETY: 绑定方法要自己那份 `self`（`OM-16`）。
        unsafe { instance.incref_object(object.as_ptr()) };
        let bound = instance.alloc(MethodObject::new(
            instance.type_named("method").expect("method 已登记"),
            native,
            object,
        ));
        return Ok(Attribute::Owned(bound.into_raw().cast::<Header>()));
    }

    // **元类那一层** ✓（第 232 轮）：对象是**类**时，属性还要到**它的元类型**的 MRO 上找 ✓
    //（参照 `type.__getattribute__` 的顺序 ✓）—— `SomeABC.register(...)` 正是这一支 ✓。
    // 先前只在"**类自己的 MRO**"上找 ✗ ⇒ 报 `'ABCMeta' object has no attribute 'register'` ✗
    //（`_collections_abc.py:321` 的 `Iterator.register(bytearray_iterator)` 就卡在这 ✓）。
    if instance.is_type_object(object) {
        let object_type = instance.type_of(object);
        if let Some(found) = instance.type_lookup(object_type, name) {
            // 元类型上的**函数** ⇒ 绑到**类本身** ✓（`self` ＝ 那个类 ✓，与实例方法同款 ✓）。
            if instance.type_of(found) == builtin_type(instance, "function") {
                return Ok(Attribute::Method {
                    function: found,
                    this: object,
                });
            }
            return Ok(Attribute::Value(found));
        }
    }

    // **每个类型／实例都有 `__doc__`** ✓（第 288／722 轮）：类型字典里没有（＝没写文档串）时是 `None` ✓
    //（参照口径 ✓；`Lib/io.py:72` 一进门就读 `_io._IOBase.__doc__` ✗ ——
    // 先前这里直接抛 `'type' object has no attribute '__doc__'` ✗）。
    // **实例**上也要能落到**类型的** `__doc__` ✓（第 722 轮 ✓）：`'x'.__doc__` 参照给 `str` 的文档串 ✓，
    // 而 `Lib/email/_policybase.py:112` 的 `attr.__doc__`（`attr` 是**字符串类属性** ✓）正靠它 ✗
    // ⇒ 先前 `'str' object has no attribute '__doc__'` 把 `_policybase`／message／parser／policy／mime.* 压在下面 ✓。
    if name == "__doc__" {
        if instance.is_type_object(object) {
            return Ok(Attribute::Value(instance.singletons().none()));
        }
        // SAFETY: object 是存活对象。
        let doc_type = unsafe { object.as_ref() }.ty();
        if let Some(found) = instance.type_lookup(doc_type, "__doc__") {
            return Ok(Attribute::Owned(instance.retain(found)));
        }
        return Ok(Attribute::Value(instance.singletons().none()));
    }

    // 实测消息：`'int' object has no attribute 'nope'`（类型名取自对象的类型）
    // SAFETY: object 是存活对象。
    let type_name = unsafe { object.as_ref().ty().as_ref() }.name();
    // **属性缺失的现场诊断** ✓（第 345 轮，门控 `PYAWA_ATTR_MISS_DEBUG=1`）：打印名字／对象的类型名／
    // 当前 Python 现场 ✓ —— **从 Rust 侧打** ✓，因为本会话已两次见到"改 Python 语句就换墙"✗（观察者效应 ✓）。
    if crate::diag::flag("PYAWA_ATTR_MISS_DEBUG") {
        eprintln!("[attr_miss] name={name} type={type_name} site={}", instance.current_site());
    }
    Err(raise_builtin(
        instance,
        "AttributeError",
        &format!("'{type_name}' object has no attribute '{name}'"),
    ))
}

/// **`OM-11` 的 `getattr` 语义**（`pa_getfield` 用）：对象 ＋ 名字 ⇒ **新引用**。
pub fn attribute_read(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<NonNull<Header>, ExecError> {
    // 先走属性通道（`TS-44`：类型字典的同名 dunder 优先，实例字典不参与）
    match attribute_lookup(instance, object, name) {
        Ok(Attribute::Owned(value)) => Ok(value),
        Ok(Attribute::Value(value)) => {
            // SAFETY: 值由字典持有，存活。
            unsafe { instance.incref_object(value.as_ptr()) };
            Ok(value)
        }
        Ok(Attribute::Method { function, this }) => {
            // 取到方法：按 `OM-11` 给"函数 ＋ self"的绑定方法对象（与 `LOAD_ATTR` 无方法位同款）
            // SAFETY: 两者都存活。
            unsafe {
                instance.incref_object(function.as_ptr());
                instance.incref_object(this.as_ptr());
            }
            let bound = instance.alloc(MethodObject::new(
                builtin_type(instance, "method"),
                function,
                this,
            ));
            Ok(bound.into_raw().cast::<Header>())
        }
        Err(error) => Err(error),
    }
}

/// **取属性但不报错**：找不到（`AttributeError`）给 `None`，别的异常照上抛。
///
/// 给 `GET_ITER` 的 `__iter__` 探测、`advance` 的 `__next__` 探测用——那两处要区分
/// "没有这个 dunder"（走别的路径或报实测消息）与"用户代码自己抛了异常"（上抛）。
pub fn attribute_optional(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Option<NonNull<Header>>, ExecError> {
    match attribute_read(instance, object, name) {
        Ok(value) => Ok(Some(value)),
        Err(ExecError::Raised { exception }) => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            if instance.is_subtype(ty, exception_type(instance, "AttributeError")) {
                release(instance, exception);
                Ok(None)
            } else {
                Err(ExecError::Raised { exception })
            }
        }
        Err(other) => Err(other),
    }
}

/// **`OM-11` 的 `setattr` 语义**（`pa_setfield` 用）：对象 ＋ 名字 ＋ 值（值为**借用**）。
pub fn attribute_write(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    value: NonNull<Header>,
) -> Result<(), ExecError> {
    // SAFETY: 调用方保证 value 存活；属性表要自己那份。
    unsafe { instance.incref_object(value.as_ptr()) };
    instance_attribute_set(instance, object, name, value, 0)
}
