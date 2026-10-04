//! 执行器：`SPEC-bytecode.md` §10 的**起步指令集**（`BC-49`）。
//!
//! **本片覆盖到哪**（其余一律返回 [`ExecError::NotImplemented`]，**不静默跳过**）：
//!
//! - 常量与局部：`RESUME`、`NOP`、`LOAD_CONST`、`LOAD_FAST`、`LOAD_FAST_CHECK`、
//!   `STORE_FAST`、`DELETE_FAST`、`POP_TOP`
//! - 运算符：`BINARY_OP`（只做整数能做的几项，见下）、`UNARY_NEGATIVE`、`UNARY_NOT`、
//!   `UNARY_INVERT`、`TO_BOOL`、`COMPARE_OP`（六元组，顺序取自 `opcode.cmp_op`）、`IS_OP`
//! - 控制流：`JUMP_FORWARD`、`JUMP_BACKWARD`、`JUMP_BACKWARD_NO_INTERRUPT`、
//!   `POP_JUMP_IF_TRUE`／`_FALSE`／`_NONE`／`_NOT_NONE`（目标按 **`BC-55`** 算）
//! - 返回：`RETURN_VALUE`
//!
//! **尚未接线**：`FOR_ITER`（要迭代器协议）、调用、容器、属性与下标、异常、生成器。
//!
//! **一处临时口径**（`OM-11` 的槽位接线后应改走协议）：判定"这是不是整数"按**类型身份**，
//! 不走协议。`TS-40` 的 `bool ⊂ int` **已接线**：`True + 1` 算 2、`-True` 算 −1
//! （两种载荷分开读，布局不同，不能互相强转）。
//!
//! 整数的**值域**：**任意精度**（`TS-45`／`P1-11` 已落地）——单例表只决定"内联还是分配"，
//! **不是**值域。历史上这里有"结果必须落在单例区间内"的说法，已随 `P1-11` 作废。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::bigint::IntValue;
use crate::decode::{parse_exception_table, DecodeError, Decoder};
use crate::frame::{Frame, FrameError};
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::opcode;
use crate::refcount::{Owned, PyRef};
use crate::builtin_objects::{
    AsendObject, AttributeObject, BoolObject, BuiltinFunctionObject, BytesObject, ExceptionObject,
    GeneratorObject, IteratorObject, MethodObject, DictObject, FloatObject, FunctionObject,
    IntObject, ListObject, SetObject, SliceObject, StrObject, TupleObject,
};
use crate::singleton::{SMALL_INT_MAX, SMALL_INT_MIN};
use crate::value::Value;

/// `execute` 的两种收尾（生成器要把"让出"与"返回"分开）。
pub enum ExecOutcome<'a> {
    /// 正常返回一个值。
    Returned(Value<'a>),
    /// **让出**一个值（生成器挂起：值栈已在帧的恢复点里，`BC-47`）。
    Yielded(NonNull<Header>),
}

/// 单条指令的结果。
enum Step<'a> {
    Continue,
    Return(Value<'a>),
    Yield(NonNull<Header>),
}

/// 执行失败的形态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecError {
    /// 解码失败（`BC-32`…`BC-36`）。
    Decode(DecodeError),
    /// 帧操作失败（`BC-43`：值栈越界等）。
    Frame(FrameError),
    /// 指令还没接线（`BC-49` 的全表随 M2 补齐）。
    NotImplemented { opcode: u8 },
    /// 本实例被请求中断（`AB-5`①：宿主 `pa_interrupt` ⇒ 执行类函数返回 `PA_ERR_INTERRUPT`）。
    Interrupted,
    /// 指令接线了，但这个形态／类型还没接线（协议槽位、大整数、容器……）。
    Unsupported { opcode: u8, what: &'static str },
    /// 读到未绑定的局部槽（CPython 的 `UnboundLocalError` 时机）。
    UnboundLocal { slot: usize },
    /// 抛出了一个 Python 异常（**异常对象由实例的 `pending_exception` 保活**）。
    ///
    /// `BC-60` ②：异常状态按实例存；这里只带一个借用的裸引用。
    Raised { exception: NonNull<Header> },
    /// 码元跑完却没有 `RETURN_VALUE`（码元一定被改坏了）。
    FellOffEnd,
}

impl From<DecodeError> for ExecError {
    fn from(error: DecodeError) -> Self {
        ExecError::Decode(error)
    }
}

impl From<FrameError> for ExecError {
    fn from(error: FrameError) -> Self {
        ExecError::Frame(error)
    }
}

fn opcode_of(name: &str) -> u8 {
    opcode::opcode(name).unwrap_or_else(|| panic!("opmap 缺 {name}")) as u8
}

/// 释放一份引用（`OM-20`）。
fn release(instance: &Instance, raw: NonNull<Header>) {
    // SAFETY: 调用方交出的是一份新引用。
    unsafe { instance.release_object(raw.as_ptr()) };
}

/// 把一份**新引用**交给帧的值栈（`BC-43`）。
fn push(instance: &Instance, frame: &Frame, raw: NonNull<Header>) -> Result<(), ExecError> {
    frame.push(instance.own(raw).into_raw())?;
    Ok(())
}

/// 把一个整数压栈（**任意精度**，`TS-45`）：单例表覆盖到的走单例（`OM-23`），
/// 其余交给 `Instance::new_int` 分配。
///
/// 早先这里要求"必须是单例"，于是 `x = 200 + 100`（结果 300 不在单例表里）与
/// `x = 9223372036854775807`（字面量）都会报 `IntOutOfRange`——那是 `P1-11` 之前的
/// i64／单例假设残留，2026-10-02 由新加的 `big_int_add` 对拍语料**抓出来**的。
fn push_int(instance: &Instance, frame: &Frame, value: i64) -> Result<(), ExecError> {
    let raw = instance.new_int(value);
    push(instance, frame, raw)
}

/// 推进一个**按下标走**的迭代器（`FOR_ITER` 与"普通迭代器的 `SEND`"共用同一份逻辑）。
///
/// 返回 `Some(元素新引用)` 或 `None`（已耗尽）。**只认**本层接线的迭代器类型。
/// **迭代推进**的公开入口（`paL_next` 与测试用）：取下一个元素；耗尽给 `None`。
///
/// 与执行器内部那条是**同一处实现**（`opcode` 只用于错误消息，公开入口给 0）。
pub fn advance(
    instance: &Instance,
    iterator: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, ExecError> {
    advance_iterator(instance, iterator, 0)
}

fn advance_iterator(
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
    if ty == builtin_type(instance, "count") {
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
    if ty == builtin_type(instance, "repeat") {
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
    // **`range()` 的两种迭代器也走这条分支** ✓（第 228 轮 ✗）：它们是**同一份载荷**（`ItStateObject` ✓）
    // ⇒ 只是**类型名**不同 ✓（参照分 `range_iterator`／`longrange_iterator` ✓）⇒ 这里必须一并认 ✓，
    // 否则改型之后会掉进 `__next__` 协议 ⇒ 迭代当场失败 ✗（实测：语料三条变红 ✓）。
    if ty == builtin_type(instance, "islice")
        || Some(ty) == instance.type_named("range_iterator")
        || Some(ty) == instance.type_named("longrange_iterator")
    {
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
    if ty == builtin_type(instance, "product") {
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
    if ty == builtin_type(instance, "permutations") {
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
    if ty == builtin_type(instance, "compress") {
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
    if ty == builtin_type(instance, "combinations")
        || ty == builtin_type(instance, "combinations_with_replacement")
    {
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
    if ty == builtin_type(instance, "zip") {
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
    if ty == builtin_type(instance, "zip_longest") {
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
    if ty == builtin_type(instance, "pairwise") {
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
    if ty == builtin_type(instance, "batched") {
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
    if ty == builtin_type(instance, "cycle") {
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
    if ty == builtin_type(instance, "accumulate") {
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
    if ty == builtin_type(instance, "starmap") {
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
    if ty == builtin_type(instance, "takewhile")
        || ty == builtin_type(instance, "dropwhile")
        || ty == builtin_type(instance, "filterfalse")
    {
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
    if ty == builtin_type(instance, "chain") {
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

/// **`iter(x)`**（`OM-11` 的 `iter` 槽位；公开面，`itertools.islice` 一类要用）。
///
/// 规则与 `GET_ITER` **同一处实现**：迭代器（含生成器）**原样**（新引用）；内建可迭代
/// 包一层按下标走的迭代器；其余走 `__iter__`；都没有 ⇒ 照参照**实测**的消息报
/// `TypeError: 'X' object is not iterable`。
pub fn iter_value(instance: &Instance, iterable: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: iterable 由调用方保证存活。
    let ty = unsafe { iterable.as_ref() }.ty();
    // 迭代器（含生成器）就是它自己的迭代器（实测 `iter(c) is c`）
    if ty == builtin_type(instance, "generator") || is_iterator_type(instance, ty) {
        // SAFETY: 同上。
        unsafe { instance.incref_object(iterable.as_ptr()) };
        return Ok(iterable);
    }
    if let Ok(iterator_type) = iterator_type_for(instance, iterable) {
        // 迭代器对象要**自己那一份**引用（本函数不消耗入参）
        // SAFETY: iterable 由调用方保证存活。
        unsafe { instance.incref_object(iterable.as_ptr()) };
        let iterator = instance.alloc(IteratorObject::new(
            iterator_type,
            iterable,
            Cell::new(0),
        ));
        return Ok(iterator.into_raw().cast::<Header>());
    }
    match attribute_optional(instance, iterable, "__iter__") {
        Ok(Some(method)) => {
            let result = call_value(instance, method, &[], &[]);
            release(instance, method);
            result
        }
        Ok(None) => {
            let name = instance.type_name(ty).to_owned();
            Err(raise_builtin(
                instance,
                "TypeError",
                &format!("'{name}' object is not iterable"),
            ))
        }
        Err(error) => Err(error),
    }
}

/// 判定真假——*临时*只覆盖单例表里的类型（`OM-11` 的 `__bool__` 槽位接线后改走协议）。
pub(crate) fn truthiness(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<bool, ExecError> {
    // SAFETY: raw 是帧值栈上的存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        return Ok(false);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 类型身份已确认（见本模块顶部"临时口径"）。
        return Ok(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value);
    }
    if ty == singletons.int_type() {
        // SAFETY: 同上。大整数走 `IntValue`（`int_value` 对它给 `None`，会被当成假）
        return Ok(instance.int_of(raw).map(|value| !value.is_zero()).unwrap_or(false));
    }
    // **内建容器的真假**（`OM-11` 的 `__bool__` 槽位接线前，按参照的**内建**规则 ✓）：
    // 空 `str`／`bytes`／`list`／`tuple`／`dict` ⇒ 假；`float` ⇒ `0.0`／`-0.0` 为假（`nan` 为真 ✓）。
    // 第 101 轮实测的触发器：`assert "x"`（上游 `importlib`／`site.py` 里满是这样用 ✓）。
    if instance.type_named("str") == Some(ty) {
        // SAFETY: 类型身份已确认是 `str`。
        return Ok(!unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().is_empty());
    }
    if instance.type_named("bytes") == Some(ty) {
        // SAFETY: 同上。
        return Ok(!unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().is_empty());
    }
    if instance.type_named("list") == Some(ty) {
        // SAFETY: 同上。
        return Ok(!unsafe { &*raw.as_ptr().cast::<ListObject>() }.items().is_empty());
    }
    if instance.type_named("tuple") == Some(ty) {
        // SAFETY: 同上。
        return Ok(!unsafe { &*raw.as_ptr().cast::<TupleObject>() }.items().is_empty());
    }
    if instance.type_named("dict") == Some(ty) {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<DictObject>() }.len() != 0);
    }
    if instance.type_named("float") == Some(ty) {
        // SAFETY: 同上。
        //  在 IEEE 里为**假** ⇒ 与参照一致（ 为假 ✓）；
        //  为真 ⇒  为真 ✓（与参照一致）。
        return Ok(unsafe { &*raw.as_ptr().cast::<FloatObject>() }.value() != 0.0);
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "真假判定只接线了 None／bool／int／str／bytes／list／tuple／dict／float（`set` 一族与 `__bool__` 协议未接线）",
    })
}

/// 把返回值从"栈上的裸引用"转成 [`Value`]：单例落回内联表示，其余包成守卫。
fn value_from_raw<'a>(instance: &'a Instance, raw: NonNull<Header>) -> Value<'a> {
    // SAFETY: raw 是刚出栈的新引用，对象存活。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        release(instance, raw);
        return Value::None;
    }
    if ty == singletons.bool_type() {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value;
        release(instance, raw);
        return Value::Bool(value);
    }
    if ty == singletons.int_type() {
        // SAFETY: 同上。
        let payload = unsafe { &*raw.as_ptr().cast::<IntObject>() }.value.clone();
        if let Some(value) = payload.to_i64() {
            if (SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
                release(instance, raw); // 单例由实例持有，交回我们这份即可
                return Value::small_int(value);
            }
        }
    }
    // SAFETY: 我们持有 raw 的那份新引用，转交给守卫。
    Value::Object(unsafe { PyRef::from_raw(raw, instance) })
}

/// 按名字取一个已注册的内建类型（`TS-41` 的表是层次的出处）。
fn builtin_type(instance: &Instance, name: &str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| panic!("TS-41：{name} 应当已注册"))
}

/// 整数载荷（int 与 bool 两种布局分开读，`TS-40`）。**含大整数**（`TS-45`）。
fn integer_payload(instance: &Instance, raw: NonNull<Header>) -> Option<IntValue> {
    instance.int_of(raw)
}

/// **下标**载荷 → `i64`：非整数与**超出 `i64` 的整数**分开报（"未接线"的理由不同）。
fn index_payload(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<i64, ExecError> {
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
fn numeric_payload(instance: &Instance, raw: NonNull<Header>) -> Option<f64> {
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

/// **值相等**（*临时*：只管道 `None`／`bool`／`int`／`float`／`str`）。
///
/// 参照实现的 `==` 走 `__eq__` 槽位（随类型系统接线）；本层先按载荷比，
/// 但**必须**保留 `TS-40` 的可观察后果（`True == 1`、`1 == 1.0` 为真）——
/// 否则 `{1: 'a', True: 'b'}` 这类字面量会多出一个键，属于对拍里的新差异。
fn values_equal(instance: &Instance, left: NonNull<Header>, right: NonNull<Header>) -> bool {
    if left == right {
        return true;
    }
    let (left_int, right_int) = (
        integer_payload(instance, left),
        integer_payload(instance, right),
    );
    if let (Some(a), Some(b)) = (left_int, right_int) {
        // 走 `BigInt` 比：`IntValue` 的两种载荷（内联／大整数）数值相等就是相等
        return a.to_bigint() == b.to_bigint();
    }
    let (left_number, right_number) = (
        numeric_payload(instance, left),
        numeric_payload(instance, right),
    );
    if let (Some(a), Some(b)) = (left_number, right_number) {
        // *临时*：整数与浮点比时按 f64 走（超大整数与浮点混用时会有精度话题，随协议槽位收口）
        return a == b;
    }
    let str_type = instance.singletons().str_type();
    // SAFETY: 两个都是存活对象。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    if left_type == str_type && right_type == str_type {
        // SAFETY: 类型身份已确认。
        let (left_text, right_text) = unsafe {
            (
                &*left.as_ptr().cast::<StrObject>(),
                &*right.as_ptr().cast::<StrObject>(),
            )
        };
        return left_text.value() == right_text.value();
    }
    // `bytes` 按字节逐位比（`P1-12`；实测 `b'ab' == b'ab'` 为真、不同长度直接不等）
    if Some(left_type) == instance.type_named("bytes") && Some(right_type) == instance.type_named("bytes") {
        // SAFETY: 类型身份已确认。
        let (left_bytes, right_bytes) = unsafe {
            (
                &*left.as_ptr().cast::<BytesObject>(),
                &*right.as_ptr().cast::<BytesObject>(),
            )
        };
        return left_bytes.value() == right_bytes.value();
    }
    // **容器按值比**（实测 3.14.4）：`list` 与 `list`、`tuple` 与 `tuple` **递归逐项**比；
    // **不同种类**一律不等（`[1] == (1,)` ⇒ `False`）。`dict`／`set` 仍需 `OM-11` 的
    // `richcompare` 槽位（本层暂按身份），这条缺口另记。
    // SAFETY: 两个都是存活对象（调用方保证）。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    let list_type = instance.type_named("list");
    let tuple_type = instance.type_named("tuple");
    if Some(left_type) == list_type && Some(right_type) == list_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe { (&*left.as_ptr().cast::<ListObject>(), &*right.as_ptr().cast::<ListObject>()) };
        if a.len() != b.len() {
            return false;
        }
        return (0..a.len()).all(|index| match (a.item(index), b.item(index)) {
            (Some(x), Some(y)) => values_equal(instance, x, y),
            _ => false,
        });
    }
    if Some(left_type) == tuple_type && Some(right_type) == tuple_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe { (&*left.as_ptr().cast::<TupleObject>(), &*right.as_ptr().cast::<TupleObject>()) };
        if a.len() != b.len() {
            return false;
        }
        return (0..a.len()).all(|index| match (a.item(index), b.item(index)) {
            (Some(x), Some(y)) => values_equal(instance, x, y),
            _ => false,
        });
    }

    // **`dict` 按值比**（实测）：长度相等 ＋ 每个键在右边**按键值相等**找到、且对应值递归相等。
    let dict_type = instance.type_named("dict");
    if Some(left_type) == dict_type && Some(right_type) == dict_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<DictObject>(),
                &*right.as_ptr().cast::<DictObject>(),
            )
        };
        if a.len() != b.len() {
            return false;
        }
        let right_entries = b.entries();
        for (key, value) in a.entries() {
            let mut matched = false;
            for (other_key, other_value) in &right_entries {
                if values_equal(instance, key, *other_key) {
                    if !values_equal(instance, value, *other_value) {
                        return false;
                    }
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }
        return true;
    }
    // **`set`／`frozenset`**：同族（含跨 `set`／`frozenset`——Python 允许，`{1} == frozenset({1})`
    // 为真）时"长度相等 ＋ 左的每一项在右里找得到"（双向包含由长度 ＋ 单向包含推出）。
    let set_type = instance.type_named("set");
    let frozen_type = instance.type_named("frozenset");
    let left_is_set = Some(left_type) == set_type || Some(left_type) == frozen_type;
    let right_is_set = Some(right_type) == set_type || Some(right_type) == frozen_type;
    if left_is_set && right_is_set {
        // SAFETY: 类型身份已确认（两种集合在实现上是同一个载荷）。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<SetObject>(),
                &*right.as_ptr().cast::<SetObject>(),
            )
        };
        if a.len() != b.len() {
            return false;
        }
        return (0..a.len()).all(|index| match a.item(index) {
            Some(item) => (0..b.len()).any(|other| match b.item(other) {
                Some(candidate) => values_equal(instance, item, candidate),
                None => false,
            }),
            None => false,
        });
    }

    false
}

/// 取出"可解包元素"（**新引用**的列表）。
///
/// *临时*：只管道 `tuple`／`list`／`str`（其余可迭代对象随迭代器族接线）。
/// 返回的每一项都是**新引用**——调用方要么压栈、要么释放。
fn sequence_items(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
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
    Err(ExecError::Unsupported {
        opcode,
        what: "解包只接线了 tuple／list／str（迭代器协议未接线）",
    })
}

/// 把一批**新引用**交出去造一个容器对象并压栈。
fn push_container<T: crate::header::PyObject>(
    instance: &Instance,
    frame: &Frame,
    object: T,
) -> Result<(), ExecError> {
    let owned = instance.alloc(object);
    frame.push(owned.into_raw().cast::<Header>())?;
    Ok(())
}

/// 把下标归一成 0 起的位置（负数从末尾数；越界返回 `None`）。
fn normalize_index(index: i64, length: usize) -> Option<usize> {
    let normalized = if index < 0 { index + length as i64 } else { index };
    if normalized < 0 || normalized >= length as i64 {
        return None;
    }
    Some(normalized as usize)
}

/// 造一个切片对象：**一律经 `slice` 类型的构造槽**（`OM-11` 的 `new` 槽）——
/// 字段校验、`None` 的含义、失败消息全都跟着 `slice(...)` 那条路走（**一处真相**）。
fn build_slice(
    instance: &Instance,
    arguments: &[NonNull<Header>],
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let slice_type = builtin_type(instance, "slice");
    let callable = instance.type_value(slice_type);
    match crate::executor::call_value(instance, callable, arguments, &[]) {
        Ok(slice) => Ok(slice),
        Err(ExecError::Unsupported { .. }) => Err(ExecError::Unsupported {
            opcode,
            what: "BUILD_SLICE／BINARY_SLICE 的实参形态还没接线",
        }),
        Err(error) => Err(error),
    }
}

/// **切片求值**（`P1-12`）：`slice.indices(len)` 的 CPython 口径/// **切片求值**（`P1-12`）：`slice.indices(len)` 的 CPython 口径——负下标先加长度、
/// 再按步长方向夹到 `[lower, upper]`；`step == 0` 报实测的 `ValueError`。
///
/// 判据是 `tests/fixture-slice-3.14.json`（`tools/gen_slice_fixture.py` 实测：16 种切法
/// × `bytes`／`str`／`list`／`tuple`）。
fn slice_bounds(
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
fn slice_positions(start: i64, stop: i64, step: i64) -> Vec<usize> {
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

/// 键是 `slice` 时的下标读：`bytes`／`list`／`tuple`／`str` 四族共用边界规则。
fn subscript_slice(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: container 是存活对象。
    let container_type = unsafe { container.as_ref() }.ty();
    if container_type == builtin_type(instance, "bytes") {
        let value = instance
            .bytes_value(container)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        let (start, stop, step) = slice_bounds(instance, key, value.len())?;
        let picked: Vec<u8> = slice_positions(start, stop, step)
            .into_iter()
            .map(|position| value[position])
            .collect();
        return Ok(instance.new_bytes(&picked));
    }
    if container_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let (start, stop, step) = slice_bounds(instance, key, object.len())?;
        let mut items: Vec<NonNull<Header>> = Vec::new();
        for position in slice_positions(start, stop, step) {
            if let Some(item) = object.item(position) {
                // SAFETY: 值由列表持有，存活；新列表要自己那份。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_list(items));
    }
    if container_type == builtin_type(instance, "tuple") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        let (start, stop, step) = slice_bounds(instance, key, object.len())?;
        let mut items: Vec<NonNull<Header>> = Vec::new();
        for position in slice_positions(start, stop, step) {
            if let Some(item) = object.item(position) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_tuple(items));
    }
    if container_type == instance.singletons().str_type() {
        // SAFETY: 同上。`str` 按**字符**切（不是字节）
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value().to_owned();
        let characters: Vec<char> = text.chars().collect();
        let (start, stop, step) = slice_bounds(instance, key, characters.len())?;
        let picked: String = slice_positions(start, stop, step)
            .into_iter()
            .map(|position| characters[position])
            .collect();
        return Ok(instance.new_str(&picked));
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "切片只接线了 bytes／list／tuple／str",
    })
}

/// 下标**读**（`BINARY_OP` ＋ `NB_SUBSCR`，3.14 无 `BINARY_SUBSCR`）。返回**新引用**。
fn subscript_get(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: container 与 key 都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    // **切片**（`P1-12`）：键是 `slice` 时走切片路径（四个序列类型共用一套边界规则）
    if Some(unsafe { key.as_ref() }.ty()) == instance.type_named("slice") {
        return subscript_slice(instance, container, key, opcode);
    }

    if container_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, object.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "tuple index out of range")),
        };
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: value 由容器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if container_type == builtin_type(instance, "list") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, object.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "list index out of range")),
        };
        let value = object.item(position).expect("已经检查过范围");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        // `KeyError` 的 `args` 就是那个键（实测：`KeyError('nope')`），不是一条消息
        let position = match object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
        {
            Some(position) => position,
            None => {
                // SAFETY: key 是帧值栈上的存活对象。
                unsafe { instance.incref_object(key.as_ptr()) };
                let exception = new_exception_with_args(
                    instance,
                    exception_type(instance, "KeyError"),
                    vec![key],
                );
                return Err(raise(instance, exception));
            }
        };
        let (_, value) = object.entry(position).expect("刚查到的位置");
        // SAFETY: 同上。
        unsafe { instance.incref_object(value.as_ptr()) };
        return Ok(value);
    }
    // **类型下标** ✓（第 214 轮）：`list[int]` ✓ —— `Lib/types.py` 要 `type(list[int])` ✓（`GenericAlias` ✓）。
    if instance.is_type_object(container) {
        // **借用** ✓：`key` 是帧值栈上的存活对象 ✓，`new_generic_alias` 记账 ✓。
        return Ok(instance.new_generic_alias(container, key));
    }
    let str_type = instance.singletons().str_type();
    if container_type == str_type {
        // SAFETY: 同上。
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value().to_owned();
        let characters: Vec<char> = text.chars().collect();
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, characters.len()) {
            Some(position) => position,
            None => {
                return Err(raise_builtin(instance, "IndexError", "string index out of range"))
            }
        };
        let object = instance.alloc(StrObject::new(str_type, characters[position].to_string()));
        return Ok(object.into_raw().cast::<Header>());
    }
    // `bytes`：整数下标给**整数**（`b'abc'[0] == 97`，实测）；切片随 `slice` 类型（M3+）再接线
    if container_type == builtin_type(instance, "bytes") {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*container.as_ptr().cast::<BytesObject>() }.value().to_vec();
        let index = index_payload(instance, key, opcode)?;
        let position = match normalize_index(index, value.len()) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "index out of range")),
        };
        return Ok(instance.new_int(i64::from(value[position])));
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标只接线了 tuple／list／dict／str／bytes",
    })
}

/// 下标**写**（`STORE_SUBSCR`；`value` 是**新引用**，无论成败都会被接手）。
fn subscript_set(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // SAFETY: 三个都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if container_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };

        // **切片写**（`a[i:j] = …`）：键是 `slice` 时替换那一整段（长度可与原段不同；
        // 带步长的"扩展切片"要求长度相等——消息照参照实测）
        if Some(unsafe { key.as_ref() }.ty()) == instance.type_named("slice") {
            let bounds = slice_bounds(instance, key, object.len());
            let (start, stop, step) = match bounds {
                Ok(bounds) => bounds,
                Err(error) => {
                    release(instance, value);
                    return Err(error);
                }
            };
            let items = match sequence_items(instance, value, opcode) {
                Ok(items) => items,
                Err(error) => {
                    release(instance, value);
                    return Err(error);
                }
            };
            release(instance, value);
            if step == 1 {
                let count = (stop - start).max(0) as usize;
                for _ in 0..count {
                    if let Some(old) = object.remove(start as usize) {
                        release(instance, old);
                    }
                }
                for (offset, item) in items.into_iter().enumerate() {
                    object.insert(start as usize + offset, item);
                }
            } else {
                let positions = slice_positions(start, stop, step);
                if positions.len() != items.len() {
                    let (given, expected) = (items.len(), positions.len());
                    for item in items {
                        release(instance, item);
                    }
                    return Err(instance.raise_builtin_error(
                        "ValueError",
                        &format!(
                            "attempt to assign sequence of size {given} to extended slice of size {expected}"
                        ),
                    ));
                }
                for (position, item) in positions.into_iter().zip(items) {
                    if let Some(old) = object.replace(position, item) {
                        release(instance, old);
                    }
                }
            }
            return Ok(());
        }

        let index = match index_payload(instance, key, opcode) {
            Ok(index) => index,
            Err(error) => {
                release(instance, value);
                return Err(error);
            }
        };
        let length = object.len();
        let position = match normalize_index(index, length) {
            Some(position) => position,
            None => {
                release(instance, value);
                return Err(raise_builtin(instance, "IndexError", "list index out of range"));
            }
        };
        if let Some(old) = object.replace(position, value) {
            release(instance, old);
        }
        return Ok(());
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key));
        match position {
            Some(slot) => {
                if let Some(old) = object.replace_value(slot, value) {
                    release(instance, old);
                }
                // 键已在表里：**调用方那份键的引用仍归调用方**（本函数借用键，见下）
            }
            None => {
                // **契约**：`subscript_set` **借用键**、**接管值**。
                // `insert_raw` 是"转移"语义（收下传进去的那份引用），所以这里必须先为字典
                // 新增一份键——否则调用方随后释放自己的键，字典里就留下一个**悬垂键指针**
                // （症状：键对象被释放后地址被别的字符串复用，查键会"命中"不相干的键）。
                // SAFETY: key 由调用方保证存活。
                unsafe { instance.incref_object(key.as_ptr()) };
                object.insert_raw(key, value);
            }
        }
        return Ok(());
    }
    release(instance, value);
    if container_type == builtin_type(instance, "tuple") {
        return Err(ExecError::Unsupported {
            opcode,
            what: "tuple 不支持下标赋值（不可变）",
        });
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标赋值只接线了 list／dict",
    })
}

/// 下标**删**（`DELETE_SUBSCR`）。
fn subscript_del(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // SAFETY: 两个都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if container_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        let index = index_payload(instance, key, opcode)?;
        let length = object.len();
        let position = match normalize_index(index, length) {
            Some(position) => position,
            None => return Err(raise_builtin(instance, "IndexError", "list index out of range")),
        };
        if let Some(removed) = object.remove(position) {
            release(instance, removed);
        }
        return Ok(());
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        let position = match object
            .entries()
            .iter()
            .position(|(existing, _)| values_equal(instance, *existing, key))
        {
            Some(position) => position,
            None => {
                // SAFETY: key 是帧值栈上的存活对象。
                unsafe { instance.incref_object(key.as_ptr()) };
                let exception = new_exception_with_args(
                    instance,
                    exception_type(instance, "KeyError"),
                    vec![key],
                );
                return Err(raise(instance, exception));
            }
        };
        if let Some((removed_key, removed_value)) = object.remove(position) {
            release(instance, removed_key);
            release(instance, removed_value);
        }
        return Ok(());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标删除只接线了 list／dict",
    })
}

/// 迭代器类型的名字（**照探测表取**；`str` 的迭代器在这台机器上叫 `str_ascii_iterator`）。
const ITERATOR_TYPE_NAMES: [&str; 28] = [
    "tuple_iterator",
    "list_iterator",
    "str_ascii_iterator",
    "bytes_iterator",
    // **`reversed(list)` 的迭代器** ✓（第 227 轮）：`_collections_abc.py:75` 要 `type(iter(reversed([])))` ✓。
    "list_reverseiterator",
    // **`range(<超出 i64 的上限>)`** ✓（第 228 轮）：`_collections_abc.py:77` 要它 ✓。
    "longrange_iterator",
    // **`range()` 的常规迭代器** ✓（第 228 轮）：参照的名字 ✓。
    "range_iterator",
    // **`zip()` 的迭代器** ✓（第 229 轮）：参照的名字也是 `zip` ✓。
    "zip",
    "dict_keyiterator",
    "set_iterator",
    // `itertools` 的（Pyawa 专有类型，`SPEC-c-modules.md` §5.2.6）
    "count",
    "repeat",
    "islice",
    "chain",
    "takewhile",
    "dropwhile",
    "filterfalse",
    "accumulate",
    "starmap",
    "cycle",
    "pairwise",
    "batched",
    "zip_longest",
    "compress",
    "combinations",
    "combinations_with_replacement",
    "permutations",
    "product",
];

/// 一个对象是不是本层接线的迭代器。
fn is_iterator_type(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    ITERATOR_TYPE_NAMES
        .iter()
        .any(|name| instance.type_named(name) == Some(ty))
}

/// 被迭代对象的元素个数。
fn iterable_length(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<usize, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<TupleObject>() }.len());
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<ListObject>() }.len());
    }
    if ty == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<DictObject>() }.len());
    }
    if ty == builtin_type(instance, "set") || Some(ty) == instance.type_named("frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮）：`UNPACK_SEQUENCE` 一族按元素个数
        // 走这条路 ✓ ⇒ 先前只认 `set` ✗ ⇒ `'frozenset' object is not iterable` 之后又撞一条 ✗。
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<SetObject>() }.len());
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().chars().count());
    }
    if Some(ty) == instance.type_named("bytes") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().len());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "只接线了 tuple／list／dict／set／str／bytes 的内建迭代器（其余走 __iter__ 协议）",
    })
}

/// 取被迭代对象的第 `index` 个元素（**新引用**；`str` 会造一个单字符 `str`）。
fn iterable_item(
    instance: &Instance,
    raw: NonNull<Header>,
    index: usize,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();

    let owned = |value: NonNull<Header>| {
        // SAFETY: value 由容器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        value
    };

    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<TupleObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<ListObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "dict") {
        // SAFETY: 同上。字典迭代的是**键**（与参照实现一致）
        let value = unsafe { &*raw.as_ptr().cast::<DictObject>() }
            .entry(index)
            .map(|(key, _)| key);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "set") || Some(ty) == instance.type_named("frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮）：按游标取元素这条路也要认它 ✓
        //（`_collections_abc` 注册基类时迭代集合 ✓，元类路径打通后当场踩到 ✓）。
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<SetObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 类型身份已确认。
        let text = unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned();
        let character = text.chars().nth(index).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        })?;
        let object = instance.alloc(StrObject::new(
            instance.singletons().str_type(),
            character.to_string(),
        ));
        return Ok(object.into_raw().cast::<Header>());
    }
    // `bytes`：迭代给**整数**（实测 `list(b'ab') == [97, 98]`）
    if Some(ty) == instance.type_named("bytes") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().to_vec();
        let byte = *value.get(index).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        })?;
        return Ok(instance.new_int(i64::from(byte)));
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "只接线了 tuple／list／dict／set／str／bytes 的迭代",
    })
}

/// 该对象该用哪个迭代器类型（名字照探测表）。
fn iterator_type_for(
    instance: &Instance,
    raw: NonNull<Header>,
) -> Result<NonNull<TypeObject>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let name = if ty == builtin_type(instance, "tuple") {
        "tuple_iterator"
    } else if ty == builtin_type(instance, "list") {
        "list_iterator"
    } else if ty == builtin_type(instance, "dict") {
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
            what: "只接线了 tuple／list／dict／set／str／bytes 的内建迭代器（其余走 __iter__ 协议）",
        });
    };
    Ok(builtin_type(instance, name))
}

/// 属性查找的结果。
enum Attribute {
    /// 一个**新引用**（`OM-11` 的 `getattr` 槽交出来的，直接压栈即可）。
    Owned(NonNull<Header>),
    /// 一个普通值（**借用**的裸引用）。
    Value(NonNull<Header>),
    /// 类型字典里查到的是函数 ⇒ 取方法：函数 ＋ 要绑的 `self`（都是**借用**）。
    Method {
        /// 函数对象（由类型字典持有）。
        function: NonNull<Header>,
        /// 要绑上去的实例。
        this: NonNull<Header>,
    },
}

/// **`super` 的属性查找** ✓（第 233 轮）：在 `type(__self__)` 的 MRO 上、**定义类之后**找 ✓。
///
/// 函数 ⇒ 绑到 `__self__`（与实例方法同款 ✓）；其余 ⇒ 原样给（**如实说** ✗：描述符的 `__get__` 随后补 ✓）。
fn super_lookup(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Option<Attribute>, ExecError> {
    // SAFETY: object 是存活的 super 对象（载荷是 `AttributeObject` ✓）。
    let attrs = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::AttributeObject>() };
    let Some(dict) = attrs.attributes() else {
        return Ok(None);
    };
    let Some(thisclass) = instance.dict_get(dict, "__thisclass__") else {
        return Ok(None);
    };
    let Some(this) = instance.dict_get(dict, "__self__") else {
        return Ok(None);
    };
    let this_type = instance.type_of(this);
    let stop = thisclass.cast::<TypeObject>();
    // SAFETY: this_type 由注册表持有。
    let mro = unsafe { this_type.as_ref() }.mro();
    // **定义类不在被查的 MRO 里** ✓ ⇒ **整条 MRO 都算数** ✓（第 234 轮实测的形态 ✓）：
    // `ABCMeta.__new__` 里的 `super()` ⇒ `super(ABCMeta, ABCMeta)` ✓ —— `ABCMeta` 是**元类自己** ✓，
    // 它**不在** `type(ABCMeta).__mro__`（＝`[type, object]`）里 ✗ ⇒ 若仍要求"跳过定义类" ⇒
    // 永远跳不过去 ⇒ 报 `'super' object has no attribute '__new__'` ✗。
    let stop_in_mro = mro.iter().any(|entry| *entry == stop);
    let mut after = !stop_in_mro;
    for entry in mro {
        if after {
            if let Some(found) = instance.type_lookup(entry, name) {
                if instance.type_of(found) == builtin_type(instance, "function") {
                    return Ok(Some(Attribute::Method {
                        function: found,
                        this,
                    }));
                }
                return Ok(Some(Attribute::Value(found)));
            }
        }
        if entry == stop {
            after = true;
        }
    }
    Ok(None)
}

