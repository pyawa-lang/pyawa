//! 执行器的可测性质（`docs/SPEC-bytecode.md` §10 的起步指令集，`BC-49`）。
//!
//! 本片覆盖"直线代码 ＋ 整数运算 ＋ 返回"；控制流与调用尚未接线（见 `lib.rs` 的清单），
//! 因此这里的程序**不含跳转**。

use core::ptr::NonNull;

use pyawa_core::flags;
use pyawa_core::opcode;
use pyawa_core::{execute, CodeObject, ExecError, Frame, Header, Instance, IntObject, TypeObject, Value};

fn op(name: &str) -> u8 {
    opcode::opcode(name).unwrap_or_else(|| panic!("opmap 缺 {name}")) as u8
}

/// `BINARY_OP` 的 oparg 从 `get_nb_ops()` 的顺序取（`BC-39`／`BC-50`：不写死编号）。
fn nb(name: &str) -> u8 {
    opcode::get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// 按 `BC-35` 发射：每条指令后面留**等宽零填充** cache 槽（宽度从指令表取）。
///
/// 手写码元最容易漏掉这些槽——本文件的第一个版本就是漏了，解码器按规矩报了
/// `MissingCacheSlots`。将来的编译器要做同一件事（`T-BC-12`）。
fn emit(instructions: &[(u8, u8)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (opcode, oparg) in instructions {
        bytes.extend([*opcode, *oparg]);
        let cache = opcode::inline_cache_entries(u16::from(*opcode));
        for _ in 0..cache {
            bytes.extend([0, 0]);
        }
    }
    bytes
}

/// 带标签的汇编条目：跳转的 oparg 由 [`assemble`] 按 `BC-55` **反解**。
enum Item {
    Instr(u8, u8),
    Label(&'static str),
    Jump(u8, &'static str),
}

/// 把带标签的条目汇编成码元（按实测宽度补 cache 槽）。
///
/// 跳转的 oparg：目标在前取 `+|差|`、在后取 `−|差|`——与 `BC-55` 的读法互为逆运算，
/// 因此"汇编 → 解码 → `jump_target`"能回到原标签（本文件的循环用例正是这么做的）。
fn assemble(items: &[Item]) -> Vec<u8> {
    let mut offsets: Vec<usize> = Vec::new();
    let mut labels: Vec<(&str, usize)> = Vec::new();
    let mut position = 0usize;
    for item in items {
        match item {
            Item::Label(name) => labels.push((name, position)),
            Item::Instr(opcode, _) | Item::Jump(opcode, _) => {
                offsets.push(position);
                position += 1 + opcode::inline_cache_entries(u16::from(*opcode)) as usize;
            }
        }
    }

    let mut bytes = Vec::new();
    let mut index = 0usize;
    for item in items {
        match item {
            Item::Label(_) => continue,
            Item::Instr(opcode, oparg) => {
                bytes.extend([*opcode, *oparg]);
                pad_cache(*opcode, &mut bytes);
            }
            Item::Jump(opcode, label) => {
                let target = labels
                    .iter()
                    .find(|(name, _)| name == label)
                    .unwrap_or_else(|| panic!("没有这个标签：{label}"))
                    .1;
                let caches = opcode::inline_cache_entries(u16::from(*opcode)) as usize;
                let base = offsets[index] + 1 + caches;
                let argument = if target >= base { target - base } else { base - target };
                assert!(argument <= u8::MAX as usize, "oparg 装不进一个字节：{argument}");
                bytes.extend([*opcode, argument as u8]);
                pad_cache(*opcode, &mut bytes);
            }
        }
        index += 1;
    }
    bytes
}

fn pad_cache(opcode_number: u8, bytes: &mut Vec<u8>) {
    for _ in 0..opcode::inline_cache_entries(u16::from(opcode_number)) {
        bytes.extend([0, 0]);
    }
}

struct Vm {
    instance: Instance,
    code_type: NonNull<TypeObject>,
    frame_type: NonNull<TypeObject>,
}

impl Vm {
    fn new() -> Self {
        let instance = Instance::new();
        let code_type = instance.new_type(
            "CodeObject",
            core::mem::size_of::<CodeObject>(),
            CodeObject::slots(),
        );
        let frame_type = instance.new_type("Frame", core::mem::size_of::<Frame>(), Frame::slots());
        Vm {
            instance,
            code_type,
            frame_type,
        }
    }

    /// 造一个 `int` 常量对象，**把那份新引用交出去**（由常量表接管）。
    fn constant(&self, value: i64) -> NonNull<Header> {
        let object = self
            .instance
            .alloc(IntObject::new(self.instance.singletons().int_type(), value));
        object.into_raw().cast::<Header>()
    }

    fn code(
        &self,
        stacksize: usize,
        nlocals: usize,
        bytes: Vec<u8>,
        consts: Vec<Option<NonNull<Header>>>,
    ) -> pyawa_core::Owned<'_, CodeObject> {
        self.instance.alloc(CodeObject::new(
            self.code_type,
            "demo",
            stacksize,
            nlocals,
            0,
            0,
            bytes,
            Vec::new(),
            consts,
        ))
    }

    fn run(&self, code: &pyawa_core::Owned<'_, CodeObject>) -> Result<Value<'_>, ExecError> {
        let frame = self.instance.alloc(Frame::for_code(self.frame_type, code));
        execute(&self.instance, &frame)
    }
}

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

    let result = execute(&vm.instance, &frame).unwrap();
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
    assert!(matches!(
        vm.run(&code),
        Err(ExecError::UnboundLocal { slot: 1 })
    ));
}

#[test]
fn out_of_range_result_is_refused_not_wrapped() {
    let vm = Vm::new();
    // 200 * 200 = 40000，超出单例区间：本层**不**回绕，直接报"需要大整数"
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
    assert!(matches!(
        vm.run(&code),
        Err(ExecError::IntOutOfRange { value: 40000 })
    ));
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
    let code = vm.code(
        2,
        0,
        emit(&[(op("LOAD_GLOBAL"), 0), (op("RETURN_VALUE"), 0)]),
        Vec::new(),
    );
    assert!(matches!(
        vm.run(&code),
        Err(ExecError::NotImplemented { opcode }) if opcode == op("LOAD_GLOBAL")
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
        unsafe { code.header().has_flag(flags::GC_TRACKED) },
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

/// `COMPARE_OP` 的 `<` 在 `opcode.cmp_op` 六元组里的下标（`BC-39`：从表里取）。
fn less_than() -> u8 {
    opcode::get_cmp_op()
        .iter()
        .position(|name| *name == "<")
        .expect("cmp_op 里应当有 <") as u8
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
    let none = vm.instance.own(vm.instance.singletons().none()).into_raw();
    let true_value = vm.instance.own(vm.instance.singletons().boolean(true)).into_raw();
    let one = vm.constant(1);

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
        vec![Some(none), Some(true_value), Some(one)],
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
        vec![Some(none), Some(true_value), Some(one)],
    );
    assert!(vm
        .run(&code)
        .unwrap()
        .is_same(&Value::Bool(false), &vm.instance));
}
