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
