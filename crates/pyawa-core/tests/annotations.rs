//! `SET_FUNCTION_ATTRIBUTE` 的 **bit4 `annotate`**（3.14 的**延迟注解**协议）。
//!
//! 依据：`SPEC-bytecode.md` 的属性位表（bit4 `annotate`）与 `SPEC-type-system.md` 的
//! "对象模型**必须**提供 `__annotate__`／`__annotations__`／`__annotate_func__`／`__annotations_cache__`"
//! （`typing.py` 引用 7 处、`dataclasses.py` 9 处——注解是标准库的**运行前提**）。
//!
//! 本组只钉**执行器这一半**：指令把可调用对象挂到函数对象上（Python 可见的 `f.__annotate__`
//! 要等函数对象的属性通道，属另一项）；编译器的发射（`__annotate__` 嵌套单元 ＋ `SET_FUNCTION_ATTRIBUTE 16`）
//! 随后一笔。

mod common;

use core::cell::RefCell;

use pyawa_core::{Frame, Header};

use common::{assemble, op, Item, Vm};

#[test]
fn the_annotate_bit_attaches_the_callable_to_the_function() {
    let vm = Vm::new();
    let none = vm.instance.singletons().none();
    // 一个"注解可调用对象"（内容不重要：这一位只负责挂上去）
    let annotate = vm.instance.new_int(7);
    // 被挂的函数
    let code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "f",
        "f".to_owned(),
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
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        vec![Some(none)],
        Vec::new(),
    ));
    let function = vm.instance.alloc(pyawa_core::FunctionObject::new(
        vm.instance.type_named("function").expect("function 已登记"),
        code.into_raw().cast::<Header>(),
        Vec::new(),
        None,
        RefCell::new(None),
        RefCell::new(None),
    ));
    let function_header = function.into_raw().cast::<Header>();

    // 实测的栈序：`[属性值, 函数]`，**函数在 TOS**；挂完把函数留在栈上
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("SET_FUNCTION_ATTRIBUTE"), 16),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(annotate), Some(function_header)],
    );
    let namespace = vm.instance.new_dict();
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    let value = match pyawa_core::execute(&vm.instance, &frame).expect("应当成功") {
        pyawa_core::ExecOutcome::Returned(value) => value,
        pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层程序不该 yield"),
    };
    let returned = value
        .as_header(&vm.instance)
        .expect("返回值应当是一个对象");
    assert_eq!(returned, function_header, "挂完把函数留在栈上");

    // SAFETY: 上面确认是函数对象。
    let object = unsafe { &*function_header.as_ptr().cast::<pyawa_core::FunctionObject>() };
    assert_eq!(
        object.annotate(),
        Some(annotate),
        "bit4 把注解可调用对象挂到了函数上"
    );
}

