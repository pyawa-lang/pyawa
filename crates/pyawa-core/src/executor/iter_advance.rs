//! **迭代器推进** —— 从 `format.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! 为什么单独成文件：`advance_iterator` 是**迭代器协议**的推进（内建迭代器 ＋ `__next__` 通道 ＋
//! itertools 状态机），与 `format` 族无关；先前它随 format 族一起落在 `format.rs` ⇒ 文件职责错位
//! （`format.rs` 1358 行里 1024 行是它）。调用方仍走 `crate::executor::advance_iterator`
//! （父模块 `pub(crate) use` 再导出）✓。

use crate::executor::*;
use crate::header::Header;
use crate::instance::Instance;
use crate::IteratorObject;
use crate::TupleObject;
use core::ptr::NonNull;

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
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "这个对象既不是内建迭代器，也没有 `__next__`",
                })
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
                    release(instance, exception);
                    Ok(None)
                } else {
                    Err(ExecError::Raised { exception })
                }
            }
            Err(other) => Err(other),
        };
    }
    if ty == builtin_type(instance, "list_reverseiterator") {
        return advance_list_reverseiterator(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "reversed") {
        return advance_reversed(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "count") {
        return advance_count(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "repeat") {
        return advance_repeat(instance, iterator, opcode);
    }
    // **`range()` 的两种迭代器也走这条分支** ✓（第 228 轮 ✗）：它们是**同一份载荷**（`ItStateObject` ✓）
    // ⇒ 只是**类型名**不同 ✓（参照分 `range_iterator`／`longrange_iterator` ✓）⇒ 这里必须一并认 ✓，
    // 否则改型之后会掉进 `__next__` 协议 ⇒ 迭代当场失败 ✗（实测：语料三条变红 ✓）。
    if ty == builtin_type(instance, "islice")
        || Some(ty) == instance.type_named("range_iterator")
        || Some(ty) == instance.type_named("longrange_iterator") {
        return advance_islice(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "product") {
        return advance_product(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "permutations") {
        return advance_permutations(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "compress") {
        return advance_compress(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "combinations")
        || ty == builtin_type(instance, "combinations_with_replacement") {
        return advance_combinations(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "zip") {
        return advance_zip(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "zip_longest") {
        return advance_zip_longest(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "pairwise") {
        return advance_pairwise(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "batched") {
        return advance_batched(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "cycle") {
        return advance_cycle(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "accumulate") {
        return advance_accumulate(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "starmap") {
        return advance_starmap(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "takewhile")
        || ty == builtin_type(instance, "dropwhile")
        || ty == builtin_type(instance, "filterfalse") {
        return advance_takewhile(instance, iterator, opcode);
    }
    if ty == builtin_type(instance, "chain") {
        return advance_chain(instance, iterator, opcode);
    }
    // SAFETY: 类型身份已确认是 IteratorObject 的某个类型。
    let object = unsafe { &*iterator.as_ptr().cast::<IteratorObject>() };
    let target = object.target();
    let index = object.index();
    let length = iterable_length(instance, target, opcode)?;
    if index >= length {
        return Ok(None);
    }
    let item = iterable_item(instance, target, index, opcode)?;
    object.advance();
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
        let arguments: Option<Vec<NonNull<Header>>> = if item_type == builtin_type(instance, "tuple")
        {
            // SAFETY: 类型身份已确认。
            let tuple = unsafe { &*item.as_ptr().cast::<TupleObject>() };
            Some((0..tuple.len()).filter_map(|index| tuple.item(index)).collect())
        } else if item_type == builtin_type(instance, "list") {
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

