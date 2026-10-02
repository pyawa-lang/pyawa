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
//! 整数的**值域**：结果必须落在单例区间内；超出一律 [`ExecError::IntOutOfRange`]——
//! 大整数对象随 `SPEC-type-system.md` 落地，**禁止**在这里悄悄回绕。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use crate::code::CodeObject;
use crate::decode::{parse_exception_table, DecodeError, Decoder};
use crate::frame::{Frame, FrameError};
use crate::header::Header;
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::opcode;
use crate::refcount::{Owned, PyRef};
use crate::builtin_objects::{
    AsendObject, AttributeObject, BoolObject, BuiltinFunctionObject, ExceptionObject, GeneratorObject,
    IteratorObject, MethodObject, DictObject, FloatObject, FunctionObject, IntObject, ListObject, SetObject, StrObject,
    TupleObject,
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
    /// 结果超出本层能表示的范围：需要大整数对象。
    IntOutOfRange { value: i64 },

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

/// 把一个小整数压栈（内部表示是单例，`OM-23`／`OM-39`）。
fn push_small_int(instance: &Instance, frame: &Frame, value: i64) -> Result<(), ExecError> {
    let raw = instance
        .singletons()
        .small_int(value)
        .ok_or(ExecError::IntOutOfRange { value })?;
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
    if ty == builtin_type(instance, "islice") {
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
            None => {
                let left = as_int(instance, previous, opcode);
                let right = as_int(instance, item, opcode);
                match (left, right) {
                    (Ok(left), Ok(right)) => match binary_op("NB_ADD", left, right) {
                        Ok(sum) => Ok(instance.new_int(sum)),
                        Err(error) => Err(error),
                    },
                    (Err(_), _) | (_, Err(_)) => Err(ExecError::Unsupported {
                        opcode,
                        what: "accumulate 无 func 时只做**整数**加法（与 BINARY_OP 同口径）；                               其它类型的 `+` 尚未接线",
                    }),
                }
            }
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
fn truthiness(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<bool, ExecError> {
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
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value != 0);
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "真假判定只接线了 None／bool／int（__bool__ 协议未接线）",
    })
}

/// 取出整数载荷——*临时*按类型身份判定（见本模块顶部"临时口径"）。
///
/// **`TS-40`**：`bool ⊂ int`，所以 `True`／`False` 在这里按 0／1 参与运算；
/// 但**两种载荷的布局不同**，必须分开读，不能互相强转。
fn as_int(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<i64, ExecError> {
    // SAFETY: raw 是帧值栈上的存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.int_type() {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 同上。
        return Ok(i64::from(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value));
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "整数运算只接线了 int 与 bool（数值塔的其余类型与协议槽位未接线）",
    })
}

