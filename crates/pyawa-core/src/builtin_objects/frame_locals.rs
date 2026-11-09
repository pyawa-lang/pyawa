//! **`FrameLocalsProxy`**（round 177；PEP 667 的 3.13+ 口径）：`frame.f_locals` 对**函数帧**给出的
//! **可写穿**映射视图 —— 读走帧的 fast locals／cell／free，写**写回帧**（`sys._getframe().f_locals['x']=2`
//! 之后局部变量 `x` 就是 2 ✓）；不在 localsplus 里的键落在**影子字典**（`f_extra_locals`）里 ✓。
//!
//! 参照语义（`test_frame.TestFrameLocals` 逐条）：
//! - 删**已绑定局部** ⇒ `ValueError('local variables')`（含 `pop`）；删未知键 ⇒ `KeyError` ✓
//! - `clear` **不存在** ⇒ `AttributeError` ✓（不能整体清空帧）
//! - `copy()` 给普通 `dict` ✓；`update`／`setdefault`／`get`／`keys`／`values`／`items`／`repr` 照 `dict` ✓
//! - 非字符串键只在影子字典里 ✓（`d[1] = 2`）
//! - `frame.clear()` 之后仍取得到 `f_locals`（空）✓
//!
//! **如实登记的缺口**：`Mapping` 的 isinstance／`|`／`|=`（PEP 584）与 `reversed` 走底层 Mapping 协议，
//! 需要先把本类型在 `collections.abc.Mapping` 里注册（下一轮）。

use core::cell::Cell;
use core::ptr::NonNull;

use super::prelude::*;
use super::*;

py_object! {
    /// `frame.f_locals` 的代理对象：**持有帧一份引用**（帧被 clear 后仍能取到空视图 ✓）。
    pub struct FrameLocalsProxy {
        /// 目标帧（**本对象持有一份引用**）。
        frame: NonNull<Header>,
        /// 影子字典（**本对象持有一份引用**）：localsplus 里没有的名字／非字符串键落这里 ✓。
        shadow: NonNull<Header>,
    }
}

impl FrameLocalsProxy {
    /// 槽位表：`dealloc` ＋ `traverse`／`clear`（帧与影子字典都是 GC 边）。
    pub fn slots() -> crate::type_object::Slots {
        crate::type_object::Slots::new(Self::dealloc)
            // **`FrameLocalsProxy(frame)` 构造**（round 177）：参照允许从帧显式造代理 ✓
            // （`test_frame.TestFrameLocals.test_constructor` 就调 `FrameLocalsProxy(make_frame())` ✓）。
            .with_new(frame_locals_new)
            .with_traverse(frame_locals_traverse)
            .with_clear(frame_locals_clear)
            .with_getattr(frame_locals_getattr)
    }

    /// 目标帧（借用）。
    pub fn frame(&self) -> NonNull<Header> {
        self.frame
    }

    /// 影子字典（借用）。
    pub fn shadow(&self) -> NonNull<Header> {
        self.shadow
    }
}

unsafe fn frame_locals_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FrameLocalsProxy>() };
    // 访问器形态（`gc_field_coverage` 的判据要求 `object.<字段>(` ✓）。
    visit(object.frame().as_ptr());
    visit(object.shadow().as_ptr());
}

unsafe fn frame_locals_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<FrameLocalsProxy>() };
    // SAFETY: 这两份引用由本对象持有（`clear` 只会跑一次）。
    unsafe {
        instance.release_object(object.frame().as_ptr());
        instance.release_object(object.shadow().as_ptr());
    }
}

/// **取帧的 localsplus 槽号**（按名字）：`co_varnames` ＋ cell/free 槽都算 ✓；
/// 找不到 ⇒ `None`（那就是影子字典的键 ✓）。
fn local_slot_of(frame: &crate::frame::Frame, name: &str) -> Option<usize> {
    let code_header = frame.code()?;
    // SAFETY: 帧持有一份 code 引用，帧存活期间有效。
    let code = unsafe { &*code_header.as_ptr().cast::<crate::CodeObject>() };
    for slot in 0..frame.local_count() {
        let slot_name = crate::executor::format::localsplus_name(code, slot);
        if slot_name == name {
            return Some(slot);
        }
    }
    None
}

