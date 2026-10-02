//! `NewFn` 槽的**行为级**验收（`OM-11` 扩的 ②(b) 批次）。
//!
//! 第 176 轮把 `NewFn` 的签名改成 `Result`，第 177 轮给 `int_new` 落了真实转换与**照实测**的失败消息。
//! 这里从**行为**上钉住它（材料见 `tests/fixtures/constructors.rs`，由
//! `tools/gen_constructors_fixture.py` 从参照导出）。
//!
//! 走的是**端到端**路径（编译 → 执行 → 取命名空间里的值）：`builtin_objects` 是私有模块，
//! 集成测试**不该**去戳它（那等于把内部结构当 API ✗）；`int` 的槽由实例注册时挂上，
//! 从 `int(...)` 这一句打进去才是**用户能看到的行为** ✓。
//!
//! 验收：
//! - `int()` ⇒ `0`；`int('12')`／`int(' 12 ')`／`int('+12')`／`int('-12')`／`int('1_2')` ⇒ 实测值
//! - `int('a')` ⇒ 抛 `ValueError`（参照实测的消息由夹具守着；这里守"类名对"）
//! - `int([])` ⇒ 抛 `TypeError`
//! - `int('c', 16)`（`base` 形态）⇒ 如实**未实现**（不是假装报 `ValueError`）

mod common;

use common::Vm;
use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use pyawa_core::Frame;

/// 编译并执行一段脚本，成功时回 `(命名空间, 值)`，失败时回异常**类名**。
fn run(vm: &Vm, source: &str, name: &str) -> Result<i64, String> {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow).expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: `namespace` 由本函数持有，帧接手一份引用。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => {
            let value = vm
                .instance
                .dict_get(namespace, name)
                .unwrap_or_else(|| panic!("命名空间里应当有 {name}"));
            Ok(vm.instance.int_value(value).expect("应当是整数"))
        }
        Err(pyawa_core::ExecError::Raised { exception }) => {
            // SAFETY: 异常对象存活。
            let ty = unsafe { exception.as_ref() }.ty();
            Err(unsafe { ty.as_ref() }.name().to_owned())
        }
        Err(other) => Err(format!("{other:?}")),
    }
}

/// 装一个最小的 `int`（测试实例默认**不装内建**）。
fn vm_with_int() -> Vm {
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    let int_object = vm
        .instance
        .type_value(vm.instance.type_named("int").expect("int 在内建表里"));
    vm.instance.dict_set(builtins, "int", int_object);
    vm.instance.set_builtins(Some(builtins));
    vm
}

#[test]
fn int_converts_strings_and_reports_the_measured_failures() {
    let vm = vm_with_int();

    // 零参 ⇒ 0（`x = int()`）
    assert_eq!(run(&vm, "x = int()\n", "x").expect("int() 应当成功"), 0);

    // 实测接受的那一档文法
    for (source, expected) in [
        ("x = int('12')\n", 12i64),
        ("x = int(' 12 ')\n", 12),
        ("x = int('+12')\n", 12),
        ("x = int('-12')\n", -12),
        ("x = int('1_2')\n", 12),
    ] {
        assert_eq!(
            run(&vm, source, "x").unwrap_or_else(|error| panic!("{source:?} 应当成功：{error}")),
            expected,
            "{source:?}"
        );
    }

    // 非法字面量 ⇒ `ValueError`（消息由夹具守着；这里守类名）
    assert_eq!(
        run(&vm, "x = int('a')\n", "x").expect_err("int('a') 应当报错"),
        "ValueError"
    );

    // 类型不对 ⇒ `TypeError`（**源级用例暂时写不了**：`[]` 这类字面量本层编译器还没支持，
    // 试 `int([])` 会在**编译期**报 `Syntax("表达式里出现 Some(LeftBracket)")` ✗）
    // ⇒ 这一格由 `tests/fixtures/constructors.rs` 的实测消息守着，等字面量补齐后再补源级用例 ✓

    // `base` 形态 ⇒ 如实未实现（回的是 VM 级 `Unsupported` 的调试串，不是 Python 异常）
    let error = run(&vm, "x = int('c', 16)\n", "x").expect_err("int('c', 16) 还没接线");
    assert!(error.contains("Unsupported"), "应当如实报未实现：{error}");
}
