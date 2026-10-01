//! 实例级内存、释放协议与循环回收（`docs/SPEC-object-model.md` §4、§7、§9）。
//!
//! 一个 [`Instance`] 就是 VM 侧一切可变状态的宿主（`DESIGN.md` §3 不变量 2）：
//! 对象堆、字节记账、类型注册表、回收链表与待处理栈都挂在它上面，**没有进程级全局状态**。

use core::cell::{Cell, OnceCell, RefCell};
use core::ptr;
use core::ptr::NonNull;
use std::collections::{HashMap, HashSet};

use crate::flags;
use crate::header::{Header, PyObject};
use crate::refcount::{Owned, PyRef};
use crate::frame::Frame;
use crate::builtin_objects::{
    AttributeObject, BoolObject, BuiltinFunctionObject, DictObject, ExceptionObject, FloatObject,
    FunctionObject, GeneratorObject, IntObject, IteratorObject, MethodObject,
    ListObject, NoneObject, NullObject, PlainObject, SetObject, StrObject, TupleObject,
};
use crate::singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
use crate::type_object::{Slots, TypeObject};

/// 一个 `str` 对象的内容是否等于给定的 Rust 字符串（属性名比较用）。
fn str_matches(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 是存活对象。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}

/// **OM-26**：回收阈值，**三元组**形态。
///
/// 参照实现（本机 CPython 3.14.4 实测）：`gc.get_threshold() == (2000, 10, 0)`。
/// 本层**单代**：只有 `.0` 生效，后两位**存而不生效**——这一"存而不生效"是**临时**的，
/// 等真分代落地（`SPEC-object-model.md` 的 `OM-26`、`DESIGN.md` §13-18）。
pub const DEFAULT_GC_THRESHOLD: (usize, usize, usize) = (2000, 10, 0);

/// **OM-1**／**OM-3**／**OM-4**：一个实例的对象堆与记账。
pub struct Instance {
    /// **OM-3**：每实例字节计数器（预算职责留在 VM 侧，禁止下放给能力接口）。
    bytes_allocated: Cell<usize>,
    /// 本实例分配、尚未释放的普通对象（`usize` = 头部地址；**O(1)** 增删）。
    live: RefCell<HashSet<usize>>,
    /// **OM-15**：类型注册表按实例存放；注册表持有每个类型对象的一份引用。
    types: RefCell<Vec<NonNull<TypeObject>>>,
    /// 元类型（类型对象的类型，自指）。
    metatype: Cell<Option<NonNull<TypeObject>>>,
    /// **OM-23**：本实例的单例表（引导期填好，之后只读）。
    singletons: OnceCell<Singletons>,
    /// **BC-60** ②：**本实例**的当前异常状态（正在处理的异常）——**禁止**进程级全局。
    exception_state: RefCell<Vec<NonNull<Header>>>,
    /// `__build_class__`（引导期建好；见 [`Instance::build_class`]）。
    build_class: Cell<Option<NonNull<Header>>>,
    /// 最近一次抛出的异常（**本实例持有一份引用**）：`ExecError::Raised` 借它保活。
    pending_exception: Cell<Option<NonNull<Header>>>,
    /// **OM-21**：待处理栈——计数归零的对象在这里排队，由最外层调用逐个清空（禁止朴素递归）。
    pending: RefCell<Vec<NonNull<Header>>>,
    /// 是否正在清空待处理栈（重入检测）。
    draining: Cell<bool>,
    /// **OM-25**：`GC_TRACKED` 对象的侵入式链表头（借头部的 `gc_prev`／`gc_next`）。
    gc_head: Cell<*mut Header>,
    /// 链表中当前的跟踪对象数。
    gc_count: Cell<usize>,
    /// **OM-26**：阈值可配置。
    gc_threshold: Cell<(usize, usize, usize)>,
    /// 自上次回收以来的分配计数。
    gc_alloc_count: Cell<usize>,
    /// 回收进行中：这些对象只减计数、由本次回收统一释放（见 [`Instance::collect`]）。
    gc_frozen: RefCell<HashSet<usize>>,
    /// 回收是否正在进行：终结器／`clear` 里再触发回收时不得嵌套（否则会动到外层手里的指针）。
    gc_running: Cell<bool>,
    /// **`OM-11` 的 `repr` 递归守卫**（`CX-3`：按实例存）：正在生成 `repr` 的对象地址。
    repr_guard: RefCell<Vec<usize>>,
    /// **`AB-5`①／`CX-3`**：本实例被请求中断（`pa_interrupt` 的落点）。
    ///
    /// **按实例**存——`CX-3` 禁止进程级共享；执行器每条指令检查一次，
    /// 于是"执行类函数随即返回"（`PA_ERR_INTERRUPT`）成立。
    interrupted: Cell<bool>,
}

impl Instance {
    /// 创建一个实例，并引导它的**元类型**。
    ///
    /// **OM-1**：每个实例有自己的堆与单例表；本函数不触碰任何进程级状态。
    pub fn new() -> Self {
        let this = Self {
            bytes_allocated: Cell::new(0),
            live: RefCell::new(HashSet::new()),
            types: RefCell::new(Vec::new()),
            metatype: Cell::new(None),
            singletons: OnceCell::new(),
            exception_state: RefCell::new(Vec::new()),
            build_class: Cell::new(None),
            pending_exception: Cell::new(None),
            pending: RefCell::new(Vec::new()),
            draining: Cell::new(false),
            gc_head: Cell::new(ptr::null_mut()),
            gc_count: Cell::new(0),
            gc_threshold: Cell::new(DEFAULT_GC_THRESHOLD),
            gc_alloc_count: Cell::new(0),
            gc_frozen: RefCell::new(HashSet::new()),
            gc_running: Cell::new(false),
            repr_guard: RefCell::new(Vec::new()),
            interrupted: Cell::new(false),
        };

        // 元类型自指：类型对象的类型就是它自己（与 CPython 的 `PyType_Type` 同理）。
        // 此处 `ty` 先落在 `NonNull::dangling()` 上，写完自指后立即成为正常类型对象。
        let metatype = this.alloc_type_raw(
            "type",
            core::mem::size_of::<TypeObject>(),
            Slots::new(TypeObject::dealloc).with_repr(crate::builtin_objects::type_repr),
        );
        // SAFETY: metatype 刚分配、尚未交给任何其他代码；写入自指后它才被引用。
        unsafe { metatype.as_ref().header.set_ty(metatype) };
        this.metatype.set(Some(metatype));

        this.bootstrap_builtin_types();
        this
    }