/// **经 `fs` 域把一个文件读成文本**（`IM-15`：I/O 一律走能力域 ✓，本层不碰平台 ✓ `CX-4`）。
fn read_file_through_fs(instance: &Instance, path: &[u8]) -> Option<String> {
    let handle = instance
        .fs_open(path, pyawa_capabilities::fs::open_flag::RDONLY, 0)
        .ok()?;
    let mut bytes: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match instance.fs_read(handle, &mut buffer) {
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(_) => {
                let _ = instance.fs_close(handle);
                return None;
            }
        }
    }
    let _ = instance.fs_close(handle);
    if std::env::var_os("PYAWA_TRACE_IMPORT").is_some() {
        // **诊断** ✓（第 200 轮）：读出多少字节 ✓（好与磁盘上的大小对照 ✗）。
        let path_text = String::from_utf8_lossy(path);
        eprintln!("[读文件] {path_text} ⇒ {} 字节", bytes.len());
    }
    String::from_utf8(bytes).ok()
}

/// **最小的模块加载器**（`IM-`／`P3-12` 的第一片）：按 `sys.path` 找 `<dir>/<名字>.py`，
/// 用 `fs` 域读进来 ⇒ 编译 ⇒ 在**新名字空间**里执行 ⇒ 登记进模块表（与 `sys.modules` 同一份 ✓）。
///
/// 返回模块对象（**新引用** ✓）。这一层就是"VM 侧的 importlib 等价物"；**未接**：包
/// （`__init__.py`／`__path__`）、相对导入、`.pyc`、`sys.meta_path`、`site.py`（`IM-24`）✓。
/// **Rust 侧那座过渡桥**（`PLAN-milestones.md` 的 A1；回填条件见那条）✓。
///
/// **交出约定（第 281 轮统一 ✓）**：返回的是**借用**（`sys.modules` 持着它 ✓），与 `dict_get` 同款 ——
/// 先前"命中表就借用、真载入还多 `incref` 一份"**两套** ✗ ⇒ 那是**泄漏**（`MS-25` 那一族最忌讳
/// 记账不齐 ✓）⇒ 现在只有一套 ✓，调用方要留就自己 `retain` ✓。
fn load_module(
    instance: &Instance,
    modules: NonNull<Header>,
    name: &str,
    opcode_number: u8,
) -> Result<NonNull<Header>, ExecError> {
    let unsupported = |what: &'static str| ExecError::Unsupported { opcode: opcode_number, what };
    // **`sys.modules` 先赢** ✓（第 197 轮真 bug 修复 ✗）：CPython 的规矩是"已经在 `sys.modules` 里就直接用" ✓
    // —— `Lib/os.py:103` 自己就写 `sys.modules['os.path'] = path` ✓；少了这一查，
    // `from os.path import …`（`os.py:104` ✓）会去找"**父包的 `__path__`**" ✗ 并报
    // `父包没有 __path__（包才有 ✓）` ✗（实测 ✓）⇒ 于是 `os.py` 只剩半截 ✗（`name`／`path` 有 ✓，
    // `sep`／`curdir`／`environ` 全没有 ✗）—— 这正是 `site.py` 卡住的那一环 ✓。
    if let Some(existing) = instance.dict_get(modules, name) {
        return Ok(existing);
    }
    // `sys.path` 从模块表里的 `sys` 模块对象上取（模块属性 ✓）
    let sys_module = instance
        .dict_get(modules, "sys")
        .ok_or(unsupported("模块表里没有 `sys`（加载器要 `sys.path`）"))?;
    let entries: Vec<String> = match attribute_lookup(instance, sys_module, "path") {
        Ok(Attribute::Owned(path)) | Ok(Attribute::Value(path)) => {
            let list_type = instance.type_named("list").expect("list 在引导期已登记");
            if instance.type_of(path) != list_type {
                // `Owned` 是新引用 ⇒ 要还回去；`Value` 是借出 ⇒ 不能释放 ✗（这里只处理列表形态）
                return Err(unsupported("`sys.path` 不是列表"));
            }
            // SAFETY: 类型身份刚确认是 list。
            let list = unsafe { &*path.as_ptr().cast::<crate::builtin_objects::ListObject>() };
            list.items()
                .iter()
                .filter_map(|item| instance.text_of(*item).map(|text| text.to_owned()))
                .collect()
        }
        _ => return Err(unsupported("`sys.path` 取不到（加载器需要它）")),
    };
    // **带点的名字**（第 135 轮）：`import a.b` ⇒ 在**父包 `a` 的 `__path__`** 里找 `b` ✓，
    // 文件名用**最后一段** ✓，并把子模块挂成父包的属性 ✓（参照语义 ✓：`a.b` 之后 `a.b` 可见 ✓）。
    let split = name.rsplit_once('.');
    let (parent, base) = match split {
        Some((parent, base)) => (Some(parent), base),
        None => (None, name),
    };
    let entries: Vec<String> = match parent {
        Some(parent) => {
            // **父包先载入（可递归）** ✓（第 281 轮修 ✗）：CPython 的 `_find_and_load` 会把
            // `a.b.c` 的**每一级父包**都先装好 ✓；先前只查表 ✗ ⇒ 像 `xml.etree.ElementInclude`
            // 这种"顶层包不自己 import 子包"的导入当场报"父包不在表里" ✗（实测 112 个模块 ✓）。
            if instance.dict_get(modules, parent).is_none() {
                load_module(instance, modules, parent, opcode_number)?;
            }
            let parent_module = instance.dict_get(modules, parent).ok_or(unsupported(
                "带点的导入要先有父包在模块表里（相对导入／按需加载随后补）",
            ))?;
            match attribute_lookup(instance, parent_module, "__path__") {
                Ok(Attribute::Owned(path)) | Ok(Attribute::Value(path)) => {
                    let list_type = instance.type_named("list").expect("list 在引导期已登记");
                    if instance.type_of(path) != list_type {
                        return Err(unsupported("父包的 `__path__` 不是列表"));
                    }
                    // SAFETY: 类型身份刚确认是 list。
                    let list = unsafe { &*path.as_ptr().cast::<crate::builtin_objects::ListObject>() };
                    list.items()
                        .iter()
                        .filter_map(|item| instance.text_of(*item).map(|text| text.to_owned()))
                        .collect()
                }
                _ => return Err(unsupported("父包没有 `__path__`（包才有 ✓）")),
            }
        }
        None => entries,
    };
    let mut last_syntax: Option<String> = None;
    // 编译时"尚未接线"的那条（**如实上抛**，不要伪装成"模块不存在" ✗）
    let mut last_unsupported: Option<String> = None;
    for entry in &entries {
        // **候选**（第 135 轮）：先 `<dir>/<名字>.py` ✓，再 `<dir>/<名字>/__init__.py` ✓（包 ✓，
        // 还要给它 `__path__` ✓）。顺序与参照的 `FileFinder` 一致：**文件先、包后** ✓。
        let candidates: [(String, Option<String>); 2] = [
            (format!("{entry}/{base}.py"), None),
            (
                format!("{entry}/{base}/__init__.py"),
                Some(format!("{entry}/{base}")),
            ),
        ];
        for (file, package_directory) in candidates {
        if std::env::var_os("PYAWA_TRACE_IMPORT").is_some() {
            eprintln!("[载入] 试 {file}（模块 {name}）");
        }
        let Some(source) = read_file_through_fs(instance, file.as_bytes()) else {
            continue; // 读不到就试下一个候选／下一个入口（`CP-2`／机器错误都当"这里没有" ✓）
        };
        let unit = match crate::compile::compile(
            &source,
            &file,
            crate::compile::Mode::PurePython,
            crate::compile::CheckTier::Shallow,
            0,
        ) {
            Ok(unit) => unit,
            Err(crate::compile::CompileError::Syntax(message)) => {
                last_syntax = Some(message);
                continue;
            }
            Err(crate::compile::CompileError::Unsupported(what)) => {
                // **如实记下**（第 139 轮）：此前静默 continue ✗ ⇒ 最后只报"找不到模块" ✗
                last_unsupported = Some(what);
                continue;
            }
        };
        // **新名字空间** ＋ `__name__`（照参照实现的模块语义 ✓）
        let namespace = instance.new_dict();
        let module_name = instance.new_str(name);
        instance.dict_set(namespace, "__name__", module_name);
        // **包**：`__path__` ＝ 该包目录（参照语义 ✓；子模块导入要靠它 ✓）
        if let Some(package_directory) = &package_directory {
            let path_text = instance.new_str(package_directory);
            let path_list = instance.new_list(vec![path_text]);
            instance.dict_set(namespace, "__path__", path_list);
        }
        // 模块对象：`AttributeObject` ＋ 名字空间（`module` 类型缺就建 ✓）
        let module_type = instance
            .type_named("module")
            .unwrap_or_else(|| instance.new_attribute_type("module"));
        let module = instance
            .alloc(crate::builtin_objects::AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<Header>();
        // **先登记再执行**（环状导入要能看到半成品 ✓，与参照一致）
        instance.dict_set(modules, name, module);
        let code = crate::compile::instantiate(instance, &unit);
        let frame_type = instance.type_named("frame").ok_or(unsupported("引导期没有 `Frame` 类型"))?;
        // SAFETY: `namespace` 由模块对象持有，存活。
        unsafe { instance.incref_object(namespace.as_ptr()) };
        let frame = instance
            .alloc(crate::Frame::for_code_with_namespace(frame_type, &code, namespace));
        let outcome = crate::execute(instance, &frame);
        // **诊断** ✓（第 197 轮）：模块**先登记**后执行 ✓ ⇒ 一旦执行出错，`sys.modules` 里会**留下半截模块** ✗。
        // 这里在**开着 `PYAWA_TRACE_IMPORT`** 时把那个错**如实打出来** ✓（默认零输出 ✓）。
        if std::env::var_os("PYAWA_TRACE_IMPORT").is_some() {
            if let Err(error) = &outcome {
                eprintln!("[载入] 模块 {name} 执行出错：{error:?}");
            }
        }
        if std::env::var_os("PYAWA_TRACE_IMPORT").is_some() {
            // **诊断** ✓（第 200 轮）：执行完**命名空间里有多少个名字** ✓（好判断"写入是否落地" ✗）。
            let size = crate::mounted_instance_dict(instance, module)
                .map(|dict| {
                    // SAFETY: dict 是存活对象。
                    unsafe { &*dict.as_ptr().cast::<crate::builtin_objects::DictObject>() }
                        .entries()
                        .len()
                })
                .unwrap_or(0);
            eprintln!("[载入] 模块 {name} 执行完：命名空间 {size} 个名字");
        }
        drop(frame);
        drop(code);
        outcome?;
        // **挂成父包的属性**（第 135 轮）：`import a.b` 之后 `a.b` 要能取到 ✓（参照语义 ✓）
        if let (Some(parent), Some(base)) = (parent, split.map(|_| base)) {
            if let Some(parent_module) = instance.dict_get(modules, parent) {
                if let Some(parent_namespace) = crate::mounted_instance_dict(instance, parent_module) {
                    instance.dict_set(parent_namespace, base, module);
                }
            }
        }
        // **借用**交出 ✓（约定见本函数文档：只有一套 ✓）
        return Ok(module);
    }
    }
    // **按实报错**（第 139 轮）：文件找到了、但编译不过 ⇒ 说清是哪个模块、哪句话 ✓；
    // 编译时撞到"尚未接线" ⇒ 原样上抛 ✓；两者都没有 ⇒ 才是真的"找不到" ✓。
    if let Some(message) = last_syntax {
        let text = format!("加载模块 '{name}'：{message}");
        return Err(instance.raise_builtin_error("SyntaxError", &text));
    }
    if let Some(what) = last_unsupported {
        // 编译撞到"尚未接线" ⇒ **原样如实上抛**（点明模块名 ✓，不再伪装成"模块不存在" ✗）
        let text = format!("加载模块 '{name}'：{what}");
        return Err(instance.raise_builtin_error("NotImplementedError", &text));
    }
    // **找不到就报 `ModuleNotFoundError`** ✓（第 162 轮）：参照里 `import 不存在的名字` 抛的就是它 ✓，
    //   而**不是**一个"未接线"的硬错误 ✗ —— 后者会让 `try: from _abc import … except ImportError:` 这类
    //   **合法回退**接不住 ✓（`Lib/abc.py:85` 正是这样 ✓：`_abc` 是 C 内建模块 ✗，参照回退到 `_py_abc` ✓）。
    Err(instance.raise_builtin_error(
        "ModuleNotFoundError",
        &format!("No module named '{name}'"),
    ))
}

/// **`CONTAINS_OP`**（`in`／`not in`）的判定：`str`／`list`／`tuple`／`dict`／`set`。
///
/// 实测的两条错误消息（**禁止**近似）：
/// - 容器不是那几类 ⇒ `argument of type 'X' is not a container or iterable`
/// - 容器是 `str` 而左操作数不是 `str` ⇒ `'in <string>' requires string as left operand, not X`
///
/// 元素比较走 [`values_equal`]（本层口径：整数／浮点／字符串按值，其余**按身份**）。
/// 所以容器之间的值相等（`[] in [[], []]`、`1 == [1]`）**尚未**接通——那是
/// `OM-11` 的 `richcompare` 槽位那一摊（`lib.rs` 的清单里记着），不是 `in` 自己的事。
fn contains(
    instance: &Instance,
    container: NonNull<Header>,
    item: NonNull<Header>,
    opcode: u8,
) -> Result<bool, ExecError> {
    // SAFETY: container 是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();
    if container_type == instance.singletons().str_type() {
        // SAFETY: 类型身份已确认。
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value();
        let needle = instance.text_value(item);
        return match needle {
            Some(needle) => Ok(text.contains(&needle)),
            None => {
                // SAFETY: item 是存活对象。
                let item_type = unsafe { item.as_ref() }.ty();
                // SAFETY: 类型名由注册表持有。
                let name = unsafe { item_type.as_ref() }.name();
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &format!("'in <string>' requires string as left operand, not {name}"),
                ))
            }
        };
    }
    // `bytes`：**子串**查找（实测 `b'ab' in b'abc'`）；左操作数不是 bytes 时报实测的消息。
    // **整数那一档**（第 284 轮按参照实测补 ✓）：`98 in b"b"` ⇒ `True` ✓、
    // `300 in b"ab"`／`(-1) in b"ab"` ⇒ `ValueError: byte must be in range(0, 256)` ✓、
    // `"a" in b"ab"` ⇒ `TypeError: a bytes-like object is required, not 'str'` ✓
    //（`Lib/` 里 `codecs`／`base64_codec` 一族真的会 `b in bytes` 判字节 ✓）。
    if Some(container_type) == instance.type_named("bytes") {
        let value = instance.bytes_value(container).unwrap_or_default().to_vec();
        if Some(instance.type_of(item)) == instance.type_named("int") {
            // 超出 `i64` 的整数一定不在 0..256 ✓（参照给的是同一条 `ValueError` ✓）
            let byte = instance.int_value(item).unwrap_or(-1);
            if !(0..256).contains(&byte) {
                return Err(raise_builtin(
                    instance,
                    "ValueError",
                    "byte must be in range(0, 256)",
                ));
            }
            return Ok(value.contains(&(byte as u8)));
        }
        let Some(needle) = instance.bytes_value(item).map(<[u8]>::to_vec) else {
            let name = instance.type_name(instance.type_of(item));
            return Err(raise_builtin(
                instance,
                "TypeError",
                &format!("a bytes-like object is required, not '{name}'"),
            ));
        };
        if needle.is_empty() {
            return Ok(true);
        }
        return Ok(value.windows(needle.len()).any(|window| window == needle.as_slice()));
    }
    if container_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        for index in 0..object.len() {
            let element = object.item(index).expect("下标在范围内");
            if values_equal(instance, element, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if container_type == builtin_type(instance, "tuple") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        for index in 0..object.len() {
            let element = object.item(index).expect("下标在范围内");
            if values_equal(instance, element, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if container_type == builtin_type(instance, "dict") {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        for (key, _) in object.entries() {
            if values_equal(instance, key, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    // **`set` 有自己的一份**（第 236 轮修 SIGSEGV）：此前这一支和 `dict` 合在一起、把 set 强转成
    // `DictObject` 再遍历 `entries()` ⇒ **类型混淆**、读越界直接崩（推导式能造集合后才被触发）
    // **`frozenset` 与 `set` 同一份载荷** ✓（第 236 轮 ✓）⇒ `in` 也要一并认 ✓
    //（第 283 轮修 ✗：先前只认 `set` ✗ ⇒ `1 in frozenset([1, 2])` 报
    //  `TypeError: argument of type 'frozenset' is not a container or iterable` ✗ ——
    //  `collections` 那一族 **12** 个模块压在它上面 ✓）。
    if container_type == builtin_type(instance, "set")
        || Some(container_type) == instance.type_named("frozenset")
    {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<SetObject>() };
        for element in object.items() {
            if values_equal(instance, element, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    // SAFETY: container 是存活对象。
    let _ = opcode;
    let name = unsafe { container_type.as_ref() }.name();
    Err(raise_builtin(
        instance,
        "TypeError",
        &format!("argument of type '{name}' is not a container or iterable"),
    ))
}

/// 在**映射**（`dict`）里按名字查一项（**新引用**交给调用方；没查到给 `None`）。
fn lookup_in_mapping(
    instance: &Instance,
    mapping: NonNull<Header>,
    name: &str,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 mapping 是本实例里存活的 dict。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .iter()
        .position(|(existing, _)| str_matches_public(instance, *existing, name))?;
    let (_, value) = dict.entry(position)?;
    // SAFETY: value 由字典持有，存活；调用方要自己那份。
    unsafe { instance.incref_object(value.as_ptr()) };
    Some(value)
}

/// 对象的属性字典（只有带 [`crate::HAS_INSTANCE_DICT`] 的实例才有）。
fn instance_attributes(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    let ty = header.ty();
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { ty.as_ref() };
    if type_object.type_flags() & crate::HAS_INSTANCE_DICT == 0 {
        return None;
    }
    // **类型对象**：它的"实例字典"就是它的**命名空间** ✓（`TypeObject.dict` ✓，**一处真相** ✓）。
    //
    // **第 201 轮真 bug 的落点** ✗：元类型（`type`）曾被错置"内联实例字典"位 ✗ ⇒ 于是把
    // `TypeObject` 当 `AttributeObject` 读 ✗ ⇒ 取出来的字典指针是**垃圾**（实测 `0x6` ✓）⇒
    // `Lib/os.py` 一类**一取类属性就段错误** ✗（且随堆布局时隐时现 ✓）。
    if instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象 ⇒ 头部就在同一地址上。
        return unsafe { &*object.as_ptr().cast::<crate::TypeObject>() }.dict();
    }
    if type_object.has_inline_instance_dict() {
        // SAFETY: 这一位保证载荷就是 AttributeObject。
        return unsafe { &*object.as_ptr().cast::<AttributeObject>() }.attributes();
    }
    // **OM-14**：固定布局的实例（宿主／子类）把字典另行挂在头部那一格上
    header.instance_dict()
}

/// **取或惰性创建**实例字典（`OM-14`：参照实现里 `obj.__dict__` 一读就给出 `{}`）。
///
/// 返回**借用**（由实例持有）；类型不带实例字典时给 `None`（调用方按缺属性报错）。
pub fn mounted_instance_dict(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    if let Some(mapping) = instance_attributes(instance, object) {
        return Some(mapping);
    }
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    let ty = header.ty();
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { ty.as_ref() };
    if type_object.type_flags() & crate::HAS_INSTANCE_DICT == 0 {
        return None;
    }
    let created = instance
        .alloc(DictObject::new(
            builtin_type(instance, "dict"),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    if type_object.has_inline_instance_dict() {
        // SAFETY: 这一位保证载荷就是 `AttributeObject`；`set_attributes` 接手新引用。
        let previous = unsafe { &*object.as_ptr().cast::<AttributeObject>() }
            .set_attributes(Some(created));
        debug_assert!(previous.is_none(), "上面确认过还没有字典");
        let _ = previous;
    } else {
        header.store_instance_dict(created);
    }
    Some(created)
}

/// 往实例的属性字典里写一项（`value` 是**新引用**，由字典接手；旧值被释放）。
pub fn instance_attribute_set(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
    // **数据描述符 `__set__`** ✓（第 192 轮）：类型 MRO 上若有 `__set__` ⇒ **调它** ✓
    //   —— 数据描述符**优先于实例字典** ✓（`__dict__` 本身另走下面那条 ✓，这里跳过 ✓）。
    if name != "__dict__" {
        // SAFETY: object 是存活对象。
        let object_type = unsafe { object.as_ref() }.ty();

        if let Some(found) = instance.type_lookup(object_type, name) {
            // SAFETY: found 由类型字典持有。
            let found_ty = unsafe { found.as_ref() }.ty();
            if let Some(setter) = instance.type_lookup(found_ty, "__set__") {
                let this = instance.retain(object);
                instance.retain(value);
                let returned = call_callable(
                    instance,
                    setter,
                    Some(found),
                    vec![this, value],
                    Vec::new(),
                    opcode,
                )?;
                release(instance, returned);
                return Ok(());
            }
        }
    }

    // **类型对象**：属性写进它的**命名空间** ✓（与 `instance_attributes` 同款口径 ✓，**一处真相** ✓）。
    if instance.is_type_object(object) {
        if name == "__dict__" {
            if unsafe { value.as_ref() }.ty() != builtin_type(instance, "dict") {
                unsafe { instance.release_object(value.as_ptr()) };
                // SAFETY: value 是存活对象。
                let value_type = unsafe { value.as_ref() }.ty();
                // SAFETY: 类型名由注册表持有。
                let value_type_name = unsafe { value_type.as_ref() }.name();
                let message = format!("__dict__ must be set to a dictionary, not a '{value_type_name}'");
                return Err(raise_builtin(instance, "TypeError", &message));
            }
            // SAFETY: object 是类型对象。
            let type_object = unsafe { &*object.as_ptr().cast::<crate::TypeObject>() };
            // 先取出旧命名空间（**借用**，不要跨 `set_dict` 持借 ✓），再把新的一份交出去 ✓。
            let previous = type_object.dict();
            type_object.set_dict(Some(value));
            if let Some(previous) = previous {
                // SAFETY: 被顶下来的那份由本函数消费。
                unsafe { instance.release_object(previous.as_ptr()) };
            }
            return Ok(());
        }
        let Some(namespace) = instance.type_namespace(object) else {
            release(instance, value);
            return Err(raise_builtin(instance, "TypeError", "类型对象没有命名空间"));
        };
        // SAFETY: namespace 是本实例里的 dict。
        let dict = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
        let position = dict
            .entries()
            .into_iter()
            .position(|(existing, _)| str_matches_public(instance, existing, name));
        match position {
            Some(slot) => {
                if let Some(old) = dict.replace_value(slot, value) {
                    release(instance, old);
                }
            }
            None => {
                let key = instance
                    .alloc(StrObject::new(instance.singletons().str_type(), name.to_owned()))
                    .into_raw()
                    .cast::<Header>();
                dict.insert_raw(key, value);
            }
        }
        let _ = opcode;
        return Ok(());
    }

    // **OM-14**：只有带实例字典的类型才收属性写入；否则报 `AttributeError`
    // （实测原话：`'dict' object has no attribute 'answer' and no __dict__ for setting new attributes`）
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { header.ty().as_ref() };
    let has_instance_dict = type_object.type_flags() & crate::HAS_INSTANCE_DICT != 0;
    // **`obj.__dict__ = {…}`**（实测）：**整体替换**挂载的字典；值不是字典就是实测那条
    // `TypeError: __dict__ must be set to a dictionary, not a 'int'`。
    if name == "__dict__" && has_instance_dict {
        let is_dict = unsafe { value.as_ref() }.ty() == builtin_type(instance, "dict");
        if !is_dict {
            // SAFETY: value 是调用方交出的新引用，这里消费掉。
            unsafe { instance.release_object(value.as_ptr()) };
            // SAFETY: value 是存活对象。
            let value_type = unsafe { value.as_ref() }.ty();
            // SAFETY: 类型名由注册表持有。
            let value_type_name = unsafe { value_type.as_ref() }.name();
            let message = format!("__dict__ must be set to a dictionary, not a '{value_type_name}'");
            return Err(raise_builtin(instance, "TypeError", &message));
        }
        let replaced = if type_object.has_inline_instance_dict() {
            // SAFETY: 这一位保证载荷就是 `AttributeObject`。
            unsafe { &*object.as_ptr().cast::<AttributeObject>() }.set_attributes(Some(value))
        } else {
            // SAFETY: 上面确认过这个实例带（另行挂载的）实例字典。
            let previous = header.take_instance_dict();
            header.store_instance_dict(value);
            previous
        };
        if let Some(previous) = replaced {
            // SAFETY: 被顶下来的那份由本函数消费。
            unsafe { instance.release_object(previous.as_ptr()) };
        }
        return Ok(());
    }
    if !has_instance_dict {
        release(instance, value);
        let message = format!(
            "'{}' object has no attribute '{name}' and no __dict__ for setting new attributes",
            type_object.name()
        );
        return Err(raise_builtin(instance, "AttributeError", &message));
    }
    // **`OM-14`**：取或惰性创建（一处真相——`obj.__dict__` 一读就要给出 `{}`）
    let mapping = mounted_instance_dict(instance, object).expect("上面确认过这个类型带实例字典");
    // SAFETY: mapping 由对象或本函数持有。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .into_iter()
        .position(|(existing, _)| str_matches_public(instance, existing, name));
    match position {
        Some(slot) => {
            if let Some(old) = dict.replace_value(slot, value) {
                release(instance, old);
            }
        }
        None => {
            let key = instance
                .alloc(StrObject::new(instance.singletons().str_type(), name.to_owned()))
                .into_raw()
                .cast::<Header>();
            dict.insert_raw(key, value);
        }
    }
    let _ = opcode;
    Ok(())
}

/// 一个 `str` 对象的内容是否等于给定的 Rust 字符串。
fn str_matches_public(instance: &Instance, raw: NonNull<Header>, expected: &str) -> bool {
    // SAFETY: 调用方保证 raw 是存活对象。
    if unsafe { raw.as_ref() }.ty() != instance.singletons().str_type() {
        return false;
    }
    // SAFETY: 类型身份已确认。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value() == expected
}

/// `LOAD_ATTR` 一族的查找顺序（本层口径，写在注释里以免以后漂）：
///
/// ① 实例字典（**非数据描述符**会被它遮住：函数就是非数据描述符，故实例属性优先）
/// ② 类型字典（沿 MRO）：查到**函数**就是取方法，查到别的值就原样返回
/// ③ 都没有 ⇒ [`ExecError::AttributeNotFound`]
fn attribute_lookup(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Attribute, ExecError> {
    // ① 类型自己的 `getattr` 槽（`OM-11`）——**内建类型的属性通道**，不许旁路
    // SAFETY: object 是存活对象。
    let object_type = unsafe { object.as_ref() }.ty();

    // ①.0 **`f.__annotations__`** 要**调用** `__annotate__`（有异常通道）⇒ 放在槽之前单独处理
    if name == "__annotations__" && object_type == builtin_type(instance, "function") {
        return crate::builtin_objects::function_annotations(instance, object.as_ptr())
            .map(Attribute::Owned);
    }
    // SAFETY: object_type 由注册表持有。
    if let Some(slot) = unsafe { object_type.as_ref() }.slots().getattr {
        // SAFETY: 槽位由类型提供，契约见 `GetAttrFn`。
        if let Some(found) = unsafe { slot(object.as_ptr(), name, instance) } {
            return Ok(Attribute::Owned(found));
        }
    }

    // ①.1 **`super` 的查表** ✓（第 233 轮）：在 `type(__self__)` 的 MRO 上、**跳过定义类**之后找 ✓
    //（`ABCMeta.__new__` 里的 `super().__new__(…)` 正是这一支 ✓）。
    // **注意**：这里要与**类型对象**比 ✗ —— `builtin_type(instance, …)` 取的是**命名空间**里那个名字 ✓，
    // 而 `super` 这个名字**绑的是 native** ✓ ⇒ 拿它比会**永远不等** ✗（本轮实测踩到 ✓）。
    if Some(object_type) == instance.type_named("super") {
        if let Some(found) = super_lookup(instance, object, name)? {
            return Ok(found);
        }
    }

    // ①.2 **类型对象的 `__name__`／`__qualname__`**（第 133 轮）：参照里 `X.__name__` 是 `"X"` ✓
    //   （`_bootstrap.py` 的 `_object_name` 就用它 ✓）。函数对象那半边早有（`function_getattr` ✓）。
    if (name == "__name__" || name == "__qualname__") && instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象。
        let info = unsafe { &*object.as_ptr().cast::<crate::TypeObject>() };
        return Ok(Attribute::Owned(instance.new_str(info.name())));
    }
    // ①.5 **`__dict__`**（实测：实例上它就是**那个字典本身**——同一个对象、透过它加属性立刻可见；
    // 没有实例字典的类型则落到最后那条 `AttributeError`，实测形如
    // `'S' object has no attribute '__dict__'`）。
    if name == "__dict__" {
        // **类型对象的 `__dict__`** ✓（第 181 轮，内建类型化 A）：用户类与内建类型都该有 ✓ ——
        //   `types.py` 的 `type(type.__dict__)`／`dict.__dict__['fromkeys']` 正是靠它 ✓。
        //   CPython 给的是 **mappingproxy** ✓，本层给**那个命名空间本身** ✗ ⇒ **已登记的偏差** ✓。
        // **用类型表里的 `type`** ✓（`builtin_type` 取的是**命名空间**里那个名字 ✗ —— 它可能是 native ✗）。
        let object_is_type = Some(unsafe { object.as_ref() }.ty()) == instance.type_named("type");
        if object_is_type {
            // 命名空间字典的**惰性挂载**在 [`crate::Instance::type_namespace`]（**一处真相** ✓）。
            if let Some(namespace) = instance.type_namespace(object) {
                // SAFETY: namespace 是存活对象；类型自己持一份，这里给调用方**再加一份** ✓。
                unsafe { instance.incref_object(namespace.as_ptr()) };
                return Ok(Attribute::Owned(namespace));
            }
        }
        if let Some(mapping) = mounted_instance_dict(instance, object) {
            // SAFETY: mapping 是存活对象，这里新增一份交给调用方。
            unsafe { instance.incref_object(mapping.as_ptr()) };
            return Ok(Attribute::Owned(mapping));
        }
    }

    // ② 实例字典
    if let Some(mapping) = instance_attributes(instance, object) {
        // **只有真的是 `dict` 才能按 `DictObject` 取项** ✓（第 185 轮实证 ✓）：`instance_attributes` 也可能给出
        // **内联属性对象**（布局不同 ✗）⇒ 照 `DictObject` 强转会**未对齐指针** ⇒ 直接 abort ✗
        //（实测就是 `executor.rs` 那行的 misaligned panic ✗）。不是 dict 就跳过这一支 ✓。
        let is_dict = instance.type_name(instance.type_of(mapping)) == "dict";
        let found = if is_dict {
            // SAFETY: 上面刚确认 mapping 的类型是 dict。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            dict.entries()
                .into_iter()
                .find(|(key, _)| str_matches_public(instance, *key, name))
        } else {
            None
        };
        if let Some((_, value)) = found {
            // **类对象不走这条** ✗（第 235 轮真 bug 修复 ✓）：类自己的名字空间归 **③** 管 ✓ ——
            // 那里会走**描述符协议** ✓。先前这里**直接给原值** ✗ ⇒ 只要**类级**取一个描述符就拿到
            // **描述符对象本身** ✗（`class R: __get__…` ＋ `class E: r = R()` ⇒ `E.r` 给 `<R object …>` ✗，
            // 参照给 `__get__(None, E)` 的结果 ✓；而 `E().r` 走另一条路 ✓、一直是好的 ✓）。
            if !instance.is_type_object(object) {
                return Ok(Attribute::Value(value));
            }
        }
    }

    // ③ 类型字典沿 MRO
    // **对象自己就是类型对象时，要查"它自己"的字典与 MRO** ✓（第 159 轮真 bug ✓）：
    //   `class C: x = 2` 之后 `C.x` 先前报 `AttributeError: 'type' object has no attribute 'x'` ✗
    //   —— 因为这里查的是对象**类型**的字典（对 `C` 来说是 `type` ✗），而不是 `C` 自己的名字空间 ✗。
    //   `Lib/` 里「类名.属性」遍地都是 ✓ ⇒ 这条必须对 ✓。
    let lookup_type = if instance.is_type_object(object) {
        // SAFETY: 刚判过它是类型对象 ⇒ 头部就在同一地址上。
        Some(object.cast::<crate::TypeObject>())
    } else {
        Some(object_type)
    };
    if let Some(found) = lookup_type.and_then(|ty| instance.type_lookup(ty, name)) {
        // SAFETY: found 由类型字典持有。
        let found_ty = unsafe { found.as_ref() }.ty();
        if found_ty == builtin_type(instance, "function") {
            // **类访问 ⇒ 不绑定** ✗（第 192 轮真 bug 修复 ✓）：参照实测 `D.deco`（沿**类自己的 MRO**
            // 取到的普通函数 ✓）给的是 `<function D.deco at …>` ✓ —— **不绑定** ✓；本层先前一律绑成
            // `<bound method D.deco of <class 'D'>>` ✗ ⇒ 于是 `@D.deco` 那条**类装饰器**把宿主 `D`
            // 当成新类传了 ✗（`Lib/genericpath.py:194` 的 `@object.__new__` 也栽在这一步 ✓）。
            //
            // **注意** ✓：**元类那一层**（`Base.hello()` ✓）**仍要绑** ✓ —— 那里"类"是**元类型的实例** ✓，
            // 与这里"沿自己的 MRO 取**类属性**"是两码事 ✓（元类那条走的是 `attribute_lookup` 里
            // 第 ①.1 段的专用分支 ✓，不经此处 ✓）。
            if instance.is_type_object(object) {
                return Ok(Attribute::Value(found));
            }
            return Ok(Attribute::Method {
                function: found,
                this: object,
            });
        }
        // **描述符协议 `__get__`** ✓（第 192 轮）：类型字典里找到的东西若**自带 `__get__`** ⇒ **调它** ✓
        //   （`C().x` ⇒ `__get__(实例, C)` ✓；`C.x` ⇒ `__get__(None, C)` ✓）。
        //   **函数不走这里** ✗（上面那支已处理绑定 ✓）；`builtin_function_or_method` 同理 ✗
        //   —— `Lib/` 里方法遍地都是 ✓，别把它们的绑定路径抢了 ✗。
        if found_ty != builtin_type(instance, "builtin_function_or_method") {
            if let Some(get) = instance.type_lookup(found_ty, "__get__") {
                // `self` 实参：实例给**实例本身** ✓；类型对象给 `None` ✓（参照口径 ✓）。
                let this = if instance.is_type_object(object) {
                    instance.retain(instance.singletons().none())
                } else {
                    instance.retain(object)
                };
                let owner = lookup_type.expect("上面判过 lookup_type 非空");
                // SAFETY: owner 是类型对象（上面的分支保证 ✓）。
                let owner_object = owner.cast::<Header>();
                instance.retain(owner_object);
                let result = crate::executor::call_callable(
                    instance,
                    get,
                    Some(found),
                    vec![this, owner_object],
                    Vec::new(),
                    0,
                )?;
                return Ok(Attribute::Value(result));
            }
        }
        return Ok(Attribute::Value(found));
    }

    // **元类那一层** ✓（第 232 轮）：对象是**类**时，属性还要到**它的元类型**的 MRO 上找 ✓
    //（参照 `type.__getattribute__` 的顺序 ✓）—— `SomeABC.register(...)` 正是这一支 ✓。
    // 先前只在"**类自己的 MRO**"上找 ✗ ⇒ 报 `'ABCMeta' object has no attribute 'register'` ✗
    //（`_collections_abc.py:321` 的 `Iterator.register(bytearray_iterator)` 就卡在这 ✓）。
    if instance.is_type_object(object) {
        let object_type = instance.type_of(object);
        if let Some(found) = instance.type_lookup(object_type, name) {
            // 元类型上的**函数** ⇒ 绑到**类本身** ✓（`self` ＝ 那个类 ✓，与实例方法同款 ✓）。
            if instance.type_of(found) == builtin_type(instance, "function") {
                return Ok(Attribute::Method {
                    function: found,
                    this: object,
                });
            }
            return Ok(Attribute::Value(found));
        }
    }

    // **每个类型都有 `__doc__`** ✓（第 288 轮）：字典里没有（＝没写文档串）时是 `None` ✓
    //（参照口径 ✓；`Lib/io.py:72` 一进门就读 `_io._IOBase.__doc__` ✗ ——
    // 先前这里直接抛 `'type' object has no attribute '__doc__'` ✗）。
    if name == "__doc__" && instance.is_type_object(object) {
        return Ok(Attribute::Value(instance.singletons().none()));
    }

    // 实测消息：`'int' object has no attribute 'nope'`（类型名取自对象的类型）
    // SAFETY: object 是存活对象。
    let type_name = unsafe { object.as_ref().ty().as_ref() }.name();
    Err(raise_builtin(
        instance,
        "AttributeError",
        &format!("'{type_name}' object has no attribute '{name}'"),
    ))
}

/// 删掉实例属性字典里的一项（`DELETE_ATTR`；参照实现只删实例属性，不碰类型）。
fn instance_attribute_delete(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<(), ExecError> {
    let missing = || {
        // SAFETY: object 是存活对象。
        let type_name = unsafe { object.as_ref().ty().as_ref() }.name();
        raise_builtin(
            instance,
            "AttributeError",
            &format!("'{type_name}' object has no attribute '{name}'"),
        )
    };
    // **数据描述符 `__delete__`** ✓（第 192 轮）：类型 MRO 上若有 `__delete__` ⇒ 调它 ✓（优先于实例字典 ✓）。
    {
        // SAFETY: object 是存活对象。
        let object_type = unsafe { object.as_ref() }.ty();
        if let Some(found) = instance.type_lookup(object_type, name) {
            // SAFETY: found 由类型字典持有。
            let found_ty = unsafe { found.as_ref() }.ty();
            if let Some(deleter) = instance.type_lookup(found_ty, "__delete__") {
                let this = instance.retain(object);
                let returned = call_callable(
                    instance,
                    deleter,
                    Some(found),
                    vec![this],
                    Vec::new(),
                    0,
                )?;
                release(instance, returned);
                return Ok(());
            }
        }
    }
    let mapping = instance_attributes(instance, object).ok_or_else(missing)?;
    // SAFETY: mapping 是属性字典（dict）。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .into_iter()
        .position(|(key, _)| str_matches_public(instance, key, name))
        .ok_or_else(missing)?;
    if let Some((key, value)) = dict.remove(position) {
        release(instance, key);
        release(instance, value);
    }
    Ok(())
}

/// **`__build_class__`**：用一个"局部变量是映射"的帧跑**类体**。
///
/// `body` 是类体函数（`MAKE_FUNCTION` 造出来的那个），`namespace` 是类命名空间。
pub(crate) fn run_class_body(
    instance: &Instance,
    body: NonNull<Header>,
    namespace: NonNull<Header>,
) -> Result<(), ExecError> {
    // 类体函数的 code object（`OM-11`：函数持有它）
    // SAFETY: body 由调用方保证存活。
    let body_ref = unsafe { &*body.as_ptr().cast::<FunctionObject>() };
    let code = body_ref.code();
    let globals = body_ref.globals();
    let frame = crate::classes::class_body_frame(instance, code, namespace, globals);
    let frame = crate::Owned::new(frame, instance);
    match execute(instance, &frame)? {
        ExecOutcome::Returned(value) => {
            // 类体正常的收尾：`LOAD_CONST None; RETURN_VALUE`
            let raw = value_into_raw(instance, value);
            release(instance, raw);
            Ok(())
        }
        ExecOutcome::Yielded(_) => Err(ExecError::Unsupported {
            opcode: 0,
            what: "类体不该让出",
        }),
    }
}

/// 按**属性通道**（`TS-44`）在对象上找一个 dunder 并调用它（找不到就什么也不做）。
///
/// 用于类创建钩子（`__init_subclass__`）一类"有就调、没有就算了"的钩子。
pub(crate) fn call_dunder_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<(), ExecError> {
    let found = match attribute_lookup(instance, object, name) {
        Ok(found) => found,
        Err(_) => return Ok(()),
    };
    let (callable, this) = match found {
        Attribute::Method { function, this } => (function, this),
        Attribute::Value(method) | Attribute::Owned(method) => (method, object),
    };
    let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        call_args.push(*argument);
    }
    let result = call_callable(instance, callable, Some(this), call_args, Vec::new(), 0)?;
    release(instance, result);
    Ok(())
}

/// **相等性**（本层的临时口径，随 `richcompare` 槽位收口）：给宿主面用。
pub fn values_equal_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
) -> bool {
    values_equal(instance, left, right)
}

/// **`truthiness` 的公开入口**（`TS-40` 的真值口径）。
///
/// stdlib 里需要"按 Python 口径判真值"的模块（`operator.truth`／`not_` 一类）用它 ——
/// **不要**在 stdlib 里另写一份真值规则（那是两处真相，`AGENTS.md` 禁止）。
/// `opcode` 只用于报错时指明来源（照内部那份的用法传即可）。
pub fn truthiness_public(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<bool, ExecError> {
    truthiness(instance, raw, opcode)
}

/// **`str % value`**（printf 风格）✓（第 281 轮）：`Lib/` 里遍地都是 ✓ —— 实测先撞上的是
/// `codecs.py` 的 `raise SystemError('… %s' % e)` ✓（`encodings.*` 那一族因此全红 ✗）。
///
/// 口径照参照**实测**：
/// - 右操作数是**元组** ⇒ 位置实参；否则 ⇒ **单个**实参；格式里出现 `%(名字)` ⇒ 右操作数必须是
///   **映射**（否则 `TypeError: format requires a mapping` ✓）；
/// - 转换字符：`s`／`r`／`a`／`d`／`i`／`u`／`o`／`x`／`X`／`f`／`F`／`e`／`E`／`g`／`G`／`c`／`%%`；
/// - 修饰：`-`／`+`／空格／`#`／`0`／宽度／`.精度`；长度修饰符（`h`／`l`／`L`）**照参照忽略** ✓；
/// - 错误消息照实测：`not enough arguments for format string`／
///   `not all arguments converted during string formatting`／
///   `%d format: a real number is required, not str` ✓。
fn percent_format(
    instance: &Instance,
    template: &str,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let _ = opcode;
    let characters: Vec<char> = template.chars().collect();
    // 右操作数的两种形态（**一处真相**：类型名取注册表 ✓）
    let items: Option<Vec<NonNull<Header>>> =
        if instance.type_name(instance.type_of(right)) == "tuple" {
            instance.tuple_items(right)
        } else {
            None
        };
    let mapping = if instance.type_name(instance.type_of(right)) == "dict" {
        Some(right)
    } else {
        None
    };
    let mut out = String::new();
    let mut cursor = 0usize;
    let mut argument_index = 0usize;
    let mut mapping_used = false;
    let mut positional_used = false;
    while cursor < characters.len() {
        if characters[cursor] != '%' {
            out.push(characters[cursor]);
            cursor += 1;
            continue;
        }
        cursor += 1;
        if cursor >= characters.len() {
            return Err(instance.raise_builtin_error("ValueError", "incomplete format"));
        }
        if characters[cursor] == '%' {
            out.push('%');
            cursor += 1;
            continue;
        }
        // `%(名字)`
        let mut key: Option<String> = None;
        if characters[cursor] == '(' {
            cursor += 1;
            let mut name = String::new();
            while cursor < characters.len() && characters[cursor] != ')' {
                name.push(characters[cursor]);
                cursor += 1;
            }
            if cursor >= characters.len() {
                return Err(instance.raise_builtin_error("ValueError", "incomplete format key"));
            }
            cursor += 1;
            key = Some(name);
        }
        // 修饰符
        let (mut left_align, mut plus, mut space, mut alternate, mut zero) =
            (false, false, false, false, false);
        while cursor < characters.len() {
            match characters[cursor] {
                '-' => left_align = true,
                '+' => plus = true,
                ' ' => space = true,
                '#' => alternate = true,
                '0' => zero = true,
                _ => break,
            }
            cursor += 1;
        }
        // 宽度
        if cursor < characters.len() && characters[cursor] == '*' {
            return Err(instance.raise_builtin_error(
                "NotImplementedError",
                "`%*` 的宽度取自实参尚未接线（宽度写死在格式串里可以）",
            ));
        }
        let mut width: Option<usize> = None;
        while cursor < characters.len() && characters[cursor].is_ascii_digit() {
            let digit = characters[cursor] as usize - '0' as usize;
            width = Some(width.unwrap_or(0) * 10 + digit);
            cursor += 1;
        }
        // 精度
        let mut precision: Option<usize> = None;
        if cursor < characters.len() && characters[cursor] == '.' {
            cursor += 1;
            let mut value = 0usize;
            while cursor < characters.len() && characters[cursor].is_ascii_digit() {
                let digit = characters[cursor] as usize - '0' as usize;
                value = value * 10 + digit;
                cursor += 1;
            }
            precision = Some(value);
        }
        // 长度修饰符（参照忽略）
        while cursor < characters.len() && matches!(characters[cursor], 'h' | 'l' | 'L') {
            cursor += 1;
        }
        if cursor >= characters.len() {
            return Err(instance.raise_builtin_error("ValueError", "incomplete format"));
        }
        let conversion = characters[cursor];
        cursor += 1;
        if conversion == '%' {
            out.push('%');
            continue;
        }
        // 取实参
        let value: NonNull<Header> = if let Some(key) = &key {
            if positional_used {
                return Err(instance.raise_builtin_error("TypeError", "format requires a mapping"));
            }
            mapping_used = true;
            let Some(mapping) = mapping else {
                return Err(instance.raise_builtin_error("TypeError", "format requires a mapping"));
            };
            // 消息**原样**给键 ✓（本层异常的 `str` 走 repr ⇒ 与参照的 `KeyError: 'a'` 同形 ✓）
            instance
                .dict_get(mapping, key)
                .ok_or_else(|| instance.raise_builtin_error("KeyError", key))?
        } else {
            if mapping_used {
                return Err(instance.raise_builtin_error("TypeError", "format requires a mapping"));
            }
            positional_used = true;
            match &items {
                Some(items) => *items.get(argument_index).ok_or_else(|| {
                    instance.raise_builtin_error("TypeError", "not enough arguments for format string")
                })?,
                None => {
                    if argument_index > 0 {
                        return Err(instance.raise_builtin_error(
                            "TypeError",
                            "not enough arguments for format string",
                        ));
                    }
                    right
                }
            }
        };
        argument_index += 1;
        // 转换
        let numeric = matches!(
            conversion,
            'd' | 'i' | 'u' | 'o' | 'x' | 'X' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G'
        );
        let sign_and_body: (String, String) = match conversion {
            's' => (String::new(), instance.object_str(value)?),
            'r' => (String::new(), instance.object_repr(value)?),
            // `%a`：参照给 **ascii()**（非 ASCII 转义）—— 本层按 `repr` 的结果再转义非 ASCII ✓
            'a' => (String::new(), ascii_escape(&instance.object_repr(value)?)),
            'c' => {
                let body = if let Some(number) = instance.int_value(value) {
                    match u32::try_from(number).ok().and_then(char::from_u32) {
                        Some(character) => character.to_string(),
                        None => {
                            return Err(instance.raise_builtin_error(
                                "OverflowError",
                                "%c arg not in range(0x110000)",
                            ))
                        }
                    }
                } else if let Some(text) = instance.text_value(value) {
                    let length = text.chars().count();
                    if length == 1 {
                        text
                    } else {
                        return Err(instance.raise_builtin_error(
                            "TypeError",
                            &format!(
                                "%c requires an int or a unicode character, not a string of length {length}"
                            ),
                        ));
                    }
                } else {
                    return Err(instance.raise_builtin_error(
                        "TypeError",
                        &format!(
                            "%c requires an int or a unicode character, not {}",
                            instance.type_name(instance.type_of(value))
                        ),
                    ));
                };
                (String::new(), body)
            }
            'd' | 'i' | 'u' | 'o' | 'x' | 'X' => {
                let base = match conversion {
                    'o' => 8,
                    'x' | 'X' => 16,
                    _ => 10,
                };
                let upper = conversion == 'X';
                let (negative, digits) =
                    integer_digits(instance, value, base, upper, conversion, base != 10)?;
                let sign = if negative {
                    "-".to_owned()
                } else if plus {
                    "+".to_owned()
                } else if space {
                    " ".to_owned()
                } else {
                    String::new()
                };
                let prefix = if alternate {
                    match conversion {
                        'x' => "0x".to_owned(),
                        'X' => "0X".to_owned(),
                        'o' => "0o".to_owned(),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                pad_number(
                    &mut out, &sign, &prefix, &digits, width, left_align, zero && !left_align,
                );
                continue;
            }
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' => {
                let body = float_digits(instance, value, conversion, precision.unwrap_or(6))?;
                (String::new(), body)
            }
            other => {
                // 参照实测：`ValueError: unsupported format character 'q' (0x71) at index 1`
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!(
                        "unsupported format character '{other}' (0x{:x}) at index {}",
                        other as u32,
                        cursor - 1
                    ),
                ));
            }
        };
        let (sign, body) = sign_and_body;
        let sign = if numeric && sign.is_empty() && !body.starts_with('-') {
            if plus {
                "+".to_owned()
            } else if space {
                " ".to_owned()
            } else {
                sign
            }
        } else {
            sign
        };
        // **精度对 `%s` 一族是截断** ✓（参照 `'%.2s' % 'abcdef'` ⇒ `'ab'` ✓）
        let body = if !numeric {
            match precision {
                Some(limit) => body.chars().take(limit).collect(),
                None => body,
            }
        } else {
            body
        };
        if numeric {
            let (negative, digits) = if let Some(rest) = body.strip_prefix('-') {
                (true, rest.to_owned())
            } else {
                (false, body.clone())
            };
            let sign = if negative { "-".to_owned() } else { sign };
            pad_number(&mut out, &sign, "", &digits, width, left_align, zero && !left_align);
        } else {
            pad_text(&mut out, &body, width, left_align);
        }
    }
    // 位置实参没被用完 ⇒ 照参照报错 ✓（`'%s' % (1, 2)`）
    if positional_used {
        if let Some(items) = &items {
            if argument_index < items.len() {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "not all arguments converted during string formatting",
                ));
            }
        }
    }
    Ok(instance.new_str(&out))
}

/// 按宽度／对齐／零填充拼一个**数值三段**（符号＋前缀＋数字）✓。
fn pad_number(
    out: &mut String,
    sign: &str,
    prefix: &str,
    digits: &str,
    width: Option<usize>,
    left_align: bool,
    zero: bool,
) {
    let total = sign.chars().count() + prefix.chars().count() + digits.chars().count();
    match width {
        Some(width) if width > total => {
            let fill = width - total;
            if left_align {
                out.push_str(sign);
                out.push_str(prefix);
                out.push_str(digits);
                for _ in 0..fill {
                    out.push(' ');
                }
            } else if zero {
                out.push_str(sign);
                out.push_str(prefix);
                for _ in 0..fill {
                    out.push('0');
                }
                out.push_str(digits);
            } else {
                for _ in 0..fill {
                    out.push(' ');
                }
                out.push_str(sign);
                out.push_str(prefix);
                out.push_str(digits);
            }
        }
        _ => {
            out.push_str(sign);
            out.push_str(prefix);
            out.push_str(digits);
        }
    }
}

/// 按宽度／对齐拼一段文本（`%s` 一族；**零填充对它无效** ✓，照参照 ✓）。
fn pad_text(out: &mut String, text: &str, width: Option<usize>, left_align: bool) {
    let length = text.chars().count();
    match width {
        Some(width) if width > length => {
            let fill = width - length;
            if left_align {
                out.push_str(text);
                for _ in 0..fill {
                    out.push(' ');
                }
            } else {
                for _ in 0..fill {
                    out.push(' ');
                }
                out.push_str(text);
            }
        }
        _ => out.push_str(text),
    }
}

/// 取整数的（符号, 数字）——`i64` 直接排；**大整数**十进制借用它的 `str` ✓（其它进制如实报未接线）；
/// **浮点**按参照**截断**（`'%d' % 3.7` ⇒ `'3'`、`'%d' % -3.7` ⇒ `'-3'` ✓）。
fn integer_digits(
    instance: &Instance,
    value: NonNull<Header>,
    base: u32,
    upper: bool,
    conversion: char,
    integer_only: bool,
) -> Result<(bool, String), ExecError> {
    let digits = |magnitude: u64| match (base, upper) {
        (10, _) => magnitude.to_string(),
        (16, false) => format!("{magnitude:x}"),
        (16, true) => format!("{magnitude:X}"),
        (8, _) => format!("{magnitude:o}"),
        _ => magnitude.to_string(),
    };
    if let Some(number) = instance.int_value(value) {
        return Ok((number < 0, digits(number.unsigned_abs())));
    }
    if instance.int_of(value).is_some() {
        let text = instance.object_str(value)?;
        if base != 10 {
            return Err(instance.raise_builtin_error(
                "NotImplementedError",
                "大整数的 `%x`／`%X`／`%o` 尚未接线（十进制可以）",
            ));
        }
        return Ok((text.starts_with('-'), text.trim_start_matches('-').to_owned()));
    }
    if let Some(number) = instance.float_value(value) {
        if integer_only {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "%{conversion} format: an integer is required, not {}",
                    instance.type_name(instance.type_of(value))
                ),
            ));
        }
        if !number.is_finite() {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "cannot convert float infinity to integer",
            ));
        }
        let truncated = number.trunc();
        if truncated.abs() < 9.2e18 {
            let integer = truncated as i64;
            return Ok((integer < 0, digits(integer.unsigned_abs())));
        }
        // 大浮点：借 `str` 的整数部分（如实；精确路径随后补 ✓）
        let text = format!("{truncated:.0}");
        return Ok((text.starts_with('-'), text.trim_start_matches('-').to_owned()));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!(
            "%{conversion} format: a real number is required, not {}",
            instance.type_name(instance.type_of(value))
        ),
    ))
}

