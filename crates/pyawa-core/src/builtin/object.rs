//! **`object` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `object_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`attribute_new`、`container_receiver` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use core::ptr::NonNull;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::container_receiver;

/// `object.__subclasshook__(cls, subclass)` ✓（第 722 轮）：参照里默认返回 **`NotImplemented`** ✓
/// —— `Lib/_py_abc.py:123` 的 `ok = cls.__subclasshook__(subclass)` 正读它 ✗（先前 `object` 上
/// 没有这个名字 ⇒ `isinstance(x, ABC)` 直接 `AttributeError` ✓ ⇒ **ABC 虚拟子类注册**整条失效 ✗，
/// `Lib/random.py` 的 `sample` 就报 `Population must be a sequence` ✓）。
pub(crate) fn object_subclasshook_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    Ok(instance.retain(instance.not_implemented()))
}

/// `object.__init_subclass__(cls, **kwargs)` ✓（第 722 轮）：参照的**默认实现** ✓ ——
/// 空 kwargs 是空操作 ✓；给了关键字才报 `TypeError: __init_subclass__() takes no keyword arguments` ✓。
/// 动因：`Lib/random.py:225` 的 `def __init_subclass__(cls, /, **kwargs): ...
/// super().__init_subclass__(**kwargs)` 要**默认实现存在** ✓（先前 `object` 上没有这个名字 ✗）。
pub(crate) fn object_init_subclass_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "__init_subclass__() takes no keyword arguments",
        ));
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`types.MethodType(function, instance)` 的构造**（round 109）：`method` 类型原先**没有 new 槽** ✗
/// ⇒ `TypeError: cannot create 'method' instances` ✗ —— `Lib/contextlib.py:476` 的
/// `MethodType(cm_exit, cm)`（`ExitStack.push`／`_GeneratorContextManager.__enter__` ✓）与
/// `unittest.mock` 的 patch 收尾都要它 ✓ ⇒ 一个失败 mock 的清理抛错 ⇒ **mock 泄漏到后续测试**
/// （官方 `test_shutil` 的 `TestWhich` 连带失败、`TestArchives` 的 `_create_exit_wrapper` ✓）。
///
/// 口径照参照实测：两个实参 ✓；第一个必须**可调用** ✓（否则 `TypeError: first argument must be callable` ✓）。
#[allow(unsafe_code)]
pub unsafe fn method_type_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("method expected 2 arguments, got {}", args.len()),
        ));
    }
    let function = args[0];
    let this = args[1];
    // 可调用判据：已知可调用族 **或** 类型 MRO 上有 `__call__` ✓。
    // SAFETY: 实参由调用方保证存活。
    let function_type = unsafe { function.as_ref() }.ty();
    let known_callable = [
        "function",
        "builtin_function_or_method",
        "method",
        "type",
    ]
    .iter()
    .any(|name| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(function_type, base))
    });
    if !known_callable && instance.type_lookup(function_type, "__call__").is_none() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "first argument must be callable",
        ));
    }
    // SAFETY: 两个实参都存活；方法对象各持一份引用。
    unsafe {
        instance.incref_object(function.as_ptr());
        instance.incref_object(this.as_ptr());
    }
    Ok(instance
        .alloc(crate::MethodObject::new(class, function, this))
        .into_raw()
        .cast::<Header>())
}

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
    Ok(instance.new_bool(crate::executor::values::values_equal_public(instance, left, *right)))
}
/// `object.__ne__(self, other)`：**`==` 的结果取反** ✓（参照文档的默认语义 ✓ —— "delegates to
/// `__eq__()` and reverses the result" ✓）。
///
/// 口径（round 100）：**显式属性调用**（`c.__ne__(d)`／`object.__ne__(a, b)`）走这条 ✓ ——
/// 用完整相等协议（`equality_public`：用户 `__eq__` 派发＋容器逐元素＋结构兜底 ✓）。
/// `x != y` **运算符**不靠这条（`compare_public` 有"子类优先级／`NotImplemented`／身份兜底"
/// 的完整协议 ✓）；这里若返回 `NotImplemented` 会偏离参照（`object.__ne__(1, 2)` 参照给 **True**
/// ✓ —— 语料 `type_dunder_attributes` 钉着 ✗）。
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
    let equal = crate::executor::protocol::equality_public(instance, left, *right)?;
    Ok(instance.new_bool(!equal))
}

