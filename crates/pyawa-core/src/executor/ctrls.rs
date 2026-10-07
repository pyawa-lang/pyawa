//! **`ctrls` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `ctrls_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`MAX_REPEAT_BYTES`、`MAX_REPEAT_ITEMS`、`builtin_type`、`opcode_of`、`push` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{MAX_REPEAT_BYTES, MAX_REPEAT_ITEMS, builtin_type, opcode_of};
use crate::builtin_objects::BytesObject;
use crate::executor::ExecError;
use crate::builtin_objects::FloatObject;
use crate::header::Header;
use crate::instance::Instance;
use crate::bigint::IntValue;
use crate::builtin_objects::ListObject;
use core::ptr::NonNull;
use crate::builtin_objects::SetObject;
use crate::builtin_objects::SliceObject;
use crate::builtin_objects::StrObject;
use crate::builtin_objects::TupleObject;
use crate::type_object::TypeObject;


/// 整数载荷（int 与 bool 两种布局分开读，`TS-40`）。**含大整数**（`TS-45`）。
pub(crate) fn integer_payload(instance: &Instance, raw: NonNull<Header>) -> Option<IntValue> {
    instance.int_of(raw)
}

/// **下标**载荷 → `i64`：非整数与**超出 `i64` 的整数**分开报（"未接线"的理由不同）。
pub(crate) fn index_payload(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<i64, ExecError> {
    let Some(value) = integer_payload(instance, raw) else {
        return Err(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（字符串／浮点键与 `__index__` 尚未接线；切片走专门路径）",
        });
    };
    value.to_i64().ok_or(ExecError::Unsupported {
        opcode,
        what: "下标超出 i64 尚未接线（`TS-45` 只点名四则／整除／取模／幂）",
    })
}

/// 数值载荷（`int`／`bool`／`float`）。
pub(crate) fn numeric_payload(instance: &Instance, raw: NonNull<Header>) -> Option<f64> {
    if let Some(value) = integer_payload(instance, raw) {
        return Some(value.to_bigint().to_f64());
    }
    // SAFETY: 调用方保证 raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty == builtin_type(instance, "float") {
        // SAFETY: 类型身份已确认。
        return Some(unsafe { &*raw.as_ptr().cast::<FloatObject>() }.value());
    }
    None
}

/// 取出"可解包元素"（**新引用**的列表）。
///
/// *临时*：只管道 `tuple`／`list`／`str`（其余可迭代对象随迭代器族接线）。
/// 返回的每一项都是**新引用**——调用方要么压栈、要么释放。
pub(crate) fn sequence_items(
    instance: &Instance,
    raw: NonNull<Header>,
    _opcode: u8,   // 兜底改成迭代器协议后不再需要它 ✓（第 315 轮修警告 ✗）
) -> Result<Vec<NonNull<Header>>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let owned = |value: NonNull<Header>| {
        // SAFETY: value 是容器持有的存活对象。
        unsafe { instance.incref_object(value.as_ptr()) };
        value
    };

    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<TupleObject>() }
            .items()
            .iter()
            .copied()
            .map(owned)
            .collect());
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<ListObject>() }
            .items()
            .into_iter()
            .map(owned)
            .collect());
    }
    if ty == builtin_type(instance, "set") {
        // **集合**也走这里：`SET_UPDATE` 的源是折叠出来的 `frozenset` 常量（第 249 轮），
        // 解包一个集合在参照里本来也合法 ⇒ 元素序照集合内部序（观测面只比集合语义）
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<SetObject>() }
            .items()
            .iter()
            .copied()
            .map(owned)
            .collect());
    }
    let str_type = instance.singletons().str_type();
    if ty == str_type {
        // SAFETY: 同上。
        let text = unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned();
        // 逐字符造新的 `str` 对象（*临时*：参照实现会intern 单字符，属实现观测面）
        return Ok(text
            .chars()
            .map(|character| {
                let object = instance.alloc(StrObject::new(str_type, character.to_string()));
                object.into_raw().cast::<Header>()
            })
            .collect());
    }
    // **兜底：走迭代器协议** ✓（第 314 轮接线；tuple／list／set／str 已在上面走快路 ✓）。
    // 契约（`executor/iter.rs`／`runtime.rs` 注释 ✓）：`iter_value` 给**新引用** ✓、
    // `advance` 给 `Some(元素新引用)` ✓ ⇒ 迭代器那份用完要还 ✓、元素直接收 ✓。
    if crate::diag::flag("PYAWA_ITER_DEBUG") {
        eprintln!("[unpack] opcode={_opcode} 走迭代器兜底");
    }
    let iterator = instance.iter_object(raw)?;
    let mut collected: Vec<NonNull<Header>> = Vec::new();
    let outcome = loop {
        match instance.advance_iterator(iterator) {
            Ok(Some(item)) => collected.push(item),
            Ok(None) => break Ok(()),
            Err(error) => break Err(error),
        }
    };
    instance.release(iterator);
    outcome?;
    Ok(collected)
}

