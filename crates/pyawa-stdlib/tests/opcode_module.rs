//! `_opcode`／`_opcode_metadata` 的 Python 层包装（`SPEC-bytecode.md` §2.1／§2.2、§5.2.5）。
//!
//! 数据与纯函数归属 `pyawa-core`（`BC-38`：**只转发不复制**）；这里验的是"包装对不对"：
//! ① 必须导出的符号都在；② 转发出来的值与 `pyawa-core` 的表**逐项一致**；
//! ③ `_specializations`／`_specialized_opmap` **必须为空**（`BC-32`，**不是**照抄参照）；
//! ④ 用法错误的几条实测消息；⑤ `get_executor` 恒 `None`（§2.1 明文允许）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};
use pyawa_stdlib::opcode::{build_opcode, build_opcode_metadata};

fn attribute(instance: &Instance, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("§2.1／§2.2：`{name}` 必须导出"))
}

fn call(
    instance: &Instance,
    function: NonNull<Header>,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // SAFETY: function 是本实例里存活的原生可调用对象。
    let handler = unsafe {
        (*function
            .as_ptr()
            .cast::<pyawa_core::BuiltinFunctionObject>())
        .function()
    };
    // SAFETY: 实参都是调用方持有的引用（handler 只借用）。
    unsafe { handler(instance, None, args, &[]) }
}

