//! 类创建：`LOAD_BUILD_CLASS` 压的 `__build_class__`（`OM-14` 的类创建钩子落点）。
//!
//! 实测的发射形状（`class C(Base): …`）：
//!
//! ```text
//! LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <类体 code>; MAKE_FUNCTION;
//! LOAD_CONST 'C'; LOAD_NAME Base; CALL 3; STORE_NAME C
//! ```
//!
//! 也就是说：**类体是一段"局部变量放在映射里"的代码**（`LOAD_NAME`／`STORE_NAME`），
//! `__build_class__` 拿到"类体函数 ＋ 名字 ＋ 基类"，跑完类体后建类型。
//!
//! **尚未接线**：`metaclass=`（非 `type` 的元类）、`__prepare__`、`__set_name__`
//! （要描述符）、`__mro_entries__`（`AB-37` 的宿主类型继承那一侧）。

use core::cell::Cell;
use core::ptr::NonNull;

use crate::builtin_objects::{BuiltinFunctionObject, DictObject, StrObject};
use crate::executor::{raise_builtin, ExecError};
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::{Frame, Header, NativeFn};

/// `__build_class__(body, name, *bases, metaclass=None, **kwds)`（原生实现）。
///
/// # Safety
///
/// 契约见 [`NativeFn`]：实参是借用视图、返回值是新引用。
pub unsafe fn build_class_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() < 2 {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "__build_class__: not enough arguments",
        ));
    }
    let body = args[0];
    let name_object = args[1];
    let bases: Vec<NonNull<Header>> = args[2..].to_vec();

    // 名字必须是 str（实测消息：`__build_class__: name is not a string`）
    // SAFETY: 调用方保证实参存活。
    if unsafe { name_object.as_ref() }.ty() != instance.singletons().str_type() {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "__build_class__: name is not a string",
        ));
    }
    // SAFETY: 类型身份已确认。
    let name = unsafe { &*name_object.as_ptr().cast::<StrObject>() }.value().to_owned();

    // `metaclass=`：本层只接 `type`（其余如实报未接线）
    for (key, _value) in kwargs {
        // SAFETY: 键由调用方保证存活。
        let key_type = unsafe { key.as_ref() }.ty();
        if key_type == instance.singletons().str_type() {
            // SAFETY: 同上。
            let text = unsafe { &*key.as_ptr().cast::<StrObject>() }.value();
            if text == "metaclass" {
                return Err(ExecError::Unsupported {
                    opcode: 0,
                    what: "__build_class__ 的 metaclass= 随后补（本层只支持默认元类 type）",
                });
            }
        }
    }

    // 类命名空间：一个空 `dict`（`__prepare__` 的默认结果）
    let namespace = instance
        .alloc(DictObject::new(
            instance
                .type_named("dict")
                .expect("dict 在引导期已登记"),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();

    // 跑类体：它的局部变量就是这个命名空间（`LOAD_NAME`／`STORE_NAME`）
    crate::executor::run_class_body(instance, body, namespace)?;

    // ---- 基类与布局（`OM-14`／`AB-37`／`AB-58`）----
    //
    // 顺序有意如此：**布局要先定下来**，类型对象才能按正确的 `instance_size` 与槽位建出来。
    let mut base_types: Vec<NonNull<TypeObject>> = Vec::new();
    for base in &bases {
        // SAFETY: base 由调用方保证存活。
        let base_type = unsafe { base.as_ref() }.ty();
        if base_type != instance.metatype() {
            return Err(raise_builtin(instance, "TypeError", "bases must be types"));
        }
        // SAFETY: base_type 由注册表持有。
        // 注意：`base_type` 是**基类自己的类型**（即元类型 `type`）——要查的是**基类本身**
        // SAFETY: base 是存活对象，且上面确认过它是类型对象。
        let info = unsafe { &*base.as_ptr().cast::<TypeObject>() };
        // **`AB-37`**：`PA_TYPE_FINAL` 的类型不可作基类（消息照参照实现）
        if info.is_final() {
            let message = format!("type '{}' is not an acceptable base type", info.name());
            return Err(raise_builtin(instance, "TypeError", &message));
        }
        base_types.push(base.cast::<TypeObject>());
    }
    if base_types.is_empty() {
        base_types.push(instance.type_named("object").expect("object 已登记"));
    }
    // 冗余基类去掉（`class X(Sub, Base)` 合法；`Base` 已被 `Sub` 覆盖）
    let mut pruned: Vec<NonNull<TypeObject>> = Vec::new();
    for (index, candidate) in base_types.iter().enumerate() {
        let redundant = base_types.iter().enumerate().any(|(other, base)| {
            other != index && {
                // SAFETY: 两者都由注册表持有。
                instance.is_subtype(*base, *candidate)
            }
        });
        if !redundant {
            pruned.push(*candidate);
        }
    }
    // **`AB-58`**：定长宿主布局只能有一个（参照实现：`multiple bases have instance lay-out conflict`）
    let mut host_base: Option<NonNull<TypeObject>> = None;
    let mut layout_conflict = false;
    for base in &pruned {
        // SAFETY: base 由注册表持有。
        if unsafe { base.as_ref() }.is_host_layout() {
            if host_base.is_some() {
                layout_conflict = true;
                break;
            }
            host_base = Some(*base);
        }
    }
    if layout_conflict {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "multiple bases have instance lay-out conflict",
        ));
    }

    // 建类型：名字要 `&'static str`（`TypeObject::name` 的临时形态）——
    // 这里把名字**泄漏**成静态串（每建一个类泄漏一次，`TS-43` 的最终形态是 `str` 对象）
    let static_name: &'static str = Box::leak(name.clone().into_boxed_str());
    let ty = match host_base {
        // **`AB-58`／`AB-37`**：宿主类型的 Python 子类**继承同一布局**——载荷按**同一尺寸**
        // 由 VM 分配（宿主无需参与），槽位沿用基类的 `dealloc`／`traverse`／终结器；
        // **没有**默认 `new`：宿主类型实例由宿主经 `pa_newhandle` 建（AB-58）
        Some(base) => {
            // SAFETY: base 由注册表持有。
            let base_info = unsafe { base.as_ref() };
            let slots = base_info.slots().inherit_host_layout();
            let created = instance.new_type(static_name, base_info.instance_size(), slots);
            // 宿主钩子（`dealloc`／`traverse`）随布局一起继承
            if let (Some(dealloc), Some(traverse)) =
                (base_info.host_dealloc(), base_info.host_traverse())
            {
                // SAFETY: created 由注册表持有。
                unsafe { created.as_ref() }.set_host_hooks(dealloc, traverse);
            }
            // 布局固定 ⇒ 实例字典**另行挂载**（`OM-14`）
            // SAFETY: 同上。
            unsafe { created.as_ref() }.mark_external_instance_dict();
            created
        }
        None => {
            let created = instance.new_type(
                static_name,
                core::mem::size_of::<crate::AttributeObject>(),
                crate::AttributeObject::slots().with_new(crate::builtin_objects::attribute_new),
            );
            // SAFETY: created 由注册表持有。
            unsafe { created.as_ref() }.mark_has_instance_dict();
            created
        }
    };

    if instance.register_bases(ty, pruned).is_none() {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "Cannot create a consistent method resolution order (MRO) for bases",
        ));
    }

    // 把类体的命名空间搬进**类型字典**（`OM-11` 的属性通道就是查它）
    // SAFETY: namespace 是刚造的 dict。
    let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
    let entries = mapping.entries();
    // 把类命名空间搬进**类型字典**（`OM-11` 的属性通道就是查它）。
    //
    // 键**原样放进**（不另造一份）：`insert_raw` 是转移语义 ⇒ 各新增一份引用；
    // 两个字典共享同一个键对象是正常的（参照实现里键都是 intern 的字符串）。
    //
    // 历史记录：这里曾经观察到**随机堆损坏**，一度被误判为"键共享"引起（当时用"各自持键"绕开）。
    // 真因是 `class_body_frame` 里少了一次 `incref`：`Owned::new` 收的是**新引用**，
    // 我却把类体函数自己那份 code 引用交了出去，守卫析构时多释放一次。修好之后
    // 共享键版本连跑 20 次全绿。
    let type_dict = match unsafe { ty.as_ref() }.dict() {
        Some(mapping) => mapping,
        None => {
            let mapping = instance
                .alloc(DictObject::new(
                    instance.type_named("dict").expect("dict 已登记"),
                    core::cell::RefCell::new(Vec::new()),
                ))
                .into_raw()
                .cast::<Header>();
            // SAFETY: ty 由注册表持有。
            unsafe { ty.as_ref() }.set_dict(Some(mapping));
            mapping
        }
    };
    for (key, value) in entries {
        // SAFETY: 键值由命名空间持有，各新增一份（insert_raw 是转移语义）。
        unsafe {
            instance.incref_object(key.as_ptr());
            instance.incref_object(value.as_ptr());
        }
        // SAFETY: type_dict 由类型对象持有，存活。
        unsafe { &*type_dict.as_ptr().cast::<DictObject>() }.insert_raw(key, value);
    }
    // SAFETY: namespace 由本函数持有。
    unsafe { instance.release_object(namespace.as_ptr()) };

    // `__init_subclass__`（`OM-14` 的类创建钩子）：在**直接基类**上找并调用
    for base in &bases {
        crate::executor::call_dunder_method(
            instance,
            *base,
            "__init_subclass__",
            &[ty.cast::<Header>()],
        )?;
    }

    // SAFETY: ty 由注册表持有；这里新增一份引用交给调用方。
    unsafe { instance.incref_object(ty.cast::<Header>().as_ptr()) };
    Ok(ty.cast::<Header>())
}

