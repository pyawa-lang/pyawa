//! 帧布局的可测性质（`docs/SPEC-bytecode.md` §9：**BC-42**…**BC-48**，以及 **BC-54** 的存储侧）。
//!
//! 本层只验**布局与托管**（值栈进出、局部槽与 cell 槽、可挂起状态、引用归属）；
//! 指令语义属执行器，还没落地。

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;

use pyawa_core::flags;
use pyawa_core::{
    py_object, CellObject, CodeObject, Frame, FrameError, Header, Instance, Owned, Slots, TypeObject,
};

py_object! {
    /// 值栈／槽位上的载荷：叶子对象，不持有任何引用。
    struct Leaf {
        value: Cell<u32>,
    }
}

struct Fixture {
    instance: Instance,
    code_type: NonNull<TypeObject>,
    frame_type: NonNull<TypeObject>,
    cell_type: NonNull<TypeObject>,
    leaf_type: NonNull<TypeObject>,
}

fn fixture() -> Fixture {
    let instance = Instance::new();
    let code_type = instance.new_type(
        "CodeObject",
        core::mem::size_of::<CodeObject>(),
        CodeObject::slots(),
    );
    let frame_type = instance.new_type("Frame", core::mem::size_of::<Frame>(), Frame::slots());
    let cell_type = instance.new_type(
        "Cell",
        core::mem::size_of::<CellObject>(),
        CellObject::slots(),
    );
    let leaf_type = instance.new_type(
        "Leaf",
        core::mem::size_of::<Leaf>(),
        Slots::new(Leaf::dealloc),
    );
    Fixture {
        instance,
        code_type,
        frame_type,
        cell_type,
        leaf_type,
    }
}

/// 造一个 code object：`code` 每码元 2 字节（`BC-33`）。
fn code(fixture: &Fixture, stacksize: usize, nlocals: usize, ncells: usize, nfree: usize) -> Owned<'_, CodeObject> {
    fixture.instance.alloc(CodeObject::new(
        fixture.code_type,
        "demo",
        "demo".to_owned(),
        "<pyawa-test>".to_owned(),
        0,
        stacksize,
        nlocals,
        0,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        ncells,
        nfree,
        vec![0, 0, 1, 0, 2, 0],
        vec![0b1000_0001, 0x02],
        Vec::new(),
    ))
}

fn leaf(fixture: &Fixture, value: u32) -> Owned<'_, Leaf> {
    fixture.instance.alloc(Leaf::new(fixture.leaf_type, Cell::new(value)))
}

/// 为"存进帧／cell"再取一个引用（`OM-16`：载荷持有一份**新引用**）。
fn share(instance: &Instance, raw: NonNull<Header>) -> NonNull<Header> {
    // SAFETY: raw 来自本实例的存活对象。
    unsafe { instance.incref_object(raw.as_ptr()) };
    raw
}

#[test]
fn frame_has_the_required_pieces() {
    let fixture = fixture();
    let code = code(&fixture, 4, 3, 1, 1);
    let frame = fixture
        .instance
        .alloc(Frame::for_code(fixture.frame_type, &code));

    assert_eq!(frame.get().stacksize(), 4, "BC-42／BC-43");
    assert_eq!(frame.get().depth(), 0);
    assert_eq!(frame.get().local_count(), 3, "BC-44：长度 = co_nlocals");
    assert_eq!(frame.get().cell_count(), 2, "BC-45：cellvars + freevars");
    assert_eq!(frame.get().instruction_pointer(), 0, "BC-42");
    assert_eq!(frame.get().exception_cursor(), 0, "BC-42");
    assert!(!frame.get().is_suspended(), "BC-47");
    assert_eq!(
        frame.get().code(),
        Some(code.as_ptr().cast::<Header>()),
        "BC-42：帧持有 code object 的引用"
    );
    assert!(
        frame.header().has_flag(flags::GC_TRACKED),
        "OM-12：帧可成环（帧 ↔ cell），必须入回收链表"
    );
    // 帧与 code object 都入回收链：前者可成环（帧 ↔ cell），后者持有常量表（`OM-12`）
    assert_eq!(fixture.instance.tracked_objects(), 2);
}

