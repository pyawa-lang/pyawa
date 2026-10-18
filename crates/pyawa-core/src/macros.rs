//! `py_object!`：声明"以头部开头"的对象类型（`docs/SPEC-object-model.md` **OM-5**）。

/// 定义一个对象类型。
///
/// 生成物：
///
/// - `#[repr(C)]` 结构体：`header` 是**第一个字段**，其余字段按声明顺序（**OM-5**）
/// - `unsafe impl PyObject`
/// - `fn new(ty, …)`：头部计数从 1 开始 —— "分配即一个新引用"（**OM-16**）
/// - `unsafe fn dealloc(*mut Header)`：**OM-11** 槽位表里的 `dealloc`
///
/// **第二臂（默认初始化字段组）**：普通字段之后可再跟一个**花括号组**（组内字段照常声明、
/// 照常进结构体布局），但 `new` **不接收**这些字段 —— 一律用 [`Default::default`] 初始化。
/// 动因：给 `FunctionObject` 加"可写属性覆盖值"这类**新字段**时不必改任何既有 `new` 调用点。
///
/// 为什么标记写成"花括号组"而不是字段属性 ✗：`macro_rules!` 不能按 `#[…]` 属性**过滤**
/// 字段（`meta` 片段不能条件匹配），而 `$ty:ty` 片段之后只允许 `{`／`[`／`,`／`;`／`:`
/// 等少数记号 —— `{` 是唯一既无歧义、又不用改既有展开的写法。
///
/// 约束：字段名不得叫 `header`（宏会加）或 `ty`（构造函数参数名）。
///
/// # 例
///
/// ```
/// use core::cell::Cell;
/// use pyawa_core::{py_object, Instance, Slots};
///
/// py_object! {
///     struct Counter { value: Cell<u64> }
/// }
///
/// let instance = Instance::new();
/// let ty = instance.new_type("Counter", core::mem::size_of::<Counter>(), Slots::new(Counter::dealloc));
/// let counter = instance.alloc(Counter::new(ty, Cell::new(0)));
/// counter.get().value.set(1);
/// assert_eq!(counter.get().value.get(), 1);
/// ```
#[macro_export]
macro_rules! py_object {
    // **第二臂**：普通字段 ＋ 一个"默认初始化字段组"（见上面文档 ✓）。
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident : $field_ty:ty
            ),* $(,)?
            // 注释在词法期就被丢掉 ⇒ 宏看不见下面这个组前面的说明 ✓
            {
                $(
                    $(#[$default_meta:meta])*
                    $default_field:ident : $default_ty:ty
                ),* $(,)?
            }
            $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(C)]
        $vis struct $name {
            /// **OM-5**：头部必须是第一个字段。
            pub header: $crate::Header,
            $(
                $(#[$field_meta])*
                pub $field: $field_ty,
            )*
            $(
                $(#[$default_meta])*
                pub $default_field: $default_ty,
            )*
        }

        // SAFETY: 上面的结构体是 `#[repr(C)]`，且 `header` 是第一个字段（OM-5）。
        unsafe impl $crate::PyObject for $name {
            fn header(&self) -> &$crate::Header {
                &self.header
            }
        }

        impl $name {
            /// 构造实例：头部计数从 1 开始，即"分配即一个新引用"（**OM-16**）。
            ///
            /// **默认初始化字段组**不在这里收（一律 `Default::default()` ✓）。
            pub fn new(
                ty: ::core::ptr::NonNull<$crate::TypeObject>,
                $($field: $field_ty),*
            ) -> Self {
                Self {
                    header: $crate::Header::new(ty),
                    $($field,)*
                    $($default_field: ::core::default::Default::default(),)*
                }
            }

            /// **OM-11** 的 `dealloc` 槽位：从头部指针还原并释放。
            ///
            /// # Safety
            ///
            /// `ptr` 必须来自本类型的一次 `Instance::alloc`；引用计数已归零，
            /// 且 `clear` 已把该对象持有的引用交出（**OM-20** 第 ③ 步的前提）。
            pub unsafe fn dealloc(ptr: *mut $crate::Header) {
                // SAFETY: 由调用方保证（见上）；OM-5 保证头部就在对象地址上。
                unsafe { ::core::mem::drop(::std::boxed::Box::from_raw(ptr.cast::<$name>())) }
            }
        }
    };
    // **第一臂（既有，原样保留）**：全部字段都由 `new` 接收。
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(C)]
        $vis struct $name {
            /// **OM-5**：头部必须是第一个字段。
            pub header: $crate::Header,
            $(
                $(#[$field_meta])*
                pub $field: $field_ty,
            )*
        }

        // SAFETY: 上面的结构体是 `#[repr(C)]`，且 `header` 是第一个字段（OM-5）。
        unsafe impl $crate::PyObject for $name {
            fn header(&self) -> &$crate::Header {
                &self.header
            }
        }

        impl $name {
            /// 构造实例：头部计数从 1 开始，即"分配即一个新引用"（**OM-16**）。
            ///
            /// 构造出来的值**还不在实例堆上**；要成为对象必须交给 `Instance::alloc`。
            pub fn new(
                ty: ::core::ptr::NonNull<$crate::TypeObject>,
                $($field: $field_ty),*
            ) -> Self {
                Self {
                    header: $crate::Header::new(ty),
                    $($field),*
                }
            }

            /// **OM-11** 的 `dealloc` 槽位：从头部指针还原并释放。
            ///
            /// # Safety
            ///
            /// `ptr` 必须来自本类型的一次 `Instance::alloc`；引用计数已归零，
            /// 且 `clear` 已把该对象持有的引用交出（**OM-20** 第 ③ 步的前提）。
            pub unsafe fn dealloc(ptr: *mut $crate::Header) {
                // SAFETY: 由调用方保证（见上）；OM-5 保证头部就在对象地址上。
                unsafe { ::core::mem::drop(::std::boxed::Box::from_raw(ptr.cast::<$name>())) }
            }
        }
    };
}
