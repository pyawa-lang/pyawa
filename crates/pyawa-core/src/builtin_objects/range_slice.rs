//! 本文件是 `builtin_objects` 的 `range_slice` 域：`range`／`slice` 的载荷、构造、下标与 `repr`。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;




/// **`slice` 的属性面**（第 151 轮）：`start`／`stop`／`step` ✓（省略的那段给 `None` ✓ —— 与参照
/// 同义 ✓）。注册在 `slice` 类型的 `getattr` 槽上 ✓（**一处真相** ✓）。
pub unsafe fn slice_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SliceObject>() };
    let value = match name {
        "start" => object.start(),
        "stop" => object.stop(),
        "step" => object.step(),
        _ => return None,
    };
    Some(match value {
        Some(number) => instance.new_int(number),
        None => instance.retain(instance.singletons().none()),
    })
}

/// 取绑定的整数（方法契约保证有 ✓）。
pub(crate) fn bound_int(instance: &Instance, bound: Option<NonNull<Header>>) -> Result<i64, crate::ExecError> {
    let owner = bound.ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "descriptor needs an argument")
    })?;
    instance.int_value(owner).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "descriptor needs an int")
    })
}


py_object! {
    /// **`range` 的载荷** ✓（第 627 轮）：三个 `i64`（`start`／`stop`／`step` ✓）＋"是否大整数上限"这一位 ✓。
    /// **不持有对象引用** ✓ ⇒ 槽只要 `Slots::new(Self::dealloc)` ✓（照 `SliceObject` ✓）。
    pub struct RangeObject {
        start: i64,
        stop: i64,
        step: i64,
        /// 上限超出 `i64` 时**饱和**为 `i64::MAX` ✓ ⇒ 迭代器给 `longrange_iterator` ✓（照旧 ✓）。
        long_range: bool,
    }
}

impl RangeObject {
    pub fn start(&self) -> i64 {
        self.start
    }
    pub fn stop(&self) -> i64 {
        self.stop
    }
    pub fn step(&self) -> i64 {
        self.step
    }
    pub fn long_range(&self) -> bool {
        self.long_range
    }
    /// 元素个数 ✓（参照口径 ✓，**含负步长** ✓）。
    pub fn count(&self) -> i64 {
        let (start, stop, step) = (self.start, self.stop, self.step);
        if step > 0 {
            if stop <= start {
                0
            } else {
                (stop - start).saturating_add(step - 1) / step
            }
        } else if stop >= start {
            0
        } else {
            (start - stop).saturating_add(-step - 1) / (-step)
        }
    }
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}

/// 造一个 `range` 实例 ✓（类型就是 `range` ✓ —— 迭代器另造 ✓）。
pub(crate) fn new_range(instance: &Instance, start: i64, stop: i64, step: i64, long_range: bool) -> NonNull<Header> {
    let ty = instance
        .type_named("range")
        .expect("引导期已登记 range 类型");
    instance
        .alloc(RangeObject::new(ty, start, stop, step, long_range))
        .into_raw()
        .cast::<Header>()
}

/// 取 `range` 载荷 ✓（**先验类型** ✓ —— 这条通道只会拿到 `range` 实例 ✓，仍如实挡住别的 ✓）。
pub(crate) fn range_payload<'a>(instance: &Instance, object: NonNull<Header>) -> Option<&'a RangeObject> {
    if Some(instance.type_of(object)) != instance.type_named("range") {
        return None;
    }
    // SAFETY: 上面确认过类型是 `range` ⇒ 载荷就是 `RangeObject` ✓。
    Some(unsafe { &*object.as_ptr().cast::<RangeObject>() })
}

/// 把下标**按参照钳到 `[lower, upper]`** ✓（`__getitem__` 的切片那一支用 ✓）。
pub(crate) fn clamp_slice_index(value: i64, count: i64, lower: i64, upper: i64, step: i64) -> i64 {
    let mut index = value;
    if index < 0 {
        index = index.saturating_add(count);
    }
    if step < 0 {
        if index > upper {
            upper
        } else if index < lower {
            lower
        } else {
            index
        }
    } else if index < lower {
        lower
    } else if index > upper {
        upper
    } else {
        index
    }
}

/// 切片长度 ✓（`start`／`stop` 已钳好 ✓）。
pub(crate) fn slice_length(start: i64, stop: i64, step: i64) -> i64 {
    let diff = stop.saturating_sub(start);
    if step > 0 {
        if diff <= 0 {
            0
        } else {
            diff.saturating_add(step - 1) / step
        }
    } else if diff >= 0 {
        0
    } else {
        (start - stop).saturating_add(-step - 1) / (-step)
    }
}

/// `range.__len__` ✓。
pub(crate) fn range_len_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(object) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range.__len__ 没有绑定实例",
        });
    };
    let Some(range) = range_payload(instance, object) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range.__len__ 拿到了非 range 实例",
        });
    };
    Ok(instance.new_int(range.count()))
}