/// `object.__repr__(self)`：本层已有的默认 repr（`object_repr`）。

/// **`object.__lt__` 一族**（round 87）：参照里 `object.__dict__` 有四个富比较 dunder，默认都返回
/// `NotImplemented`（随后由解释器抛 `TypeError: '<' not supported…`）✓。本层这些行为此前只在
/// `COMPARE_OP` 里特判、类型字典里**没有名字** ⇒ `dir(<实例>)` 补了名字后 `getattr` 取不到 ⇒
/// `rlcompleter` 的 parens 判据不齐 ✗（`test_rlcompleter.test_attr_matches` ✓）。
pub fn object_comparison_not_implemented_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    Ok(instance.retain(instance.not_implemented()))
}

/// **`object.__sizeof__()`**（round 87）：默认给**头部大小**（与参照 `object().__sizeof__() == 16` 同口径 ✓）。
pub fn object_sizeof_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    Ok(instance.new_int(core::mem::size_of::<crate::Header>() as i64))
}

/// **`object.__getstate__()`**（round 87，PEP 574 一族）：参照 3.11+ 的默认实现 ——
/// 有实例字典就给（`dict`）；没有就给 `None` ✓。`copy`／`pickle` 的默认状态路径要用它 ✓。
pub fn object_getstate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(object) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__getstate__' needs an argument",
        ));
    };
    match crate::mounted_instance_dict(instance, object) {
        Some(mapping) => Ok(instance.retain(mapping)),
        None => Ok(instance.new_none()),
    }
}

/// **`object.__reduce__()`**（round 87）：委托 `__reduce_ex__(0)` ✓（参照的默认实现同口径 ✓；
/// `rlcompleter` 只要求它**可调用**，但把它做成真实现以免影响 `copy`／`pickle` 的默认路径 ✓）。
pub fn object_reduce_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(object) = bound else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__reduce__' needs an argument",
        ));
    };
    let protocol = instance.new_int(0);
    let result = object_reduce_ex_native(instance, Some(object), &[protocol], &[]);
    instance.release(protocol);
    result
}