/// 浮点转换（`f`／`F`／`e`／`E`／`g`／`G`）——口径照 C 的 printf（精度默认 **6** ✓）。
fn float_digits(
    instance: &Instance,
    value: NonNull<Header>,
    conversion: char,
    precision: usize,
) -> Result<String, ExecError> {
    let number = instance
        .float_value(value)
        .or_else(|| instance.int_value(value).map(|integer| integer as f64));
    let Some(number) = number else {
        // 参照实测：`'%f' % 'x'` ⇒ `TypeError: must be real number, not str` ✓
        // （与 `%d` 那条 `%d format: a real number is required, not str` **不同** ✓ —— 两条都量过 ✓）
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("must be real number, not {}", instance.type_name(instance.type_of(value))),
        ));
    };
    let upper = conversion.is_ascii_uppercase();
    if number.is_nan() {
        return Ok(if upper { "NAN" } else { "nan" }.to_owned());
    }
    if number.is_infinite() {
        let text = if upper { "INF" } else { "inf" };
        return Ok(if number.is_sign_negative() {
            format!("-{text}")
        } else {
            text.to_owned()
        });
    }
    let sign = if number.is_sign_negative() { "-" } else { "" };
    let magnitude = number.abs();
    let body = match conversion {
        'f' | 'F' => format!("{magnitude:.precision$}"),
        'e' | 'E' => normalize_exponent(&format!("{magnitude:.precision$e}"), upper),
        _ => {
            // `%g` 的口径：指数 < -4 或 ≥ 精度 ⇒ 走 `e`（精度 −1），否则走 `f`（精度 −1−指数）；
            // 末尾的零与孤立的小数点**去掉**（除非给了 `#` —— 本层 `#` 的这条支随后补 ✓）
            let significant = precision.max(1);
            let exponent = if magnitude == 0.0 {
                0
            } else {
                magnitude.log10().floor() as i32
            };
            if exponent < -4 || exponent >= significant as i32 {
                let text = normalize_exponent(
                    &format!("{magnitude:.prec$e}", prec = significant - 1),
                    upper,
                );
                strip_trailing_zeros(&text)
            } else {
                let decimals = (significant as i32 - 1 - exponent).max(0) as usize;
                let text = format!("{magnitude:.decimals$}");
                strip_trailing_zeros(&text)
            }
        }
    };
    Ok(format!("{sign}{body}"))
}

/// 把 Rust 的 `1.234568e4` 归一成 C 的 `1.234568e+04` ✓（指数至少两位、带符号 ✓）。
fn normalize_exponent(text: &str, upper: bool) -> String {
    let Some((mantissa, exponent)) = text.split_once(['e', 'E']) else {
        return text.to_owned();
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let marker = if upper { 'E' } else { 'e' };
    format!("{mantissa}{marker}{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs())
}

/// 去掉 `%g` 结果末尾多余的零与孤立的小数点 ✓。
fn strip_trailing_zeros(text: &str) -> String {
    let Some((mantissa, exponent)) = text.split_once(['e', 'E']) else {
        if text.contains('.') {
            return text.trim_end_matches('0').trim_end_matches('.').to_owned();
        }
        return text.to_owned();
    };
    let trimmed = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if exponent == 0 {
        trimmed.to_owned()
    } else {
        format!("{trimmed}e{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs())
    }
}

/// `%a` 用：把非 ASCII 字符转义成 `\xNN`／`\uNNNN`／`\UNNNNNNNN` ✓（照 `ascii()` 的口径 ✓）。
fn ascii_escape(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if character.is_ascii() {
            escaped.push(character);
        } else if (character as u32) <= 0xFF {
            escaped.push_str(&format!("\\x{:02x}", character as u32));
        } else if (character as u32) <= 0xFFFF {
            escaped.push_str(&format!("\\u{:04x}", character as u32));
        } else {
            escaped.push_str(&format!("\\U{:08x}", character as u32));
        }
    }
    escaped
}

/// **序列拼接／`+` 的公开入口**（`operator.concat` 与 `operator.add` 共用；将来 `BINARY_OP` 的 `+` 也用它）。
/// 实测：`concat(['a'], ['b'])` 与 `add(['a'], ['b'])` **都是**拼接 ⇒ 两者同一条路。
/// 支持 `str`／`list`／`tuple` 拼接；其余（含整数）落到 [`arithmetic_public`] 的 `+`
/// （整数相加、非可比报实测消息）。
pub fn concat_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 两个都是存活对象（调用方保证）。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    let left_text = instance.text_value(left);
    let right_text = instance.text_value(right);
    if let (Some(a), Some(b)) = (left_text, right_text) {
        return Ok(instance.new_str(&format!("{a}{b}")));
    }
    // `bytes + bytes`（`P1-12`；实测 `b'ab' + b'cd' == b'abcd'`）
    let (left_bytes, right_bytes) = (instance.bytes_value(left), instance.bytes_value(right));
    if let (Some(a), Some(b)) = (left_bytes, right_bytes) {
        let mut joined = Vec::with_capacity(a.len() + b.len());
        joined.extend_from_slice(a);
        joined.extend_from_slice(b);
        return Ok(instance.new_bytes(&joined));
    }
    let list_type = instance.type_named("list");
    if Some(left_type) == list_type && Some(right_type) == list_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<ListObject>(),
                &*right.as_ptr().cast::<ListObject>(),
            )
        };
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(a.len() + b.len());
        for index in 0..a.len() {
            if let Some(item) = a.item(index) {
                // SAFETY: 值由列表持有，存活；新列表要自己那份。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        for index in 0..b.len() {
            if let Some(item) = b.item(index) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_list(items));
    }
    let tuple_type = instance.type_named("tuple");
    if Some(left_type) == tuple_type && Some(right_type) == tuple_type {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<TupleObject>(),
                &*right.as_ptr().cast::<TupleObject>(),
            )
        };
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(a.len() + b.len());
        for index in 0..a.len() {
            if let Some(item) = a.item(index) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        for index in 0..b.len() {
            if let Some(item) = b.item(index) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_tuple(items));
    }
    arithmetic_public(instance, left, right, "+", opcode)
}

/// **`in` 的公开入口**（`operator.contains` 用；与字节码 `CONTAINS_OP` 共用同一份实现）。
///
/// 参数顺序照参照：`contains(容器, 项)`。
pub fn contains_public(
    instance: &Instance,
    container: NonNull<Header>,
    item: NonNull<Header>,
    opcode: u8,
) -> Result<bool, ExecError> {
    contains(instance, container, item, opcode)
}

/// **一元运算的公开入口**（`operator.neg`／`pos`／`abs`／`invert`）。
///
/// `symbol` 取 `"-"`／`"+"`／`"abs"`／`"~"`；非整数按**参照实测**的消息报
/// `TypeError: bad operand type for unary -: 'str'`。
pub fn unary_public(
    instance: &Instance,
    operand: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    if let Some(value) = instance.int_of(operand) {
        // `-`／`+`／`abs`／`~` 全走任意精度（`TS-45`）
        let wide = value.to_bigint();
        let result = match symbol {
            "-" => wide.neg(),
            "+" => wide,
            "abs" => wide.abs(),
            "~" => wide.invert(),
            _ => {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "unary_public 收到了没见过的一元运算符",
                })
            }
        };
        return Ok(instance.new_int_value(IntValue::from_big(result)));
    }
    let name = instance.type_name(instance.type_of(operand));
    let shown = if symbol == "abs" { "abs()" } else { symbol };
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("bad operand type for unary {shown}: '{name}'"),
    ))
}

/// **整数算术的公开入口**（`TS-40` 的数值面；**任意精度**见 `TS-45`）。
///
/// `symbol` 取 `"+"`／`"-"`／`"*"`／`"//"`／`"%"`／`"**"`／位运算与移位
/// （`operator.*` 与 `BINARY_OP` 共用）。非整数（浮点还没落地、或字符串这类）按**参照实测**
/// 的消息报 `TypeError: unsupported operand type(s) for +: 'int' and 'str'`。
///
/// **一条真相**：四则／整除／取模／幂／位运算／移位一律走 [`crate::bigint`] 的任意精度核心；
/// 补码语义（负数）与 floor 位移由核心负责，这里只管类型检查与参照实测的消息。
pub fn arithmetic_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // **`str % value`**（printf 风格）✓（第 281 轮）：`Lib/` 里遍地都是 ✓ —— 实测第一个撞上的是
    // `codecs.py` 的 `raise SystemError('… %s' % e)` ✓（`encodings.*` 那一族因此全红 ✗）。
    if symbol == "%" {
        if let Some(template) = instance.text_value(left) {
            return percent_format(instance, &template, right, opcode);
        }
    }
    // **真除法 `/`**：结果为 **float**（实测 `7/2 == 3.5`、`0/5 == 0.0`），
    // 除零报 `ZeroDivisionError: division by zero`（与 `//`／`%` 同一条消息）；
    // 大整数超出 double ⇒ 参照报 `OverflowError: int too large to convert to float`
    // `@`（矩阵乘）：本层没有矩阵类型 ⇒ **如实报参照实测的 `TypeError`**
    // （`1 @ 2` ⇒ `unsupported operand type(s) for @: 'int' and 'int'`）；
    // `@=` 一族由 `inplace_arithmetic` 走到这里，消息里的符号随之是 `@`
    if symbol == "@" {
        return Err(unsupported_operand(instance, left, right, "@"));
    }
    if symbol == "/" {
        let left_number = numeric_payload(instance, left).ok_or_else(|| {
            unsupported_operand(instance, left, right, "/")
        })?;
        let right_number = numeric_payload(instance, right).ok_or_else(|| {
            unsupported_operand(instance, left, right, "/")
        })?;
        if let Some(value) = instance.int_of(left).map(|value| value.to_bigint()) {
            if value.to_f64().is_infinite() {
                return Err(instance.raise_builtin_error(
                    "OverflowError",
                    "int too large to convert to float",
                ));
            }
        }
        if let Some(value) = instance.int_of(right).map(|value| value.to_bigint()) {
            if value.to_f64().is_infinite() {
                return Err(instance.raise_builtin_error(
                    "OverflowError",
                    "int too large to convert to float",
                ));
            }
        }
        if right_number == 0.0 {
            return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
        }
        return Ok(instance.new_float(left_number / right_number));
    }
    if let (Some(a), Some(b)) = (instance.int_of(left), instance.int_of(right)) {
        // 除零在参照里是 `ZeroDivisionError: division by zero`（实测）——`//` 与 `%` 都一样
        if b.is_zero() && matches!(symbol, "//" | "%") {
            return Err(instance.raise_builtin_error("ZeroDivisionError", "division by zero"));
        }
        let (wide_left, wide_right) = (a.to_bigint(), b.to_bigint());
        let result = match symbol {
            "+" => wide_left.add(&wide_right),
            "-" => wide_left.sub(&wide_right),
            "*" => wide_left.mul(&wide_right),
            "//" => wide_left.divmod_floor(&wide_right).expect("除零已在上面拦下").0,
            "%" => wide_left.divmod_floor(&wide_right).expect("除零已在上面拦下").1,
            "**" => {
                // 负指数在参照里给 `float`（`2 ** -1 == 0.5`）⇒ 与 `float` 互转接线前如实报未实现
                let Some(exponent) = b.to_i64().and_then(|value| u32::try_from(value).ok()) else {
                    return Err(ExecError::Unsupported {
                        opcode,
                        what: "整数幂：指数为负（参照给 float）或超出 u32，尚未接线",
                    });
                };
                wide_left.pow_u32(exponent)
            }
            "&" | "|" | "^" => match symbol {
                // 补码语义（负数无限符号扩展），核心已按参照夹具对拍
                "&" => wide_left.bit_and(&wide_right),
                "|" => wide_left.bit_or(&wide_right),
                _ => wide_left.bit_xor(&wide_right),
            },
            "<<" | ">>" => {
                // 位移量：负数 ⇒ `ValueError`（实测 `negative shift count`）
                let Some(count) = b.to_i64() else {
                    // 装不下 `i64` 的位移量：`<<` 一律超出实现上限 ⇒ `MemoryError`（实测同款）；
                    // `>>` 一定超过位宽 ⇒ 正数 0、负数 -1（`shr` 自己处理）
                    if symbol == "<<" {
                        return Err(instance.raise_builtin_error("MemoryError", ""));
                    }
                    return Ok(instance.new_int_value(IntValue::from_big(wide_left.shr(u64::MAX))));
                };
                if count < 0 {
                    return Err(instance.raise_builtin_error("ValueError", "negative shift count"));
                }
                let count = count as u64;
                if symbol == "<<" {
                    let Some(shifted) = wide_left.shl(count) else {
                        // 实测：`1 << 2**62` ⇒ `MemoryError`（消息为空）
                        return Err(instance.raise_builtin_error("MemoryError", ""));
                    };
                    shifted
                } else {
                    wide_left.shr(count)
                }
            }
            _ => {
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "arithmetic_public 收到了没见过的运算符",
                })
            }
        };
        return Ok(instance.new_int_value(IntValue::from_big(result)));
    }
    Err(unsupported_operand(instance, left, right, symbol))
}

/// 二元运算的类型不匹配错误（**一处真相**）：`unsupported operand type(s) for <op>: 'A' and 'B'`
/// （参照实测）。
fn unsupported_operand(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
) -> ExecError {
    let left_name = instance.type_name(instance.type_of(left));
    let right_name = instance.type_name(instance.type_of(right));
    instance.raise_builtin_error(
        "TypeError",
        &format!("unsupported operand type(s) for {symbol}: '{left_name}' and '{right_name}'"),
    )
}

/// `+=` 的**就地**语义（`NB_INPLACE_ADD`）：`list` 是**就地 extend**（别名可见，实测），
/// 其余类型退化为基运算 `+`（不可变 ⇒ 重新绑定与就地不可区分）。
fn inplace_add(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    if Some(unsafe { left.as_ref() }.ty()) == instance.type_named("list") {
        let items = sequence_items(instance, right, opcode)?;
        // SAFETY: 类型身份已确认，且 left 是帧值栈上的存活对象。
        let list = unsafe { &*left.as_ptr().cast::<ListObject>() };
        for item in items {
            list.append(item);
        }
        // SAFETY: 就地改动后返回**同一个对象**（新引用），调用方随后 `STORE` 回去
        unsafe { instance.incref_object(left.as_ptr()) };
        return Ok(left);
    }
    // 非 `list`：走 `+` 的**公开入口** `concat_public`（`str`／`bytes`／`tuple` 的拼接在那儿，
    // 其余落 `arithmetic_public`）——第一版这里写的是 `arithmetic_public`，被语料 `s += 'b'` 打回
    concat_public(instance, left, right, opcode)
}

/// 其余就地运算：不可变类型等价于基运算；**可变容器**（`set`／`dict`）的就地语义不同 ⇒ 如实报。
fn inplace_arithmetic(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    let container = unsafe { left.as_ref() }.ty();
    let mutable = Some(container) == instance.type_named("set")
        || Some(container) == instance.type_named("dict")
        || Some(container) == instance.type_named("list")
        || Some(container) == instance.type_named("bytearray");
    if mutable {
        return Err(ExecError::Unsupported {
            opcode,
            what: "可变容器的就地运算（`set`／`dict`／`bytearray` 的 `|=` 一族）尚未接线",
        });
    }
    arithmetic_public(instance, left, right, symbol, opcode)
}

/// **通用比较**（`TS-40`）：`int`／`bool`／`str` **按值**比较，其余类型报**参照实测**的
/// `TypeError`（`'<' not supported between instances of 'int' and 'str'`）。
///
/// `operator` 模块的比较族与 `COMPARE_OP` **共用同一份实现**（两处各写一份就是两处真相）。
/// `symbol` 取 `"<"`／`"<="`／`"=="`／`"!="`／`">"`／`">="`（照 `cmp_op` 的名字）。
///
/// **浮点尚未接线**（本层浮点类型还在未落地清单里）⇒ 遇到浮点按"别的类型"处理（报实测消息形）。
/// **取集合元素** ✓（第 205 轮）：`set` 与 `frozenset` **同一载荷** ✓（第 236 轮 ✓）⇒ 两边都认 ✓。
fn set_items_of(instance: &Instance, object: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
    let ty = instance.type_of(object);
    let is_set = Some(ty) == instance.type_named("set") || Some(ty) == instance.type_named("frozenset");
    if !is_set {
        return None;
    }
    // SAFETY: 类型身份刚确认 ⇒ 载荷就是 `SetObject` ✓。
    Some(unsafe { &*object.as_ptr().cast::<crate::builtin_objects::SetObject>() }.items().to_vec())
}


pub fn compare_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    symbol: &str,
    opcode: u8,
) -> Result<bool, ExecError> {
    // 相等／不等：走与 `==` 同一套（整数／浮点／字符串按值，其余按身份）
    match symbol {
        "==" => return Ok(values_equal_public(instance, left, right)),
        "!=" => return Ok(!values_equal_public(instance, left, right)),
        _ => {}
    }
    // 大小比较：两边都必须是**同一族**的标量（int／bool 一族、str 一族、bytes 一族）
    // **集合比较＝子集／超集** ✓（第 205 轮）：`Lib/os.py` 的 `_have_functions` 登记用 `<=` ✓
    // ⇒ 先前只有**标量**那一支 ✗ ⇒ 报 `TypeError: '<=' not supported between instances of 'set' and 'set'` ✗。
    // 口径与参照一致 ✓：`<=` 子集、`<` 真子集、`>=` 超集、`>` 真超集、`==` 两边互相包含 ✓、`!=` 取反 ✓。
    if let (Some(left_items), Some(right_items)) = (set_items_of(instance, left), set_items_of(instance, right)) {
        let contains = |haystack: &[NonNull<Header>], needle: NonNull<Header>| {
            haystack
                .iter()
                .any(|other| values_equal_public(instance, needle, *other))
        };
        let left_subset = left_items.iter().all(|item| contains(&right_items, *item));
        let right_subset = right_items.iter().all(|item| contains(&left_items, *item));
        return match symbol {
            "==" => Ok(left_subset && right_subset),
            "!=" => Ok(!(left_subset && right_subset)),
            "<=" => Ok(left_subset),
            "<" => Ok(left_subset && !right_subset),
            ">=" => Ok(right_subset),
            ">" => Ok(right_subset && !left_subset),
            _ => Ok(false),
        };
    }
    // **数值混比**（`float` 与 `int`／`bool`）✓（第 280 轮修 ✗）：参照里 `1.0 > 0` 为真 ✓，
    // 而先前"两边必须同一族" ✗ ⇒ 报 `TypeError: '>' not supported between instances of 'float' and 'int'` ✗
    //（实测：`_thread.TIMEOUT_MAX > 0` 当场撞上 ✓）。**NaN** 参与时参照给 `False`（**不是** `TypeError` ✓）。
    // 大整数超出 `i64` 时本层仍报 `TypeError`（如实 ✓；`P1-11` 的比较面随后补 ✓）。
    let left_float = instance.float_value(left);
    let right_float = instance.float_value(right);
    if left_float.is_some() || right_float.is_some() {
        let as_float = |value: NonNull<Header>, float: Option<f64>| {
            float.or_else(|| instance.int_value(value).map(|integer| integer as f64))
        };
        if let (Some(a), Some(b)) = (as_float(left, left_float), as_float(right, right_float)) {
            let Some(ordering) = a.partial_cmp(&b) else {
                return Ok(false);
            };
            return Ok(match symbol {
                "<" => ordering.is_lt(),
                "<=" => ordering.is_le(),
                ">" => ordering.is_gt(),
                ">=" => ordering.is_ge(),
                _ => false,
            });
        }
    }
    let left_int = instance.int_of(left);
    let right_int = instance.int_of(right);
    let left_text = instance.text_value(left);
    let right_text = instance.text_value(right);
    let left_bytes = instance.bytes_value(left);
    let right_bytes = instance.bytes_value(right);
    let ordering = match (left_int, right_int, left_text, right_text, left_bytes, right_bytes) {
        (Some(a), Some(b), _, _, _, _) => Some(a.cmp(&b)),
        (_, _, Some(a), Some(b), _, _) => a.partial_cmp(&b),
        // `bytes` 按**字节**字典序（`P1-12`；实测 `b'ab' < b'b'` 为真）
        (_, _, _, _, Some(a), Some(b)) => Some(a.cmp(b)),
        _ => None,
    };
    let Some(ordering) = ordering else {
        let left_name = instance.type_name(instance.type_of(left));
        let right_name = instance.type_name(instance.type_of(right));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{symbol}' not supported between instances of '{left_name}' and '{right_name}'"),
        ));
    };
    let _ = opcode;
    Ok(match symbol {
        "<" => ordering.is_lt(),
        "<=" => ordering.is_le(),
        ">" => ordering.is_gt(),
        ">=" => ordering.is_ge(),
        _ => {
            return Err(ExecError::Unsupported {
                opcode,
                what: "compare_public 收到了没见过的比较符号",
            })
        }
    })
}

