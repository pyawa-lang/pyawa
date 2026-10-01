//! 格式化族（`docs/SPEC-bytecode.md` §10）。骨架是**实测**的（本机 3.14.4 反汇编 f-string）：
//!
//! ```text
//! f'{x}'     → LOAD_FAST x; FORMAT_SIMPLE
//! f'{x!r}'   → LOAD_FAST x; CONVERT_VALUE 2 (repr); FORMAT_SIMPLE
//! f'a{x}b'   → LOAD_CONST 'a'; LOAD_FAST x; FORMAT_SIMPLE; LOAD_CONST 'b'; BUILD_STRING 3
//! f'{x:>5}'  → LOAD_FAST x; LOAD_CONST '>5'; FORMAT_WITH_SPEC
//! ```
//!
//! 已接线：`FORMAT_SIMPLE`（净 0，`str()`）、`CONVERT_VALUE`（净 0，`!s`／`!r`／`!a`）、
//! `BUILD_STRING`（净 −(n−1)）。**未接线**：`FORMAT_WITH_SPEC`（要 `__format__` 的对齐／宽度／
//! 精度），如实报未接线。`str()`／`repr()` 现在是 `Instance` 上的**临时垫片**，
//! 真协议在 `OM-11` 的 `str`／`repr` 槽位。

mod common;

use pyawa_core::{StrObject, Value};

use common::{emit, op, Vm};

fn text_of(result: &Value<'_>, vm: &Vm) -> String {
    let raw = result.as_header(&vm.instance).expect("应当是 str 对象");
    // SAFETY: 调用方保证这是 str。
    unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned()
}

#[test]
fn format_simple_stringifies_the_top_of_stack() {
    let vm = Vm::new();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("FORMAT_SIMPLE"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(42))],
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(text_of(&result, &vm), "42", "{{}} 的 str() 是 42");
}

#[test]
fn convert_value_handles_repr() {
    // `f'{x!r}'`：x 是字符串时 repr 会加引号
    let vm = Vm::new();
    let text = vm.instance.alloc(StrObject::new(
        vm.instance.singletons().str_type(),
        "hi".to_owned(),
    ));
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("CONVERT_VALUE"), 2), // 实测：2 就是 repr
            (op("FORMAT_SIMPLE"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(text.into_raw().cast::<pyawa_core::Header>())],
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(text_of(&result, &vm), "'hi'", "repr('hi')");
}

#[test]
fn build_string_joins_the_pieces() {
    // `f'a{x}b'` 的形状：常量段 ＋ `FORMAT_SIMPLE` ＋ 常量段 ＋ `BUILD_STRING 3`
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let left = vm.instance.alloc(StrObject::new(str_type, "a".to_owned()));
    let right = vm.instance.alloc(StrObject::new(str_type, "b".to_owned()));
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 2),
            (op("FORMAT_SIMPLE"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_STRING"), 3),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(left.into_raw().cast::<pyawa_core::Header>()),
            Some(right.into_raw().cast::<pyawa_core::Header>()),
            Some(vm.constant(7)),
        ],
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(text_of(&result, &vm), "a7b", "三段拼起来");
}

#[test]
fn format_with_spec_formats_the_value() {
    // `f'{x:>5}'`（实测：`format(7, '>5') = '    7'`）——现在走 `__format__` 的真路线了
    let vm = Vm::new();
    let spec = vm.instance.alloc(StrObject::new(
        vm.instance.singletons().str_type(),
        ">5".to_owned(),
    ));
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 0),
            (op("FORMAT_WITH_SPEC"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(spec.into_raw().cast::<pyawa_core::Header>()),
            Some(vm.constant(0)),
            Some(vm.constant(7)),
        ],
    );
    let result = vm.run(&code).unwrap();
    assert_eq!(text_of(&result, &vm), "    7", "format(7, '>5')");
}