/// `object.__reduce_ex__(protocol)` ✓（参照里它**存在** ✓：协议 0/1 走 `copyreg._reconstructor`、
/// 协议 ≥2 走 `copyreg.__newobj__` ✓）。本层如实给一个**形状合理**的结果
/// `(类型, 空参数元组)` ✓ —— 导入期只需要"取得到、不抛" ✓（`enum.py` 还要在类上**设置**它 ✓）。
pub fn object_reduce_ex_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, _) = container_receiver(bound, args);
    let Some(object) = receiver else {
        return Err(instance.raise_builtin_error("TypeError", "__reduce_ex__ 需要一个实参"));
    };
    // SAFETY: object 由调用方保证存活。
    let ty = unsafe { object.as_ref() }.ty();
    // **类型自己覆写的 `__reduce__` 优先**（round 111）：参照的 `object.__reduce_ex__` 先看
    // `type(obj).__reduce__` 是不是 object 那份，是覆写就**直接调它** ✓。本层的迭代器把
    // `__reduce__` 挂在 **getattr 槽**（`iterator_getattr`）上 ⇒ 这里要能取到它 ✓
    //（`pickle` 的 protocol 0/1 否则走 `save_global` ⇒ `Can't pickle <class 'list_iterator'>` ✗）。
    // 内建**迭代器**一族把 `__reduce__` 挂在 **getattr 槽**（`iterator_getattr`）上 ⇒ 类型字典里
    // 查不到"自己的 `__reduce__`" ✓ ⇒ 这里按类型族点名认（与 `iterator_type_for` 同一张名单 ✓）。
    let iterator_like = [
        "list_iterator",
        "tuple_iterator",
        "str_ascii_iterator",
        "dict_keyiterator",
        "set_iterator",
        "bytes_iterator",
        "bytearray_iterator",
        "list_reverseiterator",
        "reversed",
    ]
    .iter()
    .any(|name| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(ty, base))
    });
    let callable_like = instance
        .type_named("builtin_function_or_method")
        .is_some_and(|base| instance.is_subtype(ty, base))
        || instance
            .type_named("method")
            .is_some_and(|base| instance.is_subtype(ty, base));
    if iterator_like || callable_like {
        if let Some(result) = crate::executor::call::call_object_method(
            instance,
            object,
            "__reduce__",
            &[],
        )? {
            return Ok(result);
        }
    }
    // **`list` 族的"可重建"形状**（round 104）：参照协议 4 给
    // `(copyreg.__newobj__, (类,), None, 元素迭代器, None)` ✓ —— 先前一律给 `(类型, 空参数元组)` ✗
    // ⇒ `copy.copy(list 子类)`／`deepcopy` 得到**空列表** ✗（官方 `test_xml_dom_minicompat` 的
    // `test_nodelist_copy`／`test_nodelist_deepcopy` ✓）。`copy._reconstruct` 按 5 元组的第 4 项
    // 逐个 `append` ✓。
    let list_type = instance.type_named("list");
    if list_type.is_some_and(|base| instance.is_subtype(ty, base)) {
        if let Ok(Some(copyreg)) =
            crate::executor::import::import_through_bridge(instance, "copyreg")
        {
            if let Some(namespace) = crate::mounted_instance_dict(instance, copyreg) {
                if let Some(newobj) = instance.dict_get(namespace, "__newobj__") {
                    let class_value = instance.retain(ty.cast::<Header>());
                    let args = instance.new_tuple(vec![class_value]);
                    let state = instance.retain(instance.singletons().none());
                    let listiter = instance.iter_object(object)?;
                    let dictiter = instance.retain(instance.singletons().none());
                    return Ok(instance.new_tuple(vec![
                        instance.retain(newobj),
                        args,
                        state,
                        listiter,
                        dictiter,
                    ]));
                }
            }
        }
    }
    // **`dict` 族的"可重建"形状**（round 225 真 bug 修 ✗）：参照协议 ≥2 给
    // `(copyreg.__newobj__, (类,), None, None, iter(obj.items()))` ✓ —— `copy._reconstruct`
    // 按第 5 项**逐对写入** ✓。先前落到通用形状（第 5 项 `None`）✗ ⇒ `copy.copy(Counter(...))`
    // 得到**空 Counter** ✗（`test_collections.TestCounter.test_copying` ✓；`Counter` 是 dict 子类 ✓）。
    let dict_type = instance.type_named("dict");
    if dict_type.is_some_and(|base| instance.is_subtype(ty, base)) {
        if let Ok(Some(copyreg)) =
            crate::executor::import::import_through_bridge(instance, "copyreg")
        {
            if let Some(namespace) = crate::mounted_instance_dict(instance, copyreg) {
                if let Some(newobj) = instance.dict_get(namespace, "__newobj__") {
                    let class_value = instance.retain(ty.cast::<Header>());
                    let args = instance.new_tuple(vec![class_value]);
                    let state = instance.retain(instance.singletons().none());
                    let listiter = instance.retain(instance.singletons().none());
                    let items = crate::executor::call::call_object_method(
                        instance,
                        object,
                        "items",
                        &[],
                    )?;
                    let dictiter = match items {
                        Some(items) => {
                            let iterator = instance.iter_object(items)?;
                            instance.release(items);
                            iterator
                        }
                        None => instance.retain(instance.singletons().none()),
                    };
                    return Ok(instance.new_tuple(vec![
                        instance.retain(newobj),
                        args,
                        state,
                        listiter,
                        dictiter,
                    ]));
                }
            }
        }
    }
    // **`tuple` 族的"可重建"形状**（round 210 真 bug 修 ✗）：参照协议 ≥2 给
    // `(copyreg.__newobj__, (类, *self.__getnewargs__()), None, None, None)` ✓ —— 先前落到通用
    // 5 元组、参数只有 `(类,)` ✗ ⇒ `NT(1,2,3)` 的 `copy.copy` 报
    // `NT.__new__() missing 3 required positional arguments` ✗（`test_collections.TestNamedTuple` 一片 ✓）。
    let protocol = args
        .first()
        .copied()
        .and_then(|value| instance.int_of(value).and_then(|number| number.to_i64()))
        .unwrap_or(0);
    let tuple_type = instance.type_named("tuple");
    if protocol >= 2
        && tuple_type.is_some_and(|base| instance.is_subtype(ty, base))
        && Some(ty) != tuple_type
    {
        if let Some(newargs) =
            crate::executor::call::call_object_method(instance, object, "__getnewargs__", &[])?
        {
            if let Ok(Some(copyreg)) =
                crate::executor::import::import_through_bridge(instance, "copyreg")
            {
                if let Some(namespace) = crate::mounted_instance_dict(instance, copyreg) {
                    if let Some(newobj) = instance.dict_get(namespace, "__newobj__") {
                        let items = instance.tuple_items(newargs).unwrap_or_default();
                        // **先逐项 retain 再放 newargs**（`tuple_items` 给的是**借用** ✓）。
                        let mut values: Vec<NonNull<Header>> =
                            Vec::with_capacity(items.len() + 1);
                        values.push(instance.retain(ty.cast::<Header>()));
                        for item in items {
                            values.push(instance.retain(item));
                        }
                        // SAFETY: newargs 是新引用，本函数接手。
                        unsafe { instance.release_object(newargs.as_ptr()) };
                        let tuple_args = instance.new_tuple(values);
                        let empty = instance.retain(instance.singletons().none());
                        return Ok(instance.new_tuple(vec![
                            instance.retain(newobj),
                            tuple_args,
                            empty,
                            instance.retain(instance.singletons().none()),
                            instance.retain(instance.singletons().none()),
                        ]));
                    }
                }
            }
        }
    }
    // **通用实例的 protocol ≥ 2 形状**（round 109 真 bug 修）：参照给
    // `(copyreg.__newobj__, (类,), state[, None, None])` ✓ —— 先前只给 `(类型, ())` ✗
    // ⇒ `copy.copy`／`copy.deepcopy` **丢所有实例属性** ✗（`tarfile` 的 `addfile` 先 `copy.copy(TarInfo)`
    // ⇒ 写出的头里名字/字段全空 ✗，官方 `test_shutil` 的归档用例与 `test_tarfile` 一族 ✓）。
    let protocol = args
        .first()
        .copied()
        .and_then(|value| instance.int_of(value).and_then(|number| number.to_i64()))
        .unwrap_or(0);
    if protocol >= 2 {
        if let Ok(Some(copyreg)) =
            crate::executor::import::import_through_bridge(instance, "copyreg")
        {
            if let Some(namespace) = crate::mounted_instance_dict(instance, copyreg) {
                if let Some(newobj) = instance.dict_get(namespace, "__newobj__") {
                    let class_value = instance.retain(ty.cast::<Header>());
                    let tuple_args = instance.new_tuple(vec![class_value]);
                    // **state**：实例属性字典的**副本**（参照给新 dict ✓）；没有属性 ⇒ `None` ✓。
                    let state = match crate::executor::protocol::instance_attributes(instance, object)
                        .and_then(|attributes| instance.dict_entries(attributes))
                    {
                        Some(entries) if !entries.is_empty() => {
                            let state = instance.new_dict();
                            for (key, value) in entries {
                                instance.dict_insert_raw(
                                    state,
                                    instance.retain(key),
                                    instance.retain(value),
                                );
                            }
                            instance.retain(state)
                        }
                        _ => instance.retain(instance.singletons().none()),
                    };
                    let empty = instance.retain(instance.singletons().none());
                    return Ok(instance.new_tuple(vec![
                        instance.retain(newobj),
                        tuple_args,
                        state,
                        empty,
                        instance.retain(instance.singletons().none()),
                    ]));
                }
            }
        }
    }
    // **协议 0／1 的形状**（round 116 真缺口）：参照给
    // `(copyreg._reconstructor, (类, object, None), state)` ✓ —— 先前落到"`(类型, 空参数元组)`" ✗
    // ⇒ 协议 0／1 的 pickle／copy **丢全部实例属性** ✗（`test_userlist.test_pickle` 的
    // `pickle.dumps(lst, 0/1)` 反序列化后 `[] != [4,5,6,7]` ✓）。
    {
        if let Ok(Some(copyreg)) = crate::executor::import::import_through_bridge(instance, "copyreg") {
            if let Some(namespace) = crate::mounted_instance_dict(instance, copyreg) {
                if let Some(reconstructor) = instance.dict_get(namespace, "_reconstructor") {
                    let class_value = instance.retain(ty.cast::<Header>());
                    // **`tuple` 子类**（round 211）：参照给 `(cls, tuple, self.__getnewargs__())` ✓ ——
                    // `_reconstructor` 再 `tuple.__new__(cls, state)` 重建成员 ✓（本层这条静态调用已成立 ✓）。
                    let tuple_base = instance.type_named("tuple");
                    let is_tuple_subclass = tuple_base
                        .is_some_and(|base| instance.is_subtype(ty, base) && Some(ty) != tuple_base);
                    let base_value = if is_tuple_subclass {
                        instance.retain(tuple_base.expect("刚判过").cast::<Header>())
                    } else {
                        instance
                            .type_named("object")
                            .map(|object_ty| instance.retain(object_ty.cast::<Header>()))
                            .unwrap_or_else(|| instance.retain(instance.singletons().none()))
                    };
                    // 第三参数：tuple 子类给 `__getnewargs__()` 结果；其余给 `None` ✓。
                    let reconstructor_state = if is_tuple_subclass {
                        match crate::executor::call::call_object_method(
                            instance,
                            object,
                            "__getnewargs__",
                            &[],
                        )? {
                            Some(newargs) => newargs,
                            None => instance.retain(instance.singletons().none()),
                        }
                    } else {
                        instance.retain(instance.singletons().none())
                    };
                    let reconstructor_args = instance.new_tuple(vec![
                        class_value,
                        base_value,
                        reconstructor_state,
                    ]);
                    let state = if is_tuple_subclass {
                        instance.retain(instance.singletons().none())
                    } else {
                        match crate::executor::protocol::instance_attributes(instance, object)
                            .and_then(|attributes| instance.dict_entries(attributes))
                        {
                            Some(entries) if !entries.is_empty() => {
                                let state = instance.new_dict();
                                for (key, value) in entries {
                                    instance.dict_insert_raw(
                                        state,
                                        instance.retain(key),
                                        instance.retain(value),
                                    );
                                }
                                instance.retain(state)
                            }
                            _ => instance.retain(instance.singletons().none()),
                        }
                    };
                    return Ok(instance.new_tuple(vec![
                        instance.retain(reconstructor),
                        reconstructor_args,
                        state,
                    ]));
                }
            }
        }
    }
    // 类型对象也是对象（本层如此登记）⇒ 直接当元组第一项 ✓。
    let ty_object = ty.cast::<Header>();
    Ok(instance.new_tuple(vec![ty_object, instance.new_tuple(Vec::new())]))
}

