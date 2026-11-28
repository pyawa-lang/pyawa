//! **A-1（round 292）**：内建容器 `repr` 的**显式 worklist**（`PLAN §12.22` 的 A 本体第一轮）。
//!
//! # 为什么
//!
//! 旧实现里 `list`／`tuple`／`dict`／`set` 的 `repr` 彼此**Rust 递归**（每嵌套一层多一摞原生栈帧），
//! 深度只能靠宿主声明的原生预算守（`D1`：CLI 声明 1000；ABI／嵌入／wasm 未声明默认 64）——
//! 小栈宿主到不了参照的语义额度 `recursion_limit`（默认 1000）。
//!
//! 本模块把"正在生成中的容器"放一个**堆上的工作栈**：每帧只记自己的位置、已写文本与
//! "待写后缀"状态（`[`／`(`／`{`、分隔符 `, `、`k: `、set 的 `frozenset({` 与尾部 `})`）。
//! 于是内建容器嵌套**不吃 Rust 原生栈**，深度口径改用语义额度 `recursion_limit`
//! （`Instance::enter_repr`，`setrecursionlimit` 可到 10000）。
//!
//! # 与旧实现逐字节一致
//!
//! 四个槽的拼接口径原样复刻（空容器、单元素元组、`dict` 的 `k: v`、`set`／`frozenset` 的
//! 精确／子类形式、循环标记 `[...]`／`(...)`／`{...}`）；元素文本仍由
//! [`crate::executor::protocol::element_repr`] 产生（用户 `__repr__` 协议与 D1 预算不变）。
//!
//! 只有"**内建容器且其 `repr` 仍由内建槽实现**"的元素才会被压帧；其余（标量、用户对象、
//! 覆写了 `__repr__` 的容器子类、换了 repr 槽的子类如 `mappingproxy`、宿主对象、实例字典里
//! 有 `__repr__` 项的对象）一律走原 `element_repr` 路径 —— 语义与副作用保持原样。

use super::prelude::*;
use super::*;
use crate::HotId;

/// 可堆化的内建容器 `repr` 家族（`frozenset` 与 `set` 同槽，靠帧里的类型信息区分形式）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerKind {
    List,
    Tuple,
    Dict,
    Set,
}

impl ContainerKind {
    /// 自引用时参照给的循环标记（与旧递归实现同一处口径）。
    fn cycle_text(self) -> &'static str {
        match self {
            Self::List => "[...]",
            Self::Tuple => "(...)",
            Self::Dict | Self::Set => "{...}",
        }
    }
}

/// `dict` 帧当前在渲染键还是值（`k: v` 的拼接状态）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum DictPhase {
    Key,
    Value,
}

/// 子元素文本回来时要落到哪一格（分隔符已在 `step` 里先写好）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    None,
    Item,
    DictKey,
    DictValue,
}

/// 一帧的迭代状态（载荷指针由 [`Frame::object`] 持有引用保证存活）。
enum Body {
    List {
        index: usize,
        /// 正在渲染的元素（**本帧持有**一份引用；文本回填后释放）。
        pending: Option<NonNull<Header>>,
    },
    Tuple {
        index: usize,
    },
    Dict {
        /// 与旧实现同口径：进入本帧时取一次条目快照（借用裸指针）。
        entries: Vec<(NonNull<Header>, NonNull<Header>)>,
        index: usize,
        phase: DictPhase,
    },
    Set {
        /// 与旧实现同口径：进入本帧时取一次元素快照（借用裸指针）。
        items: Vec<NonNull<Header>>,
        index: usize,
        /// 精确 `set` ⇒ `{...}`；其余（`frozenset`／子类）尾部多一个 `)`。
        exact_set: bool,
    },
}

/// 驱动的一步：要么完成并交回文本，要么先要一个子元素的文本。
enum Step {
    NeedChild(NonNull<Header>),
    Done(String),
}

