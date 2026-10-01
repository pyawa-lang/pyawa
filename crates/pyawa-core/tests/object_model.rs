//! 对象模型的可测性质（`docs/SPEC-object-model.md` §13 中本层已能成立的部分）。
//!
//! 每个测试在注释里标出它对应的 `OM-n`／`T-OM-n`；尚未能验证的项
//! （`gc` 模块可见行为、弱引用、宿主对象 traverse、CI 静态检查）等对应实现落地后再补。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use pyawa_core::{flags, py_object, Header, Instance, PyObject, Slots, TypeObject};

py_object! {
    /// 叶子对象：不持有任何引用（非成环类型）。
    struct Leaf {
        value: Cell<u32>,
    }
}

py_object! {
    /// 观测对象：记录终结器／clear／释放顺序（**OM-20**），并可复活一次。
    struct Watched {
        log: &'static Mutex<Vec<&'static str>>,
        resurrect: Cell<bool>,
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.log.lock().unwrap().push("free");
    }
}

py_object! {
    /// 只在被释放时记账：用于验证实例销毁路径（**OM-2**）。
    struct Tracked {
        marker: (),
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        TRACKED_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

py_object! {
    /// 可成环的容器载体（**OM-12**）：载荷按 **OM-40** 存裸引用，释放只在 `clear` 里做。
    struct Node {
        next: RefCell<Option<NonNull<Header>>>,
        /// 传 `None` 时不记录（深链测试用），避免几万条日志。
        log: Option<&'static Mutex<Vec<&'static str>>>,
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        if let Some(log) = self.log {
            log.lock().unwrap().push("free");
        }
    }
}

static ORDER_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static RESURRECT_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static TRACKED_DROPS: AtomicUsize = AtomicUsize::new(0);
static CYCLE_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static FINALIZER_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static SURVIVOR_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static THRESHOLD_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static PEER_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static REENTRY_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
/// 允许复活的次数（**OM-20** ①：终结器可以把对象救回来）。
static RESURRECT_BUDGET: AtomicUsize = AtomicUsize::new(0);

fn log_to(log: &'static Mutex<Vec<&'static str>>, event: &'static str) {
    log.lock().unwrap().push(event);
}

/// 取快照再断言：断言失败时不会握着锁，避免把日志毒化传染给别的测试。
fn snapshot(log: &'static Mutex<Vec<&'static str>>) -> Vec<&'static str> {
    log.lock().unwrap().clone()
}

fn reset(log: &'static Mutex<Vec<&'static str>>) {
    log.lock().unwrap().clear();
}

unsafe fn leaf_traverse(_ptr: *mut Header, _visit: &mut dyn FnMut(*mut Header)) {}

unsafe fn watched_finalize(ptr: *mut Header, _instance: &Instance) {
    // SAFETY: 释放协议保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<Watched>() };
    object.log.lock().unwrap().push("finalize");
}

unsafe fn watched_clear(ptr: *mut Header, _instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<Watched>() };
    object.log.lock().unwrap().push("clear");
}

unsafe fn resurrecting_finalize(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let object = unsafe { &*ptr.cast::<Watched>() };
    object.log.lock().unwrap().push("finalize");
    if object.resurrect.replace(false) {
        // **OM-20** ①：复活——把计数改回大于 0，释放必须放弃。
        unsafe { instance.resurrect_object(ptr) };
    }
}

// ---- 容器载体可成环，因此提供 traverse／clear（OM-12），并可选终结器（OM-20 ①） ----

unsafe fn node_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let node = unsafe { &*ptr.cast::<Node>() };
    if let Some(child) = *node.next.borrow() {
        visit(child.as_ptr());
    }
}

unsafe fn node_clear(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。**OM-40**：载荷中的引用只能在 clear／traverse／dealloc 内释放。
    let node = unsafe { &*ptr.cast::<Node>() };
    if let Some(child) = node.next.borrow_mut().take() {
        unsafe { instance.release_object(child.as_ptr()) };
    }
}