/// 把小整数结果压栈，越界即报错（**不**回绕）。
fn push_int_result(instance: &Instance, frame: &Frame, value: i64) -> Result<(), ExecError> {
    if !(SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
        return Err(ExecError::IntOutOfRange { value });
    }
    push_small_int(instance, frame, value)
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
        let value = unsafe { &*raw.as_ptr().cast::<IntObject>() }.value;
        if (SMALL_INT_MIN..=SMALL_INT_MAX).contains(&value) {
            release(instance, raw); // 单例由实例持有，交回我们这份即可
            return Value::small_int(value);
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

/// 整数载荷（int 与 bool 两种布局分开读，`TS-40`）。
fn integer_payload(instance: &Instance, raw: NonNull<Header>) -> Option<i64> {
    // SAFETY: 调用方保证 raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.int_type() {
        // SAFETY: 类型身份已确认。
        return Some(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 同上。
        return Some(i64::from(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value));
    }
    None
}

/// 数值载荷（`int`／`bool`／`float`）。
fn numeric_payload(instance: &Instance, raw: NonNull<Header>) -> Option<f64> {
    if let Some(value) = integer_payload(instance, raw) {
        return Some(value as f64);
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
        return a == b;
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

/// 下标**读**（`BINARY_OP` ＋ `NB_SUBSCR`，3.14 无 `BINARY_SUBSCR`）。返回**新引用**。
fn subscript_get(
    instance: &Instance,
    container: NonNull<Header>,
    key: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: container 与 key 都是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();

    if container_type == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
        })?;
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
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
        })?;
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
    let str_type = instance.singletons().str_type();
    if container_type == str_type {
        // SAFETY: 同上。
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value().to_owned();
        let characters: Vec<char> = text.chars().collect();
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
        })?;
        let position = match normalize_index(index, characters.len()) {
            Some(position) => position,
            None => {
                return Err(raise_builtin(instance, "IndexError", "string index out of range"))
            }
        };
        let object = instance.alloc(StrObject::new(str_type, characters[position].to_string()));
        return Ok(object.into_raw().cast::<Header>());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "下标只接线了 tuple／list／dict／str",
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
        let index = match integer_payload(instance, key) {
            Some(index) => index,
            None => {
                release(instance, value);
                return Err(ExecError::Unsupported {
                    opcode,
                    what: "下标必须是整数（切片要 M3+ 的 slice 类型）",
                });
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
        let index = integer_payload(instance, key).ok_or(ExecError::Unsupported {
            opcode,
            what: "下标必须是整数",
        })?;
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
const ITERATOR_TYPE_NAMES: [&str; 23] = [
    "tuple_iterator",
    "list_iterator",
    "str_ascii_iterator",
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
    if ty == builtin_type(instance, "set") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<SetObject>() }.len());
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().chars().count());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: "只接线了 tuple／list／dict／set／str 的内建迭代器（其余走 __iter__ 协议）",
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
    if ty == builtin_type(instance, "set") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<SetObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 同上。
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
    Err(ExecError::Unsupported {
        opcode,
        what: "只接线了 tuple／list／dict／set／str 的迭代",
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
    } else if ty == builtin_type(instance, "set") {
        "set_iterator"
    } else if ty == instance.singletons().str_type() {
        "str_ascii_iterator"
    } else {
        return Err(ExecError::Unsupported {
            opcode: opcode_of("GET_ITER"),
            what: "只接线了 tuple／list／dict／set／str 的内建迭代器（其余走 __iter__ 协议）",
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
    if container_type == builtin_type(instance, "dict") || container_type == builtin_type(instance, "set")
    {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        for (key, _) in object.entries() {
            if values_equal(instance, key, item) {
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
fn instance_attributes(_instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
    // SAFETY: object 是存活对象。
    let header = unsafe { object.as_ref() };
    let ty = header.ty();
    // SAFETY: ty 由注册表持有。
    let type_object = unsafe { ty.as_ref() };
    if type_object.type_flags() & crate::HAS_INSTANCE_DICT == 0 {
        return None;
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
fn mounted_instance_dict(instance: &Instance, object: NonNull<Header>) -> Option<NonNull<Header>> {
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
fn instance_attribute_set(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    value: NonNull<Header>,
    opcode: u8,
) -> Result<(), ExecError> {
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

    // ①.5 **`__dict__`**（实测：实例上它就是**那个字典本身**——同一个对象、透过它加属性立刻可见；
    // 没有实例字典的类型则落到最后那条 `AttributeError`，实测形如
    // `'S' object has no attribute '__dict__'`）。
    if name == "__dict__" {
        if let Some(mapping) = mounted_instance_dict(instance, object) {
            // SAFETY: mapping 是存活对象，这里新增一份交给调用方。
            unsafe { instance.incref_object(mapping.as_ptr()) };
            return Ok(Attribute::Owned(mapping));
        }
    }

    // ② 实例字典
    if let Some(mapping) = instance_attributes(instance, object) {
        // SAFETY: mapping 是属性字典（dict）。
        let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
        let found = dict
            .entries()
            .into_iter()
            .find(|(key, _)| str_matches_public(instance, *key, name));
        if let Some((_, value)) = found {
            return Ok(Attribute::Value(value));
        }
    }

    // ③ 类型字典沿 MRO
    if let Some(found) = instance.type_lookup(object_type, name) {
        // SAFETY: found 由类型字典持有。
        if unsafe { found.as_ref() }.ty() == builtin_type(instance, "function") {
            return Ok(Attribute::Method {
                function: found,
                this: object,
            });
        }
        return Ok(Attribute::Value(found));
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

/// **通用比较**（`TS-40`）：`int`／`bool`／`str` **按值**比较，其余类型报**参照实测**的
/// `TypeError`（`'<' not supported between instances of 'int' and 'str'`）。
///
/// `operator` 模块的比较族与 `COMPARE_OP` **共用同一份实现**（两处各写一份就是两处真相）。
/// `symbol` 取 `"<"`／`"<="`／`"=="`／`"!="`／`">"`／`">="`（照 `cmp_op` 的名字）。
///
/// **浮点尚未接线**（本层浮点类型还在未落地清单里）⇒ 遇到浮点按"别的类型"处理（报实测消息形）。
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
    // 大小比较：两边都必须是**同一族**的标量（int／bool 一族、str 一族）
    let left_int = instance.int_value(left);
    let right_int = instance.int_value(right);
    let left_text = instance.text_value(left);
    let right_text = instance.text_value(right);
    let ordering = match (left_int, right_int, left_text, right_text) {
        (Some(a), Some(b), _, _) => a.partial_cmp(&b),
        (_, _, Some(a), Some(b)) => a.partial_cmp(&b),
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
/// 覆写抛异常时：**吞掉**并把异常记在实例上（顶层 `object_repr` 的签名没有异常通道，
/// 见 `lib.rs` 的清单），退回槽位路径——这与参照实现"异常向上传播"不同，属已知偏差。
pub(crate) fn override_text(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Option<String> {
    let ty = instance.type_of(object);
    let found = instance.type_lookup(ty, name)?;
    // 内建类型不注册这两个 dunder；真要是有，也照通道走
    let _ = found;
    match call_object_method(instance, object, name, &[]) {
        Ok(Some(result)) => {
            // SAFETY: result 是新引用，存活。
            let text = instance.text_value(result);
            release(instance, result);
            text
        }
        Ok(None) => None,
        Err(ExecError::Raised { exception }) => {
            let _ = instance.set_pending_exception(Some(exception));
            None
        }
        Err(_) => None,
    }
}

/// **`TS-44`**：元素的 `repr` —— 先走属性通道的 `__repr__`，没有才落到原生槽位／默认实现。
///
/// 容器载荷的 `repr` 槽用它（`repr([x])` 里的 `x` 也要尊重 Python 级覆写）。
pub(crate) fn element_repr(instance: &Instance, object: NonNull<Header>) -> String {
    match call_object_method(instance, object, "__repr__", &[]) {
        Ok(Some(result)) => {
            // SAFETY: result 是新引用，存活。
            let is_str = unsafe { result.as_ref() }.ty() == instance.singletons().str_type();
            let text = if is_str {
                // SAFETY: 类型身份已确认。
                Some(unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned())
            } else {
                None
            };
            release(instance, result);
            text.unwrap_or_else(|| instance.object_repr(object))
        }
        Ok(None) => instance.object_repr(object),
        // 覆写里抛了异常：容器 `repr` 没有异常通道，退回原生表示（并保持异常状态不变）
        Err(ExecError::Raised { exception }) => {
            let _ = instance.set_pending_exception(Some(exception));
            instance.object_repr(object)
        }
        Err(_) => instance.object_repr(object),
    }
}

/// **`TS-44`**：元素的 `str` —— 同上，走 `__str__`。
#[allow(dead_code)] // 容器 `str`（`str([x])`）接线时用它；现在只剩 `repr` 那条在用
pub(crate) fn element_str(instance: &Instance, object: NonNull<Header>) -> String {
    match call_object_method(instance, object, "__str__", &[]) {
        Ok(Some(result)) => {
            // SAFETY: result 是新引用，存活。
            let is_str = unsafe { result.as_ref() }.ty() == instance.singletons().str_type();
            let text = if is_str {
                // SAFETY: 类型身份已确认。
                Some(unsafe { &*result.as_ptr().cast::<StrObject>() }.value().to_owned())
            } else {
                None
            };
            release(instance, result);
            text.unwrap_or_else(|| instance.object_str(object))
        }
        Ok(None) => instance.object_str_native(object),
        Err(ExecError::Raised { exception }) => {
            let _ = instance.set_pending_exception(Some(exception));
            instance.object_str_native(object)
        }
        Err(_) => instance.object_str_native(object),
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
fn line_at_offset(code: &CodeObject, offset: usize) -> u32 {
    let mut decoder = crate::decode::Decoder::new(code.code());
    let mut ordinal = 0usize;
    while let Ok(Some(instruction)) = decoder.next_instruction() {
        if instruction.offset == offset {
            if let Some((line, _, _, _)) = code.positions().get(ordinal) {
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
    if !extra.is_empty() {
        if !code.has_varargs() {
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
        let varargs_slot = argcount + kwonly;
        // **OM-23**：没有多余实参时这个元组是空的 ⇒ 走单例
        locals[varargs_slot] = Some(instance.new_tuple(extra));
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
    let call_slot = unsafe { ty.as_ref() }.slots().call;
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
        || ty == builtin_type(instance, "type")
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
    if unsafe { callable.as_ref() }.ty() == builtin_type(instance, "type") {
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
        let Some(created) = (unsafe { new_slot(class, &args, instance) }) else {
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
        if let Some(constructor) = instance.type_lookup(class, "__new__") {
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

    let frame_type = builtin_type(instance, "Frame");
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

/// `BC-49` 的整数二元运算：只做不涉及协议与值域扩张的几项。
fn binary_op(name: &str, left: i64, right: i64) -> Result<i64, ExecError> {
    let result = match name {
        "NB_ADD" => left.checked_add(right),
        "NB_SUBTRACT" => left.checked_sub(right),
        "NB_MULTIPLY" => left.checked_mul(right),
        "NB_AND" => Some(left & right),
        "NB_OR" => Some(left | right),
        "NB_XOR" => Some(left ^ right),
        "NB_LSHIFT" if (0..64).contains(&right) => left.checked_shl(right as u32),
        "NB_RSHIFT" if (0..64).contains(&right) => Some(left >> right),
        _ => {
            return Err(ExecError::Unsupported {
                opcode: opcode_of("BINARY_OP"),
                what: "该 NB_* 运算尚未接线（就地运算、除法族、下标与协议运算随后补）",
            })
        }
    };
    result.ok_or(ExecError::IntOutOfRange { value: i64::MAX })
}

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
    if entry.lasti {
        push_small_int(instance, frame, (offset_bytes / 2) as i64)?;
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
pub fn execute<'a>(
    instance: &'a Instance,
    frame: &Owned<'a, Frame>,
) -> Result<ExecOutcome<'a>, ExecError> {
    let code_header = frame.get().code().expect("BC-42：帧必须持有 code object");
    // SAFETY: 帧持有一份对 code object 的引用（BC-42），因此它在帧存活期间有效；
    // 帧由本函数的调用方持有。
    let code = unsafe { &*code_header.as_ptr().cast::<CodeObject>() };

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
            "MAKE_CELL" => {
                // 净 0：把 **cell 槽**第 `oparg` 格换成一个新 cell；初值取**同号局部槽**（若有）
                let slot = oparg as usize;
                // 同号局部槽的值当 cell 初值（类体的 `nlocals` 是 0 ⇒ `local` 会报越界 ⇒ `None`）
                let initial = frame.get().local(slot).unwrap_or(None);
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
                if let Some(old) = object.replace(Some(value)) {
                    release(instance, old);
                }
                release(instance, value);
            }
            "LOAD_DEREF" => {
                // 净 +1：压 cell 槽第 `oparg` 格那个 cell 的值
                let cell = match frame.get().cell(oparg as usize) {
                    Ok(Some(cell)) => cell,
                    Ok(None) => {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "`LOAD_DEREF` 的 cell 槽是空的（`MAKE_CELL` 没跑过）",
                        })
                    }
                    Err(error) => return Err(ExecError::Frame(error)),
                };
                // SAFETY: cell 由帧的 cell 槽持有，存活。
                let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                let Some(value) = object.value() else {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "`LOAD_DEREF` 读的 cell 还是空的",
                    })
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
                    Ok(None) => return Err(ExecError::UnboundLocal { slot: oparg }),
                    // **cell 在参照实现里也是"快速局部槽"**：类体的 `__classdict__` 只有 cell 槽
                    // （`nlocals` 是 0），而参照收尾用的是 `LOAD_FAST_BORROW 0` 读那个 cell
                    // ⇒ 局部槽越界时回落到**同号 cell 槽**（`BC-45` 的独立 cell 槽模型下的兼容）。
                    Err(crate::FrameError::SlotOutOfRange { .. }) => {
                        let cell = match frame.get().cell(oparg) {
                            Ok(Some(cell)) => cell,
                            _ => return Err(ExecError::UnboundLocal { slot: oparg }),
                        };
                        // SAFETY: cell 由帧的 cell 槽持有，存活。
                        let object = unsafe { &*cell.as_ptr().cast::<crate::cell::CellObject>() };
                        let Some(value) = object.value() else {
                            return Err(ExecError::UnboundLocal { slot: oparg });
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
                if let Some(old) = frame.get().set_local(oparg, Some(value))? {
                    release(instance, old);
                }
            }
            "DELETE_FAST" => match frame.get().set_local(oparg, None)? {
                Some(old) => release(instance, old),
                None => return Err(ExecError::UnboundLocal { slot: oparg }),
            },
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
                let number = as_int(instance, value, opcode_number)?;
                release(instance, value);
                let result = if name == "UNARY_NEGATIVE" {
                    number.checked_neg()
                } else {
                    Some(!number)
                };
                let result = result.ok_or(ExecError::IntOutOfRange { value: number })?;
                push_int_result(instance, frame.get(), result)?;
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
                // 实测：容器在 PEEK(oparg)——`PEEK` **把指令自己的操作数也算进去**（值就是 PEEK(1)）；
                // 所以弹出值之后，容器在 `oparg - 1`。
                if oparg == 0 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "LIST_APPEND／SET_ADD 的 oparg 至少为 1",
                    });
                }
                let value = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg - 1)?;
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
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "容器在栈上的位置或类型不符",
                    });
                }
            }
            "MAP_ADD" => {
                // 实测：`[.., 容器, 键, 值]`，oparg 指**容器**（PEEK 含自身操作数）⇒
                // 容器在 PEEK(oparg) = 弹出键值之后的 `oparg - 2`……实测 dict 推导式里 oparg ＝ 2、
                // 容器在 PEEK(3)，故弹出两个操作数后容器在 `oparg - 1`。
                if oparg < 2 {
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAP_ADD 的 oparg 至少为 2",
                    });
                }
                let value = frame.get().pop()?;
                let key = frame.get().pop()?;
                let container = frame.get().peek_from_top(oparg - 1)?;
                // SAFETY: container 在帧的值栈上，存活。
                let ty = unsafe { container.as_ref() }.ty();
                if ty != builtin_type(instance, "dict") {
                    release(instance, key);
                    release(instance, value);
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "MAP_ADD 的容器在栈上的位置或类型不符",
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
                        what: "容器在栈上的位置或类型不符",
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
                if argument_type != builtin_type(instance, "tuple") {
                    release(instance, callable);
                    release(instance, argument_source);
                    release(instance, keyword_source);
                    if let Some(bound) = bound_self {
                        release(instance, bound);
                    }
                    return Err(ExecError::Unsupported {
                        opcode: opcode_number,
                        what: "CALL_FUNCTION_EX 的实参必须是 tuple（编译器保证）",
                    });
                }
                // SAFETY: 类型身份已确认。
                let arguments = unsafe { &*argument_source.as_ptr().cast::<TupleObject>() };
                let mut args: Vec<NonNull<Header>> = Vec::with_capacity(arguments.len());
                for index in 0..arguments.len() {
                    let value = arguments.item(index).expect("下标在范围内");
                    // SAFETY: 元素由元组持有。
                    unsafe { instance.incref_object(value.as_ptr()) };
                    args.push(value);
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
                push_small_int(instance, frame.get(), oparg as i64)?;
            }
            "FORMAT_SIMPLE" => {
                // 净 0：TOS 换成它的 `str()`（3.14 把旧的 `FORMAT_VALUE` 拆成了三条）
                let value = frame.get().pop()?;
                // `TS-44`：先走属性通道的 `__str__`，没有才落到原生槽位／默认实现
                let text = match dunder_text(instance, value, "__str__", opcode_number)? {
                    Some(text) => text,
                    None => instance.object_str(value),
                };
                release(instance, value);
                push(instance, frame.get(), instance.new_str(&text))?;
            }
            "CONVERT_VALUE" => {
                // 净 0：`!s`／`!r`／`!a`（实测 oparg 1／2／3）
                let value = frame.get().pop()?;
                // `!s`／`!r`／`!a`（实测 oparg 1／2／3），都走 `OM-11` 的槽位
                let text = match oparg {
                    1 => match dunder_text(instance, value, "__str__", opcode_number)? {
                        Some(text) => text,
                        None => instance.object_str(value),
                    },
                    2 => match dunder_text(instance, value, "__repr__", opcode_number)? {
                        Some(text) => text,
                        None => instance.object_repr(value),
                    },
                    3 => {
                        let base = match dunder_text(instance, value, "__repr__", opcode_number)? {
                            Some(text) => text,
                            None => instance.object_repr(value),
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
                push_small_int(instance, frame.get(), length as i64)?;
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
                // 实测：`RERAISE n` 先弹 `n` 个额外值（通常是 lasti），再抛 TOS
                for _ in 0..oparg {
                    release(instance, frame.get().pop()?);
                }
                let exception = frame.get().pop()?;
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
                if oparg & 1 != 0 {
                    // 先压 `NULL`（`CALL` 的"没有 self"槽位）
                    let null = instance.singletons().null();
                    push(instance, frame.get(), null)?;
                }
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
                    Some(value) => push(instance, frame.get(), value)?,
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
                let exception = frame.get().peek()?;
                let prev = frame.get().peek_from_top(2)?;
                let self_object = frame.get().peek_from_top(3)?;
                let callable = frame.get().peek_from_top(4)?;
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
                    _ => {
                        release(instance, attribute);
                        release(instance, function);
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "SET_FUNCTION_ATTRIBUTE 只接线了 defaults(1)／kwdefaults(2)／annotate(16)；closure(8) 随后",
                        });
                    }
                }
                frame.get().push(function)?;
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
                    let left_value = as_int(instance, left, opcode_number);
                    let right_value = as_int(instance, right, opcode_number);
                    release(instance, left);
                    release(instance, right);
                    let (left_value, right_value) = (left_value?, right_value?);
                    let result = binary_op(name, left_value, right_value)?;
                    push_int_result(instance, frame.get(), result)?;
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
