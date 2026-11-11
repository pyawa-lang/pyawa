//! **迭代器推进** —— 从 `format.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! 为什么单独成文件：`advance_iterator` 是**迭代器协议**的推进（内建迭代器 ＋ `__next__` 通道 ＋
//! itertools 状态机），与 `format` 族无关；先前它随 format 族一起落在 `format.rs` ⇒ 文件职责错位
//! （`format.rs` 1358 行里 1024 行是它）。调用方仍走 `crate::executor::advance_iterator`
//! （父模块 `pub(crate) use` 再导出）✓。

use crate::executor::*;
use crate::HotId;
use crate::header::Header;
use crate::instance::Instance;
use crate::IteratorObject;
use crate::TupleObject;
use core::ptr::NonNull;

/// **专用推进分支链**（round 145 提取）：只有"有专用分支"的迭代器类型才走这里 ✓
/// （调用方先用 [`Instance::is_dedicated_iterator_type`] 一次哈希查过滤 ✓ —— 普通
/// `list_iterator`／`range_iterator` 不再逐个比 20+ 内建名 ✗）。
fn advance_dedicated(
    instance: &Instance,
    iterator: NonNull<Header>,
    opcode: u8,
) -> Option<Result<Option<NonNull<Header>>, ExecError>> {
    // SAFETY: iterator 是存活对象（调用方已确认是迭代器）✓。
    let ty = unsafe { iterator.as_ref() }.ty();
    if instance.hot_type_is(HotId::ListReverseIterator, ty) {
        return Some(advance_list_reverseiterator(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Reversed, ty) {
        return Some(advance_reversed(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Count, ty) {
        return Some(advance_count(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Repeat, ty) {
        return Some(advance_repeat(instance, iterator, opcode));
    }
    // **`range()` 的两种迭代器也走这条分支** ✓（第 228 轮 ✗）：它们是**同一份载荷**（`ItStateObject` ✓）
    // ⇒ 只是**类型名**不同 ✓（参照分 `range_iterator`／`longrange_iterator` ✓）⇒ 这里必须一并认 ✓，
    // 否则改型之后会掉进 `__next__` 协议 ⇒ 迭代当场失败 ✗（实测：语料三条变红 ✓）。
    if instance.hot_type_is(HotId::Islice, ty)
        || instance.hot_type_is(HotId::RangeIterator, ty)
        || instance.hot_type_is(HotId::LongRangeIterator, ty) {
        return Some(advance_islice(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Product, ty) {
        return Some(advance_product(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Permutations, ty) {
        return Some(advance_permutations(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Compress, ty) {
        return Some(advance_compress(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Combinations, ty)
        || instance.hot_type_is(HotId::CombinationsWithReplacement, ty) {
        return Some(advance_combinations(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Zip, ty) {
        return Some(advance_zip(instance, iterator, opcode));
    }
    // **内建 `map`／`filter`**（`P3-41` round 41）：参照里它们是**惰性迭代器** ✓。
    if instance.hot_type_is(HotId::Map, ty) {
        return Some(advance_map(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Filter, ty) {
        return Some(advance_filter(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::ZipLongest, ty) {
        return Some(advance_zip_longest(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Pairwise, ty) {
        return Some(advance_pairwise(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Batched, ty) {
        return Some(advance_batched(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Cycle, ty) {
        return Some(advance_cycle(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Accumulate, ty) {
        return Some(advance_accumulate(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Starmap, ty) {
        return Some(advance_starmap(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Takewhile, ty)
        || instance.hot_type_is(HotId::Dropwhile, ty)
        || instance.hot_type_is(HotId::Filterfalse, ty) {
        return Some(advance_takewhile(instance, iterator, opcode));
    }
    if instance.hot_type_is(HotId::Chain, ty) {
        return Some(advance_chain(instance, iterator, opcode));
    }
    None
}

pub(crate) fn advance_iterator(
    instance: &Instance,
    iterator: NonNull<Header>,
    opcode: u8,
) -> Result<Option<NonNull<Header>>, ExecError> {
    // SAFETY: iterator 是存活对象。
    let ty = unsafe { iterator.as_ref() }.ty();
    if !is_iterator_type(instance, ty) {
        // 不是内建迭代器 ⇒ 走 **`__next__` 协议**（`OM-11` 的属性通道）；
        // 耗尽（`StopIteration`）与参照一致地给 `None`。
        let method = match attribute_optional(instance, iterator, "__next__") {
            Ok(Some(method)) => method,
            Ok(None) => {
                let iterator_type = unsafe { iterator.as_ref() }.ty();
                let name = instance.type_name(iterator_type).to_owned();
                if crate::diag::flag("PYAWA_ITER_DEBUG") {
                    eprintln!(
                        "[advance_iterator] 不可迭代：type={name} site={}",
                        instance.current_site()
                    );
                }
                // **用户自定义类的迭代器 ⇒ 参照口径的 `TypeError`**（round 135）：`test_iter.test_3720`
                // 的 `BadIterator` 在 `__next__` 里 `del BadIterator.__next__` ⇒ 参照抛
                // `TypeError: 'BadIterator' object is not iterable` 且被测试捕获 ✓；先前落 `Unsupported`
                //（宿主当"未接线"直接 abort）✗ ⇒ `test_iter` 整档被首格截断 ✓。
                // **内建迭代器保持 `Unsupported`**（既有口径）：改成 `TypeError` 会让 `test_itertools`
                // 的 10M 空链用例走异常路径变慢（113 s ⇒ 越过 120 s 线 ⇒ 该套件 ok→timeout ✗，实测过 ✓）。
                if instance.is_user_type_object(iterator_type) {
                    instance.release(iterator);
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        &format!("'{name}' object is not iterable"),
                    ));
                }
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "这个对象既不是内建迭代器，也没有 `__next__`",
                });
            }
            Err(error) => return Err(error),
        };
        let result = call_value(instance, method, &[], &[]);
        release(instance, method);
        return match result {
            Ok(value) => Ok(Some(value)),
            Err(ExecError::Raised { exception }) => {
                // SAFETY: exception 是存活对象。
                let raised = unsafe { exception.as_ref() }.ty();
                if instance.is_subtype(raised, exception_type(instance, "StopIteration")) {
                    // **吞异常要连 `pending_exception` 槽那份一起还**（round 169）：`raise()` 给槽加过
                    // 一份保活；只还 `Err` 那份 ⇒ 槽永久钉住"异常＋回溯＋帧" ✗（`discard_raised` 一处真相 ✓）。
                    crate::discard_raised(instance, exception);
                    Ok(None)
                } else {
                    Err(ExecError::Raised { exception })
                }
            }
            Err(other) => Err(other),
        };
    }
    // **`range` 迭代器快路径**（round 145 性能）：`for i in range(N)` 是最高频循环 ⇒ 先直接认
    // 两种 range 迭代器（类型名 O(1) 查）再走专用分支过滤 ✓（先前要过 6+ 个内建名比较 ✗）。
    if instance.hot_type_is(HotId::RangeIterator, ty)
        || instance.hot_type_is(HotId::LongRangeIterator, ty)
    {
        return advance_islice(instance, iterator, opcode);
    }
    // **专用分支过滤**（round 145 性能）：普通迭代器一次哈希查就跳过整条链 ✓。
    if instance.is_dedicated_iterator_type(ty) {
        if let Some(result) = advance_dedicated(instance, iterator, opcode) {
            return result;
        }
    }
    // **序列协议迭代器**（round 85）：`target[index]`（走**属性通道** ⇒ 用户的 `__getitem__` ✓），
    // `IndexError` ⇒ 耗尽（结果不再交出）✓；下标只在**成功**后推进 ✓。
    if instance.hot_type_is(HotId::Iterator, ty) {
        let object = unsafe { &*iterator.as_ptr().cast::<crate::builtin_objects::SequenceIteratorObject>() };
        // **耗尽后永久耗尽**（round 187）：参照撞过 `IndexError` 后不再取下标 ✓
        //（`test_iter.test_sinkstate_sequence`：耗尽后把 `a.n` 改大也必须是 `[]` ✓）。
        if object.exhausted() {
            return Ok(None);
        }
        let target = object.target();
        let index = object.index();
        let key = instance.new_int(index as i64);
        let outcome = crate::executor::subscript::subscript_read(instance, target, key);
        release(instance, key);
        return match outcome {
            Ok(value) => {
                object.set_index(index + 1);
                Ok(Some(value))
            }
            Err(ExecError::Raised { exception }) => {
                let raised = unsafe { exception.as_ref() }.ty();
                let index_error = exception_type(instance, "IndexError");
                // **`StopIteration` 也当耗尽**（round 187）：序列协议里 `__getitem__` 抛
                // `StopIteration` 的旧式序列照参照**结束循环** ✓（`test_iter.test_stop_sequence` ✓）。
                let stop_iteration = instance
                    .hot_type(HotId::StopIteration)
                    .expect("StopIteration 在引导期已登记");
                if instance.is_subtype(raised, index_error)
                    || instance.is_subtype(raised, stop_iteration)
                {
                    crate::discard_raised(instance, exception);
                    // **标记永久耗尽**（round 187）。
                    object.set_exhausted();
                    Ok(None)
                } else {
                    Err(ExecError::Raised { exception })
                }
            }
            Err(other) => Err(other),
        };
    }
    // **`iter(callable, sentinel)`**（round 85）：每步调 `callable()`，结果 `== sentinel` ⇒ 耗尽
    // （**结果本身不再交出** ✓；参照 `callable_iterator` ✓）。两个字段是自有引用、指向迭代器外，
    // 载荷与 `IteratorObject` 不同 ⇒ 必须在通用强转**之前**分派 ✓。
    if instance.hot_type_is(HotId::CallableIterator, ty) {
        let object = unsafe { &*iterator.as_ptr().cast::<crate::builtin_objects::CallableIteratorObject>() };
        // **耗尽后永久耗尽**（round 187）：参照撞到哨兵后不再调 callable ✓
        //（`test_iter.test_sinkstate_callable` 的第二次 `list(b) == []` ✓）。
        if object.exhausted() {
            return Ok(None);
        }
        let callable = object.callable();
        let sentinel = object.sentinel();
        let result = call_value(instance, callable, &[], &[])?;
        // **重入耗尽**（round 200，gh-101892）：`callable` 自己把本迭代器耗尽了（例如它内部
        // `list(iterator)`）——参照的口径是"**本次结果不再交出**，直接 `StopIteration`" ✓；
        // 先前只看"返回值 == 哨兵" ⇒ 重入耗尽后仍把结果交出去 ✗（`test_iter.test_iter_function_
        // concealing_reentrant_exhaustion` 实测 "StopIteration not raised" ✓）。
        if object.exhausted() {
            release(instance, result);
            return Ok(None);
        }
        if crate::executor::values_equal_public(instance, result, sentinel) {
            release(instance, result);
            object.set_exhausted();
            return Ok(None);
        }
        return Ok(Some(result));
    }
    // SAFETY: 类型身份已确认是 IteratorObject 的某个类型。
    let object = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
    // **字典迭代中改键 ⇒ `RuntimeError`**（`P3-41` round 42；参照口径）——
    // 先前不检测 ⇒ `for i in d: d[i+1] = 1` 的循环**永不结束**（`test_dict.test_mutating_iteration`）。
    // 文案两档照参照：长度变了 ⇒ `changed size`；只变键集 ⇒ `changed keys` ✓。
    // **`items()`／`values()` 的迭代器同样要检测**（round 119）：参照里对 dict 的
    // key／value／item 三种迭代器都报 `RuntimeError: dictionary changed size during iteration` ✓
    //（`test_dict.test_mutating_iteration_delete_over_items`／`..._values` ✓）。
    // 用**缓存指针**比较（这几个类型是**按需建**的：惰性建之前不存在 ✓，缓存按代数重建 ✓）。
    let dict_iterator = instance.hot_type_is(HotId::DictKeyIterator, ty)
        || instance.hot_type_is(HotId::DictValueIterator, ty)
        || instance.hot_type_is(HotId::DictItemIterator, ty);
    if dict_iterator {
        // SAFETY: 类型身份刚确认。
        let dict = unsafe { &*object.target().as_ptr().cast::<crate::builtin_objects::DictObject>() };
        let changed = match object.check_dict_version(instance) {
            Err(()) => true,
            Ok(_) => false,
        };
        if changed {
            let message = if object.guard_len() != dict.len() {
                "dictionary changed size during iteration"
            } else {
                "dictionary keys changed during iteration"
            };
            return Err(instance.raise_builtin_error("RuntimeError", message));
        }
    }
    // **耗尽哨兵**（round 110）：吐过 `StopIteration` 就永久耗尽 ✓（见 `mark_exhausted` 注释）。
    if object.is_exhausted() {
        return Ok(None);
    }
    let target = object.target();
    let index = object.index();
    let dict_iterator = instance.hot_type_is(HotId::DictValueIterator, ty)
        || instance.hot_type_is(HotId::DictItemIterator, ty)
        || instance.hot_type_is(HotId::DictKeyIterator, ty);
    let item = if dict_iterator {
        let dict = unsafe { &*target.as_ptr().cast::<DictObject>() };
        let Some((position, (key, value))) = dict.next_live_entry(index) else {
            object.mark_exhausted();
            return Ok(None);
        };
        object.set_index(position + 1);
        if instance.hot_type_is(HotId::DictValueIterator, ty) {
            instance.retain(value)
        } else if instance.hot_type_is(HotId::DictItemIterator, ty) {
            instance.new_tuple(vec![instance.retain(key), instance.retain(value)])
        } else {
            instance.retain(key)
        }
    } else {
        let length = iterable_length(instance, target, opcode)?;
        if index >= length {
            object.mark_exhausted();
            return Ok(None);
        }
        let item = iterable_item(instance, target, index, opcode)?;
        object.advance();
        item
    };
    Ok(Some(item))
}

/// `list_reverseiterator` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_list_reverseiterator(instance: &Instance, iterator: NonNull<Header>, _opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // **反向遍历** ✓（第 227 轮）：`index` 存"**下一个要取的下标 ＋ 1**" ✓ ⇒ `0` ＝ 取完 ✓
        //（空表 ⇒ 初值 0 ⇒ 立刻耗尽 ✓，不必另设哨兵 ✓）。
        // SAFETY: 类型身份刚确认。
        let state = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
        let index = state.index.get();
        if index == 0 {
            return Ok(None);
        }
        state.index.set(index - 1);
        // SAFETY: target 由本对象持有一份引用，存活。
        let target = state.target;
        let list = unsafe { &*target.as_ptr().cast::<crate::builtin_objects::ListObject>() };
        let value = list.items()[index - 1];
        // SAFETY: value 由列表持有，这里新增一份引用交给调用方。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(Some(value));
}

/// `reversed` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_reversed(instance: &Instance, iterator: NonNull<Header>, _opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // **通用反向迭代** ✓（第 727 轮）：序列协议 `x[index]` ✓；`index` 是"下一个要取的下标 ＋ 1" ✓。
        // SAFETY: 类型身份刚确认。
        let state = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
        let index = state.index.get();
        if index == 0 {
            return Ok(None);
        }
        state.index.set(index - 1);
        let target = state.target;
        let key = instance.new_int((index - 1) as i64);
        // **`x[i]` 的同一处实现** ✓（内建序列与 `__getitem__` 都认 ✓）。
        let result = crate::executor::subscript::subscript_read(instance, target, key);
        release(instance, key);
        return result.map(Some);
}

/// `count` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_count(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // `itertools.count`：先吐当前值，再按步长推进。**只含整数**（不持引用）。
        // 越过 `i64` ⇒ 如实报"未接线"（任意精度的口径还没裁，见契约 §5.2.6）——**不静默回绕**。
        // SAFETY: 类型身份刚确认。
        let state = unsafe { &*iterator.as_ptr().cast::<crate::builtin_objects::CountIteratorObject>() };
        let value = state.current();
        if state.bump().is_none() {
            return Err(ExecError::Unsupported {
                opcode,
                what: "itertools.count 的下一个值超出本层 i64 范围（任意精度的口径待裁）",
            });
        }
        return Ok(Some(instance.new_int(value)));
}

/// `repeat` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_repeat(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Repeat { value, remaining } = state.kind() else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "repeat 的状态不是 Repeat",
            });
        };
        if remaining == 0 {
            return Ok(None);
        }
        if remaining > 0 {
            state.set_kind(crate::builtin_objects::ItStateKind::Repeat {
                value,
                remaining: remaining - 1,
            });
        }
        // `advance` 的约定：返回**新引用**
        // SAFETY: value 由本迭代器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(Some(value));
}

/// `islice` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_islice(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        loop {
            let crate::builtin_objects::ItStateKind::Islice {
                inner,
                start,
                position,
                stop,
                step,
            } = state.kind()
            else {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "islice 的状态不是 Islice",
                });
            };
            // 耗尽点：**实测**是 `max(start, stop)`（`start >= stop` 也照样消费到 `start`）；
            // `stop < 0` ⇒ 无上界
            let bound = if stop < 0 { i64::MAX } else { start.max(stop) };
            if position >= bound {
                return Ok(None);
            }
            // 从内层取一个：内层耗尽也 ⇒ 耗尽（短输入照参照）
            let Some(item) = advance_iterator(instance, inner, opcode)? else {
                return Ok(None);
            };
            let position = position + 1;
            let index = position - 1;
            let wanted = index >= start
                && (stop < 0 || index < stop)
                && (index - start) % step == 0;
            state.set_kind(crate::builtin_objects::ItStateKind::Islice {
                inner,
                start,
                position,
                stop,
                step,
            });
            if wanted {
                return Ok(Some(item));
            }
            // 跳过这一个（不在让出序列上）：归还引用后继续
            release(instance, item);
        }
}

/// `product` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_product(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Product {
            pools,
            indices,
            started,
            done,
        } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "product 的状态不对",
            });
        };
        if done {
            return Ok(None);
        }
        // SAFETY: pools 与 indices 都是本迭代器持有的 list。
        let pool_list = unsafe { &*pools.as_ptr().cast::<crate::ListObject>() };
        let count = pool_list.len();
        let current: Vec<i64> = {
            // SAFETY: indices 是本迭代器持有的 list。
            let cursor = unsafe { &*indices.as_ptr().cast::<crate::ListObject>() };
            (0..cursor.len())
                .map(|index| {
                    cursor
                        .item(index)
                        .and_then(|value| instance.int_value(value))
                        .unwrap_or(0)
                })
                .collect()
        };
        // 各池的长度
        let mut lengths: Vec<i64> = Vec::with_capacity(count);
        for position in 0..count {
            let pool = pool_list.item(position).expect("下标在范围内");
            // SAFETY: 池是本迭代器持有的 list。
            lengths.push(unsafe { &*pool.as_ptr().cast::<crate::ListObject>() }.len() as i64);
        }
        let positions: Option<Vec<i64>> = if !started {
            // **无输入** ⇒ 产出**一个空元组**（实测 `product()` ⇒ `[()]`）；
            // 任一池为空 ⇒ 直接穷尽（实测 `product([1], [])` ⇒ `[]`）
            if lengths.iter().any(|length| *length == 0) {
                None
            } else {
                Some(vec![0; count])
            }
        } else {
            // odometer：从右往左进位
            let mut next = current.clone();
            let mut position = next.len();
            let mut advanced = false;
            while position > 0 {
                position -= 1;
                let limit = lengths.get(position).copied().unwrap_or(0);
                next[position] += 1;
                if next[position] < limit {
                    advanced = true;
                    break;
                }
                next[position] = 0;
            }
            if advanced {
                Some(next)
            } else {
                None
            }
        };
        let Some(positions) = positions else {
            state.set_kind(crate::builtin_objects::ItStateKind::Product {
                pools,
                indices,
                started: true,
                done: true,
            });
            return Ok(None);
        };
        let mut stored: Vec<NonNull<Header>> = Vec::with_capacity(positions.len());
        for position in &positions {
            stored.push(instance.new_int(*position));
        }
        let fresh = instance.new_list(stored);
        release(instance, indices);
        state.set_kind(crate::builtin_objects::ItStateKind::Product {
            pools,
            indices: fresh,
            started: true,
            done: false,
        });
        // 逐池按下标取值（元组接手新引用）
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(positions.len());
        for (slot, position) in positions.iter().enumerate() {
            let pool = pool_list.item(slot).expect("下标在范围内");
            // SAFETY: 池是本迭代器持有的 list。
            let pool = unsafe { &*pool.as_ptr().cast::<crate::ListObject>() };
            let value = pool.item(*position as usize).expect("下标由算法保证在范围内");
            // SAFETY: 值由池持有，元组要自己那份。
            unsafe { instance.incref_object(value.as_ptr()) };
            items.push(value);
        }
        return Ok(Some(instance.new_tuple(items)));
}

/// `permutations` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_permutations(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Permutations {
            pool,
            r,
            indices,
            started,
            done,
        } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "permutations 的状态不对",
            });
        };
        if done {
            return Ok(None);
        }
        // SAFETY: pool 与 indices 都是本迭代器持有的 list。
        let pool_list = unsafe { &*pool.as_ptr().cast::<crate::ListObject>() };
        let n = pool_list.len() as i64;
        let positions: Vec<i64> = if !started {
            // `r > n` ⇒ 直接穷尽（实测）；`r = 0` ⇒ 产出**一个空元组**（实测）
            if r > n {
                state.set_kind(crate::builtin_objects::ItStateKind::Permutations {
                    pool,
                    r,
                    indices,
                    started: true,
                    done: true,
                });
                return Ok(None);
            }
            (0..r).collect()
        } else {
            // SAFETY: indices 是本迭代器持有的 list。
            let current = unsafe { &*indices.as_ptr().cast::<crate::ListObject>() };
            let mut next: Vec<i64> = (0..current.len())
                .map(|index| {
                    current
                        .item(index)
                        .and_then(|value| instance.int_value(value))
                        .unwrap_or(0)
                })
                .collect();
            // 字典序的下一个"互不相同下标"元组（实测顺序正是它）：
            // 从右往左找第一个还能**变大**的位置，把它换成本位之后最小的**未用**值，
            // 再把它右边各位依次填成最小的未用值（递增）
            let mut advanced = false;
            let mut position = next.len();
            while position > 0 {
                position -= 1;
                let used: Vec<i64> = next[..position].to_vec();
                let mut candidate = next[position] + 1;
                while candidate < n && used.contains(&candidate) {
                    candidate += 1;
                }
                if candidate < n {
                    next[position] = candidate;
                    // 右边依次填最小的未用值
                    for follow in (position + 1)..next.len() {
                        let mut fill = 0;
                        while used.contains(&fill) || next[position..follow].contains(&fill) {
                            fill += 1;
                        }
                        next[follow] = fill;
                    }
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                state.set_kind(crate::builtin_objects::ItStateKind::Permutations {
                    pool,
                    r,
                    indices,
                    started: true,
                    done: true,
                });
                return Ok(None);
            }
            next
        };
        let mut stored: Vec<NonNull<Header>> = Vec::with_capacity(positions.len());
        for position in &positions {
            stored.push(instance.new_int(*position));
        }
        let fresh = instance.new_list(stored);
        release(instance, indices);
        state.set_kind(crate::builtin_objects::ItStateKind::Permutations {
            pool,
            r,
            indices: fresh,
            started: true,
            done: false,
        });
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(positions.len());
        for position in &positions {
            let value = pool_list
                .item(*position as usize)
                .expect("下标由算法保证在范围内");
            // SAFETY: 值由池持有，元组要自己那份。
            unsafe { instance.incref_object(value.as_ptr()) };
            items.push(value);
        }
        return Ok(Some(instance.new_tuple(items)));
}

/// `compress` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_compress(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Compress { data, selectors } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "compress 的状态不对",
            });
        };
        loop {
            // 选择器或数据任一先尽 ⇒ 结束（实测短选择器就停）
            let Some(selector) = advance_iterator(instance, selectors, opcode)? else {
                return Ok(None);
            };
            let Some(item) = advance_iterator(instance, data, opcode)? else {
                release(instance, selector);
                return Ok(None);
            };
            let keep = match truthiness(instance, selector, opcode) {
                Ok(value) => value,
                Err(error) => {
                    release(instance, selector);
                    release(instance, item);
                    return Err(error);
                }
            };
            release(instance, selector);
            if keep {
                return Ok(Some(item));
            }
            release(instance, item);
        }
}

/// `combinations` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_combinations(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Combinations {
            pool,
            r,
            indices,
            started,
            done,
            replace,
        } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "combinations 的状态不对",
            });
        };
        if done {
            return Ok(None);
        }
        // SAFETY: pool 与 indices 都是本迭代器持有的 list。
        let pool_list = unsafe { &*pool.as_ptr().cast::<crate::ListObject>() };
        let n = pool_list.len() as i64;
        // `r` 为 0 ⇒ 产出**一个空元组**（实测 `combinations([1,2,3], 0)` ⇒ `[()]`）；
        // `r > n` ⇒ 直接穷尽（实测 `combinations([1,2,3], 4)` ⇒ `[]`）
        let positions: Vec<i64> = if !started {
            // `r > n` 在**可重复**那一支是合法的（实测 `cwr([1,2], 3)` 有 4 个）
            if r > n && !replace {
                state.set_kind(crate::builtin_objects::ItStateKind::Combinations {
                    pool,
                    r,
                    indices,
                    started: true,
                    done: true,
                    replace,
                });
                return Ok(None);
            }
            // **可重复**那一支从"下标全 0"起步（非降序的起点）；不可重复从 `0..r` 起步
            if replace {
                vec![0; r as usize]
            } else {
                (0..r).collect()
            }
        } else {
            // 标准的下一个组合：从右往左找第一个还能加的下标
            // SAFETY: indices 是本迭代器持有的 list。
            let current = unsafe { &*indices.as_ptr().cast::<crate::ListObject>() };
            let mut next: Vec<i64> = (0..current.len())
                .map(|index| current.item(index).and_then(|v| instance.int_value(v)).unwrap_or(0))
                .collect();
            let mut position = next.len();
            let mut advanced = false;
            while position > 0 {
                position -= 1;
                if replace {
                    // **可重复**：右起第一个还能变大的位置；它右边全部设成同一个值
                    if next[position] + 1 < n {
                        let value = next[position] + 1;
                        for follow in position..next.len() {
                            next[follow] = value;
                        }
                        advanced = true;
                        break;
                    }
                } else if next[position] != (n - (r - position as i64)) {
                    next[position] += 1;
                    for follow in (position + 1)..next.len() {
                        next[follow] = next[follow - 1] + 1;
                    }
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                state.set_kind(crate::builtin_objects::ItStateKind::Combinations {
                    pool,
                    r,
                    indices,
                    started: true,
                    done: true,
                    replace,
                });
                return Ok(None);
            }
            next
        };
        // 存下标（新 list 换旧的）
        let mut stored: Vec<NonNull<Header>> = Vec::with_capacity(positions.len());
        for position in &positions {
            stored.push(instance.new_int(*position));
        }
        let fresh = instance.new_list(stored);
        release(instance, indices);
        state.set_kind(crate::builtin_objects::ItStateKind::Combinations {
            pool,
            r,
            indices: fresh,
            started: true,
            done: false,
            replace,
        });
        // 按下标取池里的项（元组接手新引用）
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(positions.len());
        for position in &positions {
            let value = pool_list
                .item(*position as usize)
                .expect("下标由算法保证在范围内");
            // SAFETY: 值由池持有，元组要自己那份。
            unsafe { instance.incref_object(value.as_ptr()) };
            items.push(value);
        }
        return Ok(Some(instance.new_tuple(items)));
}