/// 回收序测试用的终结器：先记"finalize"，再记"clear"由 `node_clear` 负责。
unsafe fn node_finalize(ptr: *mut Header, _instance: &Instance) {
    // SAFETY: 同上。
    let node = unsafe { &*ptr.cast::<Node>() };
    if let Some(log) = node.log {
        log_to(log, "finalize");
    }
}

/// 复活一次的终结器：验证 **OM-27** 的"重判定"。
unsafe fn node_resurrect_finalize(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let node = unsafe { &*ptr.cast::<Node>() };
    if let Some(log) = node.log {
        log_to(log, "finalize");
    }
    if RESURRECT_BUDGET.load(Ordering::SeqCst) > 0 {
        RESURRECT_BUDGET.fetch_sub(1, Ordering::SeqCst);
        // 用一个"外部引用"把自己复活（OM-20 ①）
        unsafe { instance.resurrect_object(ptr) };
    }
}

/// 终结器里就去释放环内同伴：**OM-27** 要求先全部终结、再统一释放，
/// 因此这次 decref 必须只减计数、不能提前真的释放（否则外层拿着悬垂指针）。
unsafe fn node_allocating_finalize(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let node = unsafe { &*ptr.cast::<Node>() };
    if let Some(log) = node.log {
        log_to(log, "finalize");
    }
    // 在终结器里分配对象 ⇒ 阈值够低时会再触发一次回收；那一次必须让路（不得嵌套）。
    let ty = instance.new_type(
        "Leaf",
        core::mem::size_of::<Leaf>(),
        Slots::new(Leaf::dealloc),
    );
    let leaf = instance.alloc(Leaf::new(ty, Cell::new(1)));
    drop(leaf);
}

/// 带日志的 clear：记录 clear 事件后释放载荷引用。
unsafe fn node_clear_logged(ptr: *mut Header, instance: &Instance) {
    // SAFETY: 同上。
    let node = unsafe { &*ptr.cast::<Node>() };
    if let Some(log) = node.log {
        log_to(log, "clear");
    }
    if let Some(child) = node.next.borrow_mut().take() {
        unsafe { instance.release_object(child.as_ptr()) };
    }
}

fn node_type_with_releasing_finalizer(instance: &Instance) -> NonNull<TypeObject> {
    instance.new_type(
        "Node",
        core::mem::size_of::<Node>(),
        Slots::new(Node::dealloc)
            // 终结器直接借用 clear 的行为：释放载荷里对同伴的引用
            .with_finalize(node_clear_logged)
            .with_traverse(node_traverse)
            .with_clear(node_clear),
    )
}

fn node_type_with_allocating_finalizer(instance: &Instance) -> NonNull<TypeObject> {
    instance.new_type(
        "Node",
        core::mem::size_of::<Node>(),
        Slots::new(Node::dealloc)
            .with_finalize(node_allocating_finalize)
            .with_traverse(node_traverse)
            .with_clear(node_clear),
    )
}

fn leaf_type(instance: &Instance) -> NonNull<TypeObject> {
    instance.new_type(
        "Leaf",
        core::mem::size_of::<Leaf>(),
        Slots::new(Leaf::dealloc),
    )
}

fn node_type(instance: &Instance) -> NonNull<TypeObject> {
    instance.new_type(
        "Node",
        core::mem::size_of::<Node>(),
        Slots::new(Node::dealloc)
            .with_traverse(node_traverse)
            .with_clear(node_clear),
    )
}

fn node_type_with_finalizer(instance: &Instance) -> NonNull<TypeObject> {
    instance.new_type(
        "Node",
        core::mem::size_of::<Node>(),
        Slots::new(Node::dealloc)
            .with_finalize(node_finalize)
            .with_traverse(node_traverse)
            .with_clear(node_clear_logged),
    )
}

