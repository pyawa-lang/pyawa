//! 执行器的可测性质（`docs/SPEC-bytecode.md` §10 的起步指令集，`BC-49`）。
//!
//! 本片覆盖"直线代码 ＋ 整数运算 ＋ 返回"；控制流与调用尚未接线（见 `lib.rs` 的清单），
//! 因此这里的程序**不含跳转**。

mod common;

use pyawa_core::flags;
use pyawa_core::{ExecError, Frame, Value};

use common::{assemble, emit, less_than, nb, op, Item, Vm};

#[test]
fn emitted_bytes_pass_the_decoder_check() {
    let bytes = emit(&[
        (op("RESUME"), 0),
        (op("LOAD_CONST"), 0),
        (op("BINARY_OP"), nb("NB_ADD")),
        (op("RETURN_VALUE"), 0),
    ]);
    assert_eq!(
        pyawa_core::decode::validate(&bytes),
        Ok(()),
        "BC-35：带 cache 的指令必须留等宽零填充槽"
    );
    // 把 `BINARY_OP` 的 cache 槽切掉两个 ⇒ 必须报错（BC-35）
    let mut short = bytes.clone();
    short.truncate(short.len() - 4);
    assert_eq!(
        pyawa_core::decode::validate(&short),
        Err(pyawa_core::decode::DecodeError::MissingCacheSlots {
            offset: 2,
            opcode: op("BINARY_OP"),
            expected: 5,
            found: 4,
        }),
        "BC-35：cache 槽不够必须报错"
    );
}

#[test]
fn straight_line_program_returns_the_sum() {
    let vm = Vm::new();
    let consts = vec![Some(vm.constant(1)), Some(vm.constant(2))];
    let code = vm.code(
        4,
        1,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_FAST"), 0),
            (op("BINARY_OP"), nb("NB_ADD")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );

    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(3), &vm.instance),
        "2 + 1 应当得到 3"
    );
}

#[test]
fn instruction_pointer_follows_the_return() {
    let vm = Vm::new();
    let consts = vec![Some(vm.constant(7))];
    let code = vm.code(
        2,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let frame = vm.instance.alloc(Frame::for_code(vm.frame_type, &code));

    let result = common::execute_value(&vm.instance, &frame).unwrap();
    assert!(result.is_same(&Value::small_int(7), &vm.instance));
    assert_eq!(frame.get().instruction_pointer(), 2, "停在 RETURN_VALUE");
    assert_eq!(frame.get().depth(), 0, "返回值已出栈");
}

#[test]
fn unbound_local_is_reported() {
    let vm = Vm::new();
    let code = vm.code(
        2,
        2,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_FAST"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    // **第 277 轮改口径** ✓：不再报内部的 `UnboundLocal { slot }` ✗，而是照参照抛
    // `UnboundLocalError: cannot access local variable '<名>' …` ✓（更贴参照 ✓、也更好查 ✓）。
    // 本用例的代码对象**没有 `varnames`** ⇒ 名字回落成 `<未知>` ✓。
    assert!(matches!(vm.run(&code), Err(ExecError::Raised { .. })));
    let (kind, message) = vm.pending_exception().expect("未绑定的局部必须变成未捕获异常");
    assert_eq!(kind, "UnboundLocalError");
    assert_eq!(
        message.as_deref(),
        Some("cannot access local variable '<未知>' where it is not associated with a value")
    );
}

#[test]
fn results_outside_the_singleton_range_are_real_integers() {
    // `200 * 200 = 40000` 不在单例区间（`-5..=256`）——但单例表只决定"内联还是分配"，
    // **不是值域**（`TS-45` 的任意精度在 `P1-11` 已落地）。
    // 这条测试早先断言"报 `IntOutOfRange`（需要大整数）"；那套 i64／单例假设在
    // 2026-10-02 被新的 `big_int_add` 对拍语料**抓出来**并作废（`BINARY_OP` 现在走公开入口）。
    let vm = Vm::new();
    let consts = vec![Some(vm.constant(200)), Some(vm.constant(200))];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb("NB_MULTIPLY")),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).expect("40000 是合法整数");
    let raw = result.as_header(&vm.instance).expect("整数是对象");
    assert_eq!(vm.instance.int_value(raw), Some(40000));
    // 越过 `i64` 也照样算（`a + 1`，`a` 是 `i64::MAX`：走 BigInt）
    let big = vm.instance.new_int(i64::MAX);
    let one = vm.instance.new_int(1);
    let sum = pyawa_core::executor::arithmetic_public(
        &vm.instance,
        big,
        one,
        "+",
        0,
    )
    .expect("i64::MAX + 1");
    assert_eq!(
        vm.instance
            .int_of(sum)
            .expect("是整数")
            .to_decimal(),
        "9223372036854775808"
    );
}

#[test]
fn truthiness_and_unary_not() {
    let vm = Vm::new();
    let code = vm.code(
        2,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("TO_BOOL"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(0))],
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::Bool(false), &vm.instance),
        "0 为假"
    );

    let code = vm.code(
        2,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("UNARY_NOT"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(1))],
    );
    let result = vm.run(&code).unwrap();
    assert!(result.is_same(&Value::Bool(false), &vm.instance), "not 1 为假");
}