/// **`BC-39` 的 `NB_SUBSCR` 语义**（`pa_gettable` 用）：容器 ＋ 键 ⇒ **新引用**。
///
/// 实参是**借用视图**；异常经 [`ExecError::Raised`] 上抛。
pub fn subscript_read(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    subscript_get(instance, container, key, 0)
}

/// **`STORE_SUBSCR` 语义**（`pa_settable` 用）：容器 ＋ 键 ＋ 值（值为**借用**，写入时接管新引用）。
pub fn subscript_write(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    value: NonNull<Header>,
) -> Result<(), ExecError> {
    // 值要被容器接管 ⇒ 先为容器新增一份
    // SAFETY: 调用方保证 value 存活。
    unsafe { instance.incref_object(value.as_ptr()) };
    subscript_set(instance, container, key, value, 0)
}

/// **`OM-11` 的 `getattr` 语义**（`pa_getfield` 用）：对象 ＋ 名字 ⇒ **新引用**。
pub fn attribute_read(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<NonNull<Header>, ExecError> {
    // 先走属性通道（`TS-44`：类型字典的同名 dunder 优先，实例字典不参与）
    match attribute_lookup(instance, object, name) {
        Ok(Attribute::Owned(value)) => Ok(value),
        Ok(Attribute::Value(value)) => {
            // SAFETY: 值由字典持有，存活。
            unsafe { instance.incref_object(value.as_ptr()) };
            Ok(value)
        }
        Ok(Attribute::Method { function, this }) => {
            // 取到方法：按 `OM-11` 给"函数 ＋ self"的绑定方法对象（与 `LOAD_ATTR` 无方法位同款）
            // SAFETY: 两者都存活。
            unsafe {
                instance.incref_object(function.as_ptr());
                instance.incref_object(this.as_ptr());
            }
            let bound = instance.alloc(MethodObject::new(
                builtin_type(instance, "method"),
                function,
                this,
            ));
            Ok(bound.into_raw().cast::<Header>())
        }
        Err(error) => Err(error),
    }
}

/// **取属性但不报错**：找不到（`AttributeError`）给 `None`，别的异常照上抛。
///
/// 给 `GET_ITER` 的 `__iter__` 探测、`advance` 的 `__next__` 探测用——那两处要区分
/// "没有这个 dunder"（走别的路径或报实测消息）与"用户代码自己抛了异常"（上抛）。
pub fn attribute_optional(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Option<NonNull<Header>>, ExecError> {
    match attribute_read(instance, object, name) {
        Ok(value) => Ok(Some(value)),
        Err(ExecError::Raised { exception }) => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            if instance.is_subtype(ty, exception_type(instance, "AttributeError")) {
                release(instance, exception);
                Ok(None)
            } else {
                Err(ExecError::Raised { exception })
            }
        }
        Err(other) => Err(other),
    }
}

/// **`OM-11` 的 `setattr` 语义**（`pa_setfield` 用）：对象 ＋ 名字 ＋ 值（值为**借用**）。
pub fn attribute_write(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    value: NonNull<Header>,
) -> Result<(), ExecError> {
    // SAFETY: 调用方保证 value 存活；属性表要自己那份。
    unsafe { instance.incref_object(value.as_ptr()) };
    instance_attribute_set(instance, object, name, value, 0)
}

/// 按值调用一个可调用对象（`AB-24` 的宿主交接面与 `pa_call` 用）。
///
/// 实参是**借用视图**；成功返回**新引用**。异常经 [`ExecError::Raised`] 上抛。
pub fn call_value(
    instance: &Instance,
    callable: NonNull<Header>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let mut owned_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        owned_args.push(*argument);
    }
    let mut owned_kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(kwargs.len());
    for (key, value) in kwargs {
        // SAFETY: 同上。
        unsafe {
            instance.incref_object(key.as_ptr());
            instance.incref_object(value.as_ptr());
        }
        owned_kwargs.push((*key, *value));
    }
    call_callable(instance, callable, None, owned_args, owned_kwargs, 0)
}

/// 按**属性通道**在对象上找一个方法并调用（`TS-44`）：找不到返回 `Ok(None)`，
/// 找到就返回调用的结果（**新引用**）。
pub(crate) fn call_object_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<Option<NonNull<Header>>, ExecError> {
    let found = match attribute_lookup(instance, object, name) {
        Ok(found) => found,
        Err(_) => return Ok(None),
    };
    let (callable, this) = match found {
        Attribute::Method { function, this } => (function, this),
        Attribute::Value(method) | Attribute::Owned(method) => (method, object),
    };
    let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        // SAFETY: 调用方保证实参存活。
        unsafe { instance.incref_object(argument.as_ptr()) };
        call_args.push(*argument);
    }
    let result = call_callable(instance, callable, Some(this), call_args, Vec::new(), 0)?;
    Ok(Some(result))
}

/// **`TS-44`**：类型字典里若定义了某个 dunder（`__repr__`／`__str__`），就调用它并取文本。
///
/// 找不到定义 ⇒ `None`（调用方走槽位路径）。**只在类型字典里有定义时才调用**，所以内建类型
/// （它们靠槽位）零开销、行为不变；用户类的覆写则**一致地**在顶层 `repr(obj)`／`str(obj)` 与
/// 容器元素上都生效。
///
/// 覆写抛异常时：**如实上抛**（`OM-11` 扩之后 `repr`／`str` 槽能表达失败了 ⇒ 不再吞掉；
/// 此前"吞掉 + 记在实例上"的偏差随之消失）。
pub(crate) fn override_text(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<Option<String>, ExecError> {
    let ty = instance.type_of(object);
    // 类型字典里没有这个名字 ⇒ 没有覆写，直接走槽位路径（**不是**"调用失败"）
    let Some((owner, _)) = instance.type_lookup_owner(ty, name) else {
        return Ok(None);
    };
    // **`object` 自己那条不算覆写** ✓（第 211 轮 ✗）：`object.__str__`／`object.__repr__` 是本层新挂的
    // **属性面**（`Lib/types.py` 的 `type(object.__str__)` 要它 ✓）⇒ 若当覆写 ⇒ **每个**对象都会命中
    // 它 ⇒ 把 `str`／`int` 自带的 `str` 槽带跑 ✗（实测 `f"{x}"` 给 `\'1\'` ✗、四条 f-string 语料红 ✓）。
    if Some(owner) == instance.type_named("object") {
        return Ok(None);
    }
    match call_object_method(instance, object, name, &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            let text = instance.text_value(result);
            release(instance, result);
            Ok(text)
        }
        None => Ok(None),
    }
}

/// **`TS-44`**：元素的 `repr` —— 先走属性通道的 `__repr__`，没有才落到原生槽位／默认实现。
///
/// 容器载荷的 `repr` 槽用它（`repr([x])` 里的 `x` 也要尊重 Python 级覆写）。
pub(crate) fn element_repr(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<String, ExecError> {
    match call_object_method(instance, object, "__repr__", &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            let is_str = unsafe { result.as_ref() }.ty() == instance.singletons().str_type();
            let text = if is_str {
                // SAFETY: 类型身份已确认。
                Some(unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned())
            } else {
                None
            };
            release(instance, result);
            match text {
                Some(text) => Ok(text),
                None => instance.object_repr(object),
            }
        }
        None => instance.object_repr(object),
    }
}

/// **`TS-44`**：元素的 `str` —— 同上，走 `__str__`。
#[allow(dead_code)] // 容器 `str`（`str([x])`）接线时用它；现在只剩 `repr` 那条在用
pub(crate) fn element_str(
    instance: &Instance,
    object: NonNull<Header>,
) -> Result<String, ExecError> {
    match call_object_method(instance, object, "__str__", &[])? {
        Some(result) => {
            // SAFETY: result 是新引用，存活。
            let is_str = unsafe { result.as_ref() }.ty() == instance.singletons().str_type();
            let text = if is_str {
                // SAFETY: 类型身份已确认。
                Some(unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned())
            } else {
                None
            };
            release(instance, result);
            match text {
                Some(text) => Ok(text),
                None => instance.object_str(object),
            }
        }
        None => instance.object_str_native(object),
    }
}

/// **`TS-44`**：语义走**属性通道**——先查类型字典里的同名 dunder（返回 `str` 的文本），
/// 查不到就返回 `None`（调用方落到原生槽位／默认实现）。
fn dunder_text(
    instance: &Instance,
    value: NonNull<Header>,
    name: &str,
    opcode: u8,
) -> Result<Option<String>, ExecError> {
    let found = match attribute_lookup(instance, value, name) {
        Ok(found) => found,
        Err(_) => return Ok(None),
    };
    let (callable, this) = match found {
        Attribute::Method { function, this } => (function, this),
        Attribute::Value(method) | Attribute::Owned(method) => (method, value),
    };
    let result = call_callable(instance, callable, Some(this), Vec::new(), Vec::new(), opcode)?;
    // SAFETY: result 是新引用，存活。
    let result_type = unsafe { result.as_ref() }.ty();
    if result_type != instance.singletons().str_type() {
        // 实测：`TypeError: __str__ returned non-string (type int)`
        // SAFETY: 类型名由注册表持有。
        let type_name = unsafe { result_type.as_ref() }.name();
        release(instance, result);
        let message = format!("{name} returned non-string (type {type_name})");
        return Err(raise_builtin(instance, "TypeError", &message));
    }
    // SAFETY: 类型身份已确认。
    let text = unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned();
    release(instance, result);
    Ok(Some(text))
}

/// `ascii()` 的转义：非 ASCII 字符按 `\xNN`／`\uNNNN`／`\UNNNNNNNN` 写出来。
fn escape_non_ascii(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        if character.is_ascii() {
            out.push(character);
            continue;
        }
        let code = character as u32;
        if code <= 0xFF {
            out.push_str(&format!("\\x{code:02x}"));
        } else if code <= 0xFFFF {
            out.push_str(&format!("\\u{code:04x}"));
        } else {
            out.push_str(&format!("\\U{code:08x}"));
        }
    }
    out
}

/// 取一个已注册的**异常类**（`TS-41` 的表里那棵树）。
fn exception_type(instance: &Instance, name: &str) -> NonNull<TypeObject> {
    instance
        .type_named(name)
        .unwrap_or_else(|| panic!("TS-41：异常类 {name} 应当已注册"))
}

/// 这个类型是不是异常类（MRO 里有 `BaseException`）。
fn is_exception_type(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    instance.is_subtype(ty, exception_type(instance, "BaseException"))
}

/// 造一个异常实例（`args` 只有一个 `str` 消息）——**新引用**。
fn new_exception(instance: &Instance, ty: NonNull<TypeObject>, message: &str) -> NonNull<Header> {
    let text = instance
        .alloc(StrObject::new(
            instance.singletons().str_type(),
            message.to_owned(),
        ))
        .into_raw()
        .cast::<Header>();
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(vec![text]),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

/// 造一个异常实例，`args` 用给定的那批**新引用**（异常对象接手）。
fn new_exception_with_args(
    instance: &Instance,
    ty: NonNull<TypeObject>,
    args: Vec<NonNull<Header>>,
) -> NonNull<Header> {
    let object = instance.alloc(ExceptionObject::new(
        ty,
        RefCell::new(args),
        RefCell::new(None),
        RefCell::new(None),
        Cell::new(false),
        RefCell::new(None),
    ));
    object.into_raw().cast::<Header>()
}

/// 抛一个异常：记在实例上（借它保活）并交出错误（`BC-60` ②）。
fn raise(instance: &Instance, exception: NonNull<Header>) -> ExecError {
    // **两份所有权**：实例状态一份、`Err(Raised)` 一份——所以这里必须为状态**新增**一份。
    // 少了这一步就是双重所有权：状态与错误各自以为"我持有它"，先释放的一方让另一方悬垂
    // （症状：调用方拿到 `Err(Raised)` 里的异常对象时读到已释放内存）。
    // `Err(Raised)` 那一份由派发器接手（`push` 进值栈）或由调用方消费。
    // SAFETY: exception 由调用方保证存活。
    unsafe { instance.incref_object(exception.as_ptr()) };
    if let Some(previous) = instance.set_pending_exception(Some(exception)) {
        release(instance, previous);
    }
    ExecError::Raised { exception }
}

/// 抛一个内建异常（带消息）。
pub(crate) fn raise_builtin(instance: &Instance, name: &str, message: &str) -> ExecError {
    let exception = new_exception(instance, exception_type(instance, name), message);
    raise(instance, exception)
}


// ---- `BC-23` 的边界检查（`TS-10`…`TS-13`） ----

/// 边界检查：`CHECK_BOUNDARY_IN` 查**入参**、`CHECK_BOUNDARY_OUT` 查**返回值**。
///
/// * 归责方向（`TS-10`）：入参失败归**调用方**、返回值失败归**被调用方**——消息里带方向，
///   捕获方能区分（`TS-12`）
/// * 签名条目（`BC-24`）：`oparg` 是**常量表下标**，常量**必须**是**标签元组**。
///   `IN` 按顺序比对帧的局部槽 `0..`（形参），`OUT` 比对**栈顶**（**不**弹出——后面紧跟
///   `RETURN_VALUE`）
/// * 标签形态：类型对象 ⇒ 子类型判定（`TS-29`）；字符串 `"Any"` ⇒ 双向相容（`TS-28`）；
///   二元组 `(外类型, 内标签)` ⇒ 只看**外类型**（`TS-13` 默认浅层；`TS-31` 的深层档位随后补，
///   `TS-30` 的不变性因此只体现在外类型上）
/// * 失败抛 `TypeBoundaryError`，消息含 `TS-11` 的四要素：方向、期望、实际、位置（文件名＋行号）
fn boundary_check(
    instance: &Instance,
    frame: &Frame,
    oparg: u8,
    opcode: u8,
) -> Result<(), ExecError> {
    let code_raw = frame.code().ok_or(ExecError::Unsupported {
        opcode,
        what: "边界检查需要 code object",
    })?;
    // SAFETY: 帧持有一份 code object 引用，存活。
    let code = unsafe { &*code_raw.as_ptr().cast::<CodeObject>() };
    let signature = code.constant(oparg as usize).ok_or(ExecError::Unsupported {
        opcode,
        what: "边界检查的签名条目下标越界",
    })?;
    // SAFETY: 常量由常量表持有，存活。
    let signature_type = unsafe { signature.as_ref() }.ty();
    if signature_type != builtin_type(instance, "tuple") {
        return Err(ExecError::Unsupported {
            opcode,
            what: "边界检查的签名条目必须是标签元组（编译器保证）",
        });
    }
    // SAFETY: 类型身份刚确认。
    let labels = unsafe { &*signature.as_ptr().cast::<TupleObject>() };
    let incoming = opcode == opcode_of("CHECK_BOUNDARY_IN") as u8;
    let mut checked = 0usize;
    for index in 0..labels.len() {
        let label = labels.item(index).expect("下标在范围内");
        let actual = if incoming {
            // 形参不够（实参更少）时没有可查的值 ⇒ 交给调用绑定那条路去报错
            match frame.local(index) {
                Ok(Some(value)) => value,
                _ => continue,
            }
        } else {
            // 返回值在栈顶：**不**弹出
            let value = frame.peek().map_err(|_| ExecError::Unsupported {
                opcode,
                what: "边界检查要在栈顶取值，但栈是空的",
            })?;
            if index + 1 < labels.len() {
                // `OUT` 的签名条目只有一个标签；多给了就按"取第一个"处理会悄悄放行 ⇒ 如实报错
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "返回值边界检查的签名条目只能是**一个**标签",
                });
            }
            value
        };
        checked += 1;
        if boundary_accepts(instance, actual, label) {
            continue;
        }
        // SAFETY: actual 是存活对象。
        let actual_name = type_name_of(instance, unsafe { actual.as_ref() }.ty());
        let expected = render_label(instance, label);
        let line = line_at_offset(code, frame.instruction_pointer());
        let direction = if incoming { "argument" } else { "return" };
        let message = format!(
            "{direction} boundary check failed: expected {expected}, got {actual_name} \
             ({}: line {line})",
            code.filename()
        );
        return Err(raise_builtin(instance, "TypeBoundaryError", &message));
    }
    let _ = checked;
    Ok(())
}

/// 一条标签是否接受这个**实际值**（`TS-28`…`TS-30`／`TS-13` 的浅层口径）。
fn boundary_accepts(instance: &Instance, actual: NonNull<Header>, label: NonNull<Header>) -> bool {
    // SAFETY: actual 由调用方保证存活。
    let actual_type = unsafe { actual.as_ref() }.ty();
    // SAFETY: label 由常量表持有，存活。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type == builtin_type(instance, "str") {
        // SAFETY: 类型身份刚确认。
        return unsafe { &*label.as_ptr().cast::<StrObject>() }.value() == "Any";
    }
    if is_type_object(instance, label_type) {
        // SAFETY: label 是类型对象。
        let expected = label.cast::<TypeObject>();
        return instance.is_subtype(actual_type, expected);
    }
    if label_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份刚确认。
        let parts = unsafe { &*label.as_ptr().cast::<TupleObject>() };
        if parts.len() != 2 {
            return false;
        }
        let outer = parts.item(0).expect("下标在范围内");
        let inner = parts.item(1).expect("下标在范围内");
        if !is_type_object(instance, unsafe { outer.as_ref() }.ty()) {
            return false;
        }
        if !instance.is_subtype(actual_type, outer.cast::<TypeObject>()) {
            return false;
        }
        // **`TS-31` 的深层档位**：标签**带了内层**就递归比元素。浅层编译只发**裸**类型标签
        // （见 `compile::CheckTier`），所以这条分支只在深层产物里出现——`TS-13` 的
        // "默认浅层"因此是**代码生成**的结果，而不是运行期开关。
        return elements_accepted(instance, actual, actual_type, inner);
    }
    false
}

/// **`TS-31`**：容器元素逐个比（深层档位）。只深入**恰好是** `list`／`tuple` 的实际值——
/// 别的容器（子类、`dict` 等）没有统一的元素视图，**放行**并在文档里写明这条边界。
fn elements_accepted(
    instance: &Instance,
    actual: NonNull<Header>,
    actual_type: NonNull<TypeObject>,
    inner: NonNull<Header>,
) -> bool {
    if actual_type == builtin_type(instance, "list") {
        // SAFETY: 类型身份刚确认。
        let list = unsafe { &*actual.as_ptr().cast::<crate::ListObject>() };
        return (0..list.len()).all(|index| {
            list.item(index)
                .map(|item| boundary_accepts(instance, item, inner))
                .unwrap_or(true)
        });
    }
    if actual_type == builtin_type(instance, "tuple") {
        // SAFETY: 同上。
        let tuple = unsafe { &*actual.as_ptr().cast::<TupleObject>() };
        return (0..tuple.len()).all(|index| {
            tuple
                .item(index)
                .map(|item| boundary_accepts(instance, item, inner))
                .unwrap_or(true)
        });
    }
    true
}

/// 这个类型是不是"类型对象"（`type` 及其子类，`TS-41` 的层次说了算）。
fn is_type_object(instance: &Instance, ty: NonNull<TypeObject>) -> bool {
    instance.is_subtype(ty, builtin_type(instance, "type"))
}

/// 渲染一个标签给消息用：类型对象给名字、二元组给 `外[内]`、其余给 `Any`。
fn render_label(instance: &Instance, label: NonNull<Header>) -> String {
    // SAFETY: label 由常量表持有，存活。
    let label_type = unsafe { label.as_ref() }.ty();
    if label_type == builtin_type(instance, "str") {
        // SAFETY: 类型身份刚确认。
        return unsafe { &*label.as_ptr().cast::<StrObject>() }.value().to_owned();
    }
    if is_type_object(instance, label_type) {
        return type_name_of(instance, label.cast::<TypeObject>());
    }
    if label_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份刚确认。
        let parts = unsafe { &*label.as_ptr().cast::<TupleObject>() };
        if parts.len() == 2 {
            return format!(
                "{}[{}]",
                render_label(instance, parts.item(0).expect("下标在范围内")),
                render_label(instance, parts.item(1).expect("下标在范围内"))
            );
        }
    }
    "Any".to_owned()
}

/// 类型对象的 `__name__`。
fn type_name_of(instance: &Instance, ty: NonNull<TypeObject>) -> String {
    let _ = instance;
    // SAFETY: ty 由注册表持有。
    unsafe { ty.as_ref() }.name().to_owned()
}

/// 按**指令序号**取位置表里的行（`BC-18`）：`offset` 是**码元**偏移。
/// **槽号 → 名字** ✓（第 267 轮诊断用）：按 `localsplus` 那条规矩 ✓（局部在前 ✓、追加的 cell 其次 ✓、free 最后 ✓）。
fn localsplus_name(code: &CodeObject, slot: usize) -> String {
    if slot < code.nlocals() {
        if let Some(name) = code.varname(slot) {
            return name.to_owned();
        }
    }
    // 形参 cell 的判断走 `varname(slot)` ✓（`CodeObject` 没有整表的访问器 ✓）。
    let is_parameter = |name: &String| {
        (0..code.nlocals()).any(|slot| code.varname(slot) == Some(name.as_str()))
    };
    let appended: Vec<&String> = code.cellvars().iter().filter(|name| !is_parameter(name)).collect();
    if let Some(name) = appended.get(slot.saturating_sub(code.nlocals())) {
        return (*name).clone();
    }
    let base = code.nlocals() + appended.len();
    if let Some(name) = code.freevars().get(slot.saturating_sub(base)) {
        return name.clone();
    }
    "<未知>".to_owned()
}

/// **相对导入的名字解析** ✓（`IMPORT_NAME` 的 `level > 0`；第 278 轮接线）。
///
/// 参照口径：`__package__` 优先 ✓；空则看 `__path__` 在不在（在 ⇒ 当前就是包 ⇒ 用 `__name__` ✓），
/// 否则取 `__name__` 去掉最后一段 ✓；再按 `level` 往上走（`level == 1` ⇒ 当前包本身 ✓）。
/// `level` 越过顶层 ⇒ 照参照报 `ImportError: attempted relative import beyond top-level package` ✓。
fn resolve_relative_import(
    instance: &Instance,
    frame: &Frame,
    raw: &str,
    level: usize,
    opcode: u8,
) -> Result<String, ExecError> {
    // **模块级帧用 `namespace`、函数帧用 `globals`** ✓（实测：`importlib/__init__.py` 的相对导入
    // 在模块级帧上跑 ⇒ `globals` 是 `None` ✗）⇒ 两个都看 ✓。
    let globals = frame
        .globals()
        .or_else(|| frame.namespace())
        .ok_or(ExecError::Unsupported {
            opcode,
            what: "相对导入需要当前模块的名字空间（`globals`／`namespace` 都是空）",
        })?;
    let text = |key: &str| {
        instance
            .dict_get(globals, key)
            .and_then(|value| instance.text_of(value))
            .map(str::to_owned)
    };
    let name = text("__name__").unwrap_or_default();
    let package = match text("__package__") {
        Some(package) if !package.is_empty() => package,
        _ => {
            if instance.dict_get(globals, "__path__").is_some() {
                name.clone()
            } else {
                match name.rfind('.') {
                    Some(index) => name[..index].to_owned(),
                    None => String::new(),
                }
            }
        }
    };
    // 与参照的 `package.rsplit('.', level - 1)` 同义 ✓
    let bits: Vec<&str> = package.rsplitn(level, '.').collect();
    if bits.len() < level {
        return Err(instance.raise_builtin_error(
            "ImportError",
            "attempted relative import beyond top-level package",
        ));
    }
    let base = bits[bits.len() - 1];
    if raw.is_empty() {
        Ok(base.to_owned())
    } else {
        Ok(format!("{base}.{raw}"))
    }
}

/// **按对象抛**（`_codecs` 的错误处理器要用：`strict_errors` 就是"原样再抛" ✓）。
///
/// 契约与 [`raise`] 同款：调用方交**一份新引用**（`Err(Raised)` 那一份由派发器接手 ✓），
/// 函数内部再为**实例的待处理异常状态**加一份 ✓。
pub fn raise_object_public(instance: &Instance, exception: NonNull<Header>) -> ExecError {
    raise(instance, exception)
}

/// **`IMPORT_NAME` 的 fromlist 那一步**（第 279 轮接线；参照 `importlib._bootstrap._handle_fromlist`）。
///
/// 口径（照参照 ✓）：
/// - **只有包**（模块命名空间里有 `__path__` ✓）才做这一步 —— 普通模块没有子模块 ✓；
/// - 逐个名字：**已经是模块属性** ⇒ 跳过 ✓（随后 `IMPORT_FROM` 会取到它 ✓）；否则把
///   `<模块名>.<名字>` 当**子模块**导入 ✓（`load_module` 会把它挂成父包的属性 ✓）；
/// - 子模块**真不存在** ⇒ **忽略** ✓（参照的向下兼容：交给随后的 `IMPORT_FROM` 去报
///   `AttributeError` ✓）；子模块**自己执行出错**等 ⇒ **原样上抛** ✓，**不得**吞 ✗（吞了会把
///   `Lib/` 里的真 bug 伪装成"这个名字没有" ✗）。
fn handle_fromlist(
    instance: &Instance,
    modules: NonNull<Header>,
    module: NonNull<Header>,
    fromlist: NonNull<Header>,
    module_name: &str,
    opcode_number: u8,
) -> Result<(), ExecError> {
    // `fromlist` 是编译器发的**元组常量**（`import a` ⇒ 空元组 ✓）；不是元组就当作没有 ✓。
    if instance.type_name(instance.type_of(fromlist)) != "tuple" {
        return Ok(());
    }
    // **包才有子模块** ✓（参照：`hasattr(module, '__path__')` ✓）
    if !module_has_name(instance, module, "__path__") {
        return Ok(());
    }
    // SAFETY: 上面刚确认 fromlist 的类型是 `tuple`。
    let names: Vec<String> = unsafe { &*fromlist.as_ptr().cast::<TupleObject>() }
        .items()
        .iter()
        .filter_map(|item| instance.text_of(*item).map(str::to_owned))
        .collect();
    for name in names {
        if module_has_name(instance, module, &name) {
            continue;
        }
        let from = format!("{module_name}.{name}");
        match load_module(instance, modules, &from, opcode_number) {
            // **交的是借用** ✓（`load_module` 的约定，第 281 轮统一 ✓）⇒ 这里**不还** ✓
            Ok(_loaded) => {}
            Err(error) => {
                if !ignore_missing_submodule(instance, modules, &from, &error) {
                    return Err(error);
                }
                // **吞掉它** ✓（照参照的 `continue` ✓）：异常状态一份、`Err` 一份，**两份都要还** ✓
                if let ExecError::Raised { exception } = error {
                    if let Some(previous) = instance.set_pending_exception(None) {
                        release(instance, previous);
                    }
                    release(instance, exception);
                }
            }
        }
    }
    Ok(())
}

/// 探测"这个名字在不在"（`hasattr` 的等价物）—— **只查模块命名空间** ✓。
///
/// **为什么不用 `attribute_lookup`**：它以 `Err(Raised)` 报"没有"，而 `raise_builtin` 会**写
/// `pending_exception`** ✗ ⇒ 拿它当探针会**污染异常状态**（`hasattr` 是只读的 ✓）。
fn module_has_name(instance: &Instance, module: NonNull<Header>, name: &str) -> bool {
    module_value(instance, module, name).is_some()
}

/// 从模块命名空间里读一个属性（读不到 ⇒ `None`）—— **只看"在不在"，不看类型** ✓
/// （`__path__` 是**列表** ✗ ⇒ 不能拿 [`module_text`] 当存在性判据 ✗）。
fn module_value(
    instance: &Instance,
    module: NonNull<Header>,
    name: &str,
) -> Option<NonNull<Header>> {
    let namespace = mounted_instance_dict(instance, module)?;
    // **只有真的是 `dict` 才能按 `DictObject` 取项** ✓（第 185 轮的教训同款 ✗）
    if instance.type_name(instance.type_of(namespace)) != "dict" {
        return None;
    }
    // SAFETY: 上面刚确认 namespace 的类型是 `dict`。
    let entries = unsafe { &*namespace.as_ptr().cast::<DictObject>() }.entries();
    entries
        .into_iter()
        .find(|(key, _)| str_matches_public(instance, *key, name))
        .map(|(_, value)| value)
}

/// 从模块命名空间里读一个**字符串**属性（读不到 ⇒ `None`）。
fn module_text(instance: &Instance, module: NonNull<Header>, name: &str) -> Option<String> {
    module_value(instance, module, name)
        .and_then(|value| instance.text_of(value).map(str::to_owned))
}

/// 失败的子模块导入该不该**忽略** ✓（参照 `_handle_fromlist` 的向下兼容）。
///
/// 三条同时成立才忽略 ✓：① 是 `ModuleNotFoundError`；② 消息里的名字**就是** `from`；
/// ③ `sys.modules` 里**没留下**它（参照的 `sys.modules.get(from, _ERR_MSG) is _ERR_MSG` ——
/// 半截模块留在表里 ⇒ 那是"它存在但跑挂了" ✗，**必须**上抛 ✓）。
fn ignore_missing_submodule(
    instance: &Instance,
    modules: NonNull<Header>,
    from: &str,
    error: &ExecError,
) -> bool {
    let ExecError::Raised { exception } = error else {
        return false;
    };
    let exception = *exception;
    if instance.type_name(instance.type_of(exception)) != "ModuleNotFoundError" {
        return false;
    }
    let expected = format!("No module named '{from}'");
    if instance.exception_message_of(exception).as_deref() != Some(expected.as_str()) {
        return false;
    }
    instance.dict_get(modules, from).is_none()
}

/// **未绑定局部槽** ✓（第 277 轮诊断升级）：参照在同样情形给
/// `UnboundLocalError: cannot access local variable '<名>' where it is not associated with a value` ✓
/// ⇒ 照它报，并带上**变量名**（先前只报"槽 N 未绑定（未接线）" ✗ ⇒ 无从下手 ✓）。
fn unbound_local_error(
    instance: &Instance,
    frame: &Frame,
    slot: usize,
) -> ExecError {
    let name = match frame.code() {
        Some(header) => {
            let code = unsafe { &*header.as_ptr().cast::<crate::CodeObject>() };
            localsplus_name(code, slot)
        }
        None => "<未知>".to_owned(),
    };
    instance.raise_builtin_error(
        "UnboundLocalError",
        &format!("cannot access local variable '{name}' where it is not associated with a value"),
    )
}

fn line_at_offset(code: &CodeObject, offset: usize) -> u32 {
    let mut decoder = crate::decode::Decoder::new(code.code());
    let mut ordinal = 0usize;
    while let Ok(Some(instruction)) = decoder.next_instruction() {
        if instruction.offset == offset {
            // **`BC-4` 扩**后行号也可缺失（合成指令）⇒ 缺失时落到 `firstlineno`（与参照的
            // `PyCode_Addr2Line` 对无行条目一致地"不冒充行号"）
            if let Some((Some(line), _, _, _)) = code.positions().get(ordinal) {
                return *line;
            }
            break;
        }
        ordinal += 1;
    }
    code.firstlineno() as u32
}

// ---- `BC-56` 的消息：**逐条实测**（禁止手写近似文本，见 tests/calls.rs 的记录）----

fn message_too_many(name: &str, accepted: usize, required: usize, given: usize) -> String {
    // 动词也随**实参个数**变：`… but 1 was given`（实测；夹具 `fixture-argbind-3.14.json`
    // 的 `none_positional` 用例抓出来的）。名词则随**形参个数**变（`1 positional argument`）。
    let verb = if given == 1 { "was" } else { "were" };
    let noun = if accepted == 1 {
        "argument"
    } else {
        "arguments"
    };
    if required < accepted {
        format!(
            "{name}() takes from {required} to {accepted} positional {noun} but {given} {verb} given"
        )
    } else if accepted == 1 {
        format!("{name}() takes 1 positional argument but {given} {verb} given")
    } else {
        format!("{name}() takes {accepted} positional {noun} but {given} {verb} given")
    }
}

fn message_missing(name: &str, missing: &[String], keyword_only: bool) -> String {
    let kind = if keyword_only {
        "keyword-only"
    } else {
        "positional"
    };
    if missing.len() == 1 {
        return format!(
            "{name}() missing 1 required {kind} argument: '{}'",
            missing[0]
        );
    }
    let quoted: Vec<String> = missing.iter().map(|item| format!("'{item}'")).collect();
    let head = quoted[..quoted.len() - 1].join(", ");
    let last = quoted.last().cloned().unwrap_or_default();
    // 实测：两个是 `'a' and 'b'`（无逗号），三个及以上是 `'a', 'b', and 'c'`（有逗号）
    let conjunction = if quoted.len() == 2 { " and " } else { ", and " };
    format!(
        "{name}() missing {} required {kind} arguments: {head}{conjunction}{last}",
        missing.len()
    )
}

fn message_duplicate(name: &str, argument: &str) -> String {
    format!("{name}() got multiple values for argument '{argument}'")
}

fn message_unexpected_keyword(name: &str, argument: &str) -> String {
    format!("{name}() got an unexpected keyword argument '{argument}'")
}

fn message_positional_only(name: &str, arguments: &[String]) -> String {
    format!(
        "{name}() got some positional-only arguments passed as keyword arguments: '{}'",
        arguments.join(", ")
    )
}

/// 取一个 `str` 对象的文本（关键字实参的名字要用它）。
fn str_text(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<String, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty != instance.singletons().str_type() {
        return Err(ExecError::Unsupported {
            opcode,
            what: "关键字实参的名字必须是 str",
        });
    }
    // SAFETY: 类型身份已确认。
    Ok(unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned())
}

/// 取函数对象的位置默认值（**借用**，需要时自行 incref）。
/// 函数对象的闭包（**cell 列表**；每项是**借用**的裸引用，不新增引用）。
fn function_closure(instance: &Instance, callable: NonNull<Header>) -> Vec<NonNull<Header>> {
    // SAFETY: 调用方保证 callable 是存活对象；类型身份在调用路径已确认。
    let object = unsafe { &*callable.as_ptr().cast::<FunctionObject>() };
    let _ = instance;
    object.closure()
}

fn function_defaults(function: NonNull<Header>) -> (NonNull<Header>, Vec<NonNull<Header>>, Option<NonNull<Header>>) {
    // SAFETY: function 是帧值栈上的存活对象，且调用方已确认它是 function。
    let object = unsafe { &*function.as_ptr().cast::<FunctionObject>() };
    (object.code(), object.defaults().to_vec(), object.kwdefaults())
}