/// `zip` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_zip(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // **`zip` 取最短** ✓（第 229 轮）：与 `zip_longest` 同一份载荷 ✓ ⇒ 区别只在"**缺项就收摊**" ✓。
        // SAFETY: 类型身份刚确认。
        let state = unsafe { &*iterator.as_ptr().cast::<crate::builtin_objects::ItStateObject>() };
        let crate::builtin_objects::ItStateKind::Zip { iterators } = state.kind() else {
            return Err(ExecError::Unsupported { opcode, what: "zip 的状态不对" });
        };
        // SAFETY: iterators 是本迭代器持有的 list。
        let list = unsafe { &*iterators.as_ptr().cast::<crate::ListObject>() };
        // 空参数 ⇒ 立刻耗尽 ✓（实测 `list(zip())` ⇒ `[]` ✓，**不报错** ✓）
        if list.is_empty() {
            return Ok(None);
        }
        let mut row: Vec<NonNull<Header>> = Vec::with_capacity(list.len());
        for index in 0..list.len() {
            let inner = list.item(index).expect("下标在范围内");
            match advance_iterator(instance, inner, opcode)? {
                Some(item) => row.push(item),
                None => {
                    // **取最短** ✓：有一个到头 ⇒ 本轮已取的**都归还** ✓、整个迭代器收摊 ✓。
                    for item in row {
                        release(instance, item);
                    }
                    return Ok(None);
                }
            }
        }
        return Ok(Some(instance.new_tuple(row)));
}