/// 把下标归一成 0 起的位置（负数从末尾数；越界返回 `None`）。
pub(crate) fn normalize_index(index: i64, length: usize) -> Option<usize> {
    let normalized = if index < 0 { index + length as i64 } else { index };
    if normalized < 0 || normalized >= length as i64 {
        return None;
    }
    Some(normalized as usize)
}

/// **切片求值**（`P1-12`）：`slice.indices(len)` 的 CPython 口径/// **切片求值**（`P1-12`）：`slice.indices(len)` 的 CPython 口径——负下标先加长度、
/// 再按步长方向夹到 `[lower, upper]`；`step == 0` 报实测的 `ValueError`。
///
/// 判据是 `tests/fixture-slice-3.14.json`（`tools/gen_slice_fixture.py` 实测：16 种切法
/// × `bytes`／`str`／`list`／`tuple`）。
pub(crate) fn slice_bounds(
    instance: &Instance,
    key: NonNull<Header>,
    length: usize,
) -> Result<(i64, i64, i64), ExecError> {
    // SAFETY: key 是存活对象，且调用方已确认它是 `slice`。
    let slice = unsafe { &*key.as_ptr().cast::<SliceObject>() };
    let step = slice.step.unwrap_or(1);
    if step == 0 {
        return Err(instance.raise_builtin_error("ValueError", "slice step cannot be zero"));
    }
    let length = length as i64;
    let (lower, upper) = if step > 0 { (0, length) } else { (-1, length - 1) };
    let adjust = |value: i64| {
        if value < 0 {
            let shifted = value + length;
            if shifted < lower {
                lower
            } else {
                shifted
            }
        } else if value > upper {
            upper
        } else {
            value
        }
    };
    let start = match slice.start {
        None => {
            if step > 0 {
                lower
            } else {
                upper
            }
        }
        Some(value) => adjust(value),
    };
    let stop = match slice.stop {
        None => {
            if step > 0 {
                upper
            } else {
                lower
            }
        }
        Some(value) => adjust(value),
    };
    Ok((start, stop, step))
}

/// 切片要取的那些下标（有序；长度天然不超过序列长度）。
pub(crate) fn slice_positions(start: i64, stop: i64, step: i64) -> Vec<usize> {
    let mut out = Vec::new();
    if step > 0 {
        let mut at = start;
        while at < stop {
            out.push(at as usize);
            at += step;
        }
    } else {
        let mut at = start;
        while at > stop {
            out.push(at as usize);
            at += step;
        }
    }
    out
}

/// 该对象该用哪个迭代器类型（名字照探测表）。
pub(crate) fn iterator_type_for(
    instance: &Instance,
    raw: NonNull<Header>,
) -> Result<NonNull<TypeObject>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let name = if ty == builtin_type(instance, "tuple") {
        "tuple_iterator"
    } else if ty == builtin_type(instance, "list") {
        "list_iterator"
    } else if instance.is_subtype(ty, builtin_type(instance, "dict")) {
        "dict_keyiterator"
    } else if ty == builtin_type(instance, "set") || Some(ty) == instance.type_named("frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮修 ✗：先前只认 `set` ⇒
        // `'frozenset' object is not iterable` ✗ —— `_collections_abc` 的注册那一套会迭代基类集合 ✓，
        // 元类路径一打通就当场踩到 ✓）。
        "set_iterator"
    } else if ty == instance.singletons().str_type() {
        "str_ascii_iterator"
    } else if Some(ty) == instance.type_named("bytes") {
        // `P1-12`：`bytes` 的迭代器（类型名照探测表）——逐个给**整数**
        "bytes_iterator"
    } else if Some(ty) == instance.type_named("bytearray") {
        // **`bytearray` 的迭代器** ✓（第 226 轮）：类型名照探测表 ✓ —— 与 `bytes_iterator` **是两个类型** ✓
        //（`_collections_abc.py:69` 的 `type(iter(bytearray()))` 要的正是这个 ✓）。
        "bytearray_iterator"
    } else {
        return Err(ExecError::Unsupported {
            opcode: opcode_of("GET_ITER"),
            what: Box::leak(
                format!(
                    "只接线了 tuple／list／dict／set／str／bytes 的内建迭代器（其余走 __iter__ 协议）；这里是 '{}'",
                    instance.type_name(ty)
                )
                .into_boxed_str(),
            ),
        });
    };
    Ok(builtin_type(instance, name))
}