    /// **OM-23**：按实例创建单例（`None`／`True`／`False`／小整数）。
    ///
    /// 引导期还不能借出 `&Instance` 造 `Owned` 守卫，所以走 [`Instance::adopt`]：
    /// 引用由实例自己持有，随实例销毁一起释放（`OM-2`）。
    /// 按 `TS-41` 的**探测表**登记一个类型的基类（MRO 由 C3 算）。
    ///
    /// 层次**不手写**：基类名字取自 `crate::builtin_types`。基类必须**先**注册（表里的顺序
    /// 是参照实现的顺序，但引导期按依赖手工排序）。
    fn register_from_table(&self, ty: NonNull<TypeObject>) {
        // SAFETY: ty 由本实例的注册表持有。
        let name = unsafe { ty.as_ref() }.name();
        let entry = crate::builtin_types::builtin_type(name)
            .unwrap_or_else(|| panic!("TS-41：{name} 必须在探测表里"));
        let bases: Vec<NonNull<TypeObject>> = entry
            .bases
            .iter()
            .map(|base| {
                self.type_named(base).unwrap_or_else(|| {
                    panic!("TS-41：{name} 的基类 {base} 必须先于它注册")
                })
            })
            .collect();
        assert!(
            self.register_bases(ty, bases).is_some(),
            "OM-13：{name} 的 MRO 应当可线性化"
        );
    }