/// 内建 **`map`** 的推进分支（`P3-41` round 41：从"急求值成列表"改正为惰性）。
///
/// 语义同 `zip`（各 inner 各取一项，**任一耗尽就收摊** ✓）＋ 每行交 `function(*row)` ✓；
/// 函数抛异常 ⇒ 原样上抛（本轮已取的项都归还 ✓）。
fn advance_map(
    instance: &Instance,
    iterator: NonNull<Header>,
    opcode: u8,
) -> Result<Option<NonNull<Header>>, ExecError> {
    // SAFETY: 类型身份刚确认。
    let state = unsafe { &*iterator.as_ptr().cast::<crate::builtin_objects::ItStateObject>() };
    let crate::builtin_objects::ItStateKind::Map {
        function,
        iterators,
    } = state.kind()
    else {
        return Err(ExecError::Unsupported {
            opcode,
            what: "map 的状态不对",
        });
    };
    // SAFETY: iterators 是本迭代器持有的 list。
    let list = unsafe { &*iterators.as_ptr().cast::<crate::ListObject>() };
    if list.is_empty() {
        // `map(f)` 不合法（至少两个实参）⇒ 这里只是防御；空表 ⇒ 立刻耗尽。
        return Ok(None);
    }
    let mut row: Vec<NonNull<Header>> = Vec::with_capacity(list.len());
    for index in 0..list.len() {
        let inner = list.item(index).expect("下标在范围内");
        match advance_iterator(instance, inner, opcode)? {
            Some(item) => row.push(item),
            None => {
                for item in row {
                    release(instance, item);
                }
                return Ok(None);
            }
        }
    }
    // **`row` 是 `advance_iterator` 交来的新引用**（round 96 真泄漏修 ✗）：`call_value` 自己会
    // 为实参 `incref` 一份 ✓ ⇒ 这里必须把**本函数的**那份还掉 ✗（先前直接漏还 ⇒ `map` 消费的每个
    // 元素永不释放 ⇒ 官方 `test_genexps` 的「Verify re-use of tuples」拿不到 freelist 复用、
    // 且 `map` 在大流上内存一路涨 ✓）。
    let outcome = call_value(instance, function, &row, &[]);
    for item in row {
        release(instance, item);
    }
    outcome.map(Some)
}