/// 造一个"原生可调用对象"形态的 `__build_class__`（给测试与将来的 `builtins` 用）。
#[allow(dead_code)] // 引导期用等价的一份；这里留给测试与将来的 `builtins`
pub fn native_build_class(instance: &Instance) -> NonNull<Header> {
    let object = instance.alloc(BuiltinFunctionObject::new(
        instance
            .type_named("builtin_function_or_method")
            .expect("引导期已登记"),
        "__build_class__",
        Cell::new(build_class_native as NativeFn),
    ));
    object.into_raw().cast::<Header>()
}

/// 帧：把命名空间交给类体（供 [`crate::executor::run_class_body`] 用）。
///
/// 帧**自己持有一份**命名空间引用（`Frame::for_code_with_namespace` 接手的是新引用），
/// 所以这里先新增一份——少了它，帧一析构命名空间就没了（调用方那一份不算）。
pub(crate) fn class_body_frame(
    instance: &Instance,
    code: NonNull<Header>,
    namespace: NonNull<Header>,
    globals: Option<NonNull<Header>>,
) -> NonNull<Frame> {
    let frame_type = instance
        .type_named("Frame")
        .expect("Frame 在引导期已登记");
    // `Owned::new` 收的是**新引用**（`OM-16`：守卫的 `Drop` 会释放它）⇒ 这里必须自己新增一份。
    // 少了这一步，守卫析构时会释放**函数自己那份** code 引用（症状：随机时刻
    // "对已释放对象 decref"，进而在 glibc 线程退出检查里变成 tcache 堆损坏）。
    // SAFETY: code 由调用方保证存活（它来自类体函数持有的 code object）。
    unsafe { instance.incref_object(code.as_ptr()) };
    let code_reference = crate::Owned::new(code.cast::<crate::CodeObject>(), instance);
    // SAFETY: namespace 由调用方保证存活；这里为帧新增一份引用。
    unsafe { instance.incref_object(namespace.as_ptr()) };
    let frame = instance.alloc(Frame::for_code_with_namespace(
        frame_type,
        &code_reference,
        namespace,
    ));
    // **类体的全局层**（`LOAD_NAME` 的第二层）：取**类体函数**记着的 `__globals__`
    // （`MAKE_FUNCTION` 时捕获），所以类体里能读到模块级名字。
    if let Some(mapping) = globals {
        // 帧接手的是**新引用**（`Frame::clear` 会释放它）
        // SAFETY: mapping 由类体函数持有，存活。
        unsafe { instance.incref_object(mapping.as_ptr()) };
        frame.get().set_globals(mapping);
    }
    frame.into_raw().cast::<Frame>()
}