    /// **TS-41**／**TS-42** 的第一阶梯＋**容器的 M2 起步**：`object`／`type`／`NoneType`／
    /// `bool`／`int`／`float`／`str`／`tuple`／`list`／`dict`／`set`。
    ///
    /// 层次**不手写**：基类关系取自 `crate::builtin_types` 的探测表（`TS-41`），MRO 由
    /// **C3**（`OM-13`）算出。**禁止**为了省事直接写一份 MRO。
    fn bootstrap_builtin_types(&self) {
        let object_type = self.alloc_type_raw(
            "object",
            core::mem::size_of::<PlainObject>(),
            Slots::new(PlainObject::dealloc).with_new(crate::builtin_objects::plain_new),
        );
        self.register_from_table(object_type);

        // 元类型（`type`）也是对象；表里 `type` 的基类就是 `object`
        let metatype = self.metatype.get().expect("元类型在 Instance::new 里已引导");
        self.register_from_table(metatype);

        let none_type = self.alloc_type_raw(
            "NoneType",
            core::mem::size_of::<NoneObject>(),
            Slots::new(NoneObject::dealloc)
                .with_repr(crate::builtin_objects::none_repr)
                .with_str(crate::builtin_objects::none_repr),
        );
        let bool_type = self.alloc_type_raw(
            "bool",
            core::mem::size_of::<BoolObject>(),
            Slots::new(BoolObject::dealloc)
                .with_new(crate::builtin_objects::bool_new)
                .with_repr(crate::builtin_objects::bool_repr)
                .with_str(crate::builtin_objects::bool_repr),
        );
        let int_type = self.alloc_type_raw(
            "int",
            core::mem::size_of::<IntObject>(),
            Slots::new(IntObject::dealloc)
                .with_new(crate::builtin_objects::int_new)
                .with_repr(crate::builtin_objects::int_repr)
                .with_str(crate::builtin_objects::int_repr),
        );
        let float_type = self.alloc_type_raw(
            "float",
            core::mem::size_of::<FloatObject>(),
            Slots::new(FloatObject::dealloc)
                .with_new(crate::builtin_objects::float_new)
                .with_repr(crate::builtin_objects::float_repr)
                .with_str(crate::builtin_objects::float_repr),
        );
        let str_type = self.alloc_type_raw(
            "str",
            core::mem::size_of::<StrObject>(),
            Slots::new(StrObject::dealloc)
                .with_new(crate::builtin_objects::str_new)
                .with_repr(crate::builtin_objects::str_repr)
                .with_str(crate::builtin_objects::str_str),
        );

        // 容器：`TS-42` 的 M2 起步（层次取自探测表）
        let tuple_type = self.alloc_type_raw(
            "tuple",
            core::mem::size_of::<TupleObject>(),
            TupleObject::slots()
                .with_new(crate::builtin_objects::tuple_new)
                .with_repr(crate::builtin_objects::tuple_repr),
        );
        let list_type = self.alloc_type_raw(
            "list",
            core::mem::size_of::<ListObject>(),
            ListObject::slots()
                .with_new(crate::builtin_objects::list_new)
                .with_repr(crate::builtin_objects::list_repr),
        );
        let dict_type = self.alloc_type_raw(
            "dict",
            core::mem::size_of::<DictObject>(),
            DictObject::slots()
                .with_new(crate::builtin_objects::dict_new)
                .with_repr(crate::builtin_objects::dict_repr),
        );
        let set_type = self.alloc_type_raw(
            "set",
            core::mem::size_of::<SetObject>(),
            SetObject::slots()
                .with_new(crate::builtin_objects::set_new)
                .with_repr(crate::builtin_objects::set_repr),
        );

        // `function`：`TS-42` 的 M2（调用与返回族逼出来的）
        let function_type = self.alloc_type_raw(
            "function",
            core::mem::size_of::<FunctionObject>(),
            FunctionObject::slots().with_repr(crate::builtin_objects::function_repr),
        );

        // 迭代器类型：名字**照探测表**取（`str` 的迭代器在这台机器上叫 `str_ascii_iterator`）
        // 后两个的**可迭代对象**（`bytes`／`bytearray`）本身排在 M3+，故它们现在只是类型存在
        // （`TS-42` 的 M2 要求"迭代器对象"齐备），不会被 `GET_ITER` 选中。
        let iterator_types: Vec<NonNull<TypeObject>> = [
            "tuple_iterator",
            "list_iterator",
            "str_ascii_iterator",
            "dict_keyiterator",
            "set_iterator",
            "bytes_iterator",
            "bytearray_iterator",
        ]
        .iter()
        .map(|name| {
            self.alloc_type_raw(
                name,
                core::mem::size_of::<IteratorObject>(),
                IteratorObject::slots(),
            )
        })
        .collect();

        // 原生可调用对象（`AB-24` 的宿主函数、`__build_class__` 一类内建函数的落点）
        let builtin_function_type = self.alloc_type_raw(
            "builtin_function_or_method",
            core::mem::size_of::<BuiltinFunctionObject>(),
            BuiltinFunctionObject::slots(),
        );

        // 绑定方法（`OM-11` 的 `getattr` 查到函数时的产物）
        let method_type = self.alloc_type_raw(
            "method",
            core::mem::size_of::<MethodObject>(),
            MethodObject::slots().with_repr(crate::builtin_objects::method_repr),
        );

        // 生成器（`§10` 的生成器与协程族）：名字与基类照探测表
        let generator_type = self.alloc_type_raw(
            "generator",
            core::mem::size_of::<GeneratorObject>(),
            GeneratorObject::slots().with_repr(crate::builtin_objects::generator_repr),
        );

        // 异常层次（`TS-42` 的 M2）：**名字与基类都来自探测表**，按"基类先注册"的顺序反复扫。
        // 一个 `ExceptionObject` 载荷撑起整棵树（`TS-43`：布局自选）。
        let exception_names: Vec<&'static str> = crate::builtin_types::BUILTIN_TYPES
            .iter()
            .filter(|entry| entry.name == "BaseException" || entry.mro.contains(&"BaseException"))
            .map(|entry| entry.name)
            .collect();
        let exception_types: Vec<(NonNull<TypeObject>, &'static str)> = exception_names
            .iter()
            .map(|name| {
                (
                    self.alloc_type_raw(
                        name,
                        core::mem::size_of::<ExceptionObject>(),
                        ExceptionObject::slots()
                            .with_new(crate::builtin_objects::exception_new)
                            .with_repr(crate::builtin_objects::exception_repr)
                            .with_str(crate::builtin_objects::exception_repr),
                    ),
                    *name,
                )
            })
            .collect();
        let mut registered: Vec<&'static str> = vec!["object"];
        loop {
            let mut progressed = false;
            for (ty, name) in exception_types.iter() {
                if registered.contains(name) {
                    continue;
                }
                let entry = crate::builtin_types::builtin_type(name)
                    .unwrap_or_else(|| panic!("TS-41：{name} 必须在探测表里"));
                if entry.bases.iter().all(|base| registered.contains(base)) {
                    self.register_from_table(*ty);
                    registered.push(name);
                    progressed = true;
                }
            }
            if exception_types
                .iter()
                .all(|(_, name)| registered.contains(name))
            {
                break;
            }
            assert!(progressed, "TS-41：异常层次里有环或基类缺失");
        }

        // **内部** Frame 类型：执行器要给被调函数建帧（不进 `TS-41` 的内建表）
        let frame_type = self.alloc_type_raw(
            "Frame",
            core::mem::size_of::<Frame>(),
            Frame::slots(),
        );
        assert!(
            self.register_bases(frame_type, vec![object_type]).is_some(),
            "OM-13：内部 Frame 类型的基类也是 object"
        );

        // **内部哨兵**：`CALL` 的 NULL 槽位。它**不**进 `TS-41` 的内建类型表，
        // 也不许暴露给 Python，故不走 `register_from_table`。
        let null_type = self.alloc_type_raw(
            "NULL",
            core::mem::size_of::<NullObject>(),
            Slots::new(NullObject::dealloc),
        );
        assert!(
            self.register_bases(null_type, vec![object_type]).is_some(),
            "OM-13：内部哨兵的基类也是 object"
        );

        // 基类关系：`bool ⊂ int`（TS-40 点名），其余都是 `object` 的直接子类——全部查表
        for ty in iterator_types
            .iter()
            .copied()
            .chain([
                function_type,
                generator_type,
                builtin_function_type,
                method_type,
                none_type,
                int_type,
                bool_type,
                float_type,
                str_type,
                tuple_type,
                list_type,
                dict_type,
                set_type,
            ])
        {
            self.register_from_table(ty);
        }

        // **OM-23**：单例——`None`／`True`／`False`／小整数／**空串**
        let null = self.adopt(NullObject::new(null_type)).cast::<Header>();
        let none = self.adopt(NoneObject::new(none_type)).cast::<Header>();
        let true_ = self.adopt(BoolObject::new(bool_type, true)).cast::<Header>();
        let false_ = self.adopt(BoolObject::new(bool_type, false)).cast::<Header>();
        let empty_str = self
            .adopt(StrObject::new(str_type, String::new()))
            .cast::<Header>();

        let count = (SMALL_INT_MAX - SMALL_INT_MIN + 1) as usize;
        let mut small_ints = Vec::with_capacity(count);
        for value in SMALL_INT_MIN..=SMALL_INT_MAX {
            small_ints.push(self.adopt(IntObject::new(int_type, value)).cast::<Header>());
        }

        assert!(
            self.singletons
                .set(Singletons::new(
                    none_type,
                    bool_type,
                    int_type,
                    str_type,
                    null,
                    empty_str,
                    none,
                    true_,
                    false_,
                    small_ints,
                ))
                .is_ok(),
            "单例表在 Instance::new 里只设一次"
        );

        // `TS-44`：`__format__` **没有槽位** ⇒ 内建类型在**类型字典**里放**原生可调用对象**
        // （`object` 那一层给默认：空规格 ⇒ `str(x)`、非空 ⇒ TypeError，消息实测）
        for (ty, name, function) in [
            (
                object_type,
                "__format__",
                crate::builtin_objects::native_format_object as crate::NativeFn,
            ),
            (
                int_type,
                "__format__",
                crate::builtin_objects::native_format_int as crate::NativeFn,
            ),
            (
                float_type,
                "__format__",
                crate::builtin_objects::native_format_float as crate::NativeFn,
            ),
            (
                str_type,
                "__format__",
                crate::builtin_objects::native_format_str as crate::NativeFn,
            ),
        ] {
            let native = self.alloc(BuiltinFunctionObject::new(
                builtin_function_type,
                name,
                Cell::new(function),
            ));
            self.set_type_attribute(ty, name, native.into_raw().cast::<Header>());
        }

        // `LOAD_BUILD_CLASS` 要压的内建（`__build_class__`）：造一个原生可调用对象按实例存
        let build_class = self.alloc(BuiltinFunctionObject::new(
            builtin_function_type,
            "__build_class__",
            Cell::new(crate::classes::build_class_native as crate::NativeFn),
        ));
        self.build_class.set(Some(build_class.into_raw().cast::<Header>()));
    }

    /// **OM-13**：C3 线性化。基类顺序矛盾（没有可用候选）时返回 `None`。
    ///
    /// `L(C) = [C] + merge(L(B1), …, L(Bn), [B1, …, Bn])`；`merge` 每轮取"不出现在任何列表
    /// **尾部**"的第一个表头。**禁止**用"深度优先拼接"糊过去——那样 `__mro__` 与参照实现不一致。
    pub fn linearize(
        &self,
        ty: NonNull<TypeObject>,
        bases: &[NonNull<TypeObject>],
    ) -> Option<Vec<NonNull<TypeObject>>> {
        let mut sequences: Vec<Vec<NonNull<TypeObject>>> = Vec::new();
        for base in bases {
            // SAFETY: 基类由本实例的注册表持有（OM-15），在实例存活期间有效。
            sequences.push(unsafe { base.as_ref() }.mro());
        }
        sequences.push(bases.to_vec());

        let mut result = vec![ty];
        loop {
            sequences.retain(|sequence| !sequence.is_empty());
            if sequences.is_empty() {
                return Some(result);
            }
            let mut chosen = None;
            for sequence in &sequences {
                let candidate = sequence[0];
                let blocked = sequences
                    .iter()
                    .any(|other| other[1..].contains(&candidate));
                if !blocked {
                    chosen = Some(candidate);
                    break;
                }
            }
            let chosen = chosen?;
            result.push(chosen);
            for sequence in sequences.iter_mut() {
                sequence.retain(|entry| *entry != chosen);
            }
        }
    }

    /// **OM-13**／**OM-14**：登记基类，MRO 由 C3 算出并写入；不一致时返回 `None`。
    pub fn register_bases(
        &self,
        ty: NonNull<TypeObject>,
        bases: Vec<NonNull<TypeObject>>,
    ) -> Option<Vec<NonNull<TypeObject>>> {
        let mro = self.linearize(ty, &bases)?;
        // SAFETY: ty 由本实例的注册表持有。
        unsafe { ty.as_ref() }.set_bases(bases, mro.clone());
        Some(mro)
    }

    /// **`OM-10`**：沿 **MRO** 查类型字典（**借用**的裸引用；查不到返回 `None`）。
    ///
    /// 这是属性查找的"类型那一半"（`OM-11` 的 `getattr` 槽位随类型系统接线后接管分派）。
    pub fn type_lookup(&self, ty: NonNull<TypeObject>, name: &str) -> Option<NonNull<Header>> {
        // SAFETY: ty 由注册表持有，MRO 里的类型同样存活。
        for entry in unsafe { ty.as_ref() }.mro() {
            // SAFETY: 同上。
            let mapping = unsafe { entry.as_ref() }.dict();
            let Some(mapping) = mapping else { continue };
            // SAFETY: mapping 由类型对象持有。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            let found = dict
                .entries()
                .into_iter()
                .find(|(key, _)| str_matches(self, *key, name));
            if let Some((_, value)) = found {
                return Some(value);
            }
        }
        None
    }

    /// **`OM-10`**：往类型字典里写一项（**新引用**，由字典接手；返回被顶下来的旧值）。
    ///
    /// 字典惰性创建。**禁止**用这个函数给内建类型旁路属性通道——
    /// Python 可见属性一律走 `OM-11` 的 `getattr` 槽位。
    pub fn set_type_attribute(
        &self,
        ty: NonNull<TypeObject>,
        name: &str,
        value: NonNull<Header>,
    ) -> Option<NonNull<Header>> {
        // SAFETY: ty 由注册表持有。
        let type_object = unsafe { ty.as_ref() };
        let mapping = match type_object.dict() {
            Some(mapping) => mapping,
            None => {
                let mapping = self
                    .adopt(DictObject::new(
                        self.type_named("dict").expect("dict 已在引导期登记"),
                        RefCell::new(Vec::new()),
                    ))
                    .cast::<Header>();
                type_object.set_dict(Some(mapping));
                mapping
            }
        };
        // SAFETY: mapping 由类型对象持有。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let key = self
            .adopt(StrObject::new(self.singletons().str_type(), name.to_owned()))
            .cast::<Header>();
        let position = dict
            .entries()
            .into_iter()
            .position(|(existing, _)| str_matches(self, existing, name));
        match position {
            Some(slot) => {
                // 键已在表里：新键那份引用交回去
                // SAFETY: key 是刚 adopt 的对象，只有这一份引用。
                unsafe { self.release_object(key.as_ptr()) };
                dict.replace_value(slot, value)
            }
            None => {
                dict.insert_raw(key, value);
                None
            }
        }
    }

    /// 造一个整数（落在单例区间就用那个单例）——**新引用**。
    ///
    /// 给**对象类型自己的槽位实现**用（`getattr` 一类要在 crate 内造可见对象）。
    pub fn new_int(&self, value: i64) -> NonNull<Header> {
        if let Some(singleton) = self.singletons().small_int(value) {
            // SAFETY: 单例由实例持有，存活。
            unsafe { self.incref_object(singleton.as_ptr()) };
            return singleton;
        }
        let int_type = self.singletons().int_type();
        self.alloc(IntObject::new(int_type, value))
            .into_raw()
            .cast::<Header>()
    }

    /// 造一个 `str`（空串走 `OM-23` 的单例）——**新引用**。
    pub fn new_str(&self, text: &str) -> NonNull<Header> {
        if text.is_empty() {
            let empty = self.singletons().empty_str();
            // SAFETY: 单例由实例持有，存活。
            unsafe { self.incref_object(empty.as_ptr()) };
            return empty;
        }
        self.alloc(StrObject::new(
            self.singletons().str_type(),
            text.to_owned(),
        ))
        .into_raw()
        .cast::<Header>()
    }

    /// **`OM-11` 的 `str` 槽**：`str(对象)`。
    ///
    /// `SPEC-type-system.md` §8：该槽**省略时回退到 `repr`**。
    pub fn object_str(&self, object: NonNull<Header>) -> String {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        if let Some(slot) = unsafe { ty.as_ref() }.slots().str {
            // SAFETY: 槽位契约见 `StrFn`。
            if let Some(text) = unsafe { slot(object.as_ptr(), self) } {
                return text;
            }
        }
        self.object_repr(object)
    }

    /// **`OM-11` 的 `repr` 槽**：`repr(对象)`；槽位省略时给默认形式（`SPEC-type-system.md` §8）。
    pub fn object_repr(&self, object: NonNull<Header>) -> String {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        // SAFETY: ty 由注册表持有。
        if let Some(slot) = unsafe { ty.as_ref() }.slots().repr {
            // SAFETY: 槽位契约见 `ReprFn`。
            if let Some(text) = unsafe { slot(object.as_ptr(), self) } {
                return text;
            }
        }
        // 默认形式：`<X object at 0x…>`（类型名；模块／qualname 随类创建钩子接线后补）
        // SAFETY: 同上。
        format!(
            "<{} object at {:p}>",
            unsafe { ty.as_ref() }.name(),
            object.as_ptr()
        )
    }

    /// `ascii(对象)`：`repr` 且非 ASCII 字符转义。
    pub fn object_ascii(&self, object: NonNull<Header>) -> String {
        // SAFETY: object 是存活对象。
        let ty = unsafe { object.as_ref() }.ty();
        if ty == self.singletons().str_type() {
            // SAFETY: 类型身份已确认。
            let text = unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned();
            return quote_str(&text, true);
        }
        self.object_repr(object)
    }

    /// **`OM-11`**：`repr` 的递归守卫——已经在生成中的对象返回 `false`
    /// （容器据此给出 `[...]`／`{...}`，与参照实现一致）。
    pub fn enter_repr(&self, address: usize) -> bool {
        let mut guard = self.repr_guard.borrow_mut();
        if guard.contains(&address) {
            return false;
        }
        guard.push(address);
        true
    }

    /// 退出 `repr` 的递归守卫。
    pub fn leave_repr(&self, address: usize) {
        let mut guard = self.repr_guard.borrow_mut();
        if let Some(position) = guard.iter().rposition(|entry| *entry == address) {
            guard.remove(position);
        }
    }

    /// 造一个 `tuple`（元素是**新引用**，由元组接手）——**新引用**。
    pub fn new_tuple(&self, items: Vec<NonNull<Header>>) -> NonNull<Header> {
        let tuple_type = self
            .type_named("tuple")
            .expect("tuple 在引导期已登记");
        self.alloc(TupleObject::new(tuple_type, items))
            .into_raw()
            .cast::<Header>()
    }

    /// 按名字在注册表里找一个类型。
    ///
    /// 这是**内部**查询（`TS-41` 的对拍与引导期要用）；Python 可见的属性访问**必须**走
    /// `OM-11` 的 `getattr` 槽位，**禁止**用这个函数旁路属性通道。
    pub fn type_named(&self, name: &str) -> Option<NonNull<TypeObject>> {
        self.types
            .borrow()
            .iter()
            .copied()
            .find(|ty| {
                // SAFETY: 注册表里的类型都存活。
                unsafe { ty.as_ref() }.name() == name
            })
    }

    /// 造一个**实例带属性字典**的类型（用户类的实例就是这样）。
    ///
    /// 载荷用 [`AttributeObject`]（`TS-43`：布局自选），并置 [`crate::HAS_INSTANCE_DICT`]；
    /// 执行器据此决定 `STORE_ATTR` 往哪写。`object()` 自己**不**带字典——与参照实现一致。
    pub fn new_attribute_type(&self, name: &'static str) -> NonNull<TypeObject> {
        let ty = self.new_type(
            name,
            core::mem::size_of::<AttributeObject>(),
            AttributeObject::slots().with_new(crate::builtin_objects::attribute_new),
        );
        // SAFETY: ty 由注册表持有。
        unsafe { ty.as_ref() }.mark_has_instance_dict();
        ty
    }

    /// **TS-40**／**TS-29**：`subtype` 是不是 `supertype` 的子类型（含自身）。
    ///
    /// 走 **MRO**（**OM-13** 的 C3 产物）——所以 `bool ⊂ int`、任何类型 `⊂ object` 都自动成立。
    /// `__subclasshook__`／ABC 注册（`numbers.Integral` 一类）随后补。
    pub fn is_subtype(&self, subtype: NonNull<TypeObject>, supertype: NonNull<TypeObject>) -> bool {
        if subtype == supertype {
            return true;
        }
        // SAFETY: 两个类型都由本实例的注册表持有。
        unsafe { subtype.as_ref() }.mro().contains(&supertype)
    }

    /// **`AB-5`①**：请求中断本实例（幂等）。
    pub fn request_interrupt(&self) {
        self.interrupted.set(true);
    }

    /// 本实例是否被请求中断（执行器每条指令看它）。
    pub fn interrupted(&self) -> bool {
        self.interrupted.get()
    }

    /// 清掉中断请求（宿主重新开始执行前用；`pa_interrupt` 的配套）。
    pub fn clear_interrupt(&self) {
        self.interrupted.set(false);
    }

    /// **BC-60** ②：**本实例**当前正在处理的异常（**借用**）。
    pub fn current_exception(&self) -> Option<NonNull<Header>> {
        self.exception_state.borrow().last().copied()
    }

    /// **BC-60** ②：当前异常状态的层数（诊断用——`PUSH_EXC_INFO`／`POP_EXCEPT` 配对着用）。
    pub fn exception_depth(&self) -> usize {
        self.exception_state.borrow().len()
    }

    /// **BC-60** ②：压入一个正在处理的异常（**新引用**，由实例接手）。
    pub fn push_exception(&self, exception: NonNull<Header>) {
        self.exception_state.borrow_mut().push(exception);
    }

    /// **BC-60** ②：弹出当前异常（交出一份**新引用**）。
    pub fn pop_exception(&self) -> Option<NonNull<Header>> {
        self.exception_state.borrow_mut().pop()
    }

    /// 最近一次抛出的异常（**借用**；`ExecError::Raised` 借它保活）。
    pub fn pending_exception(&self) -> Option<NonNull<Header>> {
        self.pending_exception.get()
    }

    /// 记下最近一次抛出的异常（**新引用**，由实例接手；旧的那份交出去由调用方释放）。
    pub fn set_pending_exception(
        &self,
        exception: Option<NonNull<Header>>,
    ) -> Option<NonNull<Header>> {
        self.pending_exception.replace(exception)
    }

    /// **`LOAD_BUILD_CLASS`** 压的那个内建（`__build_class__`；**借用**）。
    ///
    /// 参照实现从 `builtins` 取它；Pyawa 还没有 `builtins` 模块（`P3-14`），故先按实例存一个。
    pub fn build_class(&self) -> Option<NonNull<Header>> {
        self.build_class.get()
    }

    /// **OM-23**：本实例的单例表。
    pub fn singletons(&self) -> &Singletons {
        self.singletons
            .get()
            .expect("单例表在 Instance::new 中引导，必然存在")
    }

    /// **OM-40**：从裸引用**现取**一个守卫（取得一份新引用），用完即还。
    ///
    /// 载荷里只能存裸引用；要真正使用它，必须经这个访问器借出守卫。
    pub fn own(&self, raw: NonNull<Header>) -> PyRef<'_> {
        // SAFETY: 调用方（载荷的 traverse／clear）保证 raw 指向本实例的存活对象；
        // 这里为它新增一份引用，交给守卫负责归还。
        unsafe { self.incref_object(raw.as_ptr()) };
        // SAFETY: 同上。
        unsafe { PyRef::from_raw(raw, self) }
    }