/// **`BC-56`**：把实参绑进局部槽；返回长度 ＝ `co_nlocals` 的槽数组（**新引用**）。
///
/// 顺序与报错类别都按 `BC-56`：仅位置 → 位置或关键字 → `*args` → 仅关键字 → `**kwargs`；
/// 四类错误各成一个 [`ExecError`]（参照实现的**消息**已实测记录在案，等异常对象接线后再原样产出）。
#[allow(clippy::too_many_arguments)]
fn bind_arguments(
    instance: &Instance,
    code: &CodeObject,
    args: Vec<NonNull<Header>>,
    kwargs: Vec<(NonNull<Header>, NonNull<Header>)>,
    defaults: &[NonNull<Header>],
    kwdefaults: Option<NonNull<Header>>,
    opcode: u8,
) -> Result<Vec<Option<NonNull<Header>>>, ExecError> {
    let mut locals: Vec<Option<NonNull<Header>>> = vec![None; code.nlocals()];
    let argcount = code.argcount();
    let kwonly = code.kwonlyargcount();
    let mut args = args.into_iter();

    // ① 位置实参填进前 `argcount` 个槽
    let mut given = 0usize;
    for slot in 0..argcount {
        match args.next() {
            Some(value) => {
                locals[slot] = Some(value);
                given += 1;
            }
            None => break,
        }
    }

    // ② 多出来的位置实参：收进 `*args`，否则报错
    let extra: Vec<NonNull<Header>> = args.collect();
    // **`*args` 必须无条件绑定** ✓（第 277 轮真 bug ✗）：先前整块写在 `if !extra.is_empty()` **里面** ✗
    // ⇒ **没有多余位置实参时那一格从不绑** ✗ ⇒ 函数体里一读就是"未绑定局部" ✓（CPython 会绑空元组 ✓）。
    // 实测：`def f(a, *p): return len(p)` 调 `f(1)` ⇒ 参照 `0`、本层报未绑定 ✗；
    // `Lib/posixpath.py` 的 `join(a, *p)` 与 `import site` 都撞在它上面 ✓。
    if code.has_varargs() {
        let varargs_slot = argcount + kwonly;
        // **OM-23**：没有多余实参时这个元组是空的 ⇒ 走单例
        locals[varargs_slot] = Some(instance.new_tuple(extra));
    } else if !extra.is_empty() {
        let release_all = |values: Vec<NonNull<Header>>| {
            for value in values {
                release(instance, value);
            }
        };
        release_all(extra);
        for slot in locals.iter_mut().filter_map(Option::take) {
            release(instance, slot);
        }
        let message = message_too_many(
            code.name(),
            argcount,
            argcount - defaults.len(),
            given + 1,
        );
        return Err(raise_builtin(instance, "TypeError", &message));
    }

    // ③ 关键字实参
    let mut collected: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
    for (name, value) in kwargs {
        let text = match str_text(instance, name, opcode) {
            Ok(text) => text,
            Err(error) => {
                release(instance, name);
                release(instance, value);
                for slot in locals.iter_mut().filter_map(Option::take) {
                    release(instance, slot);
                }
                return Err(error);
            }
        };

        let positional_hit = (0..argcount).find(|slot| code.varname(*slot) == Some(text.as_str()));
        let keyword_hit = (0..kwonly)
            .find(|offset| code.varname(argcount + offset) == Some(text.as_str()))
            .map(|offset| argcount + offset);

        let outcome = if let Some(slot) = positional_hit {
            if slot < code.posonlyargcount() {
                // 仅位置形参不能用关键字传
                release(instance, value);
                release(instance, name);
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &message_positional_only(code.name(), std::slice::from_ref(&text)),
                ))
            } else if locals[slot].is_some() {
                release(instance, value);
                release(instance, name);
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &message_duplicate(code.name(), &text),
                ))
            } else {
                locals[slot] = Some(value);
                release(instance, name);
                Ok(())
            }
        } else if let Some(slot) = keyword_hit {
            if locals[slot].is_some() {
                release(instance, value);
                release(instance, name);
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &message_duplicate(code.name(), &text),
                ))
            } else {
                locals[slot] = Some(value);
                release(instance, name);
                Ok(())
            }
        } else if code.has_varkeywords() {
            collected.push((name, value));
            Ok(())
        } else {
            release(instance, value);
            release(instance, name);
            Err(raise_builtin(
                instance,
                "TypeError",
                &message_unexpected_keyword(code.name(), &text),
            ))
        };

        if let Err(error) = outcome {
            for (key, item) in collected {
                release(instance, key);
                release(instance, item);
            }
            for slot in locals.iter_mut().filter_map(Option::take) {
                release(instance, slot);
            }
            return Err(error);
        }
    }

    // ④ 位置形参的默认值（对齐到**尾部**若干位置参数）；缺的一并报出来（参照实现如此）
    let mut missing_positional: Vec<String> = Vec::new();
    for slot in 0..argcount {
        if locals[slot].is_some() {
            continue;
        }
        let from_end = argcount - slot;
        if from_end <= defaults.len() {
            let value = defaults[defaults.len() - from_end];
            // SAFETY: 默认值由函数对象持有，存活。
            unsafe { instance.incref_object(value.as_ptr()) };
            locals[slot] = Some(value);
        } else {
            missing_positional.push(code.varname(slot).unwrap_or("<unknown>").to_owned());
        }
    }
    if !missing_positional.is_empty() {
        for (key, item) in collected {
            release(instance, key);
            release(instance, item);
        }
        for slot in locals.iter_mut().filter_map(Option::take) {
            release(instance, slot);
        }
        let message = message_missing(code.name(), &missing_positional, false);
        return Err(raise_builtin(instance, "TypeError", &message));
    }

    // ⑤ 仅关键字形参：先看默认值，缺了一并报出来
    let mut missing_keyword_only: Vec<String> = Vec::new();
    for offset in 0..kwonly {
        let slot = argcount + offset;
        if locals[slot].is_some() {
            continue;
        }
        let name = code.varname(slot).unwrap_or("<unknown>").to_owned();
        let mut found = None;
        if let Some(mapping) = kwdefaults {
            // SAFETY: mapping 由函数对象持有。
            let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
            let name_object = instance.alloc(StrObject::new(instance.singletons().str_type(), name.clone()));
            let name_raw = name_object.as_ptr().cast::<Header>();
            let position = dict
                .entries()
                .iter()
                .position(|(existing, _)| values_equal(instance, *existing, name_raw));
            if let Some(position) = position {
                found = dict.entry(position).map(|(_, value)| value);
            }
        }
        match found {
            Some(value) => {
                // SAFETY: 默认值由 kwdefaults 持有。
                unsafe { instance.incref_object(value.as_ptr()) };
                locals[slot] = Some(value);
            }
            None => missing_keyword_only.push(name),
        }
    }
    if !missing_keyword_only.is_empty() {
        for (key, item) in collected {
            release(instance, key);
            release(instance, item);
        }
        for slot in locals.iter_mut().filter_map(Option::take) {
            release(instance, slot);
        }
        let message = message_missing(code.name(), &missing_keyword_only, true);
        return Err(raise_builtin(instance, "TypeError", &message));
    }

    // ⑥ `**kwargs`
    if code.has_varkeywords() {
        let dict = instance.alloc(DictObject::new(
            builtin_type(instance, "dict"),
            RefCell::new(collected),
        ));
        locals[argcount + kwonly + usize::from(code.has_varargs())] =
            Some(dict.into_raw().cast::<Header>());
    } else if !collected.is_empty() {
        for (key, item) in collected {
            release(instance, key);
            release(instance, item);
        }
    }

    Ok(locals)
}

/// 调用一个可调用对象（本片只有函数对象）。
/// 调用一个可调用对象（**新引用**返回值）。
///
/// **`bound_self` 的所有权契约**：它是**借用**——调用方持有那份引用，本函数不释放它。
/// 需要长期持有（进实参表、进生成的实例）的路径各自 `incref`。
pub(crate) fn call_callable(
    instance: &Instance,
    callable: NonNull<Header>,
    bound_self: Option<NonNull<Header>>,
    args: Vec<NonNull<Header>>,
    kwargs: Vec<(NonNull<Header>, NonNull<Header>)>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: callable 是帧值栈上的存活对象。
    let ty = unsafe { callable.as_ref() }.ty();
    // 本层接线的可调用：函数、**类型对象**（`OM-11` 的 `new` 槽）与**绑定方法**。
    // 内建可调用对象（`builtin_function_or_method`）随后补。
    // `OM-11` 的 `call` 槽：类型自带调用语义（宿主函数一类走这条）
    // SAFETY: callable 是存活对象。
    // **类调用（`C(...)`）与元类型自身调用（`type(x)`）要分开** ✓（第 183 轮实证 ✓）：
    // `C` 的**类型**是元类型 ✓ ⇒ 若照抄元类型的 call 槽 ✗，`C(...)` 会被**劫走** ✗
    //（实测：13 条语料当场红 ✗，`TypeError: cannot create 'type' instances` ✓）。
    // 所以：**只有元类型自己**（self-typed ✓）被调用时才走 call 槽 ✓；其余**一切类**一律走**实例化** ✓。
    let callable_is_metatype = instance.is_type_object(callable)
        && unsafe { callable.as_ref() }.ty().as_ptr() == callable.as_ptr().cast::<TypeObject>();
    let call_slot = if instance.is_type_object(callable) && !callable_is_metatype {
        None
    } else {
        unsafe { ty.as_ref() }.slots().call
    };
    if let Some(slot) = call_slot {
        // SAFETY: 槽位契约见 `CallFn`（借用视图 ＋ 新引用返回值）。
        let result = unsafe { slot(callable.as_ptr(), bound_self, &args, &kwargs, instance) };
        for argument in args {
            release(instance, argument);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        return result;
    }

    let callable_type_ok = ty == builtin_type(instance, "function")
        || instance.is_type_object(callable)
        || ty == builtin_type(instance, "method")
        || ty == builtin_type(instance, "builtin_function_or_method");
    if !callable_type_ok {
        for value in args {
            release(instance, value);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        // 实测：不可调用的对象被调用 ⇒ `TypeError: '<类型名>' object is not callable`
        // （此前报的是 VM 级的 `Unsupported`，属"没有实测口径就当没实现"；现在照参照报）
        // SAFETY: callable 是存活对象。
        let name = instance.type_name(unsafe { callable.as_ref() }.ty());
        return Err(raise_builtin(
            instance,
            "TypeError",
            &format!("'{name}' object is not callable"),
        ));
    }

    // **类型对象被调用**（`list()`／`ValueError("x")`）：走类型自己的 `new` 槽（`OM-11`／`OM-14`），
    // 然后按 `OM-14` 找 `__init__`（Python 子类的覆写就落在那里）。
    // SAFETY: callable 是存活对象。
    // **`is_type_object` 才是对的判据** ✓（第 269 轮真 bug ✗）：先前要求"元类型**恰好是** `type`" ✗
    // ⇒ 元类型是 **Python 类**（`ABCMeta` 一族 ✓）的类被当成**不可调用** ✗（实测 `Lib/os.py` 的
    // `_Environ(...)` ⇒ `TypeError: 'ABCMeta' object is not callable` ✗）。一切**类对象**都该走实例化 ✓。
    if instance.is_type_object(callable) {
        // **`CALL` 的 `self` 槽：对"类调用"是第一个位置实参** ✓（第 279 轮真 bug 修 ✗）。
        // 参照的**装饰器**写法 `@property\ndef g(self): …` 产的是 `LOAD_NAME property; <函数>; CALL 0`
        // ✓ —— 那里**没有** `PUSH_NULL` ✓，函数落在 `self` 槽上 ✓，参照按 `property(g)` 解析 ✓
        //（实测 `dis` ✓）。先前这一支**丢掉** `bound_self` ✗ ⇒ `property(fget)` 的 `fget` 永远是 `None` ✗
        // ⇒ `@property` 描述的属性统统坏掉 ✗（与 `CHANGELOG` 里 `D.__new__(cls, a)` 报"缺 1 个实参"同源 ✓）。
        let mut args = args;
        if let Some(self_object) = bound_self {
            // 契约：`bound_self` 是**借用**（调用方持有）⇒ 为实参表新增一份 ✓
            // SAFETY: self_object 由调用方保证存活。
            unsafe { instance.incref_object(self_object.as_ptr()) };
            args.insert(0, self_object);
        }
        let class = callable.cast::<TypeObject>();
        // SAFETY: class 由注册表持有。
        let new_slot = unsafe { class.as_ref() }.slots().new;
        // SAFETY: 类型名由注册表持有，存活。
        let class_name = unsafe { class.as_ref() }.name().to_owned();
        let Some(new_slot) = new_slot else {
            let message = format!("cannot create '{class_name}' instances");
            for argument in args {
                release(instance, argument);
            }
            for (key, value) in kwargs {
                release(instance, key);
                release(instance, value);
            }
            return Err(raise_builtin(instance, "TypeError", &message));
        };
        // SAFETY: 槽位由类型提供，契约见 `NewFn`。
        let created = match unsafe { new_slot(class, &args, instance) } {
            Ok(created) => created,
            // `OM-11` 扩（裁决）：失败由**槽位**给原因，这里**直接透传**（不再由调用点猜
            // "cannot create '<类名>' instances" 那句话）
            Err(error) => {
                for argument in args {
                    release(instance, argument);
                }
                for (key, value) in kwargs {
                    release(instance, key);
                    release(instance, value);
                }
                return Err(error);
            }
        };
        // **`__new__` 分派**（`OM-14` 的"子类分派槽位"里 Python 侧那一半）。
        //
        // 实测口径：
        // - `__new__` 只可能在**类字典**里（`object` 不带默认 `__new__`，故查到的一定是覆写）
        // - 它拿到 `(cls, *args, **kwargs)`；返回值**不是**本类实例时 `__init__` **不**被调用
        //   （实测：`__new__` 返回 `42` 时 `B()` 就是 `42`）
        // - 返回值是本类（或子类）实例时照常调 `__init__`，且 `__init__` **仍拿到原实参**
        //
        // 所有权：`call_callable` 是**转移**语义（它消耗实参表），所以给 `__new__` 的那一份
        // 要自己新增；原引用留给 `__init__`，没走到 `__init__` 就归还。
        // **`type.__new__` 不算** ✗（第 190 轮真 bug 修复 ✓）：上面那行注释写的口径是
        // "`__new__` 只可能在**类字典**里" ✓，但自从 `type` 的命名空间里挂上 `__new__`（第 179 轮 ✓）
        // 之后，这里的查找会**翻到"元类型那一层"** ✗ ⇒ 于是**任何** `C()` 都变成
        // `type.__new__(C)` ✓（**1 个实参** ✗）⇒ 而 `type.__new__` 要 ≥3 个 ⇒ 报
        // "实际 0 个" 一类的怪错 ✓（实测：`import os` 就撞它 ✓）。
        // ⇒ 与第 179 轮元类那条同款处理 ✓：**是我们挂的那个就跳过** ✓，走默认实例化 ✓。
        // **`type.__new__` 与 `object.__new__` 都不算** ✗（第 193 轮补上后一半 ✓）：参照的实例化走的是
        // **类型自己的 `new` 槽** ✓（`list.__new__` 是**它自己**的 ✓，不是 `object.__new__` ✓）⇒
        // 这两个"我们自己挂的"默认实现只应作为**属性**存在 ✓（`@object.__new__` 那类用法 ✓），
        // **不参与**实例化分派 ✓；否则内建类型一被构造就撞"多给了实参" ✗（实测十多条语料当场变红 ✓）。
        let ours_new = instance
            .type_named("type")
            .and_then(|ty| instance.type_lookup(ty, "__new__"));
        let object_new = instance
            .type_named("object")
            .and_then(|ty| instance.type_lookup(ty, "__new__"));
        if let Some(constructor) = instance
            .type_lookup(class, "__new__")
            .filter(|found| Some(*found) != ours_new && Some(*found) != object_new)
        {
            let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len() + 1);
            // SAFETY: constructor 由类型字典持有；class 在注册表里；实参由调用方保证存活。
            unsafe {
                instance.incref_object(constructor.as_ptr());
                instance.incref_object(class.cast::<Header>().as_ptr());
            }
            call_args.push(class.cast::<Header>());
            for argument in args.iter().copied() {
                // SAFETY: 同上。
                unsafe { instance.incref_object(argument.as_ptr()) };
                call_args.push(argument);
            }
            let mut constructor_kwargs: Vec<(NonNull<Header>, NonNull<Header>)> =
                Vec::with_capacity(kwargs.len());
            for (key, value) in kwargs.iter().copied() {
                // SAFETY: 同上。
                unsafe {
                    instance.incref_object(key.as_ptr());
                    instance.incref_object(value.as_ptr());
                }
                constructor_kwargs.push((key, value));
            }
            let created =
                call_callable(instance, constructor, None, call_args, constructor_kwargs, opcode)?;
            // SAFETY: created 是新引用，存活。
            let is_instance = instance.is_subtype(unsafe { created.as_ref() }.ty(), class);
            let initializer = if is_instance {
                instance.type_lookup(class, "__init__")
            } else {
                None
            };
            match initializer {
                Some(initializer) => {
                    let mut init_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len() + 1);
                    // SAFETY: initializer 由类型字典持有；created 是新引用；实参仍归本函数。
                    unsafe {
                        instance.incref_object(initializer.as_ptr());
                        instance.incref_object(created.as_ptr());
                    }
                    init_args.push(created);
                    // 原实参与关键字实参转交给 `__init__`
                    init_args.extend(args.iter().copied());
                    let result = call_callable(instance, initializer, None, init_args, kwargs, opcode)?;
                    release(instance, result);
                }
                None => {
                    // 没有 `__init__`：把调用方那份实参归还
                    for argument in args.iter().copied() {
                        release(instance, argument);
                    }
                    for (key, value) in kwargs.iter().copied() {
                        release(instance, key);
                        release(instance, value);
                    }
                }
            }
            return Ok(created);
        }

        // `__init__`（`OM-14`：子类覆写要生效）。找到就"实例在先、实参在后"地调它。
        if let Some(initializer) = instance.type_lookup(class, "__init__") {
            let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(args.len() + 1);
            // SAFETY: initializer 由类型字典持有，存活；这里新增一份引用交给调用。
            unsafe { instance.incref_object(initializer.as_ptr()) };
            // SAFETY: created 是刚拿到的新引用；调用方那份由 `call_callable` 的返回交出，
            // 这里额外加一份给实参表。
            unsafe { instance.incref_object(created.as_ptr()) };
            call_args.push(created);
            call_args.extend(args.iter().copied());
            let result = call_callable(
                instance,
                initializer,
                None,
                call_args,
                kwargs,
                opcode,
            )?;
            // `__init__` 必须返回 None（`T`／参照实现如此）；返回值丢掉那份引用
            release(instance, result);
            // initializer 的那份引用由 `call_callable` 接手（它内部会按需释放）
        } else {
            // 没有 `__init__`，且走的是**通用分配**（`attribute_new`）时，带实参创建就是错的
            // （实测 `Empty(1)` ⇒ `Empty() takes no arguments`）。
            //
            // 判据必须限定在通用分配上：内建类型（`ValueError('x')` 一类）的 `new` 槽是自己的
            // 实现、本来就能吃实参，不该被这条规则误伤。用**类型标志**而不是比较函数指针
            // （`rustc` 明说函数地址不保证唯一）。
            // SAFETY: class 由注册表持有。
            let generic_allocation = unsafe { class.as_ref() }.has_generic_allocation();
            if generic_allocation && (!args.is_empty() || !kwargs.is_empty()) {
                for argument in args {
                    release(instance, argument);
                }
                for (key, value) in kwargs {
                    release(instance, key);
                    release(instance, value);
                }
                release(instance, created);
                let message = format!("{class_name}() takes no arguments");
                return Err(raise_builtin(instance, "TypeError", &message));
            }
            for argument in args {
                release(instance, argument);
            }
            for (key, value) in kwargs {
                release(instance, key);
                release(instance, value);
            }
        }
        return Ok(created);
    }

    // **先剥绑定方法**：剥出来的可能是函数（下面按"绑定位置参数"调），也可能是**原生**
    // （走原生分支、self 当 `bound` 递进去）。顺序很要紧——原生的
    // `BuiltinFunctionObject` 与 `FunctionObject` 布局不同，先当函数读会读到错位的内存
    // （症状是"misaligned pointer dereference"）。
    // SAFETY: callable 是存活对象。
    let callable_type = unsafe { callable.as_ref() }.ty();
    let (callable, bound_self) = if callable_type == builtin_type(instance, "method") {
        // SAFETY: 类型身份已确认。
        let method = unsafe { &*callable.as_ptr().cast::<MethodObject>() };
        // 函数与实例都由该方法对象持有、存活；`bound_self` 是**借用**（见本函数开头的契约）
        (method.function(), Some(method.this()))
    } else {
        (callable, bound_self)
    };

    // **原生可调用对象**（`AB-24`：宿主函数与内建函数的落点）：实参以**借用视图**递进去，
    // 返回值是**新引用**。绑定方法形态在这里剥掉绑定并当第一个位置实参。
    // SAFETY: callable 是存活对象。
    if unsafe { callable.as_ref() }.ty() == builtin_type(instance, "builtin_function_or_method") {
        // SAFETY: 类型身份已确认。
        let native = unsafe { &*callable.as_ptr().cast::<BuiltinFunctionObject>() };
        let function = native.function();
        let bound = match bound_self {
            Some(self_object) => Some(self_object),
            None => None,
        };
        // SAFETY: 签名契约见 `NativeFn`（借用视图 ＋ 新引用返回值）。
        let result = unsafe { function(instance, bound, &args, &kwargs) };
        // 借用视图：实参的引用仍归本函数，调用完要按约归还
        for argument in args {
            release(instance, argument);
        }
        for (key, value) in kwargs {
            release(instance, key);
            release(instance, value);
        }
        // `bound` 是**借用**：不在这里释放（调用方持有；契约见本函数开头）
        return result;
    }

    let (code_header, defaults, kwdefaults) = function_defaults(callable);
    // **`__globals__`**：函数帧的全局映射取自函数自己（`BC-57`）；`MAKE_FUNCTION` 时捕获。
    let function_globals = {
        // SAFETY: callable 是存活对象。
        let is_function = unsafe { callable.as_ref() }.ty() == builtin_type(instance, "function");
        if is_function {
            // SAFETY: 类型身份已确认。
            unsafe { &*callable.as_ptr().cast::<FunctionObject>() }.globals()
        } else {
            None
        }
    };
    // SAFETY: 函数持有一份对 code object 的引用，故它在函数存活期间有效。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    let mut args = args;
    if let Some(self_object) = bound_self {
        // 契约：`bound_self` 是**借用**（调用方持有那份引用）；实参表由 `bind_arguments` 接手，
        // 故这里先为它新增一份。
        // SAFETY: self_object 由调用方保证存活。
        unsafe { instance.incref_object(self_object.as_ptr()) };
        args.insert(0, self_object);
    }

    let locals = bind_arguments(instance, code, args, kwargs, &defaults, kwdefaults, opcode)?;

    // **帧类型是"内部"类型** ✓（不进探测表 ✓）⇒ 这里必须用 `type_named` ✗（用 `builtin_type` 会 panic ✓）。
    let frame_type = instance
        .type_named("frame")
        .expect("引导期已登记 frame 类型（内部类型 ✓）");
    let frame = instance.alloc(Frame::for_code(frame_type, &own_code(instance, code_header)));
    if let Some(mapping) = function_globals {
        // 帧接手的是**新引用**（`Frame::clear` 会释放它）
        // SAFETY: 映射由函数持有，存活。
        unsafe { instance.incref_object(mapping.as_ptr()) };
        frame.get().set_globals(mapping);
    }
    for (slot, value) in locals.into_iter().enumerate() {
        if let Some(value) = value {
            let _ = frame.get().set_local(slot, Some(value))?;
        }
    }
    // **建帧装闭包**（`CPython` 3.11+ 的时机）：第 i 个自由槽 ← 闭包元组第 i 项（cell 对象）
    for cell in function_closure(instance, callable) {
        // SAFETY: cell 由函数的闭包持有，存活；帧要自己那份引用。
        unsafe { instance.incref_object(cell.as_ptr()) };
        let _ = frame.get().install_closure(&[cell]);
        // SAFETY: 上面那份新增引用已交给帧（`install_closure` 接手）。
    }

    // **生成器／协程函数**（`CO_GENERATOR` ＝ 32、`CO_COROUTINE` ＝ 128，都实测过）：
    // `CALL` **不**跑函数体，而是把挂起的帧包成对应的对象交出去
    // （实测骨架：函数体第一条是 `RETURN_GENERATOR`，恢复时才从 `POP_TOP` 继续）。
    // 两者的载荷同形（一个挂起的帧 ＋ 标志），只是类型不同：`repr` 的词、以及协程**不是迭代器**。
    let wrapped_type = if code.flags() & 0x20 != 0 {
        Some("generator")
    } else if code.flags() & 0x80 != 0 {
        Some("coroutine")
    } else if code.flags() & 0x200 != 0 {
        // `CO_ASYNC_GENERATOR`（实测 0x200，`async def` ＋ `yield`）
        Some("async_generator")
    } else {
        None
    };
    if let Some(type_name) = wrapped_type {
        frame.get().suspend()?;
        // 生成器要**自己持有一份帧的引用**（`GeneratorObject` 的 traverse／clear 会释放它）——
        // 漏了这一份，`call_callable` 一返回帧就被释放，生成器拿到的是悬垂指针。
        // SAFETY: frame 由本函数持有，这里新增一份引用交给生成器。
        unsafe { instance.incref_object(frame.as_ptr().cast::<Header>().as_ptr()) };
        let generator = instance.alloc(GeneratorObject::new(
            builtin_type(instance, type_name),
            frame.as_ptr().cast::<Header>(),
            Cell::new(false),
            Cell::new(false),
        ));
        return Ok(generator.into_raw().cast::<Header>());
    }

    match execute(instance, &frame)? {
        ExecOutcome::Returned(value) => Ok(value_into_raw(instance, value)),
        ExecOutcome::Yielded(_) => Err(ExecError::Unsupported {
            opcode,
            what: "非生成器函数不该让出（码元被改坏了？）",
        }),
    }
}

/// 为 code object 现取一个 [`Owned`] 守卫（**新增一份引用**）。
fn own_code<'a>(instance: &'a Instance, header: NonNull<Header>) -> Owned<'a, CodeObject> {
    // SAFETY: header 指向本实例的存活 code object；这里新增一份引用交给守卫。
    unsafe { instance.incref_object(header.as_ptr()) };
    Owned::new(header.cast::<CodeObject>(), instance)
}

/// 把 [`Value`] 变成帧值栈要的**新引用**（内联的那几种换算成它们对应的单例）。
fn value_into_raw(instance: &Instance, value: Value<'_>) -> NonNull<Header> {
    let (raw, needs_reference) = match value {
        Value::None => (instance.singletons().none(), true),
        Value::Bool(flag) => (instance.singletons().boolean(flag), true),
        Value::Int(number) => (
            instance
                .singletons()
                .small_int(number)
                .expect("内联整数一定落在单例区间（OM-39）"),
            true,
        ),
        Value::Object(reference) => (reference.into_raw(), false),
    };
    if needs_reference {
        // SAFETY: 单例由实例持有，存活。
        unsafe { instance.incref_object(raw.as_ptr()) };
    }
    raw
}

/// `BC-56` 与调用（`CALL`／`CALL_KW`）。

/// **异常派发**（`BC-60` ①）：按异常表找到处理块，回退值栈到 `depth`、按 `lasti` 压偏移、
/// 压异常实例、跳到入口；没有处理块就把它继续往外抛。
///
/// 两条路径共用它：指令自己报错（`Err(Raised)`）与**恢复时要先抛**（生成器的 `throw`／`close`）。
fn dispatch_raise(
    instance: &Instance,
    frame: &Frame,
    exceptiontable: &[u8],
    offset_bytes: usize,
    exception: NonNull<Header>,
    decoder: &mut Decoder,
) -> Result<(), ExecError> {
    let table = parse_exception_table(exceptiontable).map_err(ExecError::Decode)?;
    let handler = table
        .iter()
        .find(|entry| entry.start <= offset_bytes && offset_bytes < entry.end);
    let Some(entry) = handler else {
        return Err(ExecError::Raised { exception });
    };
    while frame.depth() > entry.depth {
        release(instance, frame.pop()?);
    }
    // **先把值栈弹到异常表记的深度** ✓（第 164 轮的真 bug ✗）：先前漏了这一步 ⇒ 处理块带着多余的
    //   栈项开跑 ✓ ⇒ 症状有两种：「弹出个 `int`」（陈旧的栈项被当成异常 ✓）与 `StackUnderflow` ✗。
    for value in frame.truncate_stack(entry.depth as usize) {
        release(instance, value);
    }
    if entry.lasti {
        push_int(instance, frame, (offset_bytes / 2) as i64)?;
    }
    push(instance, frame, exception)?;
    decoder.set_position(entry.target / 2);
    Ok(())
}

/// **恢复一个生成器**：把 `sent` 送进挂起的帧，跑到下一次让出或跑完。
///
/// `SEND` 与生成器方法 `send`／`__next__` 共用这一段——栈效应与"跑完"的记账只有一处真相。
/// 返回的两个值都是**新引用**（调用方接手）。
pub(crate) enum GeneratorOutcome {
    /// 又让出了一次（值是**新引用**）。
    Yielded(NonNull<Header>),
    /// 跑完了（返回值是**新引用**；生成器已置"跑完"）。
    Returned(NonNull<Header>),
}

pub(crate) fn resume_generator(
    instance: &Instance,
    generator: NonNull<Header>,
    sent: Option<NonNull<Header>>,
) -> Result<GeneratorOutcome, ExecError> {
    // SAFETY: 调用方保证 generator 是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    let frame_header = object.frame();
    let generator_frame = Owned::new(
        // SAFETY: frame_header 由生成器持有，这里新增一份引用交给守卫。
        {
            unsafe { instance.incref_object(frame_header.as_ptr()) };
            frame_header.cast::<Frame>()
        },
        instance,
    );
    if generator_frame.get().is_suspended() {
        generator_frame.get().resume()?;
    }
    // "送进去的值"要落在恢复后的值栈顶（`yield` 表达式的值）；`push` 会新增一份引用，
    // 所以送出的那份随后要还（调用方给的是借用的视图或已有引用）。
    match sent {
        Some(value) => {
            push(instance, generator_frame.get(), value)?;
        }
        None => {
            let none = instance.singletons().none();
            push(instance, generator_frame.get(), none)?;
        }
    }
    match execute(instance, &generator_frame) {
        Ok(ExecOutcome::Yielded(value)) => Ok(GeneratorOutcome::Yielded(value)),
        Ok(ExecOutcome::Returned(value)) => {
            object.mark_finished();
            let raw = value_into_raw(instance, value);
            // **异步生成器**跑完不是"返回值"，而是 `StopAsyncIteration`（`async for` 靠它收尾）
            if instance.type_name(unsafe { generator.as_ref() }.ty()) == "async_generator" {
                release(instance, raw);
                return Err(async_generator_exhausted(instance));
            }
            Ok(GeneratorOutcome::Returned(raw))
        }
        Err(error) => {
            // 让出点之后出错 ⇒ 生成器就此作废（参照实现同：之后再取就是耗尽）
            object.mark_finished();
            // **协程**里逃出来的 `StopIteration` 要变成 `RuntimeError`。实测两句话都在，
            // 词由**被驱动的对象种类**决定：协程 ⇒ `coroutine raised StopIteration`，
            // `CO_ITERABLE_COROUTINE`（0x100）生成器 ⇒ `generator raised StopIteration`。
            // 转换放在这里而不是只靠 `INTRINSIC_STOPITERATION_ERROR`，是因为**这里知道种类**
            // （那条 intrinsic 在栈上只看到异常对象）。普通生成器的 `yield from` 不走这条：
            // 那里 `StopIteration` 是**返回值**机制。
            if let ExecError::Raised { exception } = &error {
                // SAFETY: exception 是存活对象。
                let ty = unsafe { exception.as_ref() }.ty();
                let stop_iteration = exception_type(instance, "StopIteration");
                // SAFETY: generator 是本实例里存活的对象。
                let is_coroutine =
                    unsafe { generator.as_ref() }.ty() == builtin_type(instance, "coroutine");
                let code_flags = {
                    let frame_header = object.frame();
                    // SAFETY: 帧由生成器持有，存活。
                    let generator_frame = unsafe { &*frame_header.as_ptr().cast::<Frame>() };
                    match generator_frame.code() {
                        // SAFETY: code 由帧持有，存活。
                        Some(code) => unsafe { code.cast::<CodeObject>().as_ref() }.flags(),
                        None => 0,
                    }
                };
                let iterable_coroutine = code_flags & 0x100 != 0;
                if instance.is_subtype(ty, stop_iteration) && (is_coroutine || iterable_coroutine) {
                    let word = if is_coroutine { "coroutine" } else { "generator" };
                    let message = format!("{word} raised StopIteration");
                    // SAFETY: 错误里那份引用在此消费。
                    unsafe { instance.release_object(exception.as_ptr()) };
                    return Err(raise_builtin(instance, "RuntimeError", &message));
                }
            }
            Err(error)
        }
    }
}

/// 异步生成器耗尽时抛的东西（`async for` 的结束信号）。
pub(crate) fn async_generator_exhausted(instance: &Instance) -> ExecError {
    let exception = crate::builtin_objects::exception_instance(
        instance,
        "StopAsyncIteration",
        Vec::new(),
    );
    raise(instance, exception)
}

/// **恢复生成器并立刻抛一个异常**（`throw`／`close`）：异常放进帧的"待抛"格，
/// 由 `execute` 按**本帧的**异常表派发（生成器体里的 `try/except` 因此能接住）。
pub(crate) fn resume_generator_with_raise(
    instance: &Instance,
    generator: NonNull<Header>,
    exception: NonNull<Header>,
) -> Result<GeneratorOutcome, ExecError> {
    // SAFETY: 调用方保证 generator 是本实例里存活的生成器。
    let object = unsafe { &*generator.as_ptr().cast::<GeneratorObject>() };
    let frame_header = object.frame();
    let generator_frame = Owned::new(
        // SAFETY: frame_header 由生成器持有，这里新增一份引用交给守卫。
        {
            unsafe { instance.incref_object(frame_header.as_ptr()) };
            frame_header.cast::<Frame>()
        },
        instance,
    );
    if generator_frame.get().is_suspended() {
        generator_frame.get().resume()?;
    }
    // 帧接手的是**新引用**（`Frame::clear` 会释放它）
    // SAFETY: exception 由调用方保证存活。
    unsafe { instance.incref_object(exception.as_ptr()) };
    if let Some(previous) = generator_frame.get().set_pending_raise(Some(exception)) {
        // SAFETY: 被顶下来的那份由帧交出。
        unsafe { instance.release_object(previous.as_ptr()) };
    }
    match execute(instance, &generator_frame) {
        Ok(ExecOutcome::Yielded(value)) => Ok(GeneratorOutcome::Yielded(value)),
        Ok(ExecOutcome::Returned(value)) => {
            object.mark_finished();
            Ok(GeneratorOutcome::Returned(value_into_raw(instance, value)))
        }
        Err(error) => {
            object.mark_finished();
            Err(error)
        }
    }
}

/// 跑一段 code object，直到 `RETURN_VALUE`。
///
/// **BC-42**：指令指针沿途写回帧（码元单位），因此挂起／恢复有据可依。
/// **当前全局映射的 RAII 守卫**（第 156 轮）：`Drop` 时恢复上一格 ✓ ⇒ `execute` 里**任何**提前返回
/// （含 `?`）都安全 ✓（生成器挂起返回时也算「本帧不活跃」✓，恢复正是对的 ✓）。
/// **当前帧的 RAII 守卫** ✓（第 230 轮）：与全局映射那只**同款** ✓ —— 进帧时公布、出帧时还原 ✓。
struct CurrentFrameGuard<'a> {
    instance: &'a Instance,
    previous: Option<NonNull<Header>>,
}

impl<'a> CurrentFrameGuard<'a> {
    fn install(instance: &'a Instance, frame: Option<NonNull<Header>>) -> Self {
        let previous = instance.set_current_frame(frame);
        Self { instance, previous }
    }
}

impl Drop for CurrentFrameGuard<'_> {
    fn drop(&mut self) {
        self.instance.set_current_frame(self.previous);
    }
}

struct CurrentGlobalsGuard<'a> {
    instance: &'a Instance,
    previous: Option<NonNull<Header>>,
}

impl<'a> CurrentGlobalsGuard<'a> {
    fn install(instance: &'a Instance, globals: Option<NonNull<Header>>) -> Self {
        let previous = instance.set_current_globals(globals);
        Self { instance, previous }
    }
}

impl Drop for CurrentGlobalsGuard<'_> {
    fn drop(&mut self) {
        self.instance.set_current_globals(self.previous);
    }
}