#[test]
fn stack_is_bounded_by_stacksize() {
    let fixture = fixture();
    let code = code(&fixture, 3, 0, 0, 0);
    let frame = fixture
        .instance
        .alloc(Frame::for_code(fixture.frame_type, &code));
    let leaf = leaf(&fixture, 7);
    let raw = leaf.as_ptr().cast::<Header>();

    for _ in 0..3 {
        assert_eq!(frame.get().push(share(&fixture.instance, raw)), Ok(()));
    }
    assert_eq!(frame.get().depth(), 3);

    let overflow = share(&fixture.instance, raw);
    assert_eq!(
        frame.get().push(overflow),
        Err(FrameError::StackOverflow { capacity: 3 }),
        "BC-43：越界必须报错"
    );
    assert_eq!(frame.get().depth(), 3, "BC-43：禁止静默扩容");
    // 失败时引用仍归调用方
    // SAFETY: overflow 是刚取的新引用。
    unsafe { fixture.instance.release_object(overflow.as_ptr()) };

    for _ in 0..3 {
        let popped = frame.get().pop().unwrap();
        // SAFETY: pop 交出的是一份新引用。
        unsafe { fixture.instance.release_object(popped.as_ptr()) };
    }
    assert_eq!(
        frame.get().pop(),
        Err(FrameError::StackUnderflow),
        "BC-43：空栈弹出必须报错而不是 UB"
    );
}

#[test]
fn stack_items_hold_references() {
    let fixture = fixture();
    let code = code(&fixture, 2, 0, 0, 0);
    let frame = fixture
        .instance
        .alloc(Frame::for_code(fixture.frame_type, &code));
    let leaf = leaf(&fixture, 1);
    let raw = leaf.as_ptr().cast::<Header>();
    assert_eq!(leaf.refcount(), 1);

    let shared = share(&fixture.instance, raw);
    assert_eq!(leaf.refcount(), 2, "BC-46：栈项持有一个引用");
    frame.get().push(shared).unwrap();

    let popped = frame.get().pop().unwrap();
    assert_eq!(popped, raw);
    // SAFETY: pop 交出的是一份新引用。
    unsafe { fixture.instance.release_object(popped.as_ptr()) };
    assert_eq!(leaf.refcount(), 1);
}

#[test]
fn dropping_the_frame_releases_everything_it_holds() {
    let fixture = fixture();
    let baseline = fixture.instance.live_objects();
    let leaf = leaf(&fixture, 1);
    let raw = leaf.as_ptr().cast::<Header>();

    {
        let code = code(&fixture, 2, 1, 1, 0);
        let frame = fixture
            .instance
            .alloc(Frame::for_code(fixture.frame_type, &code));
        frame.get().push(share(&fixture.instance, raw)).unwrap();
        assert!(frame
            .get()
            .set_local(0, Some(share(&fixture.instance, raw)))
            .unwrap()
            .is_none());
        assert!(frame
            .get()
            .set_cell(0, Some(share(&fixture.instance, raw)))
            .unwrap()
            .is_none());
        assert_eq!(leaf.refcount(), 4, "叶子 + 值栈 + 局部槽 + cell 槽");
    }

    assert_eq!(
        leaf.refcount(),
        1,
        "BC-46／OM-20 ②：帧销毁时把持有的引用全部交出并释放"
    );
    drop(leaf);
    assert_eq!(fixture.instance.live_objects(), baseline);
    assert_eq!(fixture.instance.tracked_objects(), 0);
}

#[test]
fn suspend_and_resume_restore_the_resume_point() {
    let fixture = fixture();
    let code = code(&fixture, 4, 0, 0, 0);
    let frame = fixture
        .instance
        .alloc(Frame::for_code(fixture.frame_type, &code));
    let leaf = leaf(&fixture, 1);
    let raw = leaf.as_ptr().cast::<Header>();

    frame.get().set_instruction_pointer(5);
    frame.get().set_exception_cursor(3);
    for _ in 0..2 {
        frame.get().push(share(&fixture.instance, raw)).unwrap();
    }

    frame.get().suspend().unwrap();
    assert!(frame.get().is_suspended(), "BC-47");
    assert_eq!(frame.get().depth(), 0, "BC-47：值栈被搬进恢复点");
    assert_eq!(
        frame.get().resume_point(),
        Some((5, 3, 2)),
        "BC-47：恢复点 = 指令指针 ＋ 值栈镜像 ＋ 异常表游标"
    );

    frame.get().resume().unwrap();
    assert!(!frame.get().is_suspended());
    assert_eq!(frame.get().depth(), 2, "BC-47：值栈镜像还原");
    assert_eq!(frame.get().instruction_pointer(), 5);
    assert_eq!(frame.get().exception_cursor(), 3);
    assert_eq!(frame.get().resume_point(), None);

    frame.get().suspend().unwrap();
    assert_eq!(
        frame.get().suspend(),
        Err(FrameError::WrongSuspendState { suspended: true })
    );
    frame.get().resume().unwrap();
    assert_eq!(
        frame.get().resume(),
        Err(FrameError::WrongSuspendState { suspended: false })
    );
    assert_eq!(leaf.refcount(), 3, "叶子 + 挂起前入栈的两项");
}