fn node_type_with_resurrecting_finalizer(instance: &Instance) -> NonNull<TypeObject> {
    instance.new_type(
        "Node",
        core::mem::size_of::<Node>(),
        Slots::new(Node::dealloc)
            .with_finalize(node_resurrect_finalize)
            .with_traverse(node_traverse)
            .with_clear(node_clear_logged),
    )
}

/// 造一个两节点环：`a.next = b`、`b.next = a`，并把两个局部守卫都交出去。
fn make_cycle(instance: &Instance, ty: NonNull<TypeObject>, log: Option<&'static Mutex<Vec<&'static str>>>) {
    let first = instance.alloc(Node::new(ty, RefCell::new(None), log));
    let second = instance.alloc(Node::new(ty, RefCell::new(None), log));
    let first_header = first.as_ptr().cast::<Header>();
    let second_header = second.as_ptr().cast::<Header>();

    // 载荷持有裸引用 ⇒ 必须相应 incref（OM-40）
    unsafe { instance.incref_object(second_header.as_ptr()) };
    *first.get().next.borrow_mut() = Some(second_header);
    unsafe { instance.incref_object(first_header.as_ptr()) };
    *second.get().next.borrow_mut() = Some(first_header);

    drop(first);
    drop(second);
}

#[test]
fn new_reference_protocol_and_getrefcount() {
    let instance = Instance::new();
    let ty = leaf_type(&instance);

    let first = instance.alloc(Leaf::new(ty, Cell::new(7)));
    assert_eq!(first.refcount(), 1, "分配即一个新引用（OM-16）");

    let second = first.clone();
    assert_eq!(first.refcount(), 2, "Clone 取得新引用（OM-16／OM-17）");
    assert_eq!(
        instance.getrefcount(first.get()),
        3,
        "OM-22：返回值含参数借用的一份"
    );

    let borrowed = second.borrow();
    assert_eq!(borrowed.get().value.get(), 7);
    assert_eq!(first.refcount(), 2, "借用不改变计数（OM-19）");

    drop(second);
    assert_eq!(first.refcount(), 1);
    assert_eq!(instance.live_objects(), 1);

    drop(first);
    assert_eq!(instance.live_objects(), 0, "计数归零后释放（OM-20）");
    assert_eq!(
        instance.bytes_allocated(),
        core::mem::size_of::<TypeObject>() * 2,
        "OM-3：类型对象由注册表持有，直到实例销毁"
    );
}

#[test]
fn instances_are_isolated() {
    let left = Instance::new();
    let right = Instance::new();

    let left_ty = leaf_type(&left);
    let right_ty = leaf_type(&right);
    assert_ne!(left_ty, right_ty, "OM-15：类型注册表按实例存放");

    let object = left.alloc(Leaf::new(left_ty, Cell::new(1)));
    assert_eq!(left.live_objects(), 1);
    assert_eq!(right.live_objects(), 0, "OM-1：对象不跨实例共享");
    assert_eq!(
        right.bytes_allocated(),
        core::mem::size_of::<TypeObject>() * 2,
        "OM-1／OM-3：两个实例各记各的账（元类型 + 各一个 Leaf 类型）"
    );

    drop(object);
    assert_eq!(left.live_objects(), 0);
}

#[test]
fn instance_teardown_releases_remaining_objects() {
    TRACKED_DROPS.store(0, Ordering::SeqCst);
    {
        let instance = Instance::new();
        let ty = instance.new_type(
            "Tracked",
            core::mem::size_of::<Tracked>(),
            Slots::new(Tracked::dealloc),
        );
        let object = instance.alloc(Tracked::new(ty, ()));
        core::mem::forget(object); // 模拟"实例销毁时仍被持有"的对象
        assert_eq!(instance.live_objects(), 1);
    } // OM-2：实例销毁必须释放全部内存，无论循环是否被回收过
    assert_eq!(TRACKED_DROPS.load(Ordering::SeqCst), 1);
}

