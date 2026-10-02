//! `BC-23` 的边界检查（`P3-13`）：两条 Pyawa 专有指令的**语义**与元数据。
//!
//! 验收照 `SPEC-bytecode.md` 的 `T-BC-9` 与 `SPEC-type-system.md` 的 `TS-10`…`TS-13`／
//! `TS-28`…`TS-31`：失败携带方向、期望、实际、文件名与行号；`bool ⊂ int`（`TS-40`）；
//! `Any` 双向相容；浅层只判外类型。

mod common;

use core::ptr::NonNull;

use pyawa_core::{ExecError, Frame, Header, Value};

use common::{assemble, op, Item, Vm};

/// `OM-16`：给帧的局部槽再要一份引用（`set_local` **接手**新引用）。
fn share(vm: &Vm, object: NonNull<Header>) -> NonNull<Header> {
    // SAFETY: 调用方保证 object 存活；这里只是加一份计数。
    unsafe { vm.instance.incref_object(object.as_ptr()) };
    object
}

/// 一个签名条目的三种标签形态：类型对象、`"Any"`、`(外类型, 内标签)`。
fn label_type(vm: &Vm, name: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(name).expect("TS-41：类型应当已登记");
    vm.instance.type_value(ty)
}

fn label_any(vm: &Vm) -> NonNull<Header> {
    vm.instance.new_str("Any")
}

fn label_generic(vm: &Vm, outer: &str, inner: NonNull<Header>) -> NonNull<Header> {
    vm.instance
        .new_tuple(vec![label_type(vm, outer), inner])
}

/// 造一个带签名常量与目标常量的模块，跑起来（返回值是程序的返回值）。
fn run_boundary(
    vm: &Vm,
    signature: NonNull<Header>,
    locals: Vec<Option<NonNull<Header>>>,
    body: Vec<Item>,
) -> Result<Value<'_>, ExecError> {
    let mut items = vec![Item::Instr(op("RESUME"), 0)];
    items.extend(body);
    let bytes = assemble(&items);
    // 入参检查读的是**局部槽**（形参），所以这份 code 要有 `nlocals` 与对应的 `varnames`
    let code = vm.code_with_names(
        locals.len().max(4),
        locals.len(),
        0,
        (0..locals.len()).map(|slot| format!("arg{slot}")).collect(),
        Vec::new(),
        bytes,
        vec![Some(signature)],
    );
    let namespace = vm
        .instance
        .alloc(pyawa_core::DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    for (slot, value) in locals.into_iter().enumerate() {
        if let Some(value) = value {
            frame.set_local(slot, Some(share(vm, value))).expect("设局部槽");
        }
    }
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame)? {
        pyawa_core::ExecOutcome::Returned(value) => Ok(value),
        pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层程序不该 yield"),
    }
}

#[test]
fn boundary_metadata_is_right() {
    // `BC-27`：`stack_effect` 要正确处理、`has_arg` 必须为真
    for name in ["CHECK_BOUNDARY_IN", "CHECK_BOUNDARY_OUT"] {
        let number = pyawa_core::opcode::opcode(name).expect("专有指令在表里");
        assert!(pyawa_core::opcode::has_arg(number), "BC-27：{name} 带 oparg");
        assert_eq!(
            pyawa_core::opcode::stack_effect(number, Some(0), None).expect("算得出"),
            0,
            "BC-27：{name} 不动值栈"
        );
        // `BC-31`：专有指令取**空闲编号**、且低于 instrumented 区段
        assert!(
            number < pyawa_core::opcode_metadata::MIN_INSTRUMENTED_OPCODE,
            "BC-31：{name} 不能占用 instrumented 区段"
        );
        assert!(
            pyawa_core::opcode_metadata::PYAWA_SPECIFIC
                .iter()
                .any(|(candidate, _)| *candidate == name),
            "BC-28：{name} 是 Pyawa 专有指令"
        );
    }
}

