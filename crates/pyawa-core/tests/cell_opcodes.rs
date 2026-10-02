//! **cell 族指令**（`BC-45`）的验收：`MAKE_CELL`／`LOAD_LOCALS`／`STORE_DEREF`／`LOAD_DEREF`。
//!
//! 这是**类体带 `def`** 那条路径要用的形状（参照实测：类体序言后 `LOAD_LOCALS; STORE_DEREF 0`，
//! 收尾前 `LOAD_FAST_BORROW 0; STORE_NAME __classdictcell__`）。
//! 这里手工搭一个"类体形状"的帧（无局部槽、一个 cellvar、带命名空间），验四个指令合起来
//! 能把命名空间**存进 cell 再取回来**。

mod common;

use common::{assemble, op, Item, Vm};

use pyawa_core::Frame;

#[test]
fn the_cell_family_round_trips_the_namespace() {
    let vm = Vm::new();
    let code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "C",
        "C".to_owned(),
        "<t>".to_owned(),
        1,
        4,
        0,
        0,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        // cellvars：一个 `__classdict__`
        vec!["__classdict__".to_owned()],
        Vec::new(),
        assemble(&[
            // 参照的顺序是 `MAKE_CELL` 在 `RESUME` **之前**（实测）
            Item::Instr(op("MAKE_CELL"), 0),
            Item::Instr(op("RESUME"), 0),
            // 序言之后：把类命名空间存进 cell
            Item::Instr(op("LOAD_LOCALS"), 0),
            Item::Instr(op("STORE_DEREF"), 0),
            // 再取回来当返回值（验 `LOAD_DEREF` 真的读的是那个 cell）
            Item::Instr(op("LOAD_DEREF"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ));
    let namespace = vm.instance.new_dict();
    // `Frame::for_code_with_namespace` **接手**一份命名空间引用 ⇒ 先补一份（`OM-16`）
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame).expect("四个指令应当都能跑") {
        pyawa_core::ExecOutcome::Returned(value) => match value {
            pyawa_core::Value::Object(object) => assert_eq!(
                object.as_ptr(),
                namespace,
                "`LOAD_LOCALS → STORE_DEREF → LOAD_DEREF` 走一圈应当还是那个命名空间"
            ),
            other => panic!("应当返回对象，实际 {other:?}"),
        },
        pyawa_core::ExecOutcome::Yielded(_) => panic!("不该 yield"),
    }
}