/// 一帧：一个正在生成中的内建容器。
struct Frame<'a> {
    instance: &'a Instance,
    /// 容器对象（本帧**持有**一份引用：生成期间即使被父容器改表摘掉也不悬垂）。
    object: NonNull<Header>,
    /// `repr_guard` 的地址链守卫（`Drop` 时摘除 ⇒ 循环检测与旧实现同口径，错误路径也生效）。
    /// 字段本身不被读取，只在帧析构时由 `Drop` 触发 —— 下划线前缀表明这是 **RAII 字段**。
    _scope: crate::instance::ReprScope<'a>,
    text: String,
    body: Body,
    expecting: Expect,
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        if let Body::List { pending, .. } = &mut self.body {
            if let Some(item) = pending.take() {
                // SAFETY: 这是 `step` 里为本帧加的那一份引用。
                unsafe { self.instance.release_object(item.as_ptr()) };
            }
        }
        // SAFETY: 这是 `make_frame` 里为本帧加的那一份引用。
        unsafe { self.instance.release_object(self.object.as_ptr()) };
        // `scope` 的 `Drop` 随后把地址链条目摘除（RAII）。
    }
}

impl Frame<'_> {
    /// 推进一帧（子元素文本已由 [`Frame::accept`] 回填后才会再调）。
    fn step(&mut self) -> Result<Step, ExecError> {
        match &mut self.body {
            Body::List { index, pending } => {
                debug_assert!(pending.is_none(), "子元素文本未回填就继续推进 list 帧");
                // SAFETY: 帧持有容器引用；类型槽契约 ⇒ 载荷是 `ListObject`。
                let list = unsafe { &*self.object.as_ptr().cast::<ListObject>() };
                // **活体遍历**（与旧实现同口径）：每轮重读长度；元素的 `__repr__` 可能改表。
                if *index >= list.len() {
                    self.text.push(']');
                    return Ok(Step::Done(core::mem::take(&mut self.text)));
                }
                let item = list.item(*index).expect("刚查过长度，下标在范围内");
                // SAFETY: 本帧持一份引用到文本回填（`__repr__` 可能 pop 掉它）。
                unsafe { self.instance.incref_object(item.as_ptr()) };
                *pending = Some(item);
                if *index > 0 {
                    self.text.push_str(", ");
                }
                self.expecting = Expect::Item;
                Ok(Step::NeedChild(item))
            }
            Body::Tuple { index } => {
                // SAFETY: 同上（tuple 载荷）。
                let tuple = unsafe { &*self.object.as_ptr().cast::<TupleObject>() };
                if *index >= tuple.len() {
                    if tuple.len() == 1 {
                        self.text.push(',');
                    }
                    self.text.push(')');
                    return Ok(Step::Done(core::mem::take(&mut self.text)));
                }
                let item = tuple.item(*index).expect("刚查过长度，下标在范围内");
                if *index > 0 {
                    self.text.push_str(", ");
                }
                self.expecting = Expect::Item;
                Ok(Step::NeedChild(item))
            }
            Body::Dict { entries, index, phase } => {
                if *index >= entries.len() {
                    self.text.push('}');
                    return Ok(Step::Done(core::mem::take(&mut self.text)));
                }
                let entry = entries[*index];
                match phase {
                    DictPhase::Key => {
                        if *index > 0 {
                            self.text.push_str(", ");
                        }
                        self.expecting = Expect::DictKey;
                        Ok(Step::NeedChild(entry.0))
                    }
                    DictPhase::Value => {
                        self.text.push_str(": ");
                        self.expecting = Expect::DictValue;
                        Ok(Step::NeedChild(entry.1))
                    }
                }
            }
            Body::Set { items, index, exact_set, .. } => {
                // 空集：旧实现是 `{class_text}()`（守卫先进入、再判空 ⇒ 帧仍然存在）。
                if items.is_empty() {
                    return Ok(Step::Done(core::mem::take(&mut self.text)));
                }
                if *index >= items.len() {
                    self.text.push('}');
                    if !*exact_set {
                        self.text.push(')');
                    }
                    return Ok(Step::Done(core::mem::take(&mut self.text)));
                }
                if *index > 0 {
                    self.text.push_str(", ");
                }
                self.expecting = Expect::Item;
                Ok(Step::NeedChild(items[*index]))
            }
        }
    }

    /// 回填一个子元素的文本（`element_repr` 或子帧的 `Done` 文本）。
    fn accept(&mut self, text: String) {
        match self.expecting {
            Expect::None => return,
            Expect::Item => {
                self.text.push_str(&text);
                match &mut self.body {
                    Body::List { index, pending } => {
                        *index += 1;
                        if let Some(item) = pending.take() {
                            // SAFETY: 与 `step` 里加的那份引用配对。
                            unsafe { self.instance.release_object(item.as_ptr()) };
                        }
                    }
                    Body::Tuple { index } => *index += 1,
                    Body::Set { index, .. } => *index += 1,
                    Body::Dict { .. } => {}
                }
            }
            Expect::DictKey => {
                self.text.push_str(&text);
                if let Body::Dict { phase, .. } = &mut self.body {
                    *phase = DictPhase::Value;
                }
            }
            Expect::DictValue => {
                self.text.push_str(&text);
                if let Body::Dict { index, phase, .. } = &mut self.body {
                    *index += 1;
                    *phase = DictPhase::Key;
                }
            }
        }
        self.expecting = Expect::None;
    }
}