pub fn execute<'a>(
    instance: &'a Instance,
    frame: &Owned<'a, Frame>,
) -> Result<ExecOutcome<'a>, ExecError> {
    let code_header = frame.get().code().expect("BC-42：帧必须持有 code object");
    // SAFETY: 帧持有一份对 code object 的引用（BC-42），因此它在帧存活期间有效；
    // 帧由本函数的调用方持有。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

    // **把本帧的全局映射挂到实例上**（第 156 轮）：内建 `globals()` 取它 ✓；守卫 `Drop` 时恢复 ✓。
    // **模块帧的 `globals` 那格本来就是 `None`** ✗（`BC-57`：模块体没有单独的一层，此时就是它的
    // **命名空间** ✓）⇒ 取 `globals`，没有就用 `namespace` ✓ —— 函数帧两格都有 ✓。
    let _globals_guard = CurrentGlobalsGuard::install(
        instance,
        frame.get().globals().or_else(|| frame.get().namespace()),
    );
    // **公布当前帧** ✓（第 230 轮）：`sys._getframe()` 与 `frame.f_locals` 都取它 ✓。
    // SAFETY: 帧由调用方持有，本函数运行期间存活 ✓。
    let frame_header: NonNull<Header> = frame.as_ptr().cast::<Header>();
    let _frame_guard = CurrentFrameGuard::install(instance, Some(frame_header));

    // **BC-47**：挂起的帧（生成器／await）从**恢复点**接着跑——值栈与 ip 都在恢复点里。
    // 新帧的 ip 是 0，所以"一律按帧的 ip 起步"这一条对两种情况都成立。
    if frame.get().is_suspended() {
        frame.get().resume()?;
    }
    // **`throw`／`close`**：恢复点上有"待抛异常"就先按本帧的异常表派发它
    // （所以生成器体里的 `try/except` 能接住，与参照实现"抛在挂起点"一致）。
    let mut forced_raise = frame.get().take_pending_raise();
    let mut decoder = Decoder::new(code.code());
    decoder.set_position(frame.get().instruction_pointer());
    while let Some(instruction) = decoder.next_instruction()? {
        if let Some(exception) = forced_raise.take() {
            dispatch_raise(
                instance,
                frame.get(),
                code.exceptiontable(),
                frame.get().instruction_pointer() * 2,
                exception,
                &mut decoder,
            )?;
            continue;
        }
        // **`AB-5`①**：宿主请求中断后就地停手（每条指令查一次，按实例存，`CX-3`）。
        if instance.interrupted() {
            return Err(ExecError::Interrupted);
        }
        let opcode_number = instruction.opcode;
        frame.get().set_instruction_pointer(instruction.offset);
        let oparg = instruction.oparg as usize;

        // BC-50：一律按名字分派，**禁止**依赖具体编号
        let Some(name) = opcode::opname(u16::from(opcode_number)) else {
            return Err(ExecError::NotImplemented { opcode: opcode_number });
        };

        // 把"一条指令"的执行包进闭包：这样异常能被这里接住并派发到处理块（BC-60 ①）。
        // 闭包返回 `Option<Value>`：`Some` 表示这条指令结束了整个执行（`RETURN_VALUE`）。
        let outcome = (|| -> Result<Step<'a>, ExecError> {
        match name {
            "RESUME" | "NOP" => {}
            // `BC-23`：边界检查的两条**专有**指令（`TS-10`…`TS-13`）
            "CHECK_BOUNDARY_IN" | "CHECK_BOUNDARY_OUT" => {
                boundary_check(instance, frame.get(), oparg as u8, opcode_number)?;
            }
            "LOAD_COMMON_CONSTANT" => {
                // `BC-57`＋`SPEC-bytecode.md`：oparg 索引**固定表**（实测 `dis._common_constants`：
                // 0 `AssertionError`／1 `NotImplementedError`／2 `tuple`／3 `all`／4 `any`），
                // **不是** `co_consts`／`co_names`。前三个是**类型对象**，后两个取 builtins 里的函数。
                let value = match oparg {
                    0 => instance.type_value(
                        instance
                            .type_named("AssertionError")
                            .expect("AssertionError 在内建表里"),
                    ),
                    1 => instance.type_value(
                        instance
                            .type_named("NotImplementedError")
                            .expect("NotImplementedError 在内建表里"),
                    ),
                    2 => instance.type_value(
                        instance.type_named("tuple").expect("tuple 在内建表里"),
                    ),
                    3 | 4 => {
                        let name = if oparg == 3 { "all" } else { "any" };
                        match instance
                            .builtins()
                            .and_then(|builtins| lookup_in_mapping(instance, builtins, name))
                        {
                            Some(found) => {
                                // SAFETY: found 由 builtins 持有，存活。
                                unsafe { instance.incref_object(found.as_ptr()) };
                                found
                            }
                            None => {
                                return Err(raise_builtin(
                                    instance,
                                    "NameError",
                                    &format!("name '{name}' is not defined"),
                                ))
                            }
                        }
                    }
                    _ => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "LOAD_COMMON_CONSTANT 的索引超出固定表（0…4）",
                        })
                    }
                };
                push(instance, frame.get(), value)?;
                release(instance, value);
            }
            // **cell 族**（`BC-45`）：cell 是独立对象（`CellObject`），帧的 cell 槽存"哪个 cell"。
            // 类体那条路径（`__classdict__`）与将来的闭包都用它。
            "COPY_FREE_VARS" => {
                // **3.11+ 是 no-op**：自由变量在建帧阶段由函数的闭包装入（`Frame::install_closure`）。
                // 保留这条 arm 是为了**不报 Unsupported**（如实表达"语义已由建帧承担"）。
                let _ = oparg;
            }
            "MAKE_CELL" => {
                // 净 0：把 **cell 槽**第 `oparg` 格换成一个新 cell；初值取**同号局部槽**（若有）
                let slot = oparg as usize;
                // 同号局部槽的值当 cell 初值（类体的 `nlocals` 是 0 ⇒ `local` 会报越界 ⇒ `None`）
                let initial = frame.get().raw_local(slot);
                let cell_type = instance
                    .type_named("cell")
                    .expect("引导期已登记 cell 类型");
                let cell = instance
                    .alloc(crate::cell::CellObject::new(cell_type, RefCell::new(initial)))
                    .into_raw()
                    .cast::<Header>();
                match frame.get().set_cell(slot, Some(cell)) {
                    Ok(Some(old)) => release(instance, old),
                    Ok(None) => {}
                    Err(error) => {
                        release(instance, cell);
                        return Err(ExecError::Frame(error));
                    }
                }
            }
            "LOAD_LOCALS" => {
                // 净 +1：压**本帧的命名空间映射**（类体的 `LOAD_LOCALS` 就是取那个 dict）。
                // 函数帧没有独立命名空间 ⇒ 如实报未接线。
                match frame.get().namespace() {
                    Some(mapping) => {
                        // SAFETY: mapping 由帧持有，存活。
                        unsafe { instance.incref_object(mapping.as_ptr()) };
                        push(instance, frame.get(), mapping)?;
                        release(instance, mapping);
                    }
                    None => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "`LOAD_LOCALS` 只接线了带命名空间的帧（类体）",
                        })
                    }
                }
            }
            "STORE_DEREF" => {
                // 净 −1：把 TOS 存进 cell 槽第 `oparg` 格那个 cell（cell 接手一份引用）
                let value = frame.get().pop()?;
                let slot = oparg as usize;
                let cell = match frame.get().cell(slot) {
                    Ok(cell) => cell,
                    Err(error) => {
                        release(instance, value);
                        return Err(ExecError::Frame(error));
                    }
                };
                let Some(cell) = cell else {
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "`STORE_DEREF` 的 cell 槽是空的（`MAKE_CELL` 没跑过）",
                    });
                };
                // SAFETY: cell 由帧的 cell 槽持有，存活。
                let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                // **`replace` 接管 `value` 那份引用** ✓（见 `CellObject::replace` 的契约 ✓）——
                // 这里**不得**再释放一次 ✗（第 208 轮真 bug 修复 ✗：先前多放一次 ⇒ cell 里留着
                // **没被记账的指针** ✗ ⇒ 值被提前释放 ⇒ cell 还指着它 ⇒ **释放后重用** ⇒ 堆损坏 ✓）。
                // 线索来自"**每次释放前扫全图看还有谁指着它**"那把尺子 ✓（引用者全是 `cell` ✓）。
                if let Some(old) = object.replace(Some(value)) {
                    release(instance, old);
                }
            }
            "LOAD_DEREF" => {
                // 净 +1：压 cell 槽第 `oparg` 格那个 cell 的值
                let cell = match frame.get().cell(oparg as usize) {
                    Ok(Some(cell)) => cell,
                    Ok(None) => {
                        // **诊断升级 ＋ 更接近参照** ✓（第 267 轮）：参照在同样情形给 `NameError` ✓
                        // ⇒ 把 **cell 名字**与**作用域**一起报出来 ✓（先前只说"cell 是空的" ✗ ⇒ 无从下手 ✓）。
                        let frame_ref = frame.get();
                        let (name, scope) = match frame_ref.code() {
                            Some(header) => {
                                let code = unsafe { &*header.as_ptr().cast::<crate::CodeObject>() };
                                (
                                    localsplus_name(code, oparg as usize),
                                    code.name().to_owned(),
                                )
                            }
                            None => ("?".to_owned(), "?".to_owned()),
                        };
                        let message = format!(
                            "cannot access free variable '{name}' where it is not associated with a value yet（作用域 {scope} ✓ 指令 {} ✓）",
                            frame_ref.instruction_pointer()
                        );
                        return Err(instance.raise_builtin_error("NameError", &message));
                    }
                    Err(error) => return Err(ExecError::Frame(error)),
                };
                // SAFETY: cell 由帧的 cell 槽持有，存活。
                let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                let Some(value) = object.value() else {
                    // **诊断升级 ＋ 更接近参照** ✓（第 267 轮）：参照在同样情形给 `NameError` ✓ ⇒
                    // 把 **cell 名字**与**作用域**一起报出来 ✓（先前只说“还是空的” ✗ ⇒ 无从下手 ✓）。
                    let frame_ref = frame.get();
                    let (name, scope) = match frame_ref.code() {
                        Some(header) => {
                            let code = unsafe { &*header.as_ptr().cast::<crate::CodeObject>() };
                            (localsplus_name(code, oparg as usize), code.name().to_owned())
                        }
                        None => ("?".to_owned(), "?".to_owned()),
                    };
                    let message = format!(
                        "cannot access free variable '{name}' where it is not associated with a value yet（作用域 {scope} ✓ 指令 {} ✓）",
                        frame_ref.instruction_pointer()
                    );
                    return Err(instance.raise_builtin_error("NameError", &message));
                };
                // SAFETY: 值由 cell 持有，存活。
                unsafe { instance.incref_object(value.as_ptr()) };
                push(instance, frame.get(), value)?;
                release(instance, value);
            }
            "LOAD_CONST" => {
                let raw = code
                    .constant(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "常量表下标越界",
                    })?;
                push(instance, frame.get(), raw)?;
            }
            // BC-51：LOAD_FAST 假定槽位已绑定（编译器保证）；这里宁可报错也不读垃圾
            // `LOAD_FAST_BORROW` 是 3.14 的借用形态：语义与 `LOAD_FAST` 相同（栈上不留新引用）。
            // 本层的值栈一律持有引用，故照常新增一份——**可观察语义一致**，只是少了那点优化。
            "LOAD_FAST" | "LOAD_FAST_CHECK" | "LOAD_FAST_BORROW" => {
                                match frame.get().local(oparg) {
                    Ok(Some(raw)) => push(instance, frame.get(), raw)?,
                    Ok(None) => return Err(unbound_local_error(instance, frame.get(), oparg)),
                    // **cell 在参照实现里也是"快速局部槽"**：类体的 `__classdict__` 只有 cell 槽
                    // （`nlocals` 是 0），而参照收尾用的是 `LOAD_FAST_BORROW 0` 读那个 cell
                    // ⇒ 局部槽越界时回落到**同号 cell 槽**（`BC-45` 的独立 cell 槽模型下的兼容）。
                    Err(crate::FrameError::SlotOutOfRange { .. }) => {
                        let cell = match frame.get().cell(oparg) {
                            Ok(Some(cell)) => cell,
                            _ => return Err(unbound_local_error(instance, frame.get(), oparg)),
                        };
                        // SAFETY: cell 由帧的 cell 槽持有，存活。
                        let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                        let Some(value) = object.value() else {
                            return Err(unbound_local_error(instance, frame.get(), oparg));
                        };
                        // SAFETY: 值由 cell 持有，存活。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        push(instance, frame.get(), value)?;
                        release(instance, value);
                    }
                    Err(error) => return Err(ExecError::Frame(error)),
                }
            }
            "STORE_FAST" => {
                let value = frame.get().pop()?;
                // **NULL 哨兵＝"未绑定"**（第 234 轮）：推导式的 `LOAD_FAST_AND_CLEAR` 在外层同名局部
                // **本来就没有**时会压那个哨兵，收尾的 `STORE_FAST` 要把它还原成"清空槽"而不是存一个
                // NULL 对象（否则推导式之后那个名字会变成 NULL 而不是 `NameError`）
                let restored = if value == instance.singletons().null() {
                    None
                } else {
                    Some(value)
                };
                                if let Some(old) = frame.get().set_local(oparg, restored)? {
                    release(instance, old);
                }
            }
            "DELETE_FAST" => {
                match frame.get().set_local(oparg, None)? {
                Some(old) => release(instance, old),
                None => return Err(unbound_local_error(instance, frame.get(), oparg)),
                }
            }
            "POP_TOP" => release(instance, frame.get().pop()?),
            "TO_BOOL" | "UNARY_NOT" => {
                let value = frame.get().pop()?;
                let truth = truthiness(instance, value, opcode_number)?;
                release(instance, value);
                let truth = if name == "UNARY_NOT" { !truth } else { truth };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "UNARY_NEGATIVE" | "UNARY_INVERT" => {
                let value = frame.get().pop()?;
                let symbol = if name == "UNARY_NEGATIVE" { "-" } else { "~" };
                let result = unary_public(instance, value, symbol, opcode_number);
                release(instance, value);
                frame.get().push(result?)?;
            }
            "JUMP_FORWARD" | "JUMP_BACKWARD" | "JUMP_BACKWARD_NO_INTERRUPT" => {
                let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "BC-55：这条指令没有跳转目标",
                })?;
                decoder.set_position(target);
            }
            "POP_JUMP_IF_TRUE" | "POP_JUMP_IF_FALSE" => {
                let value = frame.get().pop()?;
                let truth = truthiness(instance, value, opcode_number);
                release(instance, value);
                let jump = truth? == (name == "POP_JUMP_IF_TRUE");
                if jump {
                    let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BC-55：这条指令没有跳转目标",
                    })?;
                    decoder.set_position(target);
                }
            }
            "POP_JUMP_IF_NONE" | "POP_JUMP_IF_NOT_NONE" => {
                let value = frame.get().pop()?;
                // SAFETY: value 是刚出栈的存活对象。
                let is_none = unsafe { value.as_ref() }.ty() == instance.singletons().none_type();
                release(instance, value);
                if is_none == (name == "POP_JUMP_IF_NONE") {
                    let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BC-55：这条指令没有跳转目标",
                    })?;
                    decoder.set_position(target);
                }
            }
            "BUILD_TUPLE" | "BUILD_LIST" | "BUILD_SET" => {
                // oparg 是元素个数；压栈顺序就是元素顺序（先压的在前）
                let mut items = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    items.push(frame.get().pop()?);
                }
                items.reverse();

                match name {
                    "BUILD_TUPLE" => {
                        // **OM-23**：空元组是单例 ⇒ 必须走 `new_tuple`（`items` 为空时它给单例）
                        let tuple = instance.new_tuple(items);
                        frame.get().push(tuple)?;
                    }
                    "BUILD_LIST" => push_container(
                        instance,
                        frame.get(),
                        ListObject::new(builtin_type(instance, "list"), RefCell::new(items)),
                    )?,
                    _ => {
                        // set：按**值相等**查重，保留**先出现**的那个（与参照实现一致）
                        let set = instance.alloc(SetObject::new(
                            builtin_type(instance, "set"),
                            RefCell::new(Vec::new()),
                        ));
                        for item in items {
                            let duplicate = set
                                .get()
                                .items()
                                .iter()
                                .any(|existing| values_equal(instance, *existing, item));
                            if duplicate {
                                release(instance, item);
                            } else {
                                set.get().insert_raw(item);
                            }
                        }
                        frame.get().push(set.into_raw().cast::<Header>())?;
                    }
                }
            }
            "BUILD_SLICE" => {
                // `a[b:c:d]`（三段，至少一段非常量时参照发这条）：栈序是 lower, upper[, step]
                if !(2..=3).contains(&oparg) {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BUILD_SLICE 的 oparg 只能是 2 或 3（参照实测）",
                    });
                }
                let mut arguments = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    arguments.push(frame.get().pop()?);
                }
                arguments.reverse();
                let slice = build_slice(instance, &arguments, opcode_number)?;
                frame.get().push(slice)?;
            }
            "BINARY_SLICE" => {
                // `a[b:c]`（两段，至少一段非常量时参照发这条）：栈序是 container, lower, upper
                let upper = frame.get().pop()?;
                let lower = frame.get().pop()?;
                let container = frame.get().pop()?;
                let slice = build_slice(instance, &[lower, upper], opcode_number)?;
                let result = subscript_get(instance, container, slice, opcode_number);
                release(instance, container);
                release(instance, slice);
                frame.get().push(result?)?;
            }
            "BUILD_MAP" => {
                // 压栈顺序是 key1 value1 key2 value2 …（实测），弹出后反转成对
                let mut items = Vec::with_capacity(oparg * 2);
                for _ in 0..oparg * 2 {
                    items.push(frame.get().pop()?);
                }
                items.reverse();

                let dict = instance.alloc(DictObject::new(
                    builtin_type(instance, "dict"),
                    RefCell::new(Vec::new()),
                ));
                for pair in items.chunks(2) {
                    let (key, value) = (pair[0], pair[1]);
                    // 键按**值相等**查重：命中则**保留先出现的键**、替换值
                    let position = dict
                        .get()
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key));
                    match position {
                        Some(slot) => {
                            if let Some(old) = dict.get().replace_value(slot, value) {
                                release(instance, old);
                            }
                            release(instance, key);
                        }
                        None => dict.get().insert_raw(key, value),
                    }
                }
                frame.get().push(dict.into_raw().cast::<Header>())?;
            }
            "BUILD_STRING" => {
                let mut parts = Vec::with_capacity(oparg);
                for _ in 0..oparg {
                    parts.push(frame.get().pop()?);
                }
                parts.reverse();

                let str_type = instance.singletons().str_type();
                let mut text = String::new();
                let mut wrong_type = false;
                for part in parts {
                    // SAFETY: part 是刚出栈的存活对象。
                    if unsafe { part.as_ref() }.ty() == str_type {
                        // SAFETY: 类型身份已确认。
                        text.push_str(unsafe { &*part.as_ptr().cast::<StrObject>() }.value());
                    } else {
                        wrong_type = true;
                    }
                    release(instance, part);
                }
                if wrong_type {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BUILD_STRING 只接线了 str",
                    });
                }
                if text.is_empty() {
                    // OM-23：空串走单例
                    push(instance, frame.get(), instance.singletons().empty_str())?;
                } else {
                    push_container(instance, frame.get(), StrObject::new(str_type, text))?;
                }
            }
            "UNPACK_SEQUENCE" | "UNPACK_EX" => {
                let raw = frame.get().pop()?;
                let items = sequence_items(instance, raw, opcode_number);
                release(instance, raw);
                let items = items?;

                if name == "UNPACK_SEQUENCE" {
                    if items.len() != oparg {
                        for item in items.iter().copied() {
                            release(instance, item);
                        }
                        let message = if items.len() < oparg {
                            format!(
                                "not enough values to unpack (expected {oparg}, got {})",
                                items.len()
                            )
                        } else {
                            format!(
                                "too many values to unpack (expected {oparg})"
                            )
                        };
                        return Err(raise_builtin(instance, "ValueError", &message));
                    }
                    // 参照实现把元素**从右往左**压栈 ⇒ 最左边的目标拿到 TOS
                    for item in items.into_iter().rev() {
                        frame.get().push(item)?;
                    }
                } else {
                    // BC-38：UNPACK_EX 的 oparg ＝ 前者个数 ｜ 后者个数 << 8
                    let before = oparg & 0xFF;
                    let after = oparg >> 8;
                    if items.len() < before + after {
                        for item in items.iter().copied() {
                            release(instance, item);
                        }
                        let message = format!(
                            "not enough values to unpack (expected at least {}, got {})",
                            before + after,
                            items.len()
                        );
                        return Err(raise_builtin(instance, "ValueError", &message));
                    }
                    let total = items.len();
                    let middle = items[before..total - after].to_vec();
                    let middle_list = instance.alloc(ListObject::new(
                        builtin_type(instance, "list"),
                        RefCell::new(middle),
                    ));
                    let middle_raw = middle_list.into_raw().cast::<Header>();

                    for item in items[total - after..].iter().rev() {
                        frame.get().push(*item)?;
                    }
                    frame.get().push(middle_raw)?;
                    for item in items[..before].iter().rev() {
                        frame.get().push(*item)?;
                    }
                }
            }
            "LIST_APPEND" | "SET_ADD" => {
                // **第 235 轮实测修正**：真正的栈里 `FOR_ITER` **不弹迭代器**，迭代器夹在容器与
                // 元素之间 ⇒ `[保存值, 容器, 迭代器, 元素]`。参照的 `PEEK(oparg)` 是"从新栈顶数
                // 第 oparg 个"（`PEEK(1)` 才是 TOS）⇒ 弹出值之后容器在 `peek_from_top(oparg)`
                //（此前写成 `oparg - 1`，取到的是迭代器 ⇒ 报"容器在栈上的位置或类型不符"）
                if oparg == 0 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LIST_APPEND／SET_ADD 的 oparg 至少为 1",
                    });
                }
                let value = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg)?;
                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if name == "LIST_APPEND" && ty == builtin_type(instance, "list") {
                    // SAFETY: 类型身份已确认。
                    unsafe { &*container.as_ptr().cast::<ListObject>() }.append(value);
                } else if name == "SET_ADD" && ty == builtin_type(instance, "set") {
                    // SAFETY: 同上。
                    let set = unsafe { &*container.as_ptr().cast::<SetObject>() };
                    let duplicate = set
                        .items()
                        .iter()
                        .any(|existing| values_equal(instance, *existing, value));
                    if duplicate {
                        release(instance, value);
                    } else {
                        set.insert_raw(value);
                    }
                } else {
                    release(instance, value);
                    // **诊断**：把真实栈形状带进报错（此前只报"位置或类型不符"，看不出取错了哪一格）
                    let mut kinds: Vec<String> = Vec::new();
                    for index in 1..=5 {
                        kinds.push(match frame.get().peek_from_top(index) {
                            Ok(raw) => {
                                let raw_ty = unsafe { raw.as_ref() }.ty();
                                let mut label = "其它".to_owned();
                                for candidate in ["list", "tuple", "set", "dict", "int", "str"] {
                                    if raw_ty == builtin_type(instance, candidate) {
                                        label = candidate.to_owned();
                                    }
                                }
                                if raw == instance.singletons().none() {
                                    label = "None".to_owned();
                                }
                                if raw == instance.singletons().null() {
                                    label = "NULL".to_owned();
                                }
                                label
                            }
                            Err(_) => "越界".to_owned(),
                        });
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: Box::leak(
                            format!(
                                "{name} 的容器位置／类型不符（oparg {oparg}，栈顶往下 {kinds:?}）"
                            )
                            .into_boxed_str(),
                        ),
                    });
                }
            }
            "MAP_ADD" => {
                // **第 235 轮实测修正**：同 `LIST_APPEND`——`[保存值, 容器, 迭代器, 键, 值]`，
                // 地址从新栈顶数：弹出键值之后容器在 `peek_from_top(oparg)`（此前写成 `oparg - 1`）。
                if oparg == 0 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAP_ADD 的 oparg 至少为 1",
                    });
                }
                let value = frame.get().pop()?;
                let key = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg)?;
                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if ty != builtin_type(instance, "dict") {
                    release(instance, key);
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: Box::leak(
                            format!("MAP_ADD 的容器在栈上的位置或类型不符（opcode {opcode_number}）")
                                .into_boxed_str(),
                        ),
                    });
                }
                // SAFETY: 类型身份已确认。
                let dict = unsafe { &*container.as_ptr().cast::<DictObject>() };
                let position = dict
                    .entries()
                    .iter()
                    .position(|(existing, _)| values_equal(instance, *existing, key));
                match position {
                    Some(slot) => {
                        if let Some(old) = dict.replace_value(slot, value) {
                            release(instance, old);
                        }
                        release(instance, key);
                    }
                    None => dict.insert_raw(key, value),
                }
            }
            "LIST_EXTEND" | "SET_UPDATE" => {
                // 实测（`[*a, *b]`）：容器在 PEEK(oparg + 1)、源是 TOS ⇒ 弹出源之后容器在 PEEK(oparg)
                let source = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg)?;
                let items = sequence_items(instance, source, opcode_number);
                release(instance, source);
                let items = items?;

                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if name == "LIST_EXTEND" && ty == builtin_type(instance, "list") {
                    // SAFETY: 类型身份已确认。
                    unsafe { &*container.as_ptr().cast::<ListObject>() }.extend(items);
                } else if name == "SET_UPDATE" && ty == builtin_type(instance, "set") {
                    // SAFETY: 同上。
                    let set = unsafe { &*container.as_ptr().cast::<SetObject>() };
                    for item in items {
                        let duplicate = set
                            .items()
                            .iter()
                            .any(|existing| values_equal(instance, *existing, item));
                        if duplicate {
                            release(instance, item);
                        } else {
                            set.insert_raw(item);
                        }
                    }
                } else {
                    for item in items {
                        release(instance, item);
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "容器在栈上的位置或类型不符（指令名见下）",
                    });
                }
            }
            "GET_ITER" => {
                // 实测：GET_ITER 净 0（弹被迭代对象、压迭代器）；语义全在 `iter_value` 里
                // （`itertools.islice` 一类走同一处实现 ⇒ 消息与行为不会分叉）
                let iterable = frame.get().pop()?;
                match iter_value(instance, iterable) {
                    Ok(iterator) => {
                        // 迭代器可能**就是**入参（`iter(迭代器) is 它自己`）⇒ 别释放
                        if iterator != iterable {
                            release(instance, iterable);
                        }
                        push(instance, frame.get(), iterator)?;
                        release(instance, iterator);
                    }
                    Err(error) => {
                        release(instance, iterable);
                        return Err(error);
                    }
                }
            }
            "FOR_ITER" => {
                let iterator = frame.get().peek()?;
                // SAFETY: iterator 在帧值栈上，存活。
                let ty = unsafe { iterator.as_ref() }.ty();
                if ty == builtin_type(instance, "generator") {
                    // 生成器：`FOR_ITER` 的"取下一个"就是**恢复生成器的帧**（驱动实测骨架
                    // `CALL → GET_ITER → FOR_ITER`）；让出就压让出的值，跑完就走耗尽路径。
                    // SAFETY: 类型身份已确认。
                    let generator = unsafe { &*iterator.as_ptr().cast::<GeneratorObject>() };
                    if !generator.finished() {
                        let frame_header = generator.frame();
                        let generator_frame = Owned::new(
                            // SAFETY: frame_header 由生成器持有，存活；这里新增一份引用。
                            {
                                unsafe { instance.incref_object(frame_header.as_ptr()) };
                                frame_header.cast::<Frame>()
                            },
                            instance,
                        );
                        // 顺序**不能反**：`resume` 会用恢复点里的值栈**覆盖**当前值栈，
                        // 所以先恢复，再压"送进去的值"（首轮是 `None`，会被序言的 `POP_TOP` 丢掉；
                        // 之后 `x = yield v` 的取值就来自这里）。
                        if generator_frame.get().is_suspended() {
                            generator_frame.get().resume()?;
                        }
                        push(instance, generator_frame.get(), instance.singletons().none())?;
                        let outcome = execute(instance, &generator_frame);
                        match outcome {
                            Ok(ExecOutcome::Yielded(value)) => {
                                frame.get().push(value)?;
                            }
                            Ok(ExecOutcome::Returned(_)) => {
                                generator.mark_finished();
                                push(instance, frame.get(), instance.singletons().null())?;
                                let target = instruction.jump_target().ok_or(
                                    ExecError::Unsupported {
                                        opcode: opcode_number,
                                        what: "BC-55：这条指令没有跳转目标",
                                    },
                                )?;
                                decoder.set_position(target);
                            }
                            Err(error) => return Err(error),
                        }
                    } else {
                        push(instance, frame.get(), instance.singletons().null())?;
                        let target =
                            instruction
                                .jump_target()
                                .ok_or(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "BC-55：这条指令没有跳转目标",
                                })?;
                        decoder.set_position(target);
                    }
                    return Ok(Step::Continue);
                }
                if !is_iterator_type(instance, ty) {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "FOR_ITER 的对象不是本层接线的迭代器",
                    });
                }
                match advance_iterator(instance, iterator, opcode_number)? {
                    Some(item) => frame.get().push(item)?,
                    None => {
                        // 实测：**耗尽时 FOR_ITER 仍然 +1**（接着 END_FOR／POP_ITER 各 −1 收尾）
                        // ⇒ 这里压一个占位（内部 NULL 哨兵），随后被那两条指令弹掉。
                        push(instance, frame.get(), instance.singletons().null())?;
                        let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "BC-55：这条指令没有跳转目标",
                        })?;
                        decoder.set_position(target);
                    }
                }
            }
            "END_FOR" | "POP_ITER" => {
                // 实测两条都是 −1：前者收耗尽时压的那个占位，后者收迭代器本身
                release(instance, frame.get().pop()?);
            }
            "GET_YIELD_FROM_ITER" => {
                // 实测净 0：TOS 是生成器（或协程）就留着，否则换成 `iter(TOS)`
                let iterable = frame.get().pop()?;
                // SAFETY: iterable 是刚出栈的存活对象。
                let ty = unsafe { iterable.as_ref() }.ty();
                if ty == builtin_type(instance, "generator") || is_iterator_type(instance, ty) {
                    frame.get().push(iterable)?;
                } else {
                    match iterator_type_for(instance, iterable) {
                        Ok(iterator_type) => {
                            let iterator =
                                instance.alloc(IteratorObject::new(iterator_type, iterable, Cell::new(0)));
                            frame.get().push(iterator.into_raw().cast::<Header>())?;
                        }
                        Err(error) => {
                            release(instance, iterable);
                            return Err(error);
                        }
                    }
                }
            }
            "GET_AITER" => {
                // 实测净 0：异步生成器原样就是 async iterator；其余走 `__aiter__`；
                // 都没有 ⇒ 实测 `TypeError: 'async for' requires an object with __aiter__ method, got int`
                let value = frame.get().pop()?;
                // SAFETY: value 是帧值栈上的存活对象。
                let ty = unsafe { value.as_ref() }.ty();
                if ty == builtin_type(instance, "async_generator") {
                    push(instance, frame.get(), value)?;
                    release(instance, value);
                } else {
                    // SAFETY: value 是存活对象。
                    let name = unsafe { ty.as_ref() }.name();
                    match attribute_lookup(instance, value, "__aiter__") {
                        Ok(Attribute::Method { function, this }) => {
                            let mut arguments: Vec<NonNull<Header>> = Vec::new();
                            // SAFETY: this 由类型字典与调用方持有，这里新增一份交给调用。
                            unsafe { instance.incref_object(this.as_ptr()) };
                            arguments.push(this);
                            release(instance, value);
                            let iterator = call_callable(
                                instance,
                                function,
                                None,
                                arguments,
                                Vec::new(),
                                opcode_number,
                            )?;
                            push(instance, frame.get(), iterator)?;
                            release(instance, iterator);
                        }
                        _ => {
                            release(instance, value);
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                &format!(
                                    "'async for' requires an object with __aiter__ method, got {name}"
                                ),
                            ));
                        }
                    }
                }
            }
            "GET_ANEXT" => {
                // 实测净 +1：在 async iterator **之上**压一个 awaitable。
                // 异步生成器就压它自己（本层"await 它 ＝ 推进一次"）；其余走 `__anext__`。
                let iterator = frame.get().peek()?;
                // SAFETY: iterator 在帧值栈上，存活。
                let ty = unsafe { iterator.as_ref() }.ty();
                match attribute_lookup(instance, iterator, "__anext__") {
                    // 类型字典里的函数（取方法）
                    Ok(Attribute::Method { function, this }) => {
                        let mut arguments: Vec<NonNull<Header>> = Vec::new();
                        // SAFETY: this 由类型字典与调用方持有。
                        unsafe { instance.incref_object(this.as_ptr()) };
                        arguments.push(this);
                        let awaitable = call_callable(
                            instance,
                            function,
                            None,
                            arguments,
                            Vec::new(),
                            opcode_number,
                        )?;
                        push(instance, frame.get(), awaitable)?;
                        release(instance, awaitable);
                    }
                    // `getattr` 槽交出的**绑定方法**（异步生成器的 `__anext__` 走这条）
                    Ok(Attribute::Owned(bound)) => {
                        let awaitable =
                            call_callable(instance, bound, None, Vec::new(), Vec::new(), opcode_number)?;
                        release(instance, bound);
                        push(instance, frame.get(), awaitable)?;
                        release(instance, awaitable);
                    }
                    _ => {
                        // SAFETY: 类型身份未知，取名字用。
                        let name = unsafe { ty.as_ref() }.name();
                        return Err(raise_builtin(
                            instance,
                            "TypeError",
                            &format!("'async for' requires an object with __anext__ method, got {name}"),
                        ));
                    }
                }
            }
            "END_ASYNC_FOR" => {
                // 实测净 −2。栈是 `[async iterator, 异常]`（异常在 TOS）：
                // `StopAsyncIteration` ⇒ 丢掉异常与迭代器、跳到循环之后；
                // 其余 ⇒ 原样重抛（把异常交回派发器）。
                let exception = frame.get().pop()?;
                // SAFETY: exception 是存活对象。
                let ty = unsafe { exception.as_ref() }.ty();
                let stop_async_iteration = instance
                    .type_named("StopAsyncIteration")
                    .expect("异常层次在引导期已登记");
                if instance.is_subtype(ty, stop_async_iteration) {
                    release(instance, exception);
                    // 迭代器那一格也丢掉
                    release(instance, frame.get().pop()?);
                    if let Some(target) = instruction.jump_target() {
                        decoder.set_position(target);
                    }
                } else {
                    release(instance, frame.get().pop()?);
                    return Err(ExecError::Raised { exception });
                }
            }
            "CLEANUP_THROW" => {
                // 实测净 −1。参照实现用它收拾"`throw`／`close` 穿过当前帧"时的异常：
                // `StopIteration` ⇒ 换成它的**值**往下走；其余 ⇒ 原样留下（继续往派发器去）。
                let exception = frame.get().pop()?;
                // SAFETY: exception 是存活对象。
                let ty = unsafe { exception.as_ref() }.ty();
                let stop_iteration = instance
                    .type_named("StopIteration")
                    .expect("异常层次在引导期已登记");
                if instance.is_subtype(ty, stop_iteration) {
                    // SAFETY: 类型身份已确认。
                    let object = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };
                    let value = object.args().first().copied();
                    match value {
                        Some(value) => {
                            // SAFETY: 值由异常对象持有，新增一份交给值栈。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            release(instance, exception);
                            push(instance, frame.get(), value)?;
                            release(instance, value);
                        }
                        None => {
                            release(instance, exception);
                            push(instance, frame.get(), instance.singletons().none())?;
                        }
                    }
                } else {
                    push(instance, frame.get(), exception)?;
                    release(instance, exception);
                }
            }
            "GET_AWAITABLE" => {
                // 实测：净 0（弹一个、压一个）。协程（以及 `CO_ITERABLE_COROUTINE` 标记的
                // 生成器）**原样**就是 awaitable；其余对象走 `__await__`；
                // 都没有 ⇒ 实测 `TypeError: 'int' object can't be awaited`。
                let value = frame.get().pop()?;
                // SAFETY: value 是帧值栈上的存活对象。
                let ty = unsafe { value.as_ref() }.ty();
                let is_coroutine = ty == builtin_type(instance, "coroutine");
                let is_async_generator = ty == builtin_type(instance, "async_generator");
                // `async_generator.__anext__()` 交出的 awaitable：它**就是** awaitable
                let is_asend = ty == builtin_type(instance, "async_generator_asend");
                let is_generator = ty == builtin_type(instance, "generator");
                let iterable_coroutine = is_generator && {
                    // SAFETY: 类型身份已确认。
                    let object = unsafe { &*value.as_ptr().cast::<GeneratorObject>() };
                    let frame_header = object.frame();
                    // SAFETY: 帧由生成器持有，存活。
                    let generator_frame = unsafe { &*frame_header.as_ptr().cast::<Frame>() };
                    match generator_frame.code() {
                        // SAFETY: code 由帧持有，存活。
                        Some(code) => {
                            unsafe { code.cast::<CodeObject>().as_ref() }.flags() & 0x100 != 0
                        }
                        None => false,
                    }
                };
                // **注意**：异步生成器**不在**这里——实测 `await agen` ⇒
                // `TypeError: 'async_generator' object can't be awaited`（它要经 `__anext__()`
                // 交出的 awaitable）。第一版我图省事让它"await 一次推进一格"，被实测打回。
                let _ = is_async_generator;
                if is_coroutine || is_asend || iterable_coroutine {
                    push(instance, frame.get(), value)?;
                    release(instance, value);
                } else {
                    // SAFETY: value 是存活对象。
                    let name = unsafe { ty.as_ref() }.name();
                    match attribute_lookup(instance, value, "__await__") {
                        Ok(Attribute::Method { function, this }) => {
                            let mut arguments: Vec<NonNull<Header>> = Vec::new();
                            // SAFETY: this 由调用方与类型字典持有，这里新增一份交给调用。
                            unsafe { instance.incref_object(this.as_ptr()) };
                            arguments.push(this);
                            release(instance, value);
                            let iterator =
                                call_callable(instance, function, None, arguments, Vec::new(), opcode_number)?;
                            push(instance, frame.get(), iterator)?;
                            release(instance, iterator);
                        }
                        _ => {
                            release(instance, value);
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                &format!("'{name}' object can't be awaited"),
                            ));
                        }
                    }
                }
            }
            "SEND" => {
                // 实测：`SEND delta` 净 0 —— 栈是 `[接收者, 送进去的值]`，
                // 让出就压"让出的值"并**往下走**（下一条通常是 `YIELD_VALUE` 把它再让出去），
                // 跑完就压"接收者的返回值"并**跳转 delta**（跳到 `END_SEND`）。
                let sent = frame.get().pop()?;
                let receiver = frame.get().peek()?;
                let target = instruction.jump_target().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "BC-55：SEND 没有跳转目标",
                })?;
                // SAFETY: receiver 在帧值栈上，存活。
                let receiver_type = unsafe { receiver.as_ref() }.ty();
                // 生成器与协程走**同一条**恢复路径（载荷同形）；协程的 `throw`／`close` 也一样
                let is_generator = receiver_type == builtin_type(instance, "generator");
                let is_coroutine = receiver_type == builtin_type(instance, "coroutine");
                let is_async_generator = receiver_type == builtin_type(instance, "async_generator");
                let is_asend = receiver_type == builtin_type(instance, "async_generator_asend");
                if is_asend {
                    // `await agen.__anext__()`：推进**底层**异步生成器一次。
                    // 送进去的值以包装对象里记着的为准（`__anext__()` 是 `None`）。
                    // SAFETY: 类型身份已确认。
                    let asend = unsafe { &*receiver.as_ptr().cast::<AsendObject>() };
                    let inner = asend.generator();
                    let carried = asend.sent();
                    release(instance, sent);
                    match resume_generator(instance, inner, carried)? {
                        GeneratorOutcome::Yielded(value) => {
                            // **一步完成**：`await asend` 的语义就是"推进一次并把值交出来"
                            // （实测 `asend.send(None)` ⇒ `StopIteration(值)`），所以这里走
                            // "耗尽"那一支——压值并跳到 `END_SEND`，**不**让出去。
                            frame.get().push(value)?;
                            decoder.set_position(target);
                        }
                        GeneratorOutcome::Returned(value) => {
                            frame.get().push(value)?;
                            decoder.set_position(target);
                        }
                    }
                    return Ok(Step::Continue);
                }
                if !is_generator && !is_coroutine && !is_async_generator {
                    // 普通迭代器：参照实现的语义是"取下一个"（`yield from [1, 2]` 就走这条）。
                    // 送进去的值对没有 `send` 的对象没有去处——本层只接受 `None`（如实报其余）。
                    let sent_is_none =
                        unsafe { sent.as_ref() }.ty() == instance.singletons().none_type();
                    if !sent_is_none {
                        release(instance, sent);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "SEND 送非 None 给普通迭代器（参照实现走 `send`／`next` 协议）",
                        });
                    }
                    release(instance, sent);
                    match advance_iterator(instance, receiver, opcode_number)? {
                        Some(item) => frame.get().push(item)?,
                        None => {
                            // 耗尽：压"迭代器的返回值"——普通迭代器没有返回值，压 `None`
                            // （`yield from` 的 `END_SEND` 会把接收者收掉、留下这一格）
                            push(instance, frame.get(), instance.singletons().none())?;
                            decoder.set_position(target);
                        }
                    }
                    return Ok(Step::Continue);
                }
                // SAFETY: 类型身份已确认。
                let generator = unsafe { &*receiver.as_ptr().cast::<GeneratorObject>() };
                if generator.finished() {
                    // 已经跑完：送进去的值没用上，直接走"耗尽"那一路
                    release(instance, sent);
                    push(instance, frame.get(), instance.singletons().none())?;
                    decoder.set_position(target);
                    return Ok(Step::Continue);
                }
                // 复用"恢复生成器"的同一段逻辑（`send`／`__next__` 走的是它）
                match resume_generator(instance, receiver, Some(sent))? {
                    GeneratorOutcome::Yielded(value) => {
                        // 让出的值是**新引用**，裸 `Frame::push` 正好接手
                        frame.get().push(value)?;
                    }
                    GeneratorOutcome::Returned(value) => {
                        frame.get().push(value)?;
                        decoder.set_position(target);
                    }
                }
            }
            "END_SEND" => {
                // 净 −1，但**去掉的是 TOS1**：`SEND` 耗尽时栈是 `[接收者, 结果]`，
                // `END_SEND` 丢掉接收者、把结果留在栈顶（第一版我按"弹 TOS"写，
                // 结果把结果丢了自己留下接收者——驱动器拿到的是生成器）。
                let result = frame.get().pop()?;
                release(instance, frame.get().pop()?);
                frame.get().push(result)?;
            }
            "NOT_TAKEN" => {
                // §10 三分类②：参照实现**会发**这条（跟在 `POP_JUMP_*` 之后），
                // 但它是给专门化解释器用的提示；VM **必须容受**它（净 0，什么也不做）。
            }
            "LOAD_FAST_AND_CLEAR" => {
                // **推导式的"变量不外泄"**（第 234 轮实测）：把该局部**原值**压栈（本来没绑定就压
                // NULL 哨兵），随即**清空**这个槽；推导式收尾的 `STORE_FAST` 再把它还原
                let saved = frame.get().local(oparg)?;
                match saved {
                    Some(raw) => push(instance, frame.get(), raw)?,
                    None => push(instance, frame.get(), instance.singletons().null())?,
                }
                if let Some(old) = frame.get().set_local(oparg, None)? {
                    release(instance, old);
                }
            }
            "STORE_FAST_LOAD_FAST" => {
                // 净 0：`oparg` 打包两个局部槽——**高 4 位收 TOS**、低 4 位**再压回**（实测
                // `STORE_FAST_LOAD_FAST 0 (x, x)`：先存 `x` 再把同一个槽压回来）
                let value = frame.get().pop()?;
                let store_slot = oparg >> 4;
                let load_slot = oparg & 0x0F;
                if frame.get().set_local(store_slot, Some(value)).is_err() {
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "STORE_FAST_LOAD_FAST 的槽位越界",
                    });
                }
                let loaded = frame.get().local(load_slot)?.ok_or(ExecError::UnboundLocal {
                    slot: load_slot,
                })?;
                push(instance, frame.get(), loaded)?;
            }
            "LOAD_FAST_LOAD_FAST" | "LOAD_FAST_BORROW_LOAD_FAST_BORROW" => {
                // 实测净 +2：`oparg` 打包两个局部槽，**高 4 位先压**（`dis` 的 argrepr 就是
                // "(第一个, 第二个)"；`LOAD_FAST_BORROW_LOAD_FAST_BORROW 1 (a, b)` 里 a＝0、b＝1）
                let first = oparg >> 4;
                let second = oparg & 0x0F;
                let left = frame.get().local(first)?.ok_or(ExecError::UnboundLocal {
                    slot: first,
                })?;
                push(instance, frame.get(), left)?;
                let right = frame.get().local(second)?.ok_or(ExecError::UnboundLocal {
                    slot: second,
                })?;
                push(instance, frame.get(), right)?;
            }
            "DICT_MERGE" | "DICT_UPDATE" => {
                // 实测净 −1：把 TOS 那个字典并进 TOS1，然后弹掉 TOS。
                // `DICT_UPDATE` 覆盖同名键；`DICT_MERGE` 遇到同名键要报错——那条消息在参照实现里
                // 带着**函数的 qualname**（实测：`__main__.demo() got multiple values for keyword
                // argument 'a'`），而此刻调用者还在栈下好几层，本层取不到，所以如实报未接线。
                let source = frame.get().pop()?;
                let destination = frame.get().peek()?;
                // SAFETY: 两个都在帧值栈上，存活。
                let source_type = unsafe { source.as_ref() }.ty();
                let destination_type = unsafe { destination.as_ref() }.ty();
                if source_type != builtin_type(instance, "dict")
                    || destination_type != builtin_type(instance, "dict")
                {
                    release(instance, source);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "DICT_MERGE／DICT_UPDATE 只接线了 dict（映射协议随后补）",
                    });
                }
                // SAFETY: 类型身份已确认。
                let source_entries = unsafe { &*source.as_ptr().cast::<DictObject>() }.entries();
                // SAFETY: 同上。
                let destination_dict = unsafe { &*destination.as_ptr().cast::<DictObject>() };
                let is_merge = name == "DICT_MERGE";
                for (key, value) in source_entries {
                    let position = destination_dict
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key));
                    if let Some(existing_position) = position {
                        if is_merge {
                            release(instance, source);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "DICT_MERGE 的同名键错误要函数的 qualname（参照实现的消息带它）",
                            });
                        }
                        let (old_key, old_value) = destination_dict
                            .remove(existing_position)
                            .expect("刚查到的位置");
                        release(instance, old_key);
                        release(instance, old_value);
                    }
                    // SAFETY: 键值由源字典持有，这里各新增一份引用交给目标字典。
                    unsafe {
                        instance.incref_object(key.as_ptr());
                        instance.incref_object(value.as_ptr());
                    }
                    destination_dict.insert_raw(key, value);
                }
                release(instance, source);
            }
            "CALL_FUNCTION_EX" => {
                // 实测净 −3；栈自下而上是 `[可调用, self|NULL, 实参 tuple, 关键字 dict|NULL]`
                // （`f(*a)` 的发射里第二个 `PUSH_NULL` 就是"没有关键字"那一格）。
                let keyword_source = frame.get().pop()?;
                let argument_source = frame.get().pop()?;
                let self_or_null = frame.get().pop()?;
                let callable = frame.get().pop()?;
                // SAFETY: 都在帧值栈上（刚出栈），存活。
                let null = instance.singletons().null();
                let bound_self = if self_or_null == null {
                    None
                } else {
                    // SAFETY: 同上。
                    unsafe { instance.incref_object(self_or_null.as_ptr()) };
                    Some(self_or_null)
                };
                release(instance, self_or_null);

                // SAFETY: 类型身份检查在下面。
                let argument_type = unsafe { argument_source.as_ref() }.ty();
                // **实参可以是任意可迭代** ✓（第 149 轮实测参照：`f(*[1, 2])`／`f(*(i for i in (1, 2)))`
                //   都行 ✓）⇒ `tuple` 走快路 ✓，其余按**迭代协议**摊开 ✓（`iterable_items` 是
                //   **一处真相** ✓，且它返回的是**借用** ⇒ 每项先 `retain` ✓ —— 第 145 轮的教训 ✓）。
                let mut args: Vec<NonNull<Header>> = Vec::new();
                // **位置实参为空时参照传的是 `NULL`**（第 149 轮：`f(1, **kw)` ⇒ `LOAD f; LOAD_CONST 1;
                //   BUILD_MAP 1; …; CALL_FUNCTION_EX` ✓，其中实参那格是 `NULL` ✗ 不是空元组 ✓）
                // ⇒ 先认下这一种，再走后面的通用路 ✓。
                if argument_source == null {
                    // 空实参 ⇒ 什么也不用做 ✓（`args` 已是空表 ✓）
                } else if argument_type == builtin_type(instance, "tuple") {
                    // SAFETY: 类型身份已确认。
                    let arguments = unsafe { &*argument_source.as_ptr().cast::<TupleObject>() };
                    args.reserve(arguments.len());
                    for index in 0..arguments.len() {
                        let value = arguments.item(index).expect("下标在范围内");
                        // SAFETY: 元素由元组持有。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        args.push(value);
                    }
                } else if let Some(items) = instance.iterable_items(argument_source) {
                    args.reserve(items.len());
                    for item in items {
                        instance.retain(item);
                        args.push(item);
                    }
                } else {
                    release(instance, callable);
                    release(instance, argument_source);
                    release(instance, keyword_source);
                    if let Some(bound) = bound_self {
                        release(instance, bound);
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "argument after * must be an iterable",
                    });
                }
                release(instance, argument_source);

                let mut kwargs: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
                if keyword_source != null {
                    // SAFETY: 类型身份检查在下面。
                    let keyword_type = unsafe { keyword_source.as_ref() }.ty();
                    if keyword_type != builtin_type(instance, "dict") {
                        for value in args {
                            release(instance, value);
                        }
                        release(instance, callable);
                        release(instance, keyword_source);
                        if let Some(bound) = bound_self {
                            release(instance, bound);
                        }
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "CALL_FUNCTION_EX 的关键字必须是 dict（编译器保证）",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let mapping = unsafe { &*keyword_source.as_ptr().cast::<DictObject>() };
                    for (key, value) in mapping.entries() {
                        // SAFETY: 键值由字典持有。
                        unsafe {
                            instance.incref_object(key.as_ptr());
                            instance.incref_object(value.as_ptr());
                        }
                        kwargs.push((key, value));
                    }
                }
                release(instance, keyword_source);

                let result =
                    call_callable(instance, callable, bound_self, args, kwargs, opcode_number)?;
                frame.get().push(result)?;
            }
            "CALL_INTRINSIC_1" => {
                // 实测净 0（就地把 TOS 换掉）；oparg 是**内建表的编号**，按名字分派（`BC-50`）。
                let intrinsic = crate::opcode_metadata::INTRINSIC1_DESCS
                    .get(oparg)
                    .copied()
                    .unwrap_or("INTRINSIC_1_INVALID");
                match intrinsic {
                    "INTRINSIC_UNARY_POSITIVE" => {
                        // `+x`：本层只接整数／布尔（真协议 `__pos__` 随后补）——原地不动即可
                        let value = frame.get().peek()?;
                        // SAFETY: value 在帧值栈上，存活。
                        let ty = unsafe { value.as_ref() }.ty();
                        if ty != instance.singletons().int_type()
                            && ty != instance.singletons().bool_type()
                        {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "INTRINSIC_UNARY_POSITIVE 只接线了整数／布尔",
                            });
                        }
                    }
                    "INTRINSIC_IMPORT_STAR" => {
                        // `from <模块> import *`：把模块的**公开**名字写进当前命名空间 ✓
                        // （最小面：`__all__` 还没接 ✗ ⇒ 只取不以下划线开头的名字 ✓，与参照的默认口径同 ✓）
                        // **不动栈**：参照里随后的 `POP_TOP` 才把模块弹掉 ✓（`IMPORT_STAR` 净 0 ✓）
                        let module = frame.get().peek()?;
                        let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "`import *` 需要命名空间帧（模块／类体）",
                        })?;
                        let Some(attributes) = mounted_instance_dict(instance, module) else {
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "`import *` 的对象没有属性字典",
                            });
                        };
                        // SAFETY: attributes 由模块对象持有，存活。
                        let entries = unsafe { &*attributes.as_ptr().cast::<DictObject>() }.entries();
                        for (key, _) in entries {
                            let Some(name) = instance.text_of(key).map(|text| text.to_owned())
                            else {
                                continue;
                            };
                            if name.starts_with('_') {
                                continue;
                            }
                            let Some(value) = instance.dict_get(attributes, &name) else {
                                continue;
                            };
                            // **交一份新引用**：`dict_set` 会接管它（`OM-16` ✓）
                            // SAFETY: value 由模块的属性字典持有，存活。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            instance.dict_set(namespace, &name, value);
                        }
                    }
                    "INTRINSIC_LIST_TO_TUPLE" => {
                        // `(*[1, 2],)`：把 TOS 的列表换成元组（元素各持一份引用）
                        let value = frame.get().pop()?;
                        // SAFETY: value 是刚出栈的存活对象。
                        let ty = unsafe { value.as_ref() }.ty();
                        if ty != builtin_type(instance, "list") {
                            release(instance, value);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "INTRINSIC_LIST_TO_TUPLE 的操作数必须是 list",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        let list = unsafe { &*value.as_ptr().cast::<ListObject>() };
                        let items = list.items();
                        let mut moved: Vec<NonNull<Header>> = Vec::with_capacity(items.len());
                        for item in items {
                            // SAFETY: 元素由列表持有，这里各新增一份引用交给元组。
                            unsafe { instance.incref_object(item.as_ptr()) };
                            moved.push(item);
                        }
                        release(instance, value);
                        let tuple = instance.new_tuple(moved);
                        frame.get().push(tuple)?;
                    }
                    "INTRINSIC_STOPITERATION_ERROR" => {
                        // 生成器里漏出来的 `StopIteration` 要转成 `RuntimeError`
                        // （实测原话：`generator raised StopIteration`）——用于生成器异常表那条收尾路径。
                        let value = frame.get().peek()?;
                        // SAFETY: value 在帧值栈上，存活。
                        let ty = unsafe { value.as_ref() }.ty();
                        let stop_iteration = instance
                            .type_named("StopIteration")
                            .expect("StopIteration 在异常层次里");
                        if instance.is_subtype(ty, stop_iteration) {
                            release(instance, frame.get().pop()?);
                            let exception = new_exception(
                                instance,
                                exception_type(instance, "RuntimeError"),
                                "generator raised StopIteration",
                            );
                            frame.get().push(exception)?;
                        }
                        // 不是 `StopIteration` 就原样留着（净 0）
                    }
                    "INTRINSIC_1_INVALID" => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "CALL_INTRINSIC_1 的 oparg 越界（表里没有这一号）",
                        });
                    }
                    other => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: other,
                        });
                    }
                }
            }
            "MATCH_SEQUENCE" => {
                // 净 +1：压"是不是序列"，被测对象留着。实测 `str`／`dict` **不算**序列
                let subject = frame.get().peek()?;
                // SAFETY: subject 在帧值栈上，存活。
                let ty = unsafe { subject.as_ref() }.ty();
                let truth = ty == builtin_type(instance, "list")
                    || ty == builtin_type(instance, "tuple");
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "MATCH_MAPPING" => {
                // 净 +1：本层只有 `dict` 算映射
                let subject = frame.get().peek()?;
                // SAFETY: subject 在帧值栈上，存活。
                let truth = unsafe { subject.as_ref() }.ty() == builtin_type(instance, "dict");
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "MATCH_KEYS" => {
                // 净 +1：栈是 `[被测映射, 键的 tuple]`——**两者都留着**，再压"值的 tuple"；
                // 任一键缺失就压 `None`（实测：缺键 ⇒ 这个 case 不匹配）
                let keys = frame.get().peek()?;
                let subject = frame.get().peek_from_top(2)?;
                // SAFETY: 两个都在帧值栈上，存活。
                let keys_type = unsafe { keys.as_ref() }.ty();
                if keys_type != builtin_type(instance, "tuple") {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MATCH_KEYS 的键必须是 tuple（编译器保证）",
                    });
                }
                // SAFETY: 类型身份已确认。
                let key_items = unsafe { &*keys.as_ptr().cast::<TupleObject>() };
                // SAFETY: subject 是存活对象。
                let subject_type = unsafe { subject.as_ref() }.ty();
                if subject_type != builtin_type(instance, "dict") {
                    let none = instance.singletons().none();
                    push(instance, frame.get(), none)?;
                    return Ok(Step::Continue);
                }
                // SAFETY: 类型身份已确认。
                let mapping = unsafe { &*subject.as_ptr().cast::<DictObject>() };
                let mut values: Vec<NonNull<Header>> = Vec::with_capacity(key_items.len());
                let mut missing = false;
                for index in 0..key_items.len() {
                    let key = key_items.item(index).expect("下标在范围内");
                    match mapping
                        .entries()
                        .iter()
                        .position(|(existing, _)| values_equal(instance, *existing, key))
                    {
                        Some(position) => {
                            let (_, value) = mapping.entry(position).expect("刚查到的位置");
                            // SAFETY: value 由字典持有，存活。
                            unsafe { instance.incref_object(value.as_ptr()) };
                            values.push(value);
                        }
                        None => {
                            missing = true;
                            break;
                        }
                    }
                }
                if missing {
                    for value in values {
                        release(instance, value);
                    }
                    let none = instance.singletons().none();
                    push(instance, frame.get(), none)?;
                } else {
                    let tuple = instance.new_tuple(values);
                    frame.get().push(tuple)?;
                }
            }
            "MATCH_CLASS" => {
                // 净 −2：栈是 `[被测对象, 类, 关键字名 tuple]`——**被测对象也被吃掉**，
                // 命中就压"取出的属性 tuple"，不命中就压 `None`（实测形状：
                // `MATCH_CLASS n; COPY 1; POP_JUMP_IF_NONE L; UNPACK_SEQUENCE …`）。
                let names = frame.get().pop()?;
                let class_object = frame.get().pop()?;
                let subject = frame.get().pop()?;
                if oparg != 0 {
                    release(instance, names);
                    release(instance, class_object);
                    release(instance, subject);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MATCH_CLASS 只接线了关键字形参（位置形参随后补）",
                    });
                }
                // SAFETY: class_object 是刚出栈的存活对象。
                let class_type = unsafe { class_object.as_ref() }.ty();
                if class_type != builtin_type(instance, "type") {
                    release(instance, names);
                    release(instance, class_object);
                    release(instance, subject);
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        "called match pattern must be a type",
                    ));
                }
                let class = class_object.cast::<TypeObject>();
                // SAFETY: subject 是存活对象。
                let subject_type = unsafe { subject.as_ref() }.ty();
                let matched = instance.is_subtype(subject_type, class);
                release(instance, class_object);
                if !matched {
                    release(instance, names);
                    release(instance, subject);
                    let none = instance.singletons().none();
                    push(instance, frame.get(), none)?;
                    return Ok(Step::Continue);
                }
                // SAFETY: names 是 tuple（编译器保证）。
                let name_items = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                let mut values: Vec<NonNull<Header>> = Vec::with_capacity(name_items.len());
                for index in 0..name_items.len() {
                    let name = name_items.item(index).expect("下标在范围内");
                    // SAFETY: name 由 tuple 持有，存活。
                    let name_type = unsafe { name.as_ref() }.ty();
                    if name_type != instance.singletons().str_type() {
                        for value in values {
                            release(instance, value);
                        }
                        release(instance, names);
                        release(instance, subject);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MATCH_CLASS 的关键字名必须是 str",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let text = unsafe { &*name.as_ptr().cast::<StrObject>() }.value().to_owned();
                    match attribute_lookup(instance, subject, &text) {
                        Ok(Attribute::Owned(raw)) => values.push(raw),
                        Ok(Attribute::Value(raw)) => {
                            // SAFETY: raw 由类型／实例字典持有，存活。
                            unsafe { instance.incref_object(raw.as_ptr()) };
                            values.push(raw);
                        }
                        Ok(Attribute::Method { .. }) => {
                            // 取到的是**方法**（函数 ＋ self 绑定），本层还没有"绑定方法"对象
                            for value in values {
                                release(instance, value);
                            }
                            release(instance, names);
                            release(instance, subject);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "MATCH_CLASS 的属性是方法时要「绑定方法」对象（随后补）",
                            });
                        }
                        Err(error) => {
                            release(instance, names);
                            release(instance, subject);
                            return Err(error);
                        }
                    }
                }
                release(instance, names);
                release(instance, subject);
                let tuple = instance.new_tuple(values);
                frame.get().push(tuple)?;
            }
            "STORE_FAST_STORE_FAST" => {
                // 净 −2：`oparg` 打包两个局部槽——**高 4 位收 TOS**、低 4 位收 TOS1
                // （实测 `STORE_FAST_STORE_FAST 18 (a, b)` 里 a 是 1、b 是 2，而解包把**第一个**元素压在栈顶）
                let first = frame.get().pop()?;
                let second = frame.get().pop()?;
                let low = oparg & 0x0F;
                let high = oparg >> 4;
                if frame.get().set_local(high, Some(first)).is_err()
                    || frame.get().set_local(low, Some(second)).is_err()
                {
                    release(instance, first);
                    release(instance, second);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "STORE_FAST_STORE_FAST 的槽位越界",
                    });
                }
            }
            "LOAD_SMALL_INT" => {
                // 3.14 的新指令：直接把 oparg 当小整数压栈（不走常量表）。实测效果 +1。
                push_int(instance, frame.get(), oparg as i64)?;
            }
            "FORMAT_SIMPLE" => {
                // 净 0：TOS 换成它的 `str()`（3.14 把旧的 `FORMAT_VALUE` 拆成了三条）
                let value = frame.get().pop()?;
                // **直接走 `object_str`** ✓（第 210 轮修正 ✗）：它自己就是"**槽位优先** ＋ 覆写通道 ＋
                // 兜底" ✓ —— 与参照的默认 `object.__format__`（= `str(self)` ✓）同序 ✓。
                // 先前这里"**先查 `__str__` 覆写**" ✗ ⇒ 一旦 `object.__str__` 存在（本轮起 ✓），
                // 每个 MRO 都会命中它 ⇒ 把 `str`／`int` 自带的 `str` 槽带跑 ✗（实测 `f"{x}"` 给 `'1'` ✗）。
                let text = instance.object_str(value)?;
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "CONVERT_VALUE" => {
                // 净 0：`!s`／`!r`／`!a`（实测 oparg 1／2／3）
                let value = frame.get().pop()?;
                // `!s`／`!r`／`!a`（实测 oparg 1／2／3），都走 `OM-11` 的槽位
                let text = match oparg {
                    // **同 `FORMAT_SIMPLE`** ✓（第 210 轮修正 ✗）。
                    1 => instance.object_str(value)?,
                    2 => match dunder_text(instance, value, "__repr__", opcode_number)? {
                        Some(text) => text,
                        None => instance.object_repr(value)?,
                    },
                    3 => {
                        let base = match dunder_text(instance, value, "__repr__", opcode_number)? {
                            Some(text) => text,
                            None => instance.object_repr(value)?,
                        };
                        escape_non_ascii(&base)
                    }
                    _ => {
                        release(instance, value);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "CONVERT_VALUE 的 oparg 只能是 1／2／3",
                        });
                    }
                };
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "FORMAT_WITH_SPEC" => {
                // 实测净 −1：栈是 `[值, 规格]`（规格在 TOS）。
                // 路线：① 类型字典里的 `__format__`（Python 级覆写优先）② 类型的 `format` 槽
                // ③ 都不认 ⇒ 报错（消息**实测**：`unsupported format string passed to X.__format__`）。
                let spec_object = frame.get().pop()?;
                // SAFETY: spec_object 是刚出栈的存活对象。
                let spec_type = unsafe { spec_object.as_ref() }.ty();
                if spec_type != instance.singletons().str_type() {
                    release(instance, spec_object);
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        "format spec must be a str",
                    ));
                }
                // SAFETY: 类型身份已确认。
                let spec_text =
                    unsafe { &*spec_object.as_ptr().cast::<StrObject>() }.value().to_owned();
                release(instance, spec_object);

                let value = frame.get().pop()?;
                // SAFETY: value 是刚出栈的存活对象。
                let value_type = unsafe { value.as_ref() }.ty();
                let class_name = {
                    // SAFETY: 类型名由注册表持有。
                    unsafe { value_type.as_ref() }.name().to_owned()
                };
                // ① **属性通道**（`TS-44`）：类型字典里的 `__format__`，函数与原生可调用对象一视同仁
                // （`OM-11` 的 `getattr` 槽在查到函数时给"函数 ＋ self"，其余给值）。
                match attribute_lookup(instance, value, "__format__") {
                    Ok(Attribute::Method { function, this }) => {
                        let mut args: Vec<NonNull<Header>> = Vec::with_capacity(1);
                        args.push(instance.new_str(&spec_text));
                        let result = call_callable(
                            instance,
                            function,
                            Some(this),
                            args,
                            Vec::new(),
                            opcode_number,
                        )?;
                        release(instance, value);
                        frame.get().push(result)?;
                        return Ok(Step::Continue);
                    }
                    Ok(Attribute::Value(method)) | Ok(Attribute::Owned(method)) => {
                        // 原生可调用对象：self 经 `bound_self` 递进去
                        let mut args: Vec<NonNull<Header>> = Vec::with_capacity(1);
                        args.push(instance.new_str(&spec_text));
                        let result = call_callable(
                            instance,
                            method,
                            Some(value),
                            args,
                            Vec::new(),
                            opcode_number,
                        )?;
                        release(instance, value);
                        frame.get().push(result)?;
                        return Ok(Step::Continue);
                    }
                    Err(_) => {}
                }
                // ② 属性通道查不到 `__format__`：`TS-44` 说语义**只走属性通道**
                // （槽位是"没有 Python 级 dunder 时的原生默认实现"；`object` 那一层给默认，
                // 于是正常对象总能查到）。走到这里说明类型的 MRO 不完整 ⇒ 如实报错。
                let message = format!("unsupported format string passed to {class_name}.__format__");
                return Err(raise_builtin(instance, "TypeError", &message));
            }
            "GET_LEN" => {
                // 实测：+1（不弹原对象）
                let raw = frame.get().peek()?;
                let length = iterable_length(instance, raw, opcode_number)?;
                push_int(instance, frame.get(), length as i64)?;
            }
            "SWAP" => {
                // 参照实现：SWAP(i) 交换 TOS 与 TOS[-i]（净 0）
                frame.get().swap_from_top(oparg)?;
            }
            "COPY" => {
                // 参照实现：COPY(i) 把 TOS[-i] 复制一份压栈（+1）
                let raw = frame.get().peek_from_top(oparg)?;
                push(instance, frame.get(), raw)?;
            }
            "PUSH_EXC_INFO" => {
                // 实测骨架：处理块入口第一条就是它；栈效果 `(new_exc -- prev_exc, new_exc)`
                let exception = frame.get().pop()?;
                let previous = instance.current_exception();
                let previous_owned = match previous {
                    Some(value) => {
                        // SAFETY: value 由实例的异常状态持有，存活。
                        unsafe { instance.incref_object(value.as_ptr()) };
                        value
                    }
                    None => {
                        let none = instance.singletons().none();
                        // SAFETY: 单例由实例持有。
                        unsafe { instance.incref_object(none.as_ptr()) };
                        none
                    }
                };
                // 状态接管这份引用（当前的"正在处理的异常"）
                instance.push_exception(exception);
                frame.get().push(previous_owned)?;
                push(instance, frame.get(), exception)?;
            }
            "POP_EXCEPT" => {
                // 栈顶是 `PUSH_EXC_INFO` 压下的"上一个异常"；把它还原成当前异常
                let previous = frame.get().pop()?;
                if let Some(current) = instance.pop_exception() {
                    release(instance, current);
                }
                if unsafe { previous.as_ref() }.ty() == instance.singletons().none_type() {
                    release(instance, previous);
                } else {
                    instance.push_exception(previous);
                }
            }
            "CHECK_EXC_MATCH" => {
                // **实测**：它**弹掉类**、压回布尔（净 0）——参照实现原话是
                // "Pops TOS and pushes the boolean result of the test"。
                let class_object = frame.get().pop()?;
                let exception = frame.get().peek()?;
                // SAFETY: class_object 是刚出栈的存活对象。
                let class_type = unsafe { class_object.as_ref() }.ty();
                // SAFETY: exception 在帧值栈上，存活。
                let exception_type = unsafe { exception.as_ref() }.ty();
                let truth = if class_type == builtin_type(instance, "type") {
                    let class = class_object.cast::<TypeObject>();
                    if !is_exception_type(instance, class) {
                        release(instance, class_object);
                        return Err(raise_builtin(
                            instance,
                            "TypeError",
                            "catching classes that do not inherit from BaseException is not allowed",
                        ));
                    }
                    let matched = instance.is_subtype(exception_type, class);
                    release(instance, class_object);
                    matched
                } else if Some(class_type) == instance.type_named("tuple") {
                    // `except (A, B)`：**任一命中即匹配**（实测）。
                    //
                    // 元组里放**非类**、放**嵌套元组**、或放不是 `BaseException` 子类的类
                    // （`except str`）都报同一句 `TypeError`（实测原话见下面那条 assert）；
                    // 且**只在真的要匹配时**才报——没异常发生时该子句根本不执行。
                    // SAFETY: 类型身份已确认。
                    let items = unsafe { &*class_object.as_ptr().cast::<TupleObject>() };
                    let mut matched = false;
                    for index in 0..items.len() {
                        let item = items.item(index).expect("下标在范围内");
                        // SAFETY: item 由元组持有，存活。
                        let item_type = unsafe { item.as_ref() }.ty();
                        if item_type != builtin_type(instance, "type") {
                            release(instance, class_object);
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                "catching classes that do not inherit from BaseException is not allowed",
                            ));
                        }
                        let candidate = item.cast::<TypeObject>();
                        if !is_exception_type(instance, candidate) {
                            release(instance, class_object);
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                "catching classes that do not inherit from BaseException is not allowed",
                            ));
                        }
                        if instance.is_subtype(exception_type, candidate) {
                            matched = true;
                        }
                    }
                    release(instance, class_object);
                    matched
                } else {
                    release(instance, class_object);
                    return Err(raise_builtin(
                        instance,
                        "TypeError",
                        "catching classes that do not inherit from BaseException is not allowed",
                    ));
                };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "RERAISE" => {
                // **`RERAISE n`：先取 TOS 当异常，再弹那 `n` 个额外值** ✓（第 172 轮定案 ✓）。
                //   我们先前写成了「先弹 `n` 个、再抛 TOS」✗ —— 一弹就把**异常本身**弹掉 ✗，
                //   于是抛出去的是下面的 `lasti`（一个 **`int`** ✓）⇒ 症状就是本层那句
                //   `未捕获（状态 1）：int` ✓（参照报 `ZeroDivisionError: division by zero` ✓）。
                //   实测栈（顶在前）：`[ZeroDivisionError, NoneType, int, CM, function]` ✓ ——
                //   TOS 是异常 ✓、下面那两格是 `prev`／`lasti` ✓，与 `WITH_EXCEPT_START` 的注释一致 ✓。
                let exception = frame.get().pop()?;
                for _ in 0..oparg {
                    release(instance, frame.get().pop()?);
                }
                return Err(raise(instance, exception));
            }
            "LOAD_BUILD_CLASS" => {
                // 实测：`LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <类体>; MAKE_FUNCTION; …`
                let Some(build_class) = instance.build_class() else {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "本实例没有 __build_class__（引导期未建？）",
                    });
                };
                push(instance, frame.get(), build_class)?;
            }
            "LOAD_NAME" => {
                // 参照顺序：**局部（命名空间）→ 全局 → 内建**（`BC-57` 的注）。
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "LOAD_NAME 需要命名空间帧（模块／类体）",
                })?;
                match lookup_in_mapping(instance, namespace, &name) {
                    Some(value) => push(instance, frame.get(), value)?,
                    None => {
                        // 第二层：全局（类体帧的全局在 `PUSH_EXC_INFO` 之外的另一格上）
                        let globals = frame.get().globals();
                        let found = globals.and_then(|mapping| lookup_in_mapping(instance, mapping, &name));
                        match found.or_else(|| instance.builtins().and_then(|builtins| lookup_in_mapping(instance, builtins, &name))) {
                            Some(value) => push(instance, frame.get(), value)?,
                            None => {
                                // 实测消息：`name 'Base' is not defined`（参照实现还会附"Did you mean"
                                // 建议，那属于建议机制，已在差异清单 `DIV-6` 里登记）
                                let message = format!("name '{name}' is not defined");
                                return Err(raise_builtin(instance, "NameError", &message));
                            }
                        }
                    }
                }
            }
            "LOAD_GLOBAL" => {
                // `BC-57`：`LOAD_GLOBAL` 像 `LOAD_ATTR` 一样移位（**名字下标 ＝ `oparg >> 1`**），
                // 低位是"调用前先压 `NULL`"（实测：`dis` 的 argrepr 显示 `+ NULL`）。
                let name = code
                    .name_at(oparg >> 1)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                // **低位那把 `NULL` 要压在"值之后"** ✓（第 156 轮抓到的真 bug ✓）：`CALL` 期望
                //   `[可调用, NULL]`（NULL 在**上** ✓ —— 模块级的 `LOAD_NAME; PUSH_NULL` 就是这个形状，
                //   一直能跑 ✓）；先前这里把 NULL 压在**值之前** ✗ ⇒ `CALL` 把 NULL 当可调用 ⇒
                //   `TypeError: 'NULL' object is not callable` ✓（实测：**函数里调用任何内建都崩** ✗，
                //   而 `return 5` 之类的非调用语句都好 ✓ ⇒ 夹具只比编译 ✗、语料又没覆盖 ⇒ 一直没暴露 ✗）。
                // 顺序：**全局 → 内建**（`LOAD_GLOBAL` 不看局部）
                let found = frame
                    .get()
                    .effective_globals()
                    .and_then(|mapping| lookup_in_mapping(instance, mapping, &name))
                    .or_else(|| {
                        instance
                            .builtins()
                            .and_then(|builtins| lookup_in_mapping(instance, builtins, &name))
                    });
                match found {
                    Some(value) => {
                        push(instance, frame.get(), value)?;
                        if oparg & 1 != 0 {
                            let null = instance.singletons().null();
                            push(instance, frame.get(), null)?;
                        }
                    }
                    None => {
                        let message = format!("name '{name}' is not defined");
                        return Err(raise_builtin(instance, "NameError", &message));
                    }
                }
            }
            "STORE_GLOBAL" => {
                // `BC-57`：这两条**不移位**（名字下标就是 `oparg` 本身）
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let mapping = frame.get().effective_globals().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "STORE_GLOBAL 需要全局映射（函数记着定义处的全局）",
                })?;
                let value = frame.get().pop()?;
                // SAFETY: mapping 由帧或函数持有，存活。
                let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                let position = dict
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                if let Some(position) = position {
                    if let Some((old_key, old_value)) = dict.remove(position) {
                        release(instance, old_key);
                        release(instance, old_value);
                    }
                }
                let key = instance.new_str(&name);
                dict.insert_raw(key, value);
            }
            "DELETE_GLOBAL" => {
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let mapping = frame.get().effective_globals().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "DELETE_GLOBAL 需要全局映射",
                })?;
                // SAFETY: mapping 由帧或函数持有，存活。
                let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
                let position = dict
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                match position {
                    Some(position) => {
                        if let Some((old_key, old_value)) = dict.remove(position) {
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                    }
                    None => {
                        let message = format!("name '{name}' is not defined");
                        return Err(raise_builtin(instance, "NameError", &message));
                    }
                }
            }
            "STORE_NAME" => {
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "STORE_NAME 需要命名空间帧（模块／类体）",
                })?;
                let value = frame.get().pop()?;
                // SAFETY: namespace 由帧持有，存活。
                let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
                let position = mapping
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                if let Some(position) = position {
                    if let Some((old_key, old_value)) = mapping.remove(position) {
                        release(instance, old_key);
                        release(instance, old_value);
                    }
                }
                let key = instance.new_str(&name);
                mapping.insert_raw(key, value);
            }
            "DELETE_NAME" => {
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let namespace = frame.get().namespace().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "DELETE_NAME 需要命名空间帧（模块／类体）",
                })?;
                // SAFETY: namespace 由帧持有，存活。
                let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
                let position = mapping
                    .entries()
                    .iter()
                    .position(|(existing, _)| str_matches_public(instance, *existing, &name));
                match position {
                    Some(position) => {
                        if let Some((old_key, old_value)) = mapping.remove(position) {
                            release(instance, old_key);
                            release(instance, old_value);
                        }
                    }
                    None => {
                        let message = format!("name '{name}' is not defined");
                        return Err(raise_builtin(instance, "NameError", &message));
                    }
                }
            }
            "CONTAINS_OP" => {
                // 实测：`x in c` 的栈是 `[x, c]`（容器在 TOS）；`oparg` 0 ＝ `in`、1 ＝ `not in`
                // （`dis` 的 argrepr 就是这两个词）。
                let container = frame.get().pop()?;
                let item = frame.get().pop()?;
                let found = contains(instance, container, item, opcode_number)?;
                let truth = if oparg & 1 != 0 { !found } else { found };
                release(instance, container);
                release(instance, item);
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "LOAD_SPECIAL" => {
                // **`with` 协议的第一步**（3.14 的发射骨架实测）：
                //   `LOAD_FAST_BORROW ctx; COPY; LOAD_SPECIAL __exit__; SWAP 2; SWAP 3;
                //    LOAD_SPECIAL __enter__; CALL 0; …`
                // 净栈效应 **+1**：**弹出对象、压入 (可调用, self)**，`self` 在 TOS
                // ——`CALL` 一贯的栈形状是 `[可调用, NULL|self, 实参…]`（`PUSH_NULL` 排在可调用
                // **之后**），所以这里必须"可调用在下、self 在上"，紧随其后的 `CALL` 才取得对
                // （`__exit__` 那一份留在栈上，给正常出口与异常出口各用一次）。
                let name = crate::opcode::get_special_method_names()
                    .get(oparg as usize)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LOAD_SPECIAL 的下标不在特殊方法表里",
                    })?;
                let object = frame.get().pop()?;
                match attribute_lookup(instance, object, name) {
                    Ok(Attribute::Method { function, this }) => {
                        // 两者都是**借用**：压栈会各自 incref
                        push(instance, frame.get(), function)?;
                        push(instance, frame.get(), this)?;
                        release(instance, object);
                    }
                    Ok(Attribute::Value(value)) => {
                        push(instance, frame.get(), value)?;
                        push(instance, frame.get(), object)?;
                        release(instance, object);
                    }
                    Ok(Attribute::Owned(value)) => {
                        push(instance, frame.get(), value)?;
                        push(instance, frame.get(), object)?;
                        release(instance, object);
                        release(instance, value);
                    }
                    Err(error) => {
                        release(instance, object);
                        return Err(error);
                    }
                }
            }
            "WITH_EXCEPT_START" => {
                // 异常出口（实测骨架）：`PUSH_EXC_INFO; WITH_EXCEPT_START; TO_BOOL;
                // POP_JUMP_IF_TRUE L1; NOT_TAKEN; RERAISE 2; POP_TOP; POP_EXCEPT; POP_TOP×3; …`
                // 净栈效应 **+1**：以 `(类型, 异常, traceback)` 调 `__exit__`，**只压结果**，
                // 栈上原有的 `[self, 可调用, 异常, 上一个异常]` 一个都不动
                // （`RERAISE` 与抑制分支都要用它们）。
                // 栈（自顶向下）：**异常**、`prev`、`self`、**可调用**
                // —— `PUSH_EXC_INFO` 在本层压的是 `(prev, exc)`（`exc` 在 TOS，与 `handlers.rs`
                // 里 `CHECK_EXC_MATCH` 的取项一致）；`__exit__` 那一份在 `self` 的**下面**
                // （参照实现的文档说"调用栈上**第 4 项**"，第 4 项就是可调用）。
                // 栈（自顶向下）：**异常、prev、lasti、self、可调用**——`with` 的异常表条目
                // **带 `lasti`**（实测参照的 `depth<<1|lasti` 低位是 1），派发时压了那个偏移，
                // 而 `PUSH_EXC_INFO` 又把 `prev` 插在它上面 ⇒ `self`／可调用要再往下两格
                // （第 231 轮实测修正：原来按 3／4 取，`__exit__` 根本调不到）
                let exception = frame.get().peek()?;
                let prev = frame.get().peek_from_top(2)?;
                let lasti = frame.get().peek_from_top(3)?;
                let self_object = frame.get().peek_from_top(4)?;
                let callable = frame.get().peek_from_top(5)?;
                let _ = lasti;
                let exception_type = unsafe { exception.as_ref() }.ty();
                // `__exit__(type, exc, tb)`：`tb` 本层给 `None`（`__traceback__` 尚无对象，`DIV-6`）
                let mut arguments: Vec<NonNull<Header>> = Vec::with_capacity(3);
                unsafe {
                    instance.incref_object(exception_type.cast::<Header>().as_ptr());
                    instance.incref_object(exception.as_ptr());
                }
                arguments.push(exception_type.cast::<Header>());
                arguments.push(exception);
                arguments.push(instance.new_none());
                let result = call_callable(
                    instance,
                    callable,
                    Some(self_object),
                    arguments,
                    Vec::new(),
                    opcode_number,
                )?;
                push(instance, frame.get(), result)?;
                release(instance, result);
                let _ = prev;
            }
            "RAISE_VARARGS" => {
                // 参照实现：0 ＝ 重抛当前异常、1 ＝ `raise X`、2 ＝ `raise X from Y`（Y 在 TOS）
                return match oparg {
                    0 => {
                        // BC-60 ②：当前异常按**实例**存
                        let current = instance.current_exception().ok_or_else(|| {
                            raise_builtin(
                                instance,
                                "RuntimeError",
                                "No active exception to reraise",
                            )
                        })?;
                        // SAFETY: current 由本实例的异常状态持有，存活。
                        unsafe { instance.incref_object(current.as_ptr()) };
                        Err(raise(instance, current))
                    }
                    1 | 2 => {
                        let cause = if oparg == 2 {
                            Some(frame.get().pop()?)
                        } else {
                            None
                        };
                        let operand = frame.get().pop()?;
                        // SAFETY: operand 在帧值栈上，存活。
                        let operand_type = unsafe { operand.as_ref() }.ty();

                        let exception = if is_exception_type(instance, operand_type) {
                            operand // 已经是异常实例
                        } else if operand_type == builtin_type(instance, "type") {
                            // 是类型对象：必须是异常类，实例化它（`raise ValueError`）
                            let class = operand.cast::<TypeObject>();
                            if !is_exception_type(instance, class) {
                                release(instance, operand);
                                if let Some(cause) = cause {
                                    release(instance, cause);
                                }
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    "exceptions must derive from BaseException",
                                ));
                            }
                            release(instance, operand);
                            let object = instance.alloc(ExceptionObject::new(
                                class,
                                RefCell::new(Vec::new()),
                                RefCell::new(None),
                                RefCell::new(None),
                                Cell::new(false),
                                RefCell::new(None),
                            ));
                            object.into_raw().cast::<Header>()
                        } else {
                            release(instance, operand);
                            if let Some(cause) = cause {
                                release(instance, cause);
                            }
                            return Err(raise_builtin(
                                instance,
                                "TypeError",
                                "exceptions must derive from BaseException",
                            ));
                        };

                        // SAFETY: exception 是刚拿到的新引用，存活。
                        let object = unsafe { &*exception.as_ptr().cast::<ExceptionObject>() };

                        // 隐式上下文 ＝ 当前正在处理的异常（参照实现始终设，展示与否看抑制位）
                        if let Some(current) = instance.current_exception() {
                            // SAFETY: current 由实例的异常状态持有。
                            unsafe { instance.incref_object(current.as_ptr()) };
                            if let Some(old) = object.set_context(Some(current)) {
                                release(instance, old);
                            }
                        }

                        if let Some(cause) = cause {
                            // SAFETY: cause 是刚出栈的新引用。
                            let cause_type = unsafe { cause.as_ref() }.ty();
                            // 起因可以是**异常实例**，也可以是**异常类**（实测 `raise ValueError from TypeError` 合法）
                            let cause_is_class = cause_type == builtin_type(instance, "type")
                                && is_exception_type(instance, cause.cast::<TypeObject>());
                            if cause_type == instance.singletons().none_type() {
                                // `raise X from None`：抑制上下文，但没有 __cause__
                                release(instance, cause);
                            } else if is_exception_type(instance, cause_type) {
                                if let Some(old) = object.set_cause(Some(cause)) {
                                    release(instance, old);
                                }
                            } else if cause_is_class {
                                // 实测：起因是**类**时，参照实现会**实例化**它（`__cause__` 是 `TypeError()`），
                                // 不是把类本身存进去
                                let class = cause.cast::<TypeObject>();
                                release(instance, cause);
                                let created = instance.alloc(ExceptionObject::new(
                                    class,
                                    RefCell::new(Vec::new()),
                                    RefCell::new(None),
                                    RefCell::new(None),
                                    Cell::new(false),
                                    RefCell::new(None),
                                ));
                                let created = created.into_raw().cast::<Header>();
                                if let Some(old) = object.set_cause(Some(created)) {
                                    release(instance, old);
                                }
                            } else {
                                release(instance, cause);
                                release(instance, exception);
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    "exception causes must derive from BaseException",
                                ));
                            }
                            object.set_suppress_context(true);
                        }

                                                // **挂 `traceback`** ✓（第 213 轮：`BC-60` 的最小起步 ✓）—— `tb_frame` ＝ 抛出处的帧 ✓。
                        let traceback = instance.new_traceback(frame.as_ptr().cast::<Header>());
                        // **尽力而为** ✓（异常类型若没有实例字典，如实不挂 ✓）。
                        let _ = instance.set_attribute_value(exception, "__traceback__", traceback);
                        release(instance, traceback);
