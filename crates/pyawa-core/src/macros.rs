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
                // **先核尾哨兵** ✓（第 240 轮）：越界写 ⇒ 报类型与地址并 `abort` ✓（**别**再放过去 ✓）。
                // SAFETY: 由调用方保证（见上）；OM-5 保证头部就在对象地址上。
                let size = unsafe { (*ptr).ty().as_ref() }.instance_size;
                // SAFETY: ptr 来自本类型的一次分配 ✓，尾部有 `CANARY_BYTES` ✓。
                unsafe { $crate::canary_check(ptr, size) };
                // 载荷析构 ✓，再按"名字节 ＋ 哨兵"的布局整块释放 ✓（与 `Instance::adopt` 对称 ✓）。
                // SAFETY: 由调用方保证（见上）。
                unsafe { ::core::ptr::drop_in_place(ptr.cast::<$name>()) };
                let footprint = size + $crate::CANARY_BYTES;
                let layout = ::core::alloc::Layout::from_size_align(
                    footprint,
                    ::core::mem::align_of::<$name>(),
                )
                .expect("载荷加哨兵的布局一定合法");
                // SAFETY: 这块内存正是按同一布局分配的 ✓。
                unsafe { ::std::alloc::dealloc(ptr.cast(), layout) };
            }
        }
    };
}