/// 建帧的结果（`Cycle`／`TooDeep` 与旧 `enter_repr` 的三支对应）。
enum NewFrame<'a> {
    Frame(Frame<'a>),
    Cycle,
    TooDeep,
}

/// 建一帧：先进入 `repr_guard`（循环／深度语义与旧实现同一处），再按家族取载荷快照。
fn make_frame<'a>(
    instance: &'a Instance,
    object: NonNull<Header>,
    kind: ContainerKind,
) -> NewFrame<'a> {
    let scope = match instance.enter_repr(object.as_ptr() as usize) {
        crate::instance::ReprEntry::Cycle => return NewFrame::Cycle,
        crate::instance::ReprEntry::TooDeep => return NewFrame::TooDeep,
        crate::instance::ReprEntry::Entered(scope) => scope,
    };
    // SAFETY: 调用方交来的指针是存活对象；这一份引用保证生成期间不被释放。
    unsafe { instance.incref_object(object.as_ptr()) };
    let (text, body) = match kind {
        ContainerKind::List => (String::from("["), Body::List { index: 0, pending: None }),
        ContainerKind::Tuple => (String::from("("), Body::Tuple { index: 0 }),
        ContainerKind::Dict => {
            // SAFETY: 类型槽契约 ⇒ 载荷是 `DictObject`（`mappingproxy` 同构，走同一个槽入口）。
            let dict = unsafe { &*object.as_ptr().cast::<DictObject>() };
            (
                String::from("{"),
                Body::Dict { entries: dict.entries(), index: 0, phase: DictPhase::Key },
            )
        }
        ContainerKind::Set => {
            // SAFETY: 类型槽契约 ⇒ 载荷是 `SetObject`（`set`／`frozenset`／子类同布局）。
            let set = unsafe { &*object.as_ptr().cast::<SetObject>() };
            let items = set.items();
            // SAFETY: 存活对象 ⇒ 头部可读类型。
            let class = unsafe { &*object.as_ptr() }.ty();
            let exact_set = Some(class) == instance.hot_type(HotId::Set);
            let class_text = instance.type_name(class).to_owned();
            let text = if items.is_empty() {
                // 旧实现：空集不看精确性，一律 `{class_text}()`。
                format!("{class_text}()")
            } else if exact_set {
                String::from("{")
            } else if Some(class) == instance.hot_type(HotId::FrozenSet) {
                String::from("frozenset({")
            } else {
                format!("{class_text}({{")
            };
            (text, Body::Set { items, index: 0, exact_set })
        }
    };
    NewFrame::Frame(Frame { instance, object, _scope: scope, text, body, expecting: Expect::None })
}