/// **序列重复**（`str`／`list`／`tuple`／`bytes` `*` 整数，两个方向都认）✓。
///
/// 返回 `Ok(None)` ⇒ **两边都不是序列** ⇒ 交给整数那条路（`2 * 3` 之类 ✓）。
/// 次数为负／零 ⇒ **空序列** ✓（参照口径 ✓）。乘积过大的档口如实报 `MemoryError` ✓（不硬扛 ✗）。
pub(crate) fn sequence_repeat(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, ExecError> {
    // SAFETY: 两个都是帧值栈上的存活对象。
    let left_type = unsafe { left.as_ref() }.ty();
    let right_type = unsafe { right.as_ref() }.ty();
    let is_text_left = left_type == instance.singletons().str_type();
    let is_text_right = right_type == instance.singletons().str_type();
    let is_bytes_left = Some(left_type) == instance.type_named("bytes");
    let is_bytes_right = Some(right_type) == instance.type_named("bytes");
    let is_list_left = left_type == builtin_type(instance, "list");
    let is_list_right = right_type == builtin_type(instance, "list");
    let is_tuple_left = left_type == builtin_type(instance, "tuple");
    let is_tuple_right = right_type == builtin_type(instance, "tuple");
    let sequence_left = is_text_left || is_bytes_left || is_list_left || is_tuple_left;
    let sequence_right = is_text_right || is_bytes_right || is_list_right || is_tuple_right;
    let (sequence, count_object) = if sequence_left && !sequence_right {
        (left, right)
    } else if sequence_right && !sequence_left {
        (right, left)
    } else {
        return Ok(None);
    };
    let Some(count) = instance.int_of(count_object) else {
        return Ok(None);
    };
    let Some(count) = count.to_bigint().to_i64() else {
        return Err(instance.raise_builtin_error("OverflowError", "cannot fit 'int' into an index-sized integer"));
    };
    let count = count.max(0) as usize;
    // SAFETY: sequence 是存活对象。
    let sequence_type = unsafe { sequence.as_ref() }.ty();
    if sequence_type == instance.singletons().str_type() {
        // SAFETY: 类型身份已确认。
        let text = unsafe { &*sequence.as_ptr().cast::<StrObject>() }.value().to_owned();
        if text.len().saturating_mul(count) > MAX_REPEAT_BYTES {
            return Err(instance.raise_builtin_error("MemoryError", "string repetition too large"));
        }
        return Ok(Some(instance.new_str(&text.repeat(count))));
    }
    if Some(sequence_type) == instance.type_named("bytes") {
        // SAFETY: 同上。
        let value = unsafe { &*sequence.as_ptr().cast::<BytesObject>() }.value().to_vec();
        if value.len().saturating_mul(count) > MAX_REPEAT_BYTES {
            return Err(instance.raise_builtin_error("MemoryError", "bytes repetition too large"));
        }
        let mut repeated = Vec::with_capacity(value.len().saturating_mul(count));
        for _ in 0..count {
            repeated.extend_from_slice(&value);
        }
        return Ok(Some(instance.new_bytes(&repeated)));
    }
    let items: Vec<NonNull<Header>> = if sequence_type == builtin_type(instance, "list") {
        // SAFETY: 同上。
        let object = unsafe { &*sequence.as_ptr().cast::<ListObject>() };
        (0..object.len()).filter_map(|index| object.item(index)).collect()
    } else {
        // SAFETY: 同上。
        let object = unsafe { &*sequence.as_ptr().cast::<TupleObject>() };
        (0..object.len()).filter_map(|index| object.item(index)).collect()
    };
    if items.len().saturating_mul(count) > MAX_REPEAT_ITEMS {
        return Err(instance.raise_builtin_error("MemoryError", "sequence repetition too large"));
    }
    let mut repeated: Vec<NonNull<Header>> = Vec::with_capacity(items.len().saturating_mul(count));
    for _ in 0..count {
        for item in &items {
            // SAFETY: item 由容器持有，存活；新容器要自己那份。
            unsafe { instance.incref_object(item.as_ptr()) };
            repeated.push(*item);
        }
    }
    if sequence_type == builtin_type(instance, "list") {
        Ok(Some(instance.new_list(repeated)))
    } else {
        Ok(Some(instance.new_tuple(repeated)))
    }
}

/// 取一个已注册的**异常类**（`TS-41` 的表里那棵树）。
pub(crate) fn exception_type(instance: &Instance, name: &str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| panic!("TS-41：异常类 {name} 应当已注册"))
}