fn message_of(instance: &Instance, error: pyawa_core::ExecError) -> String {
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default()
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

#[test]
fn the_required_symbols_are_exported() {
    // `SPEC-bytecode.md` §2.1／§2.2 的"必须导出"表
    let instance = Instance::new();
    let opcode = build_opcode(&instance);
    for name in [
        "stack_effect",
        "has_arg",
        "has_const",
        "has_name",
        "has_jump",
        "has_free",
        "has_local",
        "has_exc",
        "get_intrinsic1_descs",
        "get_intrinsic2_descs",
        "get_special_method_names",
        "get_nb_ops",
        "get_executor",
    ] {
        let _ = attribute(&instance, opcode, name);
    }
    let metadata = build_opcode_metadata(&instance);
    for name in [
        "opmap",
        "_specializations",
        "_specialized_opmap",
        "HAVE_ARGUMENT",
        "MIN_INSTRUMENTED_OPCODE",
    ] {
        let _ = attribute(&instance, metadata, name);
    }
}

#[test]
fn the_predicates_forward_the_core_tables() {
    let instance = Instance::new();
    let opcode = build_opcode(&instance);
    for (name, predicate) in [
        ("has_arg", pyawa_core::opcode::has_arg as fn(u16) -> bool),
        ("has_const", pyawa_core::opcode::has_const),
        ("has_name", pyawa_core::opcode::has_name),
        ("has_jump", pyawa_core::opcode::has_jump),
        ("has_free", pyawa_core::opcode::has_free),
        ("has_local", pyawa_core::opcode::has_local),
        ("has_exc", pyawa_core::opcode::has_exc),
    ] {
        let function = attribute(&instance, opcode, name);
        for number in [0u16, 1, 43, 100, 128, 232, 233, 255] {
            let argument = instance.new_int(i64::from(number));
            let result = call(&instance, function, &[argument]).expect("一个整数实参应当成功");
            assert_eq!(
                instance.bool_value(result),
                Some(predicate(number)),
                "{name}({number}) 必须与 pyawa-core 的表一致"
            );
        }
    }
}

#[test]
fn the_specialization_tables_are_empty_for_pyawa() {
    // `BC-32`：Pyawa **不做** CPython 式特化 ⇒ 这两个表必须为空。
    // ⚠ 参照实现的它们**非空**（实测 17／84 项）——别把参照的值当期望（§2.2 的警示）。
    let instance = Instance::new();
    let metadata = build_opcode_metadata(&instance);
    for name in ["_specializations", "_specialized_opmap"] {
        let table = attribute(&instance, metadata, name);
        // SAFETY: 建命名空间时放进去的是 dict。
        let dict = unsafe { &*table.as_ptr().cast::<pyawa_core::DictObject>() };
        assert!(dict.entries().is_empty(), "{name} 必须为空（BC-32）");
    }
    // `opmap` 与核心的表同规模、抽几个名字对得上
    let opmap = attribute(&instance, metadata, "opmap");
    // SAFETY: 同上。
    let opmap = unsafe { &*opmap.as_ptr().cast::<pyawa_core::DictObject>() };
    assert_eq!(opmap.entries().len(), pyawa_core::opcode_metadata::OPMAP.len());
    for (name, number) in pyawa_core::opcode_metadata::OPMAP.iter().take(5) {
        let stored = instance
            .dict_get(attribute(&instance, metadata, "opmap"), name)
            .unwrap_or_else(|| panic!("opmap 里应当有 {name}"));
        assert_eq!(instance.int_value(stored), Some(i64::from(*number)));
    }
}

#[test]
fn getters_return_lists_like_the_reference() {
    // 实测：`get_nb_ops()` 返回 `list`，元素是 `(NB_名字, 符号)` 二元组；
    // `get_intrinsic1_descs()` 返回 `list`，元素是 `str`
    let instance = Instance::new();
    let opcode = build_opcode(&instance);
    let nb_ops = call(&instance, attribute(&instance, opcode, "get_nb_ops"), &[])
        .expect("0 个实参应当成功");
    // SAFETY: 返回的是列表。
    let list = unsafe { &*nb_ops.as_ptr().cast::<pyawa_core::ListObject>() };
    assert_eq!(list.len(), pyawa_core::opcode::get_nb_ops().len());
    let first = list.item(0).expect("非空");
    // SAFETY: 元素是二元组。
    let pair = unsafe { &*first.as_ptr().cast::<pyawa_core::TupleObject>() };
    assert_eq!(pair.len(), 2);
    assert_eq!(
        instance.text_value(pair.item(0).expect("有元素")).as_deref(),
        Some(pyawa_core::opcode::get_nb_ops()[0].0)
    );

    let descs = call(
        &instance,
        attribute(&instance, opcode, "get_intrinsic1_descs"),
        &[],
    )
    .expect("0 个实参应当成功");
    // SAFETY: 同上。
    let list = unsafe { &*descs.as_ptr().cast::<pyawa_core::ListObject>() };
    assert_eq!(list.len(), pyawa_core::opcode::get_intrinsic1_descs().len());
}

#[test]
fn error_paths_use_the_measured_messages() {
    let instance = Instance::new();
    let opcode = build_opcode(&instance);
    let has_arg = attribute(&instance, opcode, "has_arg");
    let error = call(&instance, has_arg, &[]).expect_err("缺参要报错");
    assert_eq!(
        message_of(&instance, error),
        "has_arg() missing required argument 'opcode' (pos 1)"
    );
    let text = instance.new_str("x");
    let error = call(&instance, has_arg, &[text]).expect_err("非整数要报错");
    assert_eq!(
        message_of(&instance, error),
        "'str' object cannot be interpreted as an integer"
    );
    let left = instance.new_int(100);
    let right = instance.new_int(1);
    let error = call(&instance, has_arg, &[left, right]).expect_err("多参要报错");
    assert_eq!(
        message_of(&instance, error),
        "has_arg() takes at most 1 argument (2 given)"
    );

    let stack_effect = attribute(&instance, opcode, "stack_effect");
    let error = call(&instance, stack_effect, &[]).expect_err("0 个位置实参要报错");
    assert_eq!(
        message_of(&instance, error),
        "stack_effect() takes at least 1 positional argument (0 given)"
    );
    let big = instance.new_int(99999);
    let error = call(&instance, stack_effect, &[big]).expect_err("越界要报错");
    assert_eq!(message_of(&instance, error), "invalid opcode or oparg");
}

#[test]
fn get_executor_always_gives_none() {
    // §2.1：Pyawa 无 JIT，`get_executor` **允许**恒返回 `None`
    let instance = Instance::new();
    let opcode = build_opcode(&instance);
    let result = call(&instance, attribute(&instance, opcode, "get_executor"), &[])
        .expect("应当成功");
    assert!(
        result == instance.singletons().none(),
        "get_executor 必须恒返回 None"
    );
}