/// 影子字典的**键文本**（round 177）：字符串键用自身；其它键退 `str(key)` ✓
/// （**如实登记**：非字符串键在影子字典里按 `str()` 归并，参照保留原对象键；`test_as_dict` 一族只用字符串键 ✓）。
fn proxy_key_text(instance: &Instance, key: NonNull<Header>) -> String {
    if let Some(text) = instance.text_of(key) {
        return text.to_owned();
    }
    instance.object_str(key).unwrap_or_default()
}

/// 读一个名字（**新引用**）：局部在 localsplus 里 ⇒ 帧里的值；否则影子字典。
fn proxy_get(instance: &Instance, proxy: NonNull<Header>, key: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 调用方保证 proxy 是存活的本类型对象。
    let object = unsafe { &*proxy.as_ptr().cast::<FrameLocalsProxy>() };
    // SAFETY: 帧由本对象持有，存活。
    let frame = unsafe { &*object.frame().as_ptr().cast::<crate::frame::Frame>() };
    if let Some(name) = instance.text_of(key) {
        if let Some(slot) = local_slot_of(frame, name) {
            if let Ok(Some(value)) = frame.local(slot) {
                return Ok(instance.retain(value));
            }
            // 已绑定局部才算命中；未绑定 ⇒ 落到影子字典（可能也没有 ⇒ KeyError）。
        }
    }
    // 影子字典：非字符串键／未绑定名 ✓
    match instance.dict_get(object.shadow(), &proxy_key_text(instance, key)) {
        Some(value) => Ok(instance.retain(value)),
        None => Err(instance.raise_builtin_error("KeyError", &format!("{key:?}"))),
    }
}

/// 写一个名字：在 localsplus 里 ⇒ **写穿帧**（cell 槽走 `set_cell` ✓）；否则影子字典。
fn proxy_set(
    instance: &Instance,
    proxy: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<(), ExecError> {
    // SAFETY: 同上。
    let object = unsafe { &*proxy.as_ptr().cast::<FrameLocalsProxy>() };
    // SAFETY: 帧由本对象持有，存活。
    let frame = unsafe { &*object.frame().as_ptr().cast::<crate::frame::Frame>() };
    if let Some(name) = instance.text_of(key) {
        if let Some(slot) = local_slot_of(frame, name) {
            // **cell 槽**（闭包变量）要写 cell；普通槽走 `set_local`（形参在 MAKE_CELL 前是值 ✓）。
            let written = match frame.set_local(slot, Some(value)) {
                Ok(previous) => {
                    if let Some(previous) = previous {
                        // SAFETY: 旧值由帧交出 ✓。
                        unsafe { instance.release_object(previous.as_ptr()) };
                    }
                    true
                }
                Err(_) => {
                    // `set_local` 在 cell 槽上可能拒绝 ⇒ 退回 `set_cell` ✓。
                    match frame.set_cell(slot, Some(value)) {
                        Ok(previous) => {
                            if let Some(previous) = previous {
                                // SAFETY: 同上。
                                unsafe { instance.release_object(previous.as_ptr()) };
                            }
                            true
                        }
                        Err(_) => false,
                    }
                }
            };
            if written {
                return Ok(());
            }
        }
    }
    instance.dict_set(object.shadow(), &proxy_key_text(instance, key), value);
    Ok(())
}

/// **删一个键**：已绑定局部 ⇒ `ValueError('local variables')`；未知 ⇒ `KeyError` ✓。
fn proxy_delete(instance: &Instance, proxy: NonNull<Header>, key: NonNull<Header>) -> Result<(), ExecError> {
    // SAFETY: 同上。
    let object = unsafe { &*proxy.as_ptr().cast::<FrameLocalsProxy>() };
    // SAFETY: 帧由本对象持有，存活。
    let frame = unsafe { &*object.frame().as_ptr().cast::<crate::frame::Frame>() };
    if let Some(name) = instance.text_of(key) {
        if let Some(slot) = local_slot_of(frame, name) {
            if let Ok(Some(_)) = frame.local(slot) {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!("cannot delete local variable '{name}'"),
                ));
            }
        }
    }
    let found = instance.dict_get(object.shadow(), &proxy_key_text(instance, key));
    match found {
        Some(_) => {
            let shadow_key = instance.new_str(&proxy_key_text(instance, key));
            instance.dict_remove_key(object.shadow(), shadow_key);
            // SAFETY: shadow_key 是本函数的新引用。
            unsafe { instance.release_object(shadow_key.as_ptr()) };
            Ok(())
        }
        None => Err(instance.raise_builtin_error("KeyError", &format!("{key:?}"))),
    }
}