    /// 元类型：类型对象自身的类型。
    pub fn metatype(&self) -> NonNull<TypeObject> {
        self.metatype
            .get()
            .expect("元类型在 Instance::new 中引导，必然存在")
    }

    /// **OM-3**：本实例当前占用的字节数（能力接口不承担预算，见 `CP-8`）。
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated.get()
    }

    /// 本实例中尚未释放的普通对象数（类型对象不计）。
    pub fn live_objects(&self) -> usize {
        self.live.borrow().len()
    }

    /// **OM-25**：当前参与循环回收（`GC_TRACKED`）的对象数。
    pub fn tracked_objects(&self) -> usize {
        self.gc_count.get()
    }

    /// **OM-15**：本实例注册的类型对象数。
    pub fn type_count(&self) -> usize {
        self.types.borrow().len()
    }

    /// **OM-26**：回收阈值三元组。默认值见 [`DEFAULT_GC_THRESHOLD`]。
    ///
    /// 后两位**存而不生效**（单代，**临时**）；它们照样要能读回来，纯 Python 层会解三元组。
    pub fn gc_threshold(&self) -> (usize, usize, usize) {
        self.gc_threshold.get()
    }

    /// **OM-26**：设置阈值三元组。`t0` **0 会被拒绝**——那等于每次分配都回收；
    /// `t1`／`t2` 只存不生效（单代，**临时**）。
    pub fn set_gc_threshold(&self, threshold: (usize, usize, usize)) {
        assert!(threshold.0 > 0, "OM-26：阈值必须可配置且不为 0");
        self.gc_threshold.set(threshold);
    }

    /// 在**本实例**的堆上分配一个对象，返回**新引用**（**OM-16**）。
    ///
    /// `value` 由 `T::new(ty, …)` 构造（见 [`crate::py_object!`]）；类型取自它的头部。
    /// **OM-1**：对象只属于本实例，不能跨实例共享。
    /// **OM-26**：分配计数达阈值时自动触发一次回收。
    pub fn alloc<'a, T: PyObject>(&'a self, value: T) -> Owned<'a, T> {
        let ptr = self.adopt(value);
        self.gc_alloc_count.set(self.gc_alloc_count.get() + 1);
        if self.gc_alloc_count.get() >= self.gc_threshold.get().0 {
            // 新对象此刻计数为 1、还没有交出去，因此在可达性分析里是根（不会被误回收）。
            self.collect();
        }

        Owned::new(ptr, self)
    }

    /// 把一个已构造好的对象交给本实例托管（记账 ＋ 入链 ＋ 标 `GC_TRACKED`），
    /// 返回它的指针；**引用由实例自己持有**。
    ///
    /// 引导期（`Instance::new` 造单例时）用不了 `Owned`——那需要先借出 `&Instance`。
    fn adopt<T: PyObject>(&self, value: T) -> NonNull<T> {
        let ty = value.header().ty();
        let size = core::mem::size_of::<T>();
        debug_assert_eq!(
            size,
            // SAFETY: ty 由某个实例的类型注册表持有（OM-15），在实例存活期间有效；
            // 这里在 debug 下用它校验类型元数据与 Rust 布局一致。
            unsafe { ty.as_ref() }.instance_size,
            "类型的 instance_size 与 Rust 布局不一致"
        );

        let ptr = NonNull::from(Box::leak(Box::new(value)));
        let header = ptr.cast::<Header>();

        // **OM-12**：可成环的类型必须标记 GC_TRACKED。本层用"是否提供 traverse 槽位"判定；
        // 类型对象自身也会成环（bases／dict），但它的 traverse／clear 待接线后再补标记。
        let tracked = unsafe { ty.as_ref() }.slots.traverse.is_some();
        if tracked {
            // SAFETY: header 指向刚刚分配、尚未交给其他代码的对象。
            unsafe { header.as_ref() }.set_flag(flags::GC_TRACKED);
        }

        self.live.borrow_mut().insert(header.as_ptr() as usize);
        self.bytes_allocated.set(self.bytes_allocated.get() + size);
        if tracked {
            self.link_gc(header);
        }
        ptr
    }

    /// **OM-15**：注册一个新类型。
    ///
    /// 返回的指针在本实例存活期间**稳定**：类型对象由注册表持有一份引用，不随普通对象回收。
    pub fn new_type(
        &self,
        name: &'static str,
        instance_size: usize,
        slots: Slots,
    ) -> NonNull<TypeObject> {
        let ty = self.alloc_type_raw(name, instance_size, slots);
        // CPython 里"没写基类"的类继承 `object`；MRO 仍由 C3 算（OM-13）
        let object_type = self
            .type_named("object")
            .expect("object 在 Instance::new 的引导期就已登记");
        assert!(
            self.register_bases(ty, vec![object_type]).is_some(),
            "新类型的 MRO 应当总能算出来"
        );
        ty
    }

    /// **OM-22**：`sys.getrefcount` 的可见语义——返回值**含参数借用**的那一份。
    pub fn getrefcount<T: PyObject>(&self, object: &T) -> u32 {
        object.header().refcount() + 1
    }

    /// 增加一个引用。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象。
    pub unsafe fn incref_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        unsafe { &*ptr }.incref();
    }

    /// **OM-20** ①：把**正在终结**的对象复活——计数从 0 回到 1。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中一个正在执行终结器的对象（`FINALIZING` 已置位）。
    pub unsafe fn resurrect_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        let header = unsafe { &*ptr };
        debug_assert!(
            header.has_flag(flags::FINALIZING),
            "只有终结器执行中的对象可以被复活（OM-20）"
        );
        header.set_refcount(header.refcount() + 1);
    }

    /// 释放一个**新引用**（**OM-16**）；计数归零时按 **OM-20** 的顺序处理：
    /// ① 终结器（可复活）→ ② `clear` → ③ 释放。清空走 **OM-21** 的待处理栈，不朴素递归。
    ///
    /// # Safety
    ///
    /// `ptr` 必须指向本实例中**存活**的对象，且调用方交出的是一份**新引用**。
    pub unsafe fn release_object(&self, ptr: *mut Header) {
        // SAFETY: 由调用方保证 ptr 有效。
        let header = unsafe { &*ptr };
        // **OM-24**：M1 的 `IMMORTAL` 位恒为 0；这里只是防御，不承担语义。
        if header.is_immortal() {
            return;
        }
        if header.decref() != 0 {
            return;
        }

        // 回收进行中：不可达对象由本次 `collect` 统一释放，这里只减计数（OM-27 ④）。
        if !self.gc_frozen.borrow().is_empty() && self.gc_frozen.borrow().contains(&(ptr as usize)) {
            return;
        }

        // SAFETY: ptr 非空（调用方保证）。
        self.pending
            .borrow_mut()
            .push(unsafe { NonNull::new_unchecked(ptr) });

        if self.draining.get() {
            // 已经在清空栈里：交给最外层那次调用处理。
            return;
        }
        self.draining.set(true);
        loop {
            let next = self.pending.borrow_mut().pop();
            let Some(object) = next else { break };
            self.release_one(object);
        }
        self.draining.set(false);
    }

    /// **OM-25**…**OM-30**：跑一次标记-清除，返回本次释放的对象数。
    ///
    /// 顺序按 **OM-27** 固定：① 求不可达集合 → ② 先清弱引用 → ③ 调终结器 → ④ 释放。
    /// 回收范围仅限 `GC_TRACKED` 对象（**OM-25**）；不可达但尚未释放的对象**禁止**暴露（**OM-30**）。
    pub fn collect(&self) -> usize {
        if self.gc_running.get() {
            // 终结器／clear 里又触发了一次回收：本次让路，交给外层那次。
            return 0;
        }
        self.gc_running.set(true);
        let freed = self.collect_inner();
        self.gc_running.set(false);
        freed
    }

    /// [`Instance::collect`] 的主体；进入前 `gc_running` 已置位。
    fn collect_inner(&self) -> usize {
        let unreachable = self.find_unreachable();
        self.gc_alloc_count.set(0);
        if unreachable.is_empty() {
            return 0;
        }

        // ② 先清弱引用：§10 尚未接线（`HAS_WEAKREFS` 位也还没人置位），
        //    这里是顺序上的占位点——弱引用回调必须早于终结器（OM-27、PEP 442）。

        // ③ 终结器：对每个不可达对象至多调用一次；终结器可以复活对象（OM-20 ①）。
        //    终结期间同样"冻结"这批对象：终结器可能释放环内引用，提前释放会让我们
        //    手里的指针失效——OM-27 要求先全部终结、再统一释放。
        *self.gc_frozen.borrow_mut() = unreachable
            .iter()
            .map(|header| header.as_ptr() as usize)
            .collect();
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
                let header_ref = unsafe { header.as_ref() };
                if !header_ref.has_flag(flags::FINALIZING) {
                    header_ref.set_flag(flags::FINALIZING);
                    // SAFETY: header 是本实例的存活对象。
                    unsafe { finalize(header.as_ptr(), self) };
                }
            }
        }

        // 终结器可能复活对象、也可能让别的对象重新变可达（PEP 442）⇒ 重算不可达集合。
        let unreachable = self.find_unreachable();
        let garbage: HashSet<usize> = unreachable.iter().map(|h| h.as_ptr() as usize).collect();

        // 复活的对象要清掉 FINALIZING，之后它再次死亡时还能再终结一次（OM-20）。
        for header in self.gc_headers() {
            let header_ref = unsafe { header.as_ref() };
            if header_ref.has_flag(flags::FINALIZING) && !garbage.contains(&(header.as_ptr() as usize))
            {
                header_ref.clear_flag(flags::FINALIZING);
            }
        }

        if unreachable.is_empty() {
            self.gc_frozen.borrow_mut().clear();
            return 0;
        }

        // ④ 释放。先"冻结"这批对象：`clear` 之间的 decref 只减计数，不立即释放——
        //    否则同一环里的对象会被逐个提前释放，而本函数还持有它们的指针。
        *self.gc_frozen.borrow_mut() = garbage;
        for header in &unreachable {
            let ty = unsafe { header.as_ref() }.ty();
            if let Some(clear) = unsafe { ty.as_ref() }.slots.clear {
                // SAFETY: header 是本实例的存活对象，且尚未释放（刚被冻结）。
                unsafe { clear(header.as_ptr(), self) };
            }
            // **OM-14**：另行挂载的实例字典在同一阶段交出去（`OM-27` ④）
            if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
                // SAFETY: 这份引用由该对象持有。
                unsafe { self.release_object(mapping.as_ptr()) };
            }
        }
        self.gc_frozen.borrow_mut().clear();

        for header in &unreachable {
            self.free_garbage(*header);
        }
        unreachable.len()
    }

    /// **OM-20**：单个对象的释放三步（正常引用计数路径）。
    fn release_one(&self, ptr: NonNull<Header>) {
        let ty = unsafe { ptr.as_ref() }.ty();

        // ① 终结器（`__del__`）：置 FINALIZING 防止重入；可以复活（把计数改回 > 0）。
        if let Some(finalize) = unsafe { ty.as_ref() }.slots.finalize {
            let header = unsafe { ptr.as_ref() };
            if !header.has_flag(flags::FINALIZING) {
                header.set_flag(flags::FINALIZING);
                // SAFETY: ptr 是本实例的存活对象，计数已归零且仍在待处理栈上。
                unsafe { finalize(ptr.as_ptr(), self) };
                let header = unsafe { ptr.as_ref() };
                if header.refcount() != 0 {
                    // 复活：清除 FINALIZING 并**放弃释放**（OM-20）。
                    header.clear_flag(flags::FINALIZING);
                    return;
                }
            }
        }

        // ② 清空持有的引用：释放它们（可能再入待处理栈）。
        if let Some(clear) = unsafe { ty.as_ref() }.slots.clear {
            // SAFETY: 同 ①。
            unsafe { clear(ptr.as_ptr(), self) };
        }
        // ②′ **OM-14**：另行挂载的实例字典也是本对象持有的一份引用
        if let Some(mapping) = unsafe { ptr.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由本对象持有。
            unsafe { self.release_object(mapping.as_ptr()) };
        }

        // ③ 释放内存。
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(ptr);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // SAFETY: 计数为 0，且 clear 已把持有的引用交出（OM-20 ③ 的前提）。
        unsafe { dealloc(ptr.as_ptr()) };
    }

    /// **OM-27** ④：释放一个不可达对象（`clear` 已经跑过，这里不再调终结器）。
    fn free_garbage(&self, header: NonNull<Header>) {
        let ty = unsafe { header.as_ref() }.ty();
        // **OM-14**：正常路径下 clear 阶段已经把这一格交出去了；万一还在，这里补一次释放
        if let Some(mapping) = unsafe { header.as_ref() }.take_instance_dict() {
            // SAFETY: 这份引用由该对象持有（或曾经持有）。
            unsafe { self.release_object(mapping.as_ptr()) };
        }
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        let size = unsafe { ty.as_ref() }.instance_size;
        self.unlink(header);
        self.bytes_allocated.set(self.bytes_allocated.get() - size);
        // SAFETY: 该对象已由可达性分析判为不可达，且 clear 已完成。
        unsafe { dealloc(header.as_ptr()) };
    }

    /// **OM-29**／**OM-30**：求不可达的跟踪对象。
    ///
    /// 两步：先按"引用计数 − 来自跟踪对象内部的引用数"找出根（外部引用 > 0），
    /// 再从根出发按 `traverse` 标记；没被标记的就是不可达集合。
    fn find_unreachable(&self) -> Vec<NonNull<Header>> {
        let candidates = self.gc_headers();
        if candidates.is_empty() {
            return Vec::new();
        }

        let index: HashMap<usize, usize> = candidates
            .iter()
            .enumerate()
            .map(|(position, candidate)| (candidate.as_ptr() as usize, position))
            .collect();

        let mut external: Vec<u32> = candidates
            .iter()
            // SAFETY: 候选都在本实例的回收链表上，即尚未释放。
            .map(|candidate| unsafe { candidate.as_ref() }.refcount())
            .collect();

        for candidate in &candidates {
            for child in self.children_of(*candidate) {
                if let Some(&position) = index.get(&(child as usize)) {
                    external[position] = external[position].saturating_sub(1);
                }
            }
        }

        let mut marked = vec![false; candidates.len()];
        let mut stack: Vec<usize> = (0..candidates.len()).filter(|i| external[*i] > 0).collect();
        while let Some(position) = stack.pop() {
            if marked[position] {
                continue;
            }
            marked[position] = true;
            for child in self.children_of(candidates[position]) {
                if let Some(&child_position) = index.get(&(child as usize)) {
                    if !marked[child_position] {
                        stack.push(child_position);
                    }
                }
            }
        }

        candidates
            .iter()
            .zip(marked)
            .filter(|(_, reached)| !*reached)
            .map(|(candidate, _)| *candidate)
            .collect()
    }

    /// 按 `traverse` 槽位取一个对象的直接引用（**OM-12**／**OM-29**／**OM-36**）。
    fn children_of(&self, header: NonNull<Header>) -> Vec<*mut Header> {
        let ty = unsafe { header.as_ref() }.ty();
        let mut children = Vec::new();
        // **OM-14**／**OM-36**：另行挂载的实例字典也算本对象的直接引用（漏报会永久泄漏）
        if let Some(mapping) = unsafe { header.as_ref() }.instance_dict() {
            children.push(mapping.as_ptr());
        }
        if let Some(traverse) = unsafe { ty.as_ref() }.slots.traverse {
            // SAFETY: header 是本实例的存活对象；回调只收集指针，不做解引用。
            unsafe { traverse(header.as_ptr(), &mut |child| children.push(child)) };
        }
        children
    }

    /// 回收链表上的全部对象（**OM-25**：只有 `GC_TRACKED` 入链）。
    fn gc_headers(&self) -> Vec<NonNull<Header>> {
        let mut result = Vec::with_capacity(self.gc_count.get());
        let mut cursor = self.gc_head.get();
        while !cursor.is_null() {
            // SAFETY: 链上的指针都由本实例分配且尚未释放。
            result.push(unsafe { NonNull::new_unchecked(cursor) });
            cursor = unsafe { (*cursor).gc_next() };
        }
        result
    }

    fn link_gc(&self, header: NonNull<Header>) {
        let head = self.gc_head.get();
        // SAFETY: header 刚分配；head 若非空则它是链上存活对象。
        unsafe {
            header.as_ref().set_gc_prev(ptr::null_mut());
            header.as_ref().set_gc_next(head);
            if !head.is_null() {
                (*head).set_gc_prev(header.as_ptr());
            }
        }
        self.gc_head.set(header.as_ptr());
        self.gc_count.set(self.gc_count.get() + 1);
    }

    fn unlink_gc(&self, header: NonNull<Header>) {
        // SAFETY: header 在本实例的回收链表上。
        let (prev, next) = unsafe {
            let header_ref = header.as_ref();
            (header_ref.gc_prev(), header_ref.gc_next())
        };
        if prev.is_null() {
            self.gc_head.set(next);
        } else {
            // SAFETY: prev 是链上存活对象。
            unsafe { (*prev).set_gc_next(next) };
        }
        if !next.is_null() {
            // SAFETY: next 是链上存活对象。
            unsafe { (*next).set_gc_prev(prev) };
        }
        // SAFETY: 同上。
        unsafe {
            header.as_ref().set_gc_prev(ptr::null_mut());
            header.as_ref().set_gc_next(ptr::null_mut());
        }
        self.gc_count.set(self.gc_count.get() - 1);
    }

    /// 从"存活集合"与回收链表上同时摘除。
    fn unlink(&self, header: NonNull<Header>) {
        self.live.borrow_mut().remove(&(header.as_ptr() as usize));
        // SAFETY: header 尚未释放。
        if unsafe { header.as_ref() }.has_flag(flags::GC_TRACKED) {
            self.unlink_gc(header);
        }
    }

    fn alloc_type_raw(
        &self,
        name: &'static str,
        instance_size: usize,
        slots: Slots,
    ) -> NonNull<TypeObject> {
        // 引导期（元类型自身）还没有类型可指，先用悬垂但非空的指针占位；随后立刻写回自指。
        let ty = self
            .metatype
            .get()
            .unwrap_or_else(NonNull::<TypeObject>::dangling);
        let object = TypeObject::new(
            ty,
            name,
            RefCell::new(Vec::new()),
            RefCell::new(Vec::new()),
            Cell::new(0),
            slots,
            RefCell::new(None),
            instance_size,
        );
        let ptr = NonNull::from(Box::leak(Box::new(object)));
        // 类型对象也走同一本账（OM-3），但由注册表持有：不进 `live`，销毁时统一释放（OM-2）。
        self.bytes_allocated
            .set(self.bytes_allocated.get() + core::mem::size_of::<TypeObject>());
        self.types.borrow_mut().push(ptr);
        ptr
    }
}