/// `range.__getitem__` ✓：**整数下标给整数** ✓、**切片给新 `range`** ✓（参照正是这样 ✓）。
pub(crate) fn range_getitem_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(object) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range.__getitem__ 没有绑定实例",
        });
    };
    let Some(range) = range_payload(instance, object) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range.__getitem__ 拿到了非 range 实例",
        });
    };
    let Some(key) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "range.__getitem__ 至少要一个实参"));
    };
    let count = range.count();
    if instance.type_name(instance.type_of(key)) == "slice" {
        // SAFETY: 上面确认过它是 `slice` ⇒ 载荷就是 `SliceObject`。
        let slice = unsafe { &*key.as_ptr().cast::<SliceObject>() };
        let step = slice.step().unwrap_or(1);
        if step == 0 {
            return Err(instance.raise_builtin_error("ValueError", "slice step cannot be zero"));
        }
        let lower = if step < 0 { -1 } else { 0 };
        let upper = if step < 0 { count - 1 } else { count };
        let start_index = match slice.start() {
            None => {
                if step < 0 {
                    upper
                } else {
                    lower
                }
            }
            Some(value) => clamp_slice_index(value, count, lower, upper, step),
        };
        let stop_index = match slice.stop() {
            None => {
                if step < 0 {
                    lower
                } else {
                    upper
                }
            }
            Some(value) => clamp_slice_index(value, count, lower, upper, step),
        };
        let new_step = range.step().saturating_mul(step);
        let new_start = range
            .start()
            .saturating_add(range.step().saturating_mul(start_index));
        let new_stop = new_start.saturating_add(new_step.saturating_mul(slice_length(start_index, stop_index, step)));
        return Ok(new_range(instance, new_start, new_stop, new_step, false));
    }
    let Some(index) = instance.index_value(key)? else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "range indices must be integers or slices, not {}",
                instance.type_name(instance.type_of(key))
            ),
        ));
    };
    let index = if index < 0 { index.saturating_add(count) } else { index };
    if index < 0 || index >= count {
        return Err(instance.raise_builtin_error("IndexError", "range object index out of range"));
    }
    Ok(instance.new_int(
        range
            .start()
            .saturating_add(range.step().saturating_mul(index)),
    ))
}

/// `range.__iter__` ✓：照旧用 `count` ＋ `islice` 这一套 ✓，但**此刻才改型**成迭代器 ✓
/// （先前是在 `range()` 里就改型 ✗ ⇒ `range` 根本不是 `range` ✓）。
pub(crate) fn range_iter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(object) = bound else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range.__iter__ 没有绑定实例",
        });
    };
    let Some(range) = range_payload(instance, object) else {
        return Err(crate::ExecError::Unsupported {
            opcode: 0,
            what: "range.__iter__ 拿到了非 range 实例",
        });
    };
    let inner = instance.new_count_iterator(range.start(), range.step());
    let iterator = instance.new_islice_iterator(inner, 0, range.count(), 1);
    let wanted = if range.long_range() {
        "longrange_iterator"
    } else {
        "range_iterator"
    };
    if let Some(ty) = instance.type_named(wanted) {
        instance.set_type_of(iterator, ty);
    }
    Ok(iterator)
}

/// `range.__repr__`：`range(0, 5)`／`range(0, 10, 3)` ✓（步长为 1 时不写第三步 ✓，与参照一致 ✓）。
pub unsafe fn range_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, crate::ExecError> {
    // SAFETY: 契约保证 ptr 是 `range` 实例 ✓。
    let range = unsafe { &*ptr.cast::<RangeObject>() };
    if range.step() == 1 {
        Ok(format!("range({}, {})", range.start(), range.stop()))
    } else {
        Ok(format!(
            "range({}, {}, {})",
            range.start(),
            range.stop(),
            range.step()
        ))
    }
}

/// `range` 的**方法面** ✓（照 `list_getattr` 的包装式 ✓）。
pub unsafe fn range_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    let handler: NativeFn = match name {
        "__len__" => range_len_native,
        "__getitem__" => range_getitem_native,
        "__iter__" => range_iter_native,
        _ => return None,
    };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "range", Cell::new(handler)));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    Some(bound.into_raw().cast::<Header>())
}