#[test]
fn release_order_is_finalize_clear_free() {
    ORDER_LOG.lock().unwrap().clear();
    let instance = Instance::new();
    let ty = instance.new_type(
        "Watched",
        core::mem::size_of::<Watched>(),
        Slots::new(Watched::dealloc)
            .with_finalize(watched_finalize)
            .with_clear(watched_clear),
    );

    let object = instance.alloc(Watched::new(ty, &ORDER_LOG, Cell::new(false)));
    drop(object);

    assert_eq!(
        *ORDER_LOG.lock().unwrap(),
        vec!["finalize", "clear", "free"],
        "OM-20：① 终结器 → ② clear → ③ 释放"
    );
    assert_eq!(instance.live_objects(), 0);
}

#[test]
fn resurrection_aborts_the_release() {
    RESURRECT_LOG.lock().unwrap().clear();
    let instance = Instance::new();
    let ty = instance.new_type(
        "Watched",
        core::mem::size_of::<Watched>(),
        Slots::new(Watched::dealloc)
            .with_finalize(resurrecting_finalize)
            .with_clear(watched_clear),
    );

    let object = instance.alloc(Watched::new(ty, &RESURRECT_LOG, Cell::new(true)));
    let raw = object.as_ptr().cast::<Header>();
    drop(object); // 计数 1→0 → 终结器复活 → 放弃释放

    assert_eq!(*RESURRECT_LOG.lock().unwrap(), vec!["finalize"]);
    assert_eq!(instance.live_objects(), 1, "复活后对象必须仍然存活");

    // 复活的那份引用最终也要释放：这次终结器不再复活，走完三步。
    unsafe { instance.release_object(raw.as_ptr()) };
    assert_eq!(
        *RESURRECT_LOG.lock().unwrap(),
        vec!["finalize", "finalize", "clear", "free"]
    );
    assert_eq!(instance.live_objects(), 0);
}

#[test]
fn type_objects_are_per_instance_and_self_typed() {
    let instance = Instance::new();
    let ty = leaf_type(&instance);

    let metatype = instance.metatype();
    // SAFETY: metatype 由实例注册表持有，实例存活期间有效。
    assert_eq!(
        unsafe { metatype.as_ref() }.header.ty(),
        metatype,
        "OM-9：类型对象自身也是对象，元类型自指"
    );
    assert_eq!(instance.type_count(), 2, "元类型 + Leaf");

    // SAFETY: ty 由实例注册表持有。
    let ty_ref = unsafe { ty.as_ref() };
    assert_eq!(ty_ref.name(), "Leaf");
    assert_eq!(ty_ref.instance_size(), core::mem::size_of::<Leaf>());
    assert_eq!(ty_ref.header().ty(), metatype);
}

#[test]
fn gc_tracked_bit_follows_the_traverse_slot() {
    let instance = Instance::new();
    let leaf = leaf_type(&instance);
    let trackable = instance.new_type(
        "Leaf",
        core::mem::size_of::<Leaf>(),
        Slots::new(Leaf::dealloc).with_traverse(leaf_traverse),
    );

    let plain_object = instance.alloc(Leaf::new(leaf, Cell::new(0)));
    let trackable_object = instance.alloc(Leaf::new(trackable, Cell::new(0)));

    assert!(!plain_object.header().has_flag(flags::GC_TRACKED));
    assert!(
        trackable_object.header().has_flag(flags::GC_TRACKED),
        "OM-12：可成环的类型必须标记 GC_TRACKED"
    );
    assert_eq!(instance.tracked_objects(), 1, "OM-25：只有跟踪对象入链");
}

// ---- §9 循环回收（OM-25…OM-30） ----