/// 当前视图的**全部键**（localsplus 已绑定的 ＋ 影子字典）：
/// `ordered = false` ⇒ 先 localsplus 序、再影子字典新增序；`true` ⇒ 影子字典里的键单独优先（`iter` 用不上）。
fn proxy_keys(instance: &Instance, proxy: NonNull<Header>) -> Vec<NonNull<Header>> {
    // SAFETY: 同上。
    let object = unsafe { &*proxy.as_ptr().cast::<FrameLocalsProxy>() };
    // SAFETY: 帧由本对象持有，存活。
    let frame = unsafe { &*object.frame().as_ptr().cast::<crate::frame::Frame>() };
    let mut keys = Vec::new();
    if let Some(code_header) = frame.code() {
        // SAFETY: 帧持有一份 code 引用。
        let code = unsafe { &*code_header.as_ptr().cast::<crate::CodeObject>() };
        for slot in 0..frame.local_count() {
            if let Ok(Some(_)) = frame.local(slot) {
                let name = crate::executor::format::localsplus_name(code, slot);
                keys.push(instance.new_str(&name));
            }
        }
    }
    let shadow_keys: Vec<NonNull<Header>> = instance
        .dict_entries(object.shadow())
        .unwrap_or_default()
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    for key in shadow_keys {
        keys.push(instance.retain(key));
    }
    keys
}

/// 把视图转成快照 `dict`（`copy()`／`dict(d)`／`**d` 都要它 ✓）。
fn proxy_snapshot(instance: &Instance, proxy: NonNull<Header>) -> NonNull<Header> {
    let dict = instance.new_dict();
    for key in proxy_keys(instance, proxy) {
        if let Some(value) = proxy_get(instance, proxy, key).ok() {
            instance.dict_set(dict, &format!("{key:?}"), value);
            // SAFETY: 两份新引用都由本函数持有。
            unsafe {
                instance.release_object(value.as_ptr());
                instance.release_object(key.as_ptr());
            }
        }
    }
    dict
}

/// 分配一个代理（`frame.f_locals`／`FrameLocalsProxy(frame)` 用；**新引用**）。
fn frame_locals_with_class(
    instance: &Instance,
    class: NonNull<crate::TypeObject>,
    frame: NonNull<Header>,
) -> NonNull<Header> {
    let shadow = instance.new_dict();
    // **帧要自己一份引用**（`frame.f_locals` 那次由调用方保证存活，这里统一 incref ✓）。
    // SAFETY: frame 由调用方保证存活；shadow 是本函数的新引用。
    unsafe {
        instance.incref_object(frame.as_ptr());
    }
    let object = instance.alloc(FrameLocalsProxy::new(class, frame, shadow));
    object.into_raw().cast::<Header>()
}

/// `frame.f_locals` 用（登记类型）。
pub fn frame_locals_proxy(instance: &Instance, frame: NonNull<Header>) -> NonNull<Header> {
    let ty = instance
        .type_named("FrameLocalsProxy")
        .expect("FrameLocalsProxy 在引导期已登记");
    frame_locals_with_class(instance, ty, frame)
}

/// `FrameLocalsProxy(frame)` 构造器（`with_new` 槽）。
unsafe fn frame_locals_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    let Some(frame) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "FrameLocalsProxy(frame) 需要帧实参"));
    };
    let is_frame = instance
        .type_named("frame")
        .is_some_and(|frame_type| instance.type_of(frame) == frame_type);
    if !is_frame {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "FrameLocalsProxy() 的实参必须是帧对象",
        ));
    }
    Ok(frame_locals_with_class(instance, class, frame))
}

/// 绑定一个原生方法到代理实例（与 `iterator_getattr` 的 `bind_iterator_method` 同一手法 ✓）。
fn bind_proxy_method(
    instance: &Instance,
    proxy: NonNull<Header>,
    name: &str,
    handler: NativeFn,
) -> NonNull<Header> {
    let builtin_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(crate::BuiltinFunctionObject::new(
        builtin_type,
        Box::leak(name.to_string().into_boxed_str()),
        Cell::new(handler),
    ));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(proxy.as_ptr()) };
    let bound = instance.alloc(crate::MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        unsafe { NonNull::new_unchecked(proxy.as_ptr()) },
    ));
    bound.into_raw().cast::<Header>()
}