#[test]
fn an_int_boundary_accepts_bool() {
    // `TS-40`：`bool ⊂ int` ⇒ `int` 标注必须接受 `True`
    let vm = Vm::new();
    let signature = vm.instance.new_tuple(vec![label_type(&vm, "int")]);
    let truth = vm.instance.new_bool(true);
    let result = run_boundary(
        &vm,
        signature,
        vec![Some(truth)],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("bool 应当被 int 标注接受");
    let mut expected = Value::small_int(1);
    let _ = &mut expected;
    assert!(
        vm.instance.truth_of(result.as_header(&vm.instance).expect("有返回值")),
        "bool 应当被 int 标注接受并原样返回"
    );
}

#[test]
fn a_failed_incoming_check_blames_the_caller() {
    // `T-BC-9`：消息里要有**真的**行号 ⇒ 这里的 code object 自带位置表（`BC-18`）。
    let vm = Vm::new();
    let signature = vm.instance.new_tuple(vec![label_type(&vm, "int")]);
    let text = vm.instance.new_str("not an int");
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
        Item::Instr(op("LOAD_FAST"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "<module>",
        "probe".to_owned(),
        "probe.py".to_owned(),
        7,
        4,
        1,
        1,
        0,
        0,
        0,
        vec!["arg0".to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        bytes,
        Vec::new(),
        vec![Some(signature), Some(text)],
        // 位置表与指令一一对应：**每条**都算在第 7 行（`BC-18`）
        vec![(7, 7, 0, 1); 4],
    ));
    let namespace = vm
        .instance
        .alloc(pyawa_core::DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    frame.set_local(0, Some(share(&vm, text))).expect("设局部槽");
    let frame = vm.instance.alloc(frame);
    assert!(
        pyawa_core::execute(&vm.instance, &frame).is_err(),
        "str 不该通过 int 的入参检查"
    );
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    let message = message.expect("消息要带上四要素");
    assert_eq!(type_name, "TypeBoundaryError", "TS-12：归责异常");
    // `TS-11`／`T-BC-9`：四要素——方向、期望、实际、位置
    assert!(message.contains("argument"), "方向（入参）：{message}");
    assert!(message.contains("expected int"), "期望类型：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");
    assert!(message.contains("probe.py"), "文件名：{message}");
    assert!(message.contains("line 7"), "行号：{message}");
}

#[test]
fn a_failed_return_check_blames_the_callee() {
    let vm = Vm::new();
    let signature = vm.instance.new_tuple(vec![label_type(&vm, "int")]);
    let text = vm.instance.new_str("wrong");
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("CHECK_BOUNDARY_OUT"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(signature), Some(text)],
    );
    let _ = vm.run(&code).expect_err("str 不该通过 int 的出参检查");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    let message = message.expect("消息要带上四要素");
    assert_eq!(type_name, "TypeBoundaryError");
    assert!(message.contains("return"), "方向（返回值）：{message}");
    assert!(message.contains("got str"), "实际类型：{message}");
}

#[test]
fn any_accepts_everything_and_generics_are_shallow() {
    let vm = Vm::new();
    // `TS-28`：`Any` 与一切类型相容
    let any = vm.instance.new_tuple(vec![label_any(&vm)]);
    let text = vm.instance.new_str("anything");
    run_boundary(
        &vm,
        any,
        vec![Some(text)],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("Any 必须接受一切");

    // `TS-13`：**浅层**＝编译器只发**裸**类型标签（`list`）⇒ 不看元素
    let shallow_list = vm.instance.new_tuple(vec![label_type(&vm, "list")]);
    let mixed = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_str("x"),
    ]);
    run_boundary(
        &vm,
        shallow_list,
        vec![Some(share(&vm, mixed))],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("裸 `list` 标签是浅层：元素类型不进检查");

    // **`TS-31` 深层**：标签带内层（`(list, int)`）⇒ 递归比元素（下面几条专测）
    let generic = vm.instance.new_tuple(vec![label_generic(&vm, "list", label_type(&vm, "int"))]);
    let empty = vm.instance.new_list(Vec::new());
    run_boundary(
        &vm,
        generic,
        vec![Some(empty)],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("浅层检查只判外类型");

    // 外类型不对 ⇒ 拒绝
    let generic = vm.instance.new_tuple(vec![label_generic(&vm, "list", label_type(&vm, "int"))]);
    let text = vm.instance.new_str("not a list");
    run_boundary(
        &vm,
        generic,
        vec![Some(text)],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect_err("外类型不对必须拒绝");
    let (_, message) = vm.pending_exception().expect("应当有异常");
    let message = message.expect("消息要带上四要素");
    assert!(message.contains("expected list[int]"), "泛型标签的渲染：{message}");
}

#[test]
fn the_deep_tier_checks_container_elements() {
    // `TS-31`：**深层档位**＝标签带内层 ⇒ 对容器元素**递归检查**（`TS-13` 的浅层仍是默认，
    // 因为浅层编译只发裸标签）。这是"代码生成差异"，不是运行期开关。
    let vm = Vm::new();
    let int_list = || vm.instance.new_tuple(vec![label_generic(&vm, "list", label_type(&vm, "int"))]);

    // 元素全对 ⇒ 过
    let good = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_bool(true), // `bool ⊂ int`（TS-40）
    ]);
    run_boundary(
        &vm,
        int_list(),
        vec![Some(share(&vm, good))],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("list[int] 收下 [1, True]");

    // 有一个元素不对 ⇒ 拒（归责消息仍是四要素）
    let bad = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_str("x"),
    ]);
    run_boundary(
        &vm,
        int_list(),
        vec![Some(share(&vm, bad))],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect_err("list[int] 必须拒掉 ['1', 'x']");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    let message = message.expect("消息要带上四要素");
    assert_eq!(type_name, "TypeBoundaryError");
    // `TS-11` 的四要素：期望给的是**声明标签**（带内层），实际给的是实际类型
    assert!(message.contains("expected list[int]"), "期望标签：{message}");
    assert!(message.contains("got list"), "实际类型：{message}");

    // `list[Any]` ⇒ 元素不设限
    let any_list = vm.instance.new_tuple(vec![label_generic(&vm, "list", label_any(&vm))]);
    let mixed = vm.instance.new_list(vec![
        vm.instance.new_int(1),
        vm.instance.new_str("x"),
    ]);
    run_boundary(
        &vm,
        any_list,
        vec![Some(share(&vm, mixed))],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("list[Any] 收下任何元素");

    // 嵌套：`list[list[int]]`（标签自己也是二元的）
    let nested_label = vm.instance.new_tuple(vec![
        label_type(&vm, "list"),
        vm.instance
            .new_tuple(vec![label_type(&vm, "list"), label_type(&vm, "int")]),
    ]);
    let nested_ok = vm.instance.new_list(vec![
        vm.instance.new_list(vec![vm.instance.new_int(1)]),
        vm.instance.new_list(vec![vm.instance.new_int(2)]),
    ]);
    run_boundary(
        &vm,
        nested_label,
        vec![Some(share(&vm, nested_ok))],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect("list[list[int]] 收下 [[1], [2]]");

    let nested_bad = vm.instance.new_list(vec![
        vm.instance.new_list(vec![vm.instance.new_int(1)]),
        vm.instance.new_list(vec![vm.instance.new_str("x")]),
    ]);
    let nested_label = vm.instance.new_tuple(vec![
        label_type(&vm, "list"),
        vm.instance
            .new_tuple(vec![label_type(&vm, "list"), label_type(&vm, "int")]),
    ]);
    run_boundary(
        &vm,
        nested_label,
        vec![Some(share(&vm, nested_bad))],
        vec![
            Item::Instr(op("CHECK_BOUNDARY_IN"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ],
    )
    .expect_err("嵌套层里出现 str ⇒ 必须拒");
}

#[test]
fn annotations_become_boundary_checks_only_in_extension_mode_and_deep_tier() {
    // `BC-25`②＋`TS-31`：检查指令**只**在「扩展模式 ＋ 深层档位」下发射；函数还得**带标注**。
    use pyawa_core::compile::{compile, CheckTier, Constant, Mode};

    let source = "def f(x: int) -> int:\n    return x\n";
    let names_of = |unit: &pyawa_core::compile::CompiledUnit| -> Vec<String> {
        let mut decoder = pyawa_core::decode::Decoder::new(&unit.code);
        let mut out = Vec::new();
        while let Ok(Some(instruction)) = decoder.next_instruction() {
            out.push(
                pyawa_core::opcode::opname(u16::from(instruction.opcode))
                    .unwrap_or("<未知>")
                    .to_owned(),
            );
        }
        out
    };
    /// 取某个嵌套单元（模块产物里 `Constant::Code` 那些）。
    fn nested<'a>(
        unit: &'a pyawa_core::compile::CompiledUnit,
        name: &str,
    ) -> &'a pyawa_core::compile::CompiledUnit {
        unit.constants
            .iter()
            .find_map(|constant| match constant {
                Constant::Code(inner) if inner.name == name => Some(&**inner),
                _ => None,
            })
            .unwrap_or_else(|| panic!("产物里应当有 {name} 的 code"))
    }

    let deep = compile(source, "<t>", Mode::Extension, CheckTier::Deep, 0).expect("编得过");
    let ops = names_of(nested(&deep, "f"));
    assert!(ops.contains(&"CHECK_BOUNDARY_IN".to_owned()), "深层＋扩展模式要发入参检查：{ops:?}");
    assert!(ops.contains(&"CHECK_BOUNDARY_OUT".to_owned()), "有返回注解就要发出参检查：{ops:?}");
    // 签名条目：形参标签元组 ＋ 返回值标签元组
    assert!(
        nested(&deep, "f")
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Tuple(_))),
        "签名条目必须进嵌套单元的常量表"
    );
    // 复合标签：`list[int]` ⇒ `(list, int)`
    let composite = compile(
        "def g(x: list[int]) -> list[int]:\n    return x\n",
        "<t>",
        Mode::Extension,
        CheckTier::Deep,
        0,
    )
    .expect("编得过");
    // 复合标签可能嵌在"形参标签元组"里 ⇒ 递归找
    fn has_list_of_int(constant: &Constant) -> bool {
        match constant {
            Constant::Tuple(parts) => {
                (matches!(parts.as_slice(),
                    [Constant::Type(outer), Constant::Type(inner)]
                        if outer == "list" && inner == "int"))
                    || parts.iter().any(has_list_of_int)
            }
            _ => false,
        }
    }
    assert!(
        nested(&composite, "g").constants.iter().any(has_list_of_int),
        "`list[int]` 要变成复合标签：{:?}",
        nested(&composite, "g").constants
    );

    // 浅层 ⇒ 不发（`TS-13` 的默认）
    let shallow = compile(source, "<t>", Mode::Extension, CheckTier::Shallow, 0).expect("编得过");
    assert!(!names_of(nested(&shallow, "f")).contains(&"CHECK_BOUNDARY_IN".to_owned()));
    // 纯 Python 模式 ⇒ 不发（`BC-25`②：专有指令禁止出现在纯 Python 产物里）
    let pure = compile(source, "<t>", Mode::PurePython, CheckTier::Deep, 0).expect("编得过");
    assert!(!names_of(nested(&pure, "f")).contains(&"CHECK_BOUNDARY_IN".to_owned()));
    // 没标注 ⇒ 深层也不发
    let plain = compile(
        "def h(x):\n    return x\n",
        "<t>",
        Mode::Extension,
        CheckTier::Deep,
        0,
    )
    .expect("编得过");
    assert!(!names_of(nested(&plain, "h")).contains(&"CHECK_BOUNDARY_IN".to_owned()));
}

#[test]
fn a_deep_compiled_function_enforces_its_annotations_end_to_end() {
    // 端到端：编译（扩展模式 ＋ 深层）→ 实例化 → 跑到模块里定义 `f` → 用错实参调它 ⇒
    // 归责异常；用对实参 ⇒ 正常返回。这才是"发射器真的接上了执行器"的证据。
    use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};

    let vm = Vm::new();
    let module = compile(
        "def f(x: int) -> int:\n    return x\n",
        "<t>",
        Mode::Extension,
        CheckTier::Deep,
        0,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &module);
    let namespace = vm
        .instance
        .alloc(pyawa_core::DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            core::cell::RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("定义 f 应当成功");

    /// 在给定命名空间里跑 `f(<value>)`（`LOAD_NAME` 取 `f` ⇒ 与定义它的模块同一个名字空间）。
    fn call_f<'a>(
        vm: &'a Vm,
        namespace: NonNull<Header>,
        value: NonNull<Header>,
    ) -> Result<Value<'a>, ExecError> {
        let program = assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_NAME"), 0),
            Item::Instr(op("PUSH_NULL"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("CALL"), 1),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]);
        let code = vm.code_with_names(
            8,
            0,
            0,
            Vec::new(),
            vec!["f".to_owned()],
            program,
            // 常量表**接手**新引用 ⇒ 单例与实参各 `own` 一份（`OM-16`）
            vec![
                Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
                Some(vm.instance.own(value).into_raw()),
            ],
        );
        let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
        let frame = vm.instance.alloc(frame);
        match pyawa_core::execute(&vm.instance, &frame)? {
            pyawa_core::ExecOutcome::Returned(value) => Ok(value),
            pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层程序不该 yield"),
        }
    }

    // 对：`int`（含 `bool ⊂ int`）
    call_f(&vm, namespace, vm.instance.new_int(42)).expect("int 实参应当通过");
    // SAFETY: namespace 上面已加过引用（常量表与帧各要一份）。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    // 错：`str` ⇒ `TypeBoundaryError`（归责调用方）
    assert!(
        call_f(&vm, namespace, vm.instance.new_str("x")).is_err(),
        "str 实参必须被拒"
    );
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    let message = message.expect("消息要带四要素");
    assert_eq!(type_name, "TypeBoundaryError");
    assert!(message.contains("argument"), "方向：{message}");
    assert!(message.contains("expected int"), "期望：{message}");
}