#[test]
fn slots_are_bounded() {
    let fixture = fixture();
    let code = code(&fixture, 1, 2, 1, 0);
    let frame = fixture
        .instance
        .alloc(Frame::for_code(fixture.frame_type, &code));

    assert_eq!(
        frame.get().local(2),
        Err(FrameError::SlotOutOfRange { slot: 2, count: 2 }),
        "BC-44／BC-42：局部槽越界必须报错"
    );
    assert_eq!(
        frame.get().set_local(2, None),
        Err(FrameError::SlotOutOfRange { slot: 2, count: 2 })
    );
    assert_eq!(frame.get().local(0), Ok(None));
    assert_eq!(
        frame.get().cell(1),
        Err(FrameError::SlotOutOfRange { slot: 1, count: 1 }),
        "BC-45：cell 槽独立编号，越界必须报错"
    );
    assert_eq!(frame.get().cell(0), Ok(None));
}

#[test]
fn cell_cycles_are_collected() {
    let fixture = fixture();
    let base_live = fixture.instance.live_objects();
    let first = fixture.instance.alloc(CellObject::new(
        fixture.cell_type,
        RefCell::new(None),
    ));
    let second = fixture.instance.alloc(CellObject::new(
        fixture.cell_type,
        RefCell::new(Some(share(&fixture.instance, first.as_ptr().cast::<Header>()))),
    ));
    // first → second，构成环
    assert!(first
        .get()
        .replace(Some(share(&fixture.instance, second.as_ptr().cast::<Header>())))
        .is_none());

    assert!(
        first.header().has_flag(flags::GC_TRACKED),
        "BC-45：cell 必须是 GC_TRACKED"
    );
    assert_eq!(fixture.instance.tracked_objects(), 2, "OM-25：入回收链表");

    drop(first);
    drop(second);
    assert_eq!(fixture.instance.live_objects(), base_live + 2, "环靠计数收不掉");

    assert_eq!(
        fixture.instance.collect(),
        2,
        "BC-45：cell 成环也要被回收（traverse／clear 必须完整）"
    );
    assert_eq!(fixture.instance.live_objects(), base_live);
    assert_eq!(fixture.instance.tracked_objects(), 0);
}

#[test]
fn code_object_keeps_the_byte_strings() {
    let fixture = fixture();
    let code = code(&fixture, 2, 1, 0, 0);

    assert_eq!(code.get().name(), "demo");
    assert_eq!(code.get().code(), &[0, 0, 1, 0, 2, 0]);
    assert_eq!(code.get().instruction_count(), 3, "BC-33：每码元 2 字节");
    assert_eq!(
        code.get().exceptiontable(),
        &[0b1000_0001, 0x02],
        "BC-54：异常表按字节串原样保存（解析随执行器补）"
    );
    assert_eq!(code.get().nfreevars(), 0);
    assert_eq!(code.get().const_count(), 0, "BC-4：常量表可以为空");
}

#[test]
fn t_om_9_container_payloads_are_released_only_by_clear() {
    // T-OM-9：容器载荷按 `OM-40` 释放——构造容器 → 释放 → 子对象计数正确归零；
    // 且"除 `clear` 外无释放路径"：载荷里不得含会自行释放子引用的 Rust 析构。
    let fixture = fixture();
    let baseline = fixture.instance.live_objects();

    let leaf = leaf(&fixture, 3);
    let raw = leaf.as_ptr().cast::<Header>();

    {
        let code = code(&fixture, 2, 0, 0, 0);
        let frame = fixture
            .instance
            .alloc(Frame::for_code(fixture.frame_type, &code));
        frame.get().push(share(&fixture.instance, raw)).unwrap();

        let cell = fixture.instance.alloc(CellObject::new(
            fixture.cell_type,
            RefCell::new(Some(share(&fixture.instance, raw))),
        ));
        assert_eq!(leaf.refcount(), 3, "叶子 + 帧的值栈 + cell 的载荷");

        drop(cell);
        assert_eq!(leaf.refcount(), 2, "cell 释放时经 `clear` 交出引用");
        drop(frame);
        assert_eq!(leaf.refcount(), 1, "帧释放时经 `clear` 交出引用");
    }

    drop(leaf);
    assert_eq!(fixture.instance.live_objects(), baseline, "子对象计数正确归零");

    // 结构半：载荷类型不含任何会触碰子对象的 Rust 侧析构（`OM-40` 禁止"持有子引用的 Drop"），
    // 也不含对实例的借用——`Owned` 守卫带生命周期参数，真·载荷里存不进去。
    assert!(
        !core::mem::needs_drop::<CellObject>(),
        "cell 载荷不得含 Rust 侧析构：引用只允许经 `clear` 交出"
    );
    fn assert_static<T: 'static>() {}
    assert_static::<CellObject>();
    assert_static::<Frame>();
}