/// **`range(...)` 的构造槽** ✓（第 237 轮：从 stdlib 挪进 core ✓ —— 这样 `range` 才能是**类型** ✓，
/// 而"名字改指类型"那张表要求 `type_named("range")` 真的存在 ✓）。
///
/// **如实说** ✗：本层的 `range(n)` 给出的是**迭代器**（`islice(count(...))` ✓）⇒ 类型名取 `range` ✓
/// 以对齐参照 `type(range(n))` ✓；但 `next(range(3))` 在本层仍可用 ✗（参照会报 `TypeError` ✓）—— 既有偏差 ✓。
pub fn range_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let _ = class;
    if args.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "range expected at least 1 argument, got 0"));
    }
    let mut numbers: Vec<i64> = Vec::with_capacity(args.len().min(3));
    // **上限超出 i64** ✓（第 228 轮）：参照支持任意精度 ✓ ⇒ 本层**饱和**到 `i64::MAX` ✓ 并记下
    // `long_range` ✓ ⇒ 迭代器给 `longrange_iterator` ✓（`_collections_abc.py:77` 的 `range(1 << 1000)` ✓）。
    let mut long_range = false;
    for value in args.iter().take(3) {
        let number = if instance.type_name(instance.type_of(*value)) == "int" {
            match instance.index_value(*value)? {
                Some(number) => Some(number),
                None => {
                    long_range = true;
                    Some(i64::MAX)
                }
            }
        } else {
            instance.index_value(*value)?
        };
        let Some(number) = number else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "range() 的参数要整数或 `__index__`，拿到 {} 值 {}",
                    instance.type_name(instance.type_of(*value)),
                    instance.object_repr(*value).unwrap_or_else(|_| "<读不出>".to_owned())
                ),
            ));
        };
        numbers.push(number);
    }
    let (start, stop, step) = match numbers.as_slice() {
        [stop] => (0, *stop, 1),
        [start, stop] => (*start, *stop, 1),
        [start, stop, step] => (*start, *stop, *step),
        _ => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "range expected at most 3 arguments",
            ))
        }
    };
    if step == 0 {
        return Err(instance.raise_builtin_error("ValueError", "range() arg 3 must not be zero"));
    }
    // **给的是真 `range`** ✓（第 627 轮真偏差修 ✗）：先前在这里直接造 `islice(count(...))` 并**改型**成
    // `range_iterator` ✗ ⇒ `type(range(5))` 给迭代器 ✓、`range(5)[1:3]` 直接报"切不了 range_iterator" ✗
    //（`Lib/re` 正是踩在这上面 ✓）。负步长也一样放行 ✓（参照支持 ✓）。
    Ok(new_range(instance, start, stop, step, long_range))
}


py_object! {
    /// `slice` 的实例（`P1-12` 点名的"索引／切片"要它）。
    ///
    /// **第一刀只接线整数与 `None`**：参照允许任意对象（靠 `__index__`），那要属性通道，
    /// 随后补——非整数字段如实报实测的那条 `TypeError`，不猜。
    pub struct SliceObject {
        /// `start`（`None` ＝ 省略）。
        start: Option<i64>,
        /// `stop`。
        stop: Option<i64>,
        /// `step`。
        step: Option<i64>,
    }
}

impl SliceObject {
    /// **三段访问器**（第 151 轮，`slice.start`／`stop`／`step` 用 ✓）。
    pub fn start(&self) -> Option<i64> {
        self.start
    }
    pub fn stop(&self) -> Option<i64> {
        self.stop
    }
    pub fn step(&self) -> Option<i64> {
        self.step
    }
    /// 见 [`TupleObject::slots`]：载荷是三个 `Option<i64>`，不持有对象引用。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
    }
}

/// `slice` 的 `repr`：`slice(1, 2, 3)`／`slice(None, None, None)`（实测）。
pub unsafe fn slice_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<SliceObject>() };
    let show = |value: Option<i64>| match value {
        Some(number) => number.to_string(),
        None => "None".to_owned(),
    };
    Ok(format!(
        "slice({}, {}, {})",
        show(object.start),
        show(object.stop),
        show(object.step)
    ))
}


pub unsafe fn slice_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, crate::ExecError> {
    let field = |argument: &NonNull<Header>| -> Result<Option<i64>, crate::ExecError> {
        if Some(instance.type_of(*argument)) == instance.type_named("NoneType") {
            return Ok(None);
        }
        match instance.int_of(*argument).and_then(|value| value.to_i64()) {
            Some(value) => Ok(Some(value)),
            None => Err(instance.raise_builtin_error(
                "TypeError",
                // 实测：`b'abc'[slice("a")]` ⇒ 这条
                "slice indices must be integers or None or have an __index__ method",
            )),
        }
    };
    let (start, stop, step) = match args {
        [] => {
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "slice() 至少要一个实参",
            })
        }
        [stop] => (None, field(stop)?, None),
        [start, stop] => (field(start)?, field(stop)?, None),
        [start, stop, step] => (field(start)?, field(stop)?, field(step)?),
        _ => {
            return Err(crate::ExecError::Unsupported {
                opcode: 0,
                what: "slice() 最多三个实参",
            })
        }
    };
    // 步长为 0：实测在**切片求值**时报 `ValueError: slice step cannot be zero`
    // （`slice(1, 2, 0)` 本身可以构造）⇒ 这条留给 `subscript_get` 的切片路径报
    Ok(instance
        .alloc(SliceObject::new(class, start, stop, step))
        .into_raw()
        .cast::<Header>())
}