/// 内建 **`filter`** 的推进分支（`P3-41` round 41：从"急求值成列表"改正为惰性）。
///
/// `predicate` 为 `None` ⇒ 按**真值**筛（参照语义 ✓）；否则「谓词为真就让出、为假继续取」✓。
fn advance_filter(
    instance: &Instance,
    iterator: NonNull<Header>,
    opcode: u8,
) -> Result<Option<NonNull<Header>>, ExecError> {
    // SAFETY: 类型身份刚确认。
    let state = unsafe { &*iterator.as_ptr().cast::<crate::builtin_objects::ItStateObject>() };
    let crate::builtin_objects::ItStateKind::Filter { predicate, inner } = state.kind() else {
        return Err(ExecError::Unsupported {
            opcode,
            what: "filter 的状态不对",
        });
    };
    loop {
        let Some(item) = advance_iterator(instance, inner, opcode)? else {
            return Ok(None);
        };
        // **真值判定与"让出那份引用"是两件事**（round 88 真 bug 修 ✗）：先前 `predicate is None`
        // 分支把 `item` 当 `verdict` 一起 `release`，再 `return Ok(Some(item))` ✗ ⇒ **返回的是
        // 已经释放的指针** ⇒ 调用方（`str.join`／`list` 等）拿它再放一次就是双放／堆损坏 ✗
        // （实测最小复现：`prog = '%(p)s' % dict(p="p"); ' '.join(filter(None, [prog]));
        // prog.removeprefix(...)` 在 `PYAWA_QUARANTINE=1` 下报"incref 撞上已释放对象（str）" ✗；
        // 这也让 `test_platform`／`test_random`／`test_urlparse`／`test_webbrowser` 在
        // `PYAWA_DANGLING=1` 下报内存安全类失败 ✓）。
        // 正确口径：**判真值期间 `item` 一直归本函数** ✓；`truthy` 才把那份引用**交出去**，
        // 否则归还 ✓。有 predicate 时 predicate 的返回值另算一份（`call_value` 会 incref 实参 ✓）。
        let truthy = if predicate == instance.singletons().none() {
            match truthiness(instance, item, opcode) {
                Ok(value) => value,
                Err(error) => {
                    release(instance, item);
                    return Err(error);
                }
            }
        } else {
            let verdict = match call_value(instance, predicate, &[item], &[]) {
                Ok(value) => value,
                Err(error) => {
                    release(instance, item);
                    return Err(error);
                }
            };
            let truthy = match truthiness(instance, verdict, opcode) {
                Ok(value) => value,
                Err(error) => {
                    release(instance, verdict);
                    release(instance, item);
                    return Err(error);
                }
            };
            release(instance, verdict);
            truthy
        };
        if truthy {
            return Ok(Some(item));
        }
        release(instance, item);
    }
}