#[test]
fn a_compiled_annotated_def_carries_a_callable_annotate() {
    // 端到端：编译带注解的 `def` ⇒ 实例化 ⇒ 跑模块 ⇒ 取函数 ⇒ 它的 `__annotate__` 可调用，
    // 且按 `format` 参数给出正确的注解字典（`{'a': int, 'return': int}`）。
    use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};

    let vm = Vm::new();
    let module = compile(
        "def f(a: int) -> int:\n    return a\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &module);
    // 注解表达式里的 `int` 走 `LOAD_GLOBAL` ⇒ 先查全局、再查 **builtins**（`BC-57`）。
    // 正式运行时装的是 `builtins` 模块；这里装一个最小的（只有 `int`）。
    let builtins = vm.instance.new_dict();
    let int_object = vm
        .instance
        .type_value(vm.instance.type_named("int").expect("int 在内建表里"));
    vm.instance.dict_set(builtins, "int", int_object);
    vm.instance.set_builtins(Some(builtins));
    let namespace = vm.instance.new_dict();
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).expect("定义 f 应当成功");

    let function = vm
        .instance
        .dict_get(namespace, "f")
        .expect("命名空间里应当有 f");
    // SAFETY: 上面刚执行的是 `def f`。
    let object = unsafe { &*function.as_ptr().cast::<pyawa_core::FunctionObject>() };
    let annotate = object.annotate().expect("带注解的 def 必须挂 __annotate__");

    let int_type = vm.instance.type_value(
        vm.instance.type_named("int").expect("int 在内建表里"),
    );
    // `format = 2`（参照支持的版本）⇒ 给出注解字典
    let format = vm.instance.new_int(2);
    let result = match pyawa_core::executor::call_value(&vm.instance, annotate, &[format], &[]) {
        Ok(value) => value,
        Err(error) => panic!("调用 __annotate__ 失败：{error:?}／pending={:?}", vm.pending_exception()),
    };
    // SAFETY: 返回的是 dict。
    let mapping = unsafe { &*result.as_ptr().cast::<pyawa_core::DictObject>() };
    let lookup = |name: &str| {
        vm.instance
            .dict_get(result, name)
            .unwrap_or_else(|| panic!("注解字典里应当有 {name}"))
    };
    assert_eq!(lookup("a"), int_type, "'a' 的注解是 int");
    assert_eq!(lookup("return"), int_type, "'return' 的注解是 int");
    assert_eq!(mapping.entries().len(), 2, "只有两个键");

    // 参照的守卫：`format > 2` ⇒ `NotImplementedError`（实测的合成单元就是这条语义）
    let too_new = vm.instance.new_int(3);
    let error = pyawa_core::executor::call_value(&vm.instance, annotate, &[too_new], &[])
        .expect_err("format > 2 应当报错");
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            assert_eq!(vm.instance.type_name(ty), "NotImplementedError");
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

#[test]
fn the_common_constant_table_is_the_measured_one() {
    // `BC-57`：`LOAD_COMMON_CONSTANT` 的 oparg 是**固定表**（实测 `dis._common_constants`：
    // 0 `AssertionError`／1 `NotImplementedError`／2 `tuple`／3 `all`／4 `any`）。
    // 注解单元的守卫用 1；这里把表逐项钉住（`all`／`any` 取自 builtins）。
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    for name in ["all", "any"] {
        let value = vm.instance.new_int(0); // 内容不重要：只验"取自 builtins"
        vm.instance.dict_set(builtins, name, value);
    }
    vm.instance.set_builtins(Some(builtins));
    for (oparg, expected) in [
        (0u8, "AssertionError"),
        (1u8, "NotImplementedError"),
        (2u8, "tuple"),
    ] {
        let code = vm.code_with_names(
            4,
            0,
            0,
            Vec::new(),
            Vec::new(),
            assemble(&[
                Item::Instr(op("RESUME"), 0),
                Item::Instr(op("LOAD_COMMON_CONSTANT"), oparg),
                Item::Instr(op("RETURN_VALUE"), 0),
            ]),
            Vec::new(),
        );
        let namespace = vm.instance.new_dict();
        let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
        let frame = vm.instance.alloc(frame);
        let value = match pyawa_core::execute(&vm.instance, &frame).expect("应当成功") {
            pyawa_core::ExecOutcome::Returned(value) => value,
            pyawa_core::ExecOutcome::Yielded(_) => panic!("不该 yield"),
        };
        let returned = value.as_header(&vm.instance).expect("有返回值");
        // SAFETY: 表里前三个都是类型对象。
        let returned_type = unsafe { &*returned.as_ptr() }.ty();
        assert_eq!(
            vm.instance.type_name(returned_type),
            "type",
            "下标 {oparg} 应当是类型对象"
        );
        // 类型对象自身是"值"时用 `type_value` 拿到；这里直接比身份（表里那三个类型）
        let expected_type = vm
            .instance
            .type_named(expected)
            .unwrap_or_else(|| panic!("{expected} 在内建表里"));
        assert_eq!(
            vm.instance.type_value(expected_type),
            returned,
            "下标 {oparg} 应当是 {expected} 本身"
        );
    }
}