#[test]
fn cycle_is_collected() {
    reset(&CYCLE_LOG);
    let instance = Instance::new();
    let ty = node_type(&instance);
    make_cycle(&instance, ty, Some(&CYCLE_LOG));

    assert_eq!(instance.live_objects(), 2, "环还在（计数不为零）");
    assert_eq!(instance.tracked_objects(), 2);

    let freed = instance.collect();
    assert_eq!(freed, 2, "OM-25：不可达的环必须被回收");
    assert_eq!(instance.live_objects(), 0);
    assert_eq!(instance.tracked_objects(), 0);
    assert_eq!(snapshot(&CYCLE_LOG), vec!["free", "free"]);
}

#[test]
fn reachable_cycle_survives_collection() {
    let instance = Instance::new();
    let ty = node_type(&instance);

    let first = instance.alloc(Node::new(ty, RefCell::new(None), None));
    let second = instance.alloc(Node::new(ty, RefCell::new(None), None));
    let first_header = first.as_ptr().cast::<Header>();
    let second_header = second.as_ptr().cast::<Header>();
    unsafe { instance.incref_object(second_header.as_ptr()) };
    *first.get().next.borrow_mut() = Some(second_header);
    unsafe { instance.incref_object(first_header.as_ptr()) };
    *second.get().next.borrow_mut() = Some(first_header);
    drop(second);

    // `first` 仍被外部守卫持有 ⇒ 整个环可达
    assert_eq!(instance.collect(), 0, "OM-29：可达对象不得被回收");
    assert_eq!(instance.live_objects(), 2);
    assert_eq!(unsafe { first_header.as_ref() }.refcount(), 2);

    drop(first);
    assert_eq!(instance.collect(), 2, "外部引用消失后，环才成为垃圾");
    assert_eq!(instance.live_objects(), 0);
}

#[test]
fn cycle_finalizers_run_once_before_free() {
    reset(&FINALIZER_LOG);
    let instance = Instance::new();
    let ty = node_type_with_finalizer(&instance);
    make_cycle(&instance, ty, Some(&FINALIZER_LOG));

    instance.collect();

    assert_eq!(
        snapshot(&FINALIZER_LOG),
        vec!["finalize", "finalize", "clear", "clear", "free", "free"],
        "OM-27：① 求不可达 → ② 清弱引用 → ③ 终结器 → ④ 释放"
    );
    assert_eq!(instance.live_objects(), 0);
}

#[test]
fn resurrection_during_collection_is_respected() {
    reset(&SURVIVOR_LOG);
    RESURRECT_BUDGET.store(1, Ordering::SeqCst);
    let instance = Instance::new();
    let ty = node_type_with_resurrecting_finalizer(&instance);

    // 手工造环，留住裸指针以便事后还掉"复活"出来的那份引用
    let first = instance.alloc(Node::new(ty, RefCell::new(None), Some(&SURVIVOR_LOG)));
    let second = instance.alloc(Node::new(ty, RefCell::new(None), Some(&SURVIVOR_LOG)));
    let first_header = first.as_ptr().cast::<Header>();
    let second_header = second.as_ptr().cast::<Header>();
    unsafe { instance.incref_object(second_header.as_ptr()) };
    *first.get().next.borrow_mut() = Some(second_header);
    unsafe { instance.incref_object(first_header.as_ptr()) };
    *second.get().next.borrow_mut() = Some(first_header);
    drop(first);
    drop(second);

    // 第一个终结的对象把自己复活成外部引用 ⇒ 重判定后整环可达，一个都不许释放
    assert_eq!(instance.collect(), 0, "OM-27：复活的对象必须被重新判定为可达");
    assert_eq!(instance.live_objects(), 2);
    assert_eq!(snapshot(&SURVIVOR_LOG), vec!["finalize", "finalize"]);

    // 还掉复活出来的那份引用，环重新变成垃圾；这次没有复活，走完释放
    unsafe { instance.release_object(second_header.as_ptr()) };
    assert_eq!(instance.collect(), 2);
    assert_eq!(instance.live_objects(), 0);
    assert_eq!(
        snapshot(&SURVIVOR_LOG),
        vec![
            "finalize", "finalize", // 第一次回收：两个终结器，其中一个复活
            "finalize", "finalize", // 第二次回收：FINALIZING 已清，可以再终结一次（OM-20）
            "clear", "clear", "free", "free",
        ]
    );
}