/// 某个元素能否直接压帧（是内建容器、且 `repr` 仍由内建槽实现）。
///
/// 保守判定：只要存在"用户能改变文本"的通道（用户层 `__repr__`、宿主属性钩子、实例字典里
/// 的同名项、子类换了 repr 槽）就不内联，退回 [`crate::executor::protocol::element_repr`]，
/// 保证协议、异常与副作用与旧实现完全一致。
fn inline_kind(instance: &Instance, object: NonNull<Header>) -> Option<ContainerKind> {
    if instance.is_type_object(object) {
        return None;
    }
    // SAFETY: object 是存活对象。
    let ty = unsafe { object.as_ref() }.ty();
    let list = instance.hot_type(HotId::List)?;
    let tuple = instance.hot_type(HotId::Tuple)?;
    let dict = instance.hot_type(HotId::Dict)?;
    let set = instance.hot_type(HotId::Set)?;
    let frozen = instance.hot_type(HotId::FrozenSet)?;
    // **精确内建容器**（热路径）：类型不可变 ⇒ 不可能有用户覆写／实例字典／宿主钩子。
    if ty == list {
        return Some(ContainerKind::List);
    }
    if ty == tuple {
        return Some(ContainerKind::Tuple);
    }
    if ty == dict {
        return Some(ContainerKind::Dict);
    }
    if ty == set || ty == frozen {
        return Some(ContainerKind::Set);
    }
    // **容器子类**：槽 repr 必须仍是家族槽（`mappingproxy` 这类换了 repr 槽的继承者除外）。
    let (base, kind) = if instance.is_subtype(ty, list) {
        (list, ContainerKind::List)
    } else if instance.is_subtype(ty, tuple) {
        (tuple, ContainerKind::Tuple)
    } else if instance.is_subtype(ty, dict) {
        (dict, ContainerKind::Dict)
    } else if instance.is_subtype(ty, set) {
        (set, ContainerKind::Set)
    } else if instance.is_subtype(ty, frozen) {
        (frozen, ContainerKind::Set)
    } else {
        return None;
    };
    let slot_of = |class: NonNull<crate::TypeObject>| {
        // SAFETY: 类型对象存活。
        unsafe { class.as_ref() }.slots().repr.map(|function| function as usize)
    };
    if slot_of(ty) != slot_of(base) {
        return None;
    }
    // 用户层 `__repr__`（类型字典／MRO 上不是 `object` 的项）⇒ 尊重协议，走原路径。
    if let Some((owner, _)) = instance.type_lookup_owner(ty, "__repr__") {
        if Some(owner) != instance.hot_type(HotId::Object) {
            return None;
        }
    }
    // 宿主对象的属性钩子可能是唯一实现（`AB-73`／`AB-75` 口径）⇒ 走原路径。
    if instance.has_host_object_ops(object) {
        return None;
    }
    // 实例字典里的 `__repr__`：旧路径的 `call_object_method` 会先命中它 ⇒ 不内联，保持旧行为。
    if let Some(mapping) = crate::executor::protocol::instance_attributes(instance, object) {
        if instance.dict_get(mapping, "__repr__").is_some() {
            return None;
        }
    }
    Some(kind)
}

/// **内建容器 `repr` 的唯一驱动**（四个 `repr` 槽都转到这里）。
///
/// 循环检测／深度守卫走 [`Instance::enter_repr`]（每帧进入、`Drop` 退出）；"太深"⇒ 本函数
/// 返回 [`Instance::repr_recursion_error`] 的可捕获 `RecursionError`，不消耗原生栈。
pub(crate) fn container_repr(
    instance: &Instance,
    object: NonNull<Header>,
    kind: ContainerKind,
) -> Result<String, ExecError> {
    let mut stack: Vec<Frame<'_>> = Vec::new();
    match make_frame(instance, object, kind) {
        NewFrame::Frame(frame) => stack.push(frame),
        NewFrame::Cycle => return Ok(kind.cycle_text().to_owned()),
        NewFrame::TooDeep => return Err(instance.repr_recursion_error()),
    }
    loop {
        let step = stack.last_mut().expect("栈里至少有根帧").step()?;
        match step {
            Step::NeedChild(child) => {
                if let Some(child_kind) = inline_kind(instance, child) {
                    match make_frame(instance, child, child_kind) {
                        NewFrame::Frame(frame) => {
                            stack.push(frame);
                            continue;
                        }
                        // 环：子容器已在生成中 ⇒ 直接把循环标记当子文本回填（与旧实现同文本）。
                        NewFrame::Cycle => {
                            let parent = stack.last_mut().expect("子帧失败时父帧仍在");
                            parent.accept(child_kind.cycle_text().to_owned());
                            continue;
                        }
                        NewFrame::TooDeep => return Err(instance.repr_recursion_error()),
                    }
                }
                // 非内建容器元素：仍走原 `element_repr`（用户 `__repr__` 协议／D1 预算不变）。
                let text = crate::executor::protocol::element_repr(instance, child)?;
                stack.last_mut().expect("父帧仍在").accept(text);
            }
            Step::Done(text) => {
                // 先弹出子帧（Drop 释放它持有的一份引用、摘掉地址链条目），再回填父帧。
                let _finished = stack.pop();
                match stack.last_mut() {
                    Some(parent) => parent.accept(text),
                    None => return Ok(text),
                }
            }
        }
    }
}
