//! 对象模型的可测性质（`docs/SPEC-object-model.md` §13 中本层已能成立的部分）。
//!
//! 每个测试在注释里标出它对应的 `OM-n`／`T-OM-n`；尚未能验证的 T-OM 项
//! （深链表回收、宿主对象 traverse、CI 静态检查）等对应实现落地后再补。

use core::cell::Cell;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use pyawa_core::{py_object, flags, Header, Instance, PyObject, Slots, TypeObject};

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

static ORDER_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static RESURRECT_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static TRACKED_DROPS: AtomicUsize = AtomicUsize::new(0);

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

fn leaf_type(instance: &Instance) -> core::ptr::NonNull<TypeObject> {
    instance.new_type(
        "Leaf",
        core::mem::size_of::<Leaf>(),
        Slots::new(Leaf::dealloc),
    )
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
}