impl Default for Instance {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Instance {
    /// **OM-2**：实例销毁**必须**释放其堆内全部内存，**无论循环是否被回收过**——
    /// 不依赖回收器先跑完。
    fn drop(&mut self) {
        // 正常路径下 `live` 已经空了。仍有残留 ⇒ 计数环或未交出的引用，
        // 此时**强制释放**：不调终结器、不再 clear（环的语义已由 §9 的回收器负责，
        // 走到这里说明调用方没有 collect，而不是回收器做不到）。
        //
        // 本层能这样做，是因为生命周期把 `Owned` 钉在 `&Instance` 上：对象载荷里
        // **不可能**存着 `Owned` 守卫（那需要 `&'static Instance`），所以这里释放
        // 任何一个对象都不会回头去碰别的对象。
        let live: Vec<usize> = self.live.borrow().iter().copied().collect();
        for address in live {
            let header = unsafe { NonNull::new_unchecked(address as *mut Header) };
            // SAFETY: 每个地址都由本实例分配且尚未释放；类型对象在下一段之前一直存活。
            unsafe { Self::force_free(header) };
        }

        let types = core::mem::take(&mut *self.types.borrow_mut());
        for ty in types {
            // SAFETY: 类型对象由注册表持有，销毁时统一释放（OM-15）。它的类型就是元类型，
            // 可能已被释放，因此直接按 `TypeObject` 释放，不再读 `ty()`。
            unsafe { TypeObject::dealloc(ty.cast::<Header>().as_ptr()) };
        }

        self.gc_head.set(ptr::null_mut());
        self.gc_count.set(0);
        self.bytes_allocated.set(0);
    }
}

impl Instance {
    /// **OM-2** 的实例销毁路径：不调终结器、不做 clear。
    ///
    /// # Safety
    ///
    /// `header` 必须由本实例分配、尚未释放，且其载荷**不得**持有 `Owned` 守卫。
    unsafe fn force_free(header: NonNull<Header>) {
        // SAFETY: 调用方保证 header 有效；类型对象在本次销毁的第二段才释放。
        let ty = unsafe { header.as_ref() }.ty();
        let dealloc = unsafe { ty.as_ref() }.slots.dealloc;
        // SAFETY: 调用方保证。
        unsafe { dealloc(header.as_ptr()) };
    }
}

/// 字符串的引号形态（实测参照实现：能用单引号就用单引号；内容里有单引号而**没有**双引号时
/// 改用双引号）。`ascii` 为真时把非 ASCII 字符转义（`ascii()` 的语义）。
///
/// **临时**：`repr` 的完整规则属于类型自己的槽位（`OM-11`），接线后由那里说了算。
pub(crate) fn quote_str(text: &str, ascii: bool) -> String {
    let has_single = text.contains('\'');
    let has_double = text.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if character == quote => {
                out.push('\\');
                out.push(character);
            }
            _ if ascii && !character.is_ascii() => {
                let code = character as u32;
                if code <= 0xFF {
                    out.push_str(&format!("\\x{code:02x}"));
                } else if code <= 0xFFFF {
                    out.push_str(&format!("\\u{code:04x}"));
                } else {
                    out.push_str(&format!("\\U{code:08x}"));
                }
            }
            _ => out.push(character),
        }
    }
    out.push(quote);
    out
}