/// `zip_longest` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_zip_longest(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::ZipLongest {
            iterators,
            fillvalue,
        } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "zip_longest 的状态不对",
            });
        };
        // SAFETY: iterators 是本迭代器持有的 list。
        let list = unsafe { &*iterators.as_ptr().cast::<crate::ListObject>() };
        // 空参数 ⇒ 立刻耗尽（实测 `zip_longest()` ⇒ `[]`，**不报错**）
        if list.is_empty() {
            return Ok(None);
        }
        let mut row: Vec<NonNull<Header>> = Vec::with_capacity(list.len());
        let mut items: Vec<Option<NonNull<Header>>> = Vec::with_capacity(list.len());
        let mut any = false;
        for index in 0..list.len() {
            let inner = list.item(index).expect("下标在范围内");
            match advance_iterator(instance, inner, opcode)? {
                Some(item) => {
                    any = true;
                    items.push(Some(item));
                }
                None => items.push(None),
            }
        }
        if !any {
            // 全耗尽：把已经攒下的补齐值归还
            for item in items.into_iter().flatten() {
                release(instance, item);
            }
            return Ok(None);
        }
        for item in items {
            match item {
                Some(item) => row.push(item),
                None => {
                    // 补齐值：元组接手一份新引用
                    // SAFETY: fillvalue 由本迭代器持有，存活。
                    unsafe { instance.incref_object(fillvalue.as_ptr()) };
                    row.push(fillvalue);
                }
            }
        }
        return Ok(Some(instance.new_tuple(row)));
}