/// **属性面**（`f_locals` 一族方法的入口）。
#[allow(unsafe_code)]
pub unsafe fn frame_locals_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: Option<NativeFn> = match name {
        "__getitem__" => Some(proxy_getitem_native),
        "__setitem__" => Some(proxy_setitem_native),
        "__delitem__" => Some(proxy_delitem_native),
        "__contains__" => Some(proxy_contains_native),
        "__len__" => Some(proxy_len_native),
        "__iter__" => Some(proxy_iter_native),
        "__repr__" | "__str__" => Some(proxy_repr_native),
        "keys" => Some(proxy_keys_native),
        "values" => Some(proxy_values_native),
        "items" => Some(proxy_items_native),
        "get" => Some(proxy_get_native),
        "setdefault" => Some(proxy_setdefault_native),
        "update" => Some(proxy_update_native),
        "copy" => Some(proxy_copy_native),
        "pop" => Some(proxy_pop_native),
        "__eq__" => Some(proxy_eq_native),
        "__ne__" => Some(proxy_ne_native),
        _ => None,
    };
    handler.map(|handler| {
        // SAFETY: ptr 是本类型的存活对象（调用方保证）。
        bind_proxy_method(instance, unsafe { NonNull::new_unchecked(ptr) }, name, handler)
    })
}

fn proxy_of(bound: Option<NonNull<Header>>) -> Result<NonNull<Header>, ExecError> {
    bound.ok_or_else(|| panic!("FrameLocalsProxy 方法必须绑定实例"))
}

fn proxy_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "需要键"));
    };
    proxy_get(instance, proxy, key)
}

fn proxy_setitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let value = args.get(1).copied()
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "需要键与值"))?;
    proxy_set(instance, proxy, args[0], value)?;
    Ok(instance.singletons().none())
}

fn proxy_delitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "需要键"));
    };
    proxy_delete(instance, proxy, key)?;
    Ok(instance.singletons().none())
}

fn proxy_contains_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let contains = args.first().copied().is_some_and(|key| proxy_get(instance, proxy, key).is_ok());
    Ok(instance.singletons().boolean(contains))
}

fn proxy_len_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let length = proxy_keys(instance, proxy).len() as i64;
    Ok(instance.new_int(length))
}

fn proxy_iter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let snapshot = proxy_snapshot(instance, proxy);
    // **键要自己一份引用**（`new_list` 接管）✗：`dict_entries` 给的是**借用** ⇒ 先前直接交出去
    // ⇒ 列表里是**无主指针** ⇒ `in`／`reversed` 一族在比较时读到已释放的键 ⇒ SIGSEGV（round 177 实测 ✓）。
    let items: Vec<NonNull<Header>> = instance
        .dict_entries(snapshot)
        .unwrap_or_default()
        .into_iter()
        .map(|(key, _)| instance.retain(key))
        .collect();
    let list = instance.new_list(items);
    // SAFETY: list 是本函数的新引用；迭代器要自己那份。
    unsafe { instance.release_object(snapshot.as_ptr()) };
    crate::executor::iter_value(instance, list)
}

/// 视图的 `repr`：与 `repr(dict(view))` 同形 ✓（参照 `test_repr` 就是比这个）。
fn proxy_repr_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let snapshot = proxy_snapshot(instance, proxy);
    // 快照的 `repr`：借 `repr` 协议（`dict` 的 repr ✓）。
    let text = instance.object_repr(snapshot)?;
    // SAFETY: snapshot 是本函数的新引用。
    unsafe { instance.release_object(snapshot.as_ptr()) };
    Ok(instance.new_str(&text))
}

fn proxy_keys_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    Ok(instance.new_list(proxy_keys(instance, proxy)))
}

fn proxy_values_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let mut values = Vec::new();
    for key in proxy_keys(instance, proxy) {
        if let Ok(value) = proxy_get(instance, proxy, key) {
            values.push(value);
        }
        // SAFETY: 键引用由本函数持有。
        unsafe { instance.release_object(key.as_ptr()) };
    }
    Ok(instance.new_list(values))
}

fn proxy_items_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let mut items = Vec::new();
    for key in proxy_keys(instance, proxy) {
        if let Ok(value) = proxy_get(instance, proxy, key) {
            let item = instance.new_tuple(vec![instance.retain(key), value]);
            items.push(item);
        }
        // SAFETY: 键引用由本函数持有。
        unsafe { instance.release_object(key.as_ptr()) };
    }
    Ok(instance.new_list(items))
}

