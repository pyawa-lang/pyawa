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