/// **直接调用"接收者类型的 repr 槽"**（round 114）：类型级 `__repr__` 包装器用它 ✓ ——
/// 参照里 `set.__repr__(子类实例)` 调的是 **C 的 slot** ✓（**不**走子类的 `__repr__` 覆写 ✓）——
/// 先前包装器用 [`object_repr_native`]（会走覆写通道 ✗）⇒ 子类的
/// `def __repr__(self): return set.__repr__(self)` **自我循环** ✗（`test_pprint.test_set_reprs`：
/// 给 `<无法取 repr>` ✗）。本函数只碰槽位 ✓，语义与 `PyObject_Repr` 的 `tp_repr` 直调一致 ✓。
pub fn object_repr_slot_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, _) = container_receiver(bound, args);
    let Some(object) = receiver else {
        return Err(instance.raise_builtin_error("TypeError", "__repr__ expected 1 argument"));
    };
    // SAFETY: receiver 是存活对象。
    let ty = unsafe { object.as_ref() }.ty();
    // SAFETY: ty 由注册表持有。
    let slot = unsafe { ty.as_ref() }.slots().repr;
    let Some(slot) = slot else {
        // **没有 repr 槽的类型**（用户类实例等）⇒ 走 `object.__repr__` 的默认文本 ✓
        // （`object.__repr__(C())` 参照给 `<C object at 0x…>` ✓；先前报 `type has no repr slot` ✗
        // —— 官方语料 `type_dunder_attributes` 当场抓住 ✓）。
        return Ok(instance.new_str(&instance.object_repr(object)?));
    };
    // SAFETY: 槽位契约（ptr 是本类存活实例；新引用返回值由本函数转交调用方）。
    let text = unsafe { slot(object.as_ptr(), instance) }?;
    Ok(instance.new_str(&text))
}

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
    crate::executor::attribute::attribute_write(instance, object, &text, *value)?;
    Ok(instance.retain(instance.singletons().none()))
}
/// `object.__delattr__(self, name)`（round 95）：走 `instance_attribute_delete` ✓。
///
/// 动因：`unittest.mock` 的 `NonCallableMock.__delattr__` 在"非魔法名"分支里 `super().__delattr__(name)`
/// ✗ ⇒ `object` 类型字典里没有 `__delattr__` 就 `AttributeError: 'super' object has no attribute '__delattr__'` ✗
/// （官方 `test__colorize` 的 mock 收尾 ✓）。注意 `instance_attribute_delete` 里"用户 `__delattr__` 优先"
/// 的判据会**跳过 `object` 的默认实现**（防递归 ✓）⇒ 本函数直接进入删实例字典那条路 ✓。
pub fn object_delattr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let (receiver, rest) = container_receiver(bound, args);
    let (Some(object), Some(name)) = (receiver, rest.first()) else {
        return Err(instance.raise_builtin_error("TypeError", "__delattr__ expected 2 arguments"));
    };
    let Some(text) = instance.text_of(*name).map(|text| text.to_owned()) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    crate::executor::protocol::instance_attribute_delete_default(instance, object, &text)?;
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
    crate::executor::attribute::attribute_read(instance, object, &text)
}
/// **内建类型的 `__iter__` 属性面** ✓（round 54）：参照里 `d.__iter__()`／`s.__iter__()`
/// 都可显式取用（`dict`／`list`／`tuple`／`str`／`bytes`／`set`／`range` …）。本层先前只在
/// **迭代协议**里内建处理 ⇒ 显式 `d.__iter__()` 报
/// `AttributeError: 'dict' object has no attribute '__iter__'` ✗ ——
/// `test_unpack_ex` 的 `CrazyDict.__iter__` 里 `self.d.__iter__()` 正是它 ✓。
pub(crate) fn builtin_iter_dunder_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound.or_else(|| args.first().copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "descriptor '__iter__' of 'object' needs an argument",
        ));
    };
    instance.iter_object(this)
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
    // **参照语义** ✓（第 412 轮，task-22 收尾 ✓）：`object.__str__` **等价** `object.__repr__` ✓
    // ⇒ 要先看**属性通道里的 `__repr__` 覆写** ✓（真 enum 的 `_simple_enum` 类体里
    // `__str__ = object.__str__` 就是要这一跳：成员 `str()` 应给 **成员 repr** `<F.A: 2>` ✓，
    // 而 `object_repr_native` 只走**槽** ⇒ IntFlag 成员落到 int 的 repr 槽 ⇒ 给 `'2'` ✗）。
    // **防自递归** ✓：`object.__repr__` 自己就是本函数（`object` 类型字典里的那个 native ✓）——
    // 若命中的覆写**就是**它，跳过（否则 `__repr__ = object.__repr__` 的类会无限递归 ✗）。
    // SAFETY: this 由调用方保证存活。
    let target_type = unsafe { this.as_ref() }.ty();
    if let Some((_owner, found)) = instance.type_lookup_owner(target_type, "__repr__") {
        let is_object_repr = instance.type_name(instance.type_of(found))
            == "builtin_function_or_method"
            && {
                // SAFETY: 类型身份刚确认 ⇒ `BuiltinFunctionObject` 载荷。
                let native = unsafe {
                    &*found.as_ptr().cast::<crate::builtin_objects::BuiltinFunctionObject>()
                };
                core::ptr::fn_addr_eq(native.function(), object_text_native as crate::NativeFn)
            };
        if !is_object_repr {
            if let Some(result) =
                crate::executor::call::call_object_method(instance, this, "__repr__", &[])?
            {
                // SAFETY: result 是新引用，存活。
                let text = instance.text_value(result);
                // SAFETY: result 是本次调用的新引用，这里归还。
                unsafe { instance.release_object(result.as_ptr()) };
                if let Some(text) = text {
                    return Ok(instance.new_str(&text));
                }
            }
        }
    }
    let text = instance.object_repr_native(this)?;
    Ok(instance.new_str(&text))
}
/// **`object.__init__`** ✓（第 210 轮落地）：`Lib/types.py:50` 的 `type(object.__init__)` 要它 ✓。
/// **`object.__dir__()`**（round 87）：参照里 `object.__dict__` 有 `__dir__` ⇒ `dir(<实例>)` 的
/// 补全走 `getattr(None, '__dir__')` 时拿到的是**绑定方法**（`rlcompleter` 靠它判断可调用 ✓）。
/// 实现直接复用 `dir_new`（一处真相 ✓）。
pub fn object_dir_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "descriptor '__dir__' needs an argument"));
    };
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("__dir__() takes no arguments ({} given)", args.len()),
        ));
    }
    crate::builtin_objects::dir_new(instance, None, &[this], &[])
}

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