/// `pairwise` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_pairwise(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        loop {
            let crate::builtin_objects::ItStateKind::Pairwise { inner, previous } =
                state.kind()
            else {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "pairwise 的状态不对",
                });
            };
            let Some(item) = advance_iterator(instance, inner, opcode)? else {
                // 内层耗尽：上一项那份引用随迭代器一起放着（等 `clear`）
                return Ok(None);
            };
            match previous {
                // 头一项只当"上一项"，不产出
                None => {
                    state.set_kind(crate::builtin_objects::ItStateKind::Pairwise {
                        inner,
                        previous: Some(item),
                    });
                    continue;
                }
                Some(last) => {
                    // 产出 `(上一项, 当前项)`：元组接手两份新引用
                    // SAFETY: 两者都存活。
                    unsafe {
                        instance.incref_object(last.as_ptr());
                        instance.incref_object(item.as_ptr());
                    }
                    let pair = instance.new_tuple(vec![last, item]);
                    // 上一项那份（迭代器持有的那份）归还，换成当前项
                    state.set_kind(crate::builtin_objects::ItStateKind::Pairwise {
                        inner,
                        previous: Some(item),
                    });
                    release(instance, last);
                    return Ok(Some(pair));
                }
            }
        }
}

/// `batched` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_batched(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Batched { inner, size, done } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "batched 的状态不对",
            });
        };
        if done {
            return Ok(None);
        }
        let mut batch: Vec<NonNull<Header>> = Vec::with_capacity(size as usize);
        while (batch.len() as i64) < size {
            match advance_iterator(instance, inner, opcode)? {
                Some(item) => batch.push(item),
                None => {
                    state.set_kind(crate::builtin_objects::ItStateKind::Batched {
                        inner,
                        size,
                        done: true,
                    });
                    break;
                }
            }
        }
        if batch.is_empty() {
            return Ok(None);
        }
        // `new_tuple` **接手**这些引用（里面已经都是新引用）
        return Ok(Some(instance.new_tuple(batch)));
}