Err(raise(instance, exception))
                    }
                    _ => Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "RAISE_VARARGS 的 oparg 只能是 0／1／2",
                    }),
                };
            }
            "PUSH_NULL" => {
                // CALL 的"没有 self"槽位（参照实现在栈上放 NULL 指针，这里放内部哨兵）
                push(instance, frame.get(), instance.singletons().null())?;
            }
            "MAKE_FUNCTION" => {
                // 实测：MAKE_FUNCTION **只**吃 code 对象；默认值随后由 SET_FUNCTION_ATTRIBUTE 挂
                let code_header = frame.get().pop()?;
                // SAFETY: code_header 是本实例的存活对象（由常量表持有）。
                let code_type_ok = unsafe { code_header.as_ref() }.ty();
                let code_object_type = instance
                    .type_named("CodeObject")
                    .map(|ty| ty)
                    .unwrap_or_else(|| builtin_type(instance, "object"));
                if code_type_ok != code_object_type {
                    release(instance, code_header);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAKE_FUNCTION 只接受 code object（闭包与注解随后补）",
                    });
                }
                // **`__globals__`**：函数"记住"定义处的全局映射（`BC-57` 的 `LOAD_GLOBAL` 要它）。
                // 模块体没有单独的一层 ⇒ 取命名空间（`effective_globals`）。
                let captured = frame.get().effective_globals();
                if let Some(mapping) = captured {
                    // SAFETY: 映射由帧持有，函数要自己那份。
                    unsafe { instance.incref_object(mapping.as_ptr()) };
                }
                let object = instance.alloc(FunctionObject::new(
                    builtin_type(instance, "function"),
                    code_header,
                    Vec::new(),
                    None,
                    RefCell::new(captured),
                    RefCell::new(Vec::new()),
                    RefCell::new(None),
            core::cell::RefCell::new(None)));
                frame.get().push(object.into_raw().cast::<Header>())?;
            }
            "SET_FUNCTION_ATTRIBUTE" => {
                // 实测：栈是 [属性值, 函数]，**函数在 TOS**；挂完把函数留在栈上
                let function = frame.get().pop()?;
                let attribute = frame.get().pop()?;
                // SAFETY: function 是帧值栈上的存活对象。
                if unsafe { function.as_ref() }.ty() != builtin_type(instance, "function") {
                    release(instance, attribute);
                    release(instance, function);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "SET_FUNCTION_ATTRIBUTE 只接线了函数对象",
                    });
                }
                // SAFETY: 类型身份已确认。
                let object = unsafe { &mut *function.as_ptr().cast::<FunctionObject>() };
                match oparg {
                    1 => {
                        // defaults：一个 tuple（实测）
                        let items = sequence_items(instance, attribute, opcode_number);
                        release(instance, attribute);
                        object.set_defaults(items?);
                    }
                    2 => {
                        // kwdefaults：一个 dict（实测）
                        if unsafe { attribute.as_ref() }.ty() != builtin_type(instance, "dict") {
                            release(instance, attribute);
                            release(instance, function);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "kwdefaults 必须是 dict",
                            });
                        }
                        if let Some(old) = object.set_kwdefaults(Some(attribute)) {
                            release(instance, old);
                        }
                    }
                    16 => {
                        // **bit4 `annotate`**（3.14 的延迟注解协议，`SPEC-bytecode.md` 的属性位表）：
                        // 值是那个"按 `format` 产出注解字典"的**可调用对象**，挂在函数上
                        if let Some(old) = object.set_annotate(Some(attribute)) {
                            release(instance, old);
                        }
                    }
                    8 => {
                        // **bit3 `closure`**：值是 **cell 元组**（`BUILD_TUPLE n` 造的），
                        // 建帧时装进自由槽（`SPEC-bytecode.md` 的属性位表）
                        let items = sequence_items(instance, attribute, opcode_number);
                        release(instance, attribute);
                        match items {
                            Ok(items) => {
                                for value in object.set_closure(items) {
                                    release(instance, value);
                                }
                            }
                            Err(error) => {
                                release(instance, function);
                                return Err(error);
                            }
                        }
                    }
                    _ => {
                        release(instance, attribute);
                        release(instance, function);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "SET_FUNCTION_ATTRIBUTE 只接线了 defaults(1)／kwdefaults(2)／closure(8)／annotate(16)",
                        });
                    }
                }
                frame.get().push(function)?;
            }
            "IMPORT_NAME" => {
                // 栈：`[level, fromlist]`（编译器先压两项 ✓）；`level > 0` 是**相对导入**（第 278 轮 ✓）。
                let fromlist = frame.get().pop()?;
                let level = frame.get().pop()?;
                let level_value = instance.int_value(level);
                release(instance, level);
                // `fromlist` **留到装载之后再放** ✓（第 279 轮：`from . import 子模块` 那一步要用它 ✓）
                let raw = code
                    .name_at(oparg as usize)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                // **相对导入** ✓（第 278 轮接线）：`level > 0` 时按当前模块的**包上下文**把名字解析成
                // **绝对名** ✓ ⇒ 之后**只走下面这一条路** ✓（不复制第二套查找逻辑 ✓）。
                let full = if level_value.unwrap_or(0) > 0 {
                    resolve_relative_import(
                        instance,
                        frame.get(),
                        &raw,
                        level_value.unwrap_or(0) as usize,
                        opcode_number,
                    )?
                } else {
                    raw
                };
                // `import a.b.c` 交出的是**顶层模块**（随后 `STORE_NAME a` ✓，照参照实测）
                let top = full.split('.').next().unwrap_or(full.as_str()).to_owned();
                let modules = instance.modules().ok_or(ExecError::Unsupported {
                    opcode: opcode_number,
                    what: "模块表未装配（import 的加载器未接：`P3-12`）",
                })?;
                // **`sys.modules` 里已有"整条带点名字"⇒ 直接用它** ✓（第 203 轮真 bug 修复 ✗）：
                // `Lib/os.py:103` 正是 `sys.modules['os.path'] = path` ✓、104 再 `from os.path import …` ✓
                // ⇒ 先前**先**去加载顶层 `os` ✗ —— 而 `os` 是**模块**不是包 ✗ ⇒ 报 `No module named 'os'`-ish ✗
                //（实测最小复现：`sys.modules['demo.sub'] = itertools` 之后 `from demo.sub import count` ✗，
                //  参照成功 ✓）。⇒ 与 CPython 同序 ✓：**先查模块表** ✓。
                let module = match instance.dict_get(modules, &full) {
                    Some(found) => found,
                    None => {
                        let loaded = match instance.dict_get(modules, &top) {
                            Some(found) => found,
                            // **加载器**（`IM-` 最小面）：按 `sys.path` 经 `fs` 域读 `<dir>/<名字>.py` ✓
                            None => load_module(instance, modules, &top, opcode_number)?,
                        };
                        // **带点名字要把整条链都导入**（第 135 轮）：`import a.b` 之后 `a.b` 必须可见 ✓
                        // （参照语义 ✓；`load_module` 会在父包的 `__path__` 里找子模块并挂成属性 ✓）。
                        // 顶层仍然交出（随后 `STORE_NAME a` ✓，与参照实测一致 ✓）。
                        if full != top && instance.dict_get(modules, &full).is_none() {
                            load_module(instance, modules, &full, opcode_number)?;
                        }
                        loaded
                    }
                };
                // **`fromlist`：把"名字"当子模块载入** ✓（第 279 轮；参照的 `_handle_fromlist` ✓）
                // —— `from . import _bootstrap` 就是靠这一步让 `importlib` 一族 4 个文件过线的 ✓。
                // 模块名取**模块自己的 `__name__`** ✓（照参照 ✓；`sys.modules['os.path'] = posixpath`
                // 那一类里，`full` 与模块真名**可以不同** ✗）。
                let own_name =
                    module_text(instance, module, "__name__").unwrap_or_else(|| full.clone());
                let handled = handle_fromlist(
                    instance,
                    modules,
                    module,
                    fromlist,
                    &own_name,
                    opcode_number,
                );
                release(instance, fromlist);
                handled?;
                // 交出一份**新引用**（`dict_get` 是借出 ✓）
                // SAFETY: module 由模块表持有，活到实例销毁。
                unsafe { instance.incref_object(module.as_ptr()) };
                frame.get().push(module)?;
            }
            "IMPORT_FROM" => {
                let name = code
                    .name_at(oparg as usize)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let module = frame.get().pop()?;
                let found = attribute_lookup(instance, module, &name);
                match found {
                    Ok(Attribute::Owned(value)) => {
                        frame.get().push(module)?;
                        frame.get().push(value)?;
                    }
                    Ok(Attribute::Value(value)) => {
                        frame.get().push(module)?;
                        push(instance, frame.get(), value)?;
                    }
                    Ok(Attribute::Method { function, this }) => {
                        // 模块属性理论上不会是方法 ✗，但**不吞错**：照 `LOAD_ATTR` 的取方法形态绑 ✓
                        // SAFETY: function／this 都还活着（类型字典与调用方持有）。
                        unsafe {
                            instance.incref_object(function.as_ptr());
                            instance.incref_object(this.as_ptr());
                        }
                        let bound = instance.alloc(crate::builtin_objects::MethodObject::new(
                            instance
                                .type_named("method")
                                .expect("`method` 类型已登记"),
                            function,
                            this,
                        ));
                        frame.get().push(module)?;
                        frame.get().push(bound.into_raw().cast::<Header>())?;
                    }
                    Err(error) => {
                        // **`from M import 缺名` 要报 `ImportError`** ✓（第 288 轮）：照参照实测
                        // `cannot import name 'x' from 'm'` ✓ —— 上游 `Lib/io.py:93` 的
                        // `try: from _io import _WindowsConsoleIO / except ImportError: pass` 正是靠它 ✓；
                        // 先前直接抛 `AttributeError` ✗ ⇒ 那个 `try` **接不住** ✗ ⇒ 整个 `io` 导入失败 ✓。
                        let attribute_error = builtin_type(instance, "AttributeError");
                        let is_missing = match &error {
                            ExecError::Raised { exception } => {
                                // SAFETY: 抛出的异常由实例持有，存活。
                                unsafe { exception.as_ref() }.ty() == attribute_error
                            }
                            _ => false,
                        };
                        if is_missing {
                            // 模块名：从模块命名空间里借读 `__name__` ✓（借用 ⇒ 不还引用 ✓）
                            let module_name = crate::executor::module_text(instance, module, "__name__")
                                .unwrap_or_else(|| "?".to_owned());
                            let converted = raise_builtin(
                                instance,
                                "ImportError",
                                &format!("cannot import name '{name}' from '{module_name}'"),
                            );
                            release(instance, module);
                            return Err(converted);
                        }
                        release(instance, module);
                        return Err(error);
                    }
                }
            }
            "LOAD_ATTR" => {
                // 实测：名字下标 ＝ `oparg >> 1`，**低位 ＝ 取方法**（`dis` 的 argrepr 显示 `+ NULL|self`）
                let name = code
                    .name_at(oparg >> 1)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let method_flag = oparg & 1 != 0;
                let object = frame.get().pop()?;
                let found = attribute_lookup(instance, object, &name);
                match found {
                    Ok(Attribute::Owned(value)) => {
                        // 槽位交出的就是新引用 ⇒ 直接压栈，不再 incref
                        frame.get().push(value)?;
                        if method_flag {
                            push(instance, frame.get(), instance.singletons().null())?;
                        }
                    }
                    Ok(Attribute::Value(value)) => {
                        push(instance, frame.get(), value)?;
                        if method_flag {
                            // 取方法形态对非方法值也要补一个 NULL 槽，好让 CALL 统一处理
                            push(instance, frame.get(), instance.singletons().null())?;
                        }
                    }
                    Ok(Attribute::Method { function, this }) => {
                        if method_flag {
                            // 编译器的取方法位：栈上给"函数 ＋ self"，CALL 直接按 [可调用, self] 处理
                            push(instance, frame.get(), function)?;
                            push(instance, frame.get(), this)?;
                        } else {
                            // `obj.method`（**不调用**）：产出一个**绑定方法对象**
                            // SAFETY: function／this 都还活着（由类型字典与调用方持有）。
                            unsafe {
                                instance.incref_object(function.as_ptr());
                                instance.incref_object(this.as_ptr());
                            }
                            let bound = instance.alloc(MethodObject::new(
                                builtin_type(instance, "method"),
                                function,
                                this,
                            ));
                            frame.get().push(bound.into_raw().cast::<Header>())?;
                        }
                    }
                    Err(error) => {
                        release(instance, object);
                        return Err(error);
                    }
                }
                release(instance, object);
            }
            "STORE_ATTR" => {
                // `BC-57`：**只有** `LOAD_GLOBAL`／`LOAD_ATTR`／`LOAD_SUPER_ATTR` 移位——
                // `STORE_ATTR` 的名字下标就是 `oparg` 本身（实测下标 4 的 `obj.epsilon` 给 4）。
                // 栈是 `[值, 对象]`（**对象在 TOS**）
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let object = frame.get().pop()?;
                let value = frame.get().pop()?;
                let outcome =
                    instance_attribute_set(instance, object, &name, value, opcode_number);
                release(instance, object);
                outcome?;
            }
            "DELETE_ATTR" => {
                // 实测：名字下标 ＝ `oparg`（**不移位**）；栈是 `[对象]`
                let name = code
                    .name_at(oparg)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "co_names 下标越界",
                    })?
                    .to_owned();
                let object = frame.get().pop()?;
                let outcome = instance_attribute_delete(instance, object, &name);
                release(instance, object);
                outcome?;
            }
            "CALL" | "CALL_KW" => {
                // 实测：`[可调用, NULL|self, 位置实参…]`；`CALL_KW` 另把**关键字名元组**放在 TOS
                let names = if name == "CALL_KW" {
                    Some(frame.get().pop()?)
                } else {
                    None
                };
                let keyword_count = match names {
                    Some(names) => {
                        // SAFETY: names 是帧值栈上的存活对象。
                        if unsafe { names.as_ref() }.ty() != builtin_type(instance, "tuple") {
                            release(instance, names);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "CALL_KW 的关键字名表必须是 tuple",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        unsafe { &*names.as_ptr().cast::<TupleObject>() }.len()
                    }
                    None => 0,
                };

                let mut keywords = Vec::with_capacity(keyword_count);
                for _ in 0..keyword_count {
                    keywords.push(frame.get().pop()?);
                }
                keywords.reverse();

                let positional_count = oparg.saturating_sub(keyword_count);
                let mut args = Vec::with_capacity(positional_count);
                for _ in 0..positional_count {
                    args.push(frame.get().pop()?);
                }
                args.reverse();

                let self_or_null = frame.get().pop()?;
                let callable = frame.get().pop()?;

                let bound_self = if self_or_null == instance.singletons().null() {
                    release(instance, self_or_null);
                    None
                } else {
                    Some(self_or_null)
                };

                let mut kwargs = Vec::with_capacity(keyword_count);
                if let Some(names) = names {
                    // SAFETY: 上面确认过它是 tuple。
                    let table = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                    for (index, value) in keywords.into_iter().enumerate() {
                        let key = match table.item(index) {
                            Some(key) => key,
                            None => {
                                release(instance, value);
                                release(instance, names);
                                release(instance, callable);
                                for value in args {
                                    release(instance, value);
                                }
                                if let Some(self_object) = bound_self {
                                    release(instance, self_object);
                                }
                                for (key, value) in kwargs {
                                    release(instance, key);
                                    release(instance, value);
                                }
                                return Err(ExecError::Unsupported {
                                    opcode: opcode_number,
                                    what: "CALL_KW 的关键字个数与名表长度不符",
                                });
                            }
                        };
                        // SAFETY: key 由元组持有，存活。
                        unsafe { instance.incref_object(key.as_ptr()) };
                        kwargs.push((key, value));
                    }
                    release(instance, names);
                }

                let result = call_callable(
                    instance,
                    callable,
                    bound_self,
                    args,
                    kwargs,
                    opcode_number,
                );
                release(instance, callable);
                frame.get().push(result?)?;
            }
            "IS_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;
                // OM-39：`is` 就是对象身份——栈上放的是真对象，直接比指针
                let identical = left == right;
                release(instance, left);
                release(instance, right);
                let truth = if oparg == 0 { identical } else { !identical };
                let raw = instance.singletons().boolean(truth);
                push(instance, frame.get(), raw)?;
            }
            "COMPARE_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;
                // **通用比较**（`TS-40`）：`int`／`bool`／`str` 按值；其余报实测的 `TypeError`。
                // 整数路径与从前一致（同一套 `partial_cmp`），只是不再把非整数当成"未接线"。

                // `BC-39`／`BC-58`：cmp 下标 ＝ `oparg >> 5`；bit 4（`& 16`）是 `bool(...)` 标志，
                // 低 4 位是参照实现的编译期信息（`dis` 不读、本层**不解释**但**必须容受**）
                let operator = opcode::get_cmp_op().get(oparg >> 5).copied().ok_or(
                    ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "COMPARE_OP 的 cmp 下标（oparg >> 5）超出 cmp_op 的六元组（BC-39／BC-58）",
                    },
                )?;
                let truth = compare_public(instance, left, right, operator, opcode_number);
                release(instance, left);
                release(instance, right);
                let raw = instance.singletons().boolean(truth?);
                push(instance, frame.get(), raw)?;
            }
            "BINARY_OP" => {
                let right = frame.get().pop()?;
                let left = frame.get().pop()?;

                // BC-39：oparg 对应 `get_nb_ops()` 的顺序；BC-50：名字从表里取，不写死
                let name = opcode::get_nb_ops()
                    .get(oparg)
                    .map(|(name, _)| *name)
                    .ok_or(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "BINARY_OP 的 oparg 超出 get_nb_ops() 的范围（BC-39）",
                    })?;

                if name == "NB_SUBSCR" {
                    // 下标读：`[容器, 键]`（实测）
                    let result = subscript_get(instance, left, right, opcode_number);
                    release(instance, left);
                    release(instance, right);
                    frame.get().push(result?)?;
                } else {
                    // **一处真相**：二元运算走公开入口（`P1-11` 的任意精度核心 ＋ 参照实测的消息），
                    // 不再在这里用 `i64` ＋ "结果必须落在单例区间"那套旧假设
                    let result = match name {
                        // `+` 走 concat：`str`／`bytes`／`list`／`tuple` 拼接，其余落到算术
                        "NB_ADD" => concat_public(instance, left, right, opcode_number),
                        "NB_SUBTRACT" => arithmetic_public(instance, left, right, "-", opcode_number),
                        "NB_MULTIPLY" => arithmetic_public(instance, left, right, "*", opcode_number),
                        "NB_FLOOR_DIVIDE" => {
                            arithmetic_public(instance, left, right, "//", opcode_number)
                        }
                        "NB_REMAINDER" => arithmetic_public(instance, left, right, "%", opcode_number),
                        // 真除法：结果是 **float**（`arithmetic_public` 的 `/` 分支）
                        "NB_TRUE_DIVIDE" => {
                            arithmetic_public(instance, left, right, "/", opcode_number)
                        }
                        "NB_POWER" => arithmetic_public(instance, left, right, "**", opcode_number),
                        "NB_AND" => arithmetic_public(instance, left, right, "&", opcode_number),
                        "NB_OR" if instance.is_type_object(left) && instance.is_type_object(right) =>
                            // **类型的 `|`** ✓（第 214 轮）：`int | str` ⇒ 联合类型 ✓（`Lib/types.py` 的 `UnionType` ✓）。
                            Ok(instance.new_union_type(left, right)),
                        "NB_OR" => arithmetic_public(instance, left, right, "|", opcode_number),
                        "NB_XOR" => arithmetic_public(instance, left, right, "^", opcode_number),
                        // `@`（矩阵乘）：`arithmetic_public` 对 `@` 一律如实报参照实测的 `TypeError`
                        "NB_MATRIX_MULTIPLY" => {
                            arithmetic_public(instance, left, right, "@", opcode_number)
                        }
                        "NB_LSHIFT" => arithmetic_public(instance, left, right, "<<", opcode_number),
                        "NB_RSHIFT" => arithmetic_public(instance, left, right, ">>", opcode_number),
                        // **增强赋值**（`+=` 一族，`NB_INPLACE_*`）：不可变类型（`int`／`bool`／
                        // `float`／`str`／`bytes`／`tuple`）的"就地"就是基运算 + 重新绑定 ⇒ 直接
                        // 落基运算；**可变容器**里 `list` 的 `+=` 是**就地 extend**（别名可见，
                        // 实测）⇒ 单独走；`set`／`dict` 的就地运算**尚未接线**（如实报，不悄悄
                        // 换成"重新绑定"——那会与参照的可观察行为不同）
                        "NB_INPLACE_ADD" => inplace_add(instance, left, right, opcode_number),
                        "NB_INPLACE_SUBTRACT" => {
                            inplace_arithmetic(instance, left, right, "-", opcode_number)
                        }
                        "NB_INPLACE_MULTIPLY" => {
                            inplace_arithmetic(instance, left, right, "*", opcode_number)
                        }
                        "NB_INPLACE_TRUE_DIVIDE" => {
                            inplace_arithmetic(instance, left, right, "/", opcode_number)
                        }
                        "NB_INPLACE_FLOOR_DIVIDE" => {
                            inplace_arithmetic(instance, left, right, "//", opcode_number)
                        }
                        "NB_INPLACE_REMAINDER" => {
                            inplace_arithmetic(instance, left, right, "%", opcode_number)
                        }
                        "NB_INPLACE_POWER" => {
                            inplace_arithmetic(instance, left, right, "**", opcode_number)
                        }
                        "NB_INPLACE_LSHIFT" => {
                            inplace_arithmetic(instance, left, right, "<<", opcode_number)
                        }
                        "NB_INPLACE_RSHIFT" => {
                            inplace_arithmetic(instance, left, right, ">>", opcode_number)
                        }
                        "NB_INPLACE_AND" => {
                            inplace_arithmetic(instance, left, right, "&", opcode_number)
                        }
                        "NB_INPLACE_XOR" => {
                            inplace_arithmetic(instance, left, right, "^", opcode_number)
                        }
                        "NB_INPLACE_OR" => {
                            inplace_arithmetic(instance, left, right, "|", opcode_number)
                        }
                        "NB_INPLACE_MATRIX_MULTIPLY" => {
                            inplace_arithmetic(instance, left, right, "@", opcode_number)
                        }
                        _ => Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "该 NB_* 运算尚未接线（矩阵乘 `@`／`@=` 随后补）",
                        }),
                    };
                    release(instance, left);
                    release(instance, right);
                    frame.get().push(result?)?;
                }
            }
            "STORE_SUBSCR" => {
                // 实测：`[值, 容器, 键]`，键在 TOS
                let key = frame.get().pop()?;
                let container = frame.get().pop()?;
                let value = frame.get().pop()?;
                let outcome = subscript_set(instance, container, key, value, opcode_number);
                release(instance, container);
                release(instance, key);
                outcome?;
            }
            "DELETE_SUBSCR" => {
                // 实测：`[容器, 键]`
                let key = frame.get().pop()?;
                let container = frame.get().pop()?;
                let outcome = subscript_del(instance, container, key, opcode_number);
                release(instance, container);
                release(instance, key);
                outcome?;
            }
            "RETURN_VALUE" => {
                return Ok(Step::Return(value_from_raw(instance, frame.get().pop()?)));
            }
            "YIELD_VALUE" => {
                // 实测骨架：`YIELD_VALUE` 之后是 `RESUME`／`POP_TOP`。让出时把值栈交给
                // 帧的恢复点（`BC-47`），并让 ip 指向**下一条**指令——恢复就从那里继续。
                let value = frame.get().pop()?;
                frame
                    .get()
                    .set_instruction_pointer(instruction.offset + instruction.size);
                frame.get().suspend()?;
                return Ok(Step::Yield(value));
            }
            "RETURN_GENERATOR" => {
                // 本层的 `CALL` 在见到 `CO_GENERATOR` 时**已经**把帧包成生成器了，
                // 所以这条指令在恢复执行时是空操作（它只负责"造并返回生成器"那一半）。
            }
            _ => return Err(ExecError::NotImplemented { opcode: opcode_number }),
        }
        Ok(Step::Continue)
        })();

        match outcome {
            Ok(Step::Return(value)) => return Ok(ExecOutcome::Returned(value)),
            Ok(Step::Yield(value)) => return Ok(ExecOutcome::Yielded(value)),
            Ok(Step::Continue) => {}
            Err(ExecError::Raised { exception }) => {
                // BC-60 ①：按异常表回退值栈到 `depth`、按 `lasti` 压最后一条指令偏移、
                // 压异常实例、跳到处理块入口（实测：入口就是 `PUSH_EXC_INFO` 那条指令）。
                dispatch_raise(
                    instance,
                    frame.get(),
                    code.exceptiontable(),
                    instruction.offset * 2,
                    exception,
                    &mut decoder,
                )?;
            }
            Err(other) => return Err(other),
        }
    }
    Err(ExecError::FellOffEnd)
}