#[test]
fn instructions_outside_this_slice_are_reported() {
    let vm = Vm::new();
    // 这条用例的用途是"切片外的指令要如实报告"，所以挑一条**当前确实**没实现的。
    // 注意：每次接线一条指令，这里就要换一条（`CONTAINS_OP` 与 `WITH_EXCEPT_START`
    // 都已接线）——目前用的是 `LOAD_SUPER_ATTR`（要 `super()` 的 `__class__` cell，
    // `lib.rs` 的「尚未接线」节里记着）。
    let code = vm.code(
        4,
        0,
        emit(&[(op("LOAD_SUPER_ATTR"), 0), (op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    assert!(matches!(
        vm.run(&code),
        Err(ExecError::NotImplemented { opcode }) if opcode == op("LOAD_SUPER_ATTR")
    ));

    // 码元跑完却没有 RETURN_VALUE
    let code = vm.code(1, 0, emit(&[(op("NOP"), 0)]), Vec::new());
    assert!(matches!(vm.run(&code), Err(ExecError::FellOffEnd)));
}

#[test]
fn code_objects_own_their_constant_table() {
    let vm = Vm::new();
    let base_live = vm.instance.live_objects();

    let constant = vm.constant(42); // 那份新引用直接交给常量表
    let code = vm.code(1, 0, emit(&[(op("RETURN_VALUE"), 0)]), vec![Some(constant)]);

    assert_eq!(
        vm.instance.live_objects(),
        base_live + 2,
        "常量对象 + code object"
    );
    assert_eq!(code.get().const_count(), 1);
    assert_eq!(code.get().constant(0), Some(constant), "BC-4：借用取回同一个对象");
    assert!(
        // SAFETY: code 此刻存活。
        code.header().has_flag(flags::GC_TRACKED),
        "OM-12：常量表持有引用 ⇒ code object 必须入回收链"
    );

    drop(code);
    assert_eq!(
        vm.instance.live_objects(),
        base_live,
        "OM-40：常量表在 clear 里交出引用，常量随之释放"
    );
}

#[test]
fn while_loop_counts_to_three() {
    // `x = 0; while x < 3: x = x + 1; return x` —— 控制流 ＋ 整数运算 ＋ 比较
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(0)),
        Some(vm.constant(3)),
        Some(vm.constant(1)),
    ];
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Label("L1"),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("COMPARE_OP"), less_than()),
        Item::Jump(op("POP_JUMP_IF_FALSE"), "L2"),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
        Item::Instr(op("STORE_FAST"), 0),
        Item::Jump(op("JUMP_BACKWARD"), "L1"),
        Item::Label("L2"),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    assert_eq!(
        pyawa_core::decode::validate(&bytes),
        Ok(()),
        "BC-35：汇编出来的码元必须过体检"
    );

    let code = vm.code(4, 1, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(3), &vm.instance),
        "循环跑完 x 应当是 3"
    );
}

#[test]
fn bool_is_a_subtype_of_int() {
    // TS-40：`bool ⊂ int`——层次、子类型判定与运算三处都要成立
    let vm = Vm::new();
    let singletons = vm.instance.singletons();
    assert!(
        vm.instance
            .is_subtype(singletons.bool_type(), singletons.int_type()),
        "TS-40：isinstance(True, int) 必须为真"
    );
    assert!(
        !vm.instance
            .is_subtype(singletons.int_type(), singletons.bool_type()),
        "反向不成立"
    );
    // SAFETY: 两个类型都由实例持有。
    let bool_bases = unsafe { singletons.bool_type().as_ref() }.bases();
    assert_eq!(bool_bases, vec![singletons.int_type()], "bool 的基类是 int");

    // True + 1 == 2（TS-40 点名的那条：禁止把 bool 与 int 当成不相干的两种类型）
    let truth = vm.instance.own(singletons.boolean(true)).into_raw();
    let consts = vec![Some(truth), Some(vm.constant(1))];
    let bytes = emit(&[
        (op("LOAD_CONST"), 0),
        (op("LOAD_CONST"), 1),
        (op("BINARY_OP"), nb("NB_ADD")),
        (op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code(4, 0, bytes, consts);
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(2), &vm.instance),
        "TS-40：True + 1 必须等于 2"
    );
}

#[test]
fn identity_is_object_identity() {
    let vm = Vm::new();
    // 每个 code object 都要有**自己**的常量对象（常量表持有那些引用，OM-40）
    let constants = |vm: &Vm| {
        vec![
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
            Some(vm.instance.own(vm.instance.singletons().boolean(true)).into_raw()),
            Some(vm.constant(1)),
        ]
    };

    // None is None ⇒ 真
    let code = vm.code(
        2,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 0),
            (op("IS_OP"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        constants(&vm),
    );
    assert!(vm
        .run(&code)
        .unwrap()
        .is_same(&Value::Bool(true), &vm.instance));

    // True is 1 ⇒ 假（OM-39：`is` 按对象身份，不按数值）
    let code = vm.code(
        2,
        0,
        emit(&[
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("IS_OP"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        constants(&vm),
    );
    assert!(vm
        .run(&code)
        .unwrap()
        .is_same(&Value::Bool(false), &vm.instance));
}

#[test]
fn load_small_int_pushes_the_argument() {
    // 3.14 的新指令：`oparg` 直接当小整数（不走常量表）。实测净 +1。
    let vm = Vm::new();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_SMALL_INT"), 7),
            (op("LOAD_SMALL_INT"), 35),
            (op("BINARY_OP"), nb("NB_ADD")),
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(42), &vm.instance),
        "7 + 35 应当走小整数路径得到 42"
    );
}