/// `cycle` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_cycle(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        loop {
            let crate::builtin_objects::ItStateKind::Cycle {
                inner,
                cache,
                filling,
                index,
            } = state.kind()
            else {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "cycle 的状态不对",
                });
            };
            if filling {
                // 还在从内层取：取到就**边取边缓存**（惰性，实测如此）
                match advance_iterator(instance, inner, opcode)? {
                    Some(item) => {
                        // 缓存接手一份引用（`ListObject::append` 的约定），返回值自己那份
                        // SAFETY: item 是存活对象。
                        unsafe { instance.incref_object(item.as_ptr()) };
                        // SAFETY: cache 是本迭代器持有的 list。
                        unsafe { &*cache.as_ptr().cast::<crate::ListObject>() }.append(item);
                        return Ok(Some(item));
                    }
                    None => {
                        state.set_kind(crate::builtin_objects::ItStateKind::Cycle {
                            inner,
                            cache,
                            filling: false,
                            index: 0,
                        });
                        continue;
                    }
                }
            }
            // 重放缓存；缓存为空 ⇒ 耗尽（实测 `cycle([])` ⇒ 空）
            // SAFETY: cache 是本迭代器持有的 list。
            let list = unsafe { &*cache.as_ptr().cast::<crate::ListObject>() };
            if list.is_empty() {
                return Ok(None);
            }
            let length = list.len() as i64;
            let item = list.item(index as usize).expect("游标在范围内");
            // SAFETY: item 由缓存持有，存活。
            unsafe { instance.incref_object(item.as_ptr()) };
            state.set_kind(crate::builtin_objects::ItStateKind::Cycle {
                inner,
                cache,
                filling: false,
                index: (index + 1) % length,
            });
            return Ok(Some(item));
        }
}

/// `accumulate` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_accumulate(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Accumulate {
            inner,
            function,
            total,
        } = state.kind()
        else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "accumulate 的状态不对",
            });
        };
        let Some(item) = advance_iterator(instance, inner, opcode)? else {
            return Ok(None);
        };
        let Some(previous) = total else {
            // 头一个元素既是累计值也是要吐的值（`total` 自己持一份）
            // SAFETY: item 是存活对象。
            unsafe { instance.incref_object(item.as_ptr()) };
            state.set_kind(crate::builtin_objects::ItStateKind::Accumulate {
                inner,
                function,
                total: Some(item),
            });
            return Ok(Some(item));
        };
        // 有累计函数就走普通调用；没有就按**加法**（本层 `BINARY_OP` 目前只做整数 ⇒ 同口径）
        let next = match function {
            Some(callable) => call_value(instance, callable, &[previous, item], &[]),
            None => concat_public(instance, previous, item, opcode),
        };
        release(instance, item);
        let next = match next {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        // 换上新的累计值（旧的那份归还）
        release(instance, previous);
        // SAFETY: next 是新引用，缓存自己持一份。
        unsafe { instance.incref_object(next.as_ptr()) };
        state.set_kind(crate::builtin_objects::ItStateKind::Accumulate {
            inner,
            function,
            total: Some(next),
        });
        return Ok(Some(next));
}