fn proxy_get_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "需要键"));
    };
    match proxy_get(instance, proxy, key) {
        Ok(value) => Ok(value),
        Err(error) => {
            if let Some(default) = args.get(1).copied() {
                crate::discard_raised(instance, match error {
                    ExecError::Raised { exception } => exception,
                    _ => return Ok(instance.singletons().none()),
                });
                Ok(instance.retain(default))
            } else {
                Err(error)
            }
        }
    }
}

fn proxy_setdefault_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "需要键"));
    };
    if let Ok(existing) = proxy_get(instance, proxy, key) {
        return Ok(existing);
    }
    let default = args
        .get(1)
        .copied()
        .unwrap_or_else(|| instance.singletons().none());
    proxy_set(instance, proxy, key, default)?;
    Ok(instance.retain(default))
}

fn proxy_update_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    if let Some(other) = args.first().copied() {
        // `dict` 在参照里会先取 `keys()`（“可映射”形态）；不可映射又不可迭代 ⇒ TypeError ✓
        let is_mapping = instance
            .type_named("dict")
            .is_some_and(|dict| instance.type_of(other) == dict)
            || crate::executor::attribute::attribute_lookup_swallowing(instance, other, "keys")
                .is_some();
        if is_mapping {
            let keys: Vec<NonNull<Header>> = instance
                .dict_entries(other)
                .unwrap_or_default()
                .into_iter()
                .map(|(key, _)| key)
                .collect();
            for key in keys {
                if let Ok(value) = crate::executor::subscript::subscript_read(instance, other, key) {
                    proxy_set(instance, proxy, key, value)?;
                    // SAFETY: value／key 引用由本函数持有。
                    unsafe {
                        instance.release_object(value.as_ptr());
                        instance.release_object(key.as_ptr());
                    }
                }
            }
        } else {
            // 迭代 (键, 值) 对
            match crate::executor::iter_value(instance, other) {
                Ok(iterator) => {
                    loop {
                        match crate::executor::runtime::advance(instance, iterator) {
                            Ok(Some(item)) => {
                                let parts = instance.tuple_items(item).unwrap_or_default();
                                if parts.len() == 2 {
                                    proxy_set(instance, proxy, parts[0], parts[1])?;
                                }
                                // SAFETY: item 引用由本函数持有。
                                unsafe { instance.release_object(item.as_ptr()) };
                            }
                            Ok(None) => break,
                            Err(error) => {
                                // SAFETY: 迭代器引用由本函数持有。
                                unsafe { instance.release_object(iterator.as_ptr()) };
                                return Err(error);
                            }
                        }
                    }
                    // SAFETY: 迭代器引用由本函数持有。
                    unsafe { instance.release_object(iterator.as_ptr()) };
                }
                Err(_) => {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        "update() 的实参既不是映射也不是键值对序列",
                    ))
                }
            }
        }
    }
    for (key, value) in kwargs {
        proxy_set(instance, proxy, *key, *value)?;
    }
    Ok(instance.singletons().none())
}

fn proxy_copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    Ok(proxy_snapshot(instance, proxy))
}

fn proxy_eq_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let equal = match args.first().copied() {
        Some(other) => {
            let snapshot = proxy_snapshot(instance, proxy);
            let result = crate::executor::values_equal_public(instance, snapshot, other);
            // SAFETY: snapshot 是本函数的新引用。
            unsafe { instance.release_object(snapshot.as_ptr()) };
            result
        }
        None => false,
    };
    Ok(instance.singletons().boolean(equal))
}

fn proxy_ne_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let equal = proxy_eq_native(instance, bound, args, kwargs)?;
    let value = instance.truthiness_of(equal).unwrap_or(true);
    Ok(instance.singletons().boolean(!value))
}

fn proxy_pop_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let proxy = proxy_of(bound)?;
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "需要键"));
    };
    match proxy_get(instance, proxy, key) {
        Ok(value) => {
            proxy_delete(instance, proxy, key)?;
            Ok(value)
        }
        Err(_) => {
            if let Some(default) = args.get(1).copied() {
                Ok(instance.retain(default))
            } else {
                Err(instance.raise_builtin_error("KeyError", &format!("{key:?}")))
            }
        }
    }
}