#[test]
fn auto_collection_triggers_at_threshold() {
    reset(&THRESHOLD_LOG);
    let instance = Instance::new();
    instance.set_gc_threshold(8);
    let node_ty = node_type(&instance);
    let leaf_ty = leaf_type(&instance);

    make_cycle(&instance, node_ty, Some(&THRESHOLD_LOG));
    assert_eq!(instance.tracked_objects(), 2, "OM-26：阈值还没到");

    for _ in 0..6 {
        let _ = instance.alloc(Leaf::new(leaf_ty, Cell::new(0)));
    }

    assert_eq!(instance.tracked_objects(), 0, "OM-26：分配计数达阈值即自动回收");
    assert_eq!(snapshot(&THRESHOLD_LOG), vec!["free", "free"]);
}

#[test]
fn deep_chain_is_released_without_recursion() {
    // T-OM-5 要求 ≥1e6；默认就按 1e6 跑（调试构建约 1.3 s），可用 PYAWA_CHAIN_LEN 调小。
    // 释放走的是显式待处理栈（OM-21），与链长无关，不会消耗 C 栈。
    let length: usize = std::env::var("PYAWA_CHAIN_LEN")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000_000);
    let instance = Instance::new();
    instance.set_gc_threshold(usize::MAX); // 本测试只验证释放不递归，不掺自动回收
    let ty = node_type(&instance);

    let mut tail = instance.alloc(Node::new(ty, RefCell::new(None), None));
    for _ in 1..length {
        let child: NonNull<Header> = tail.as_ptr().cast::<Header>();
        let parent = instance.alloc(Node::new(ty, RefCell::new(Some(child)), None));
        unsafe { instance.incref_object(child.as_ptr()) };
        drop(tail); // 引用已转交到 parent 的载荷里
        tail = parent;
    }
    assert_eq!(instance.tracked_objects(), length);

    drop(tail);

    assert_eq!(instance.live_objects(), 0, "OM-21：深链必须释放且不递归");
    assert_eq!(instance.tracked_objects(), 0);
    assert_eq!(
        instance.bytes_allocated(),
        core::mem::size_of::<TypeObject>() * 2
    );
}

#[test]
fn finalizer_releasing_a_cycle_peer_does_not_double_free() {
    reset(&PEER_LOG);
    let instance = Instance::new();
    let ty = node_type_with_releasing_finalizer(&instance);
    make_cycle(&instance, ty, Some(&PEER_LOG));

    assert_eq!(
        instance.collect(),
        2,
        "OM-27：终结器在终结阶段释放环内引用，释放阶段仍必须各释放一次"
    );
    assert_eq!(instance.live_objects(), 0);
    assert_eq!(snapshot(&PEER_LOG), vec!["clear", "clear", "free", "free"]);
}

#[test]
fn nested_collection_from_a_finalizer_is_deferred() {
    reset(&REENTRY_LOG);
    let instance = Instance::new();
    instance.set_gc_threshold(1); // 任何一次分配都会尝试触发回收
    let ty = node_type_with_allocating_finalizer(&instance);
    make_cycle(&instance, ty, Some(&REENTRY_LOG));

    assert_eq!(instance.collect(), 2, "嵌套回收必须让路，由外层完成本次回收");
    assert_eq!(instance.live_objects(), 0, "终结器里分配的临时对象也要各归各位");
    assert_eq!(
        snapshot(&REENTRY_LOG),
        vec!["finalize", "finalize", "free", "free"]
    );
}