/// `starmap` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_starmap(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        let crate::builtin_objects::ItStateKind::Starmap { inner, function } = state.kind() else {
            return Err(ExecError::Unsupported {
                opcode,
                what: "starmap 的状态不对",
            });
        };
        let Some(item) = advance_iterator(instance, inner, opcode)? else {
            return Ok(None);
        };
        // 元素要能**展开**成实参：本层认 tuple 与 list（实测 `starmap(pow, [1])` ⇒
        // `'int' object is not iterable`）
        // SAFETY: item 是存活对象。
        let item_type = unsafe { item.as_ref() }.ty();
        let arguments: Option<Vec<NonNull<Header>>> = if instance.hot_type_is(HotId::Tuple, item_type)
        {
            // SAFETY: 类型身份已确认。
            let tuple = unsafe { &*item.as_ptr().cast::<TupleObject>() };
            Some((0..tuple.len()).filter_map(|index| tuple.item(index)).collect())
        } else if instance.hot_type_is(HotId::List, item_type) {
            // SAFETY: 类型身份已确认。
            let list = unsafe { &*item.as_ptr().cast::<crate::ListObject>() };
            Some((0..list.len()).filter_map(|index| list.item(index)).collect())
        } else {
            None
        };
        let result = match arguments {
            Some(arguments) => {
                let outcome = call_value(instance, function, &arguments, &[]);
                outcome
            }
            None => {
                release(instance, item);
                return Err(raise_builtin(
                    instance,
                    "TypeError",
                    "'int' object is not iterable",
                ));
            }
        };
        release(instance, item);
        return result.map(Some);
}

/// `takewhile` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_takewhile(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        loop {
            let crate::builtin_objects::ItStateKind::FilterLike {
                inner,
                predicate,
                mode,
                state: flag,
            } = state.kind()
            else {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "谓词迭代器的状态不对",
                });
            };
            // `takewhile` 停过就永久耗尽
            if mode == 0 && flag {
                return Ok(None);
            }
            let Some(item) = advance_iterator(instance, inner, opcode)? else {
                return Ok(None);
            };
            // 谓词走**普通调用**（`OM-11` 的 call 槽；异常照上抛）
            let verdict = match call_value(instance, predicate, &[item], &[]) {
                Ok(value) => value,
                Err(error) => {
                    release(instance, item);
                    return Err(error);
                }
            };
            let truthy = match truthiness(instance, verdict, opcode) {
                Ok(value) => value,
                Err(error) => {
                    release(instance, verdict);
                    release(instance, item);
                    return Err(error);
                }
            };
            release(instance, verdict);
            match mode {
                // `takewhile`：谓词为假 ⇒ 停（这一项**不产出**）
                0 => {
                    if truthy {
                        return Ok(Some(item));
                    }
                    release(instance, item);
                    state.set_kind(crate::builtin_objects::ItStateKind::FilterLike {
                        inner,
                        predicate,
                        mode,
                        state: true,
                    });
                    return Ok(None);
                }
                // `dropwhile`：还没出过 ⇒ 谓词为真就丢；一旦出过就原样给
                1 => {
                    if flag {
                        return Ok(Some(item));
                    }
                    if truthy {
                        release(instance, item);
                        continue;
                    }
                    state.set_kind(crate::builtin_objects::ItStateKind::FilterLike {
                        inner,
                        predicate,
                        mode,
                        state: true,
                    });
                    return Ok(Some(item));
                }
                // `filterfalse`：谓词为真 ⇒ 丢
                _ => {
                    if truthy {
                        release(instance, item);
                        continue;
                    }
                    return Ok(Some(item));
                }
            }
        }
}

/// `chain` 的推进分支（从 `advance_iterator` 原样搬来）。
fn advance_chain(instance: &Instance, iterator: NonNull<Header>, opcode: u8) -> Result<Option<NonNull<Header>>, ExecError> {
        // SAFETY: 类型身份刚确认。
        let state = unsafe {
            &*iterator
                .as_ptr()
                .cast::<crate::builtin_objects::ItStateObject>()
        };
        loop {
            let crate::builtin_objects::ItStateKind::Chain { outer, current } = state.kind() else {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "chain 的状态不是 Chain",
                });
            };
            if let Some(inner) = current {
                if let Some(item) = advance_iterator(instance, inner, opcode)? {
                    return Ok(Some(item));
                }
                // 内层用完 ⇒ 放掉它，换下一个（`None` 那份引用归本迭代器，这里归还）
                release(instance, inner);
                state.set_kind(crate::builtin_objects::ItStateKind::Chain {
                    outer,
                    current: None,
                });
                continue;
            }
            let Some(next_iterable) = advance_iterator(instance, outer, opcode)? else {
                return Ok(None);
            };
            // 元素必须是可迭代：`iter_value` 借用入参、返回新引用；不是可迭代就按实测消息报错
            let inner = match iter_value(instance, next_iterable) {
                Ok(inner) => inner,
                Err(error) => {
                    release(instance, next_iterable);
                    return Err(error);
                }
            };
            release(instance, next_iterable);
            state.set_kind(crate::builtin_objects::ItStateKind::Chain {
                outer,
                current: Some(inner),
            });
        }
}

