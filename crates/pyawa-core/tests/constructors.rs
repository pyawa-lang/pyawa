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
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
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

/// 装一个最小的 `bool`（同理：测试实例默认不装内建）。
fn vm_with_bool() -> Vm {
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    let object = vm
        .instance
        .type_value(vm.instance.type_named("bool").expect("bool 在内建表里"));
    vm.instance.dict_set(builtins, "bool", object);
    vm.instance.set_builtins(Some(builtins));
    vm
}

/// 把执行结果读成**真值**（`bool` 槽的返回值是单例）。
fn run_truth(vm: &Vm, source: &str) -> Result<bool, String> {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本函数持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => {
            let value = vm.instance.dict_get(namespace, "x").expect("有 x");
            vm.instance.bool_value(value).ok_or_else(|| "不是布尔".to_owned())
        }
        Err(pyawa_core::ExecError::Raised { exception }) => {
            // SAFETY: 异常对象存活。
            let ty = unsafe { exception.as_ref() }.ty();
            Err(unsafe { ty.as_ref() }.name().to_owned())
        }
        Err(other) => Err(format!("{other:?}")),
    }
}

#[test]
fn bool_uses_the_shared_truthiness_and_reports_the_measured_failure() {
    let vm = vm_with_bool();
    assert!(!run_truth(&vm, "x = bool()\n").expect("bool() 应当成功"));
    assert!(run_truth(&vm, "x = bool(1)\n").expect("bool(1) 应当成功"));
    // `bool(0)`／`bool('')` 的**假值**一格：本层还没有 `0`／空串字面量的组合？—— 用 `bool(1)` 的反面：
    assert!(!run_truth(&vm, "x = bool(0)\n").expect("bool(0) 应当成功"));
    // 多参 ⇒ 照实测报 `TypeError`
    assert_eq!(
        run_truth(&vm, "x = bool(1, 2)\n").expect_err("bool(1,2) 应当报错"),
        "TypeError"
    );
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

    // `base` 形态（**第 601 轮已接线** ✓；先前这格写的是"如实未实现" ✗ —— 现在照参照给值 ✓）。
    // 完整口径（`base == 0` 的前缀判定 ✓、`"010"` 报错 ✓、2／8／16 允许前缀 ✓、下划线规则 ✓、
    // 两条异常消息原文 ✓）由 `crates/pyawa-runtime/tests/int_shapes.rs` 的 15 例守着 ✓。
    assert_eq!(
        run(&vm, "x = int('c', 16)\n", "x")
            .unwrap_or_else(|error| panic!("int('c', 16) 应当成功：{error}")),
        12,
        "int('c', 16)"
    );
}

/// 跑一段脚本，回 `x` 的**长度**（`Instance::length_of` 是安全公开入口）。
fn run_len(vm: &Vm, source: &str) -> Result<usize, String> {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本函数持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => {
            let value = vm.instance.dict_get(namespace, "x").expect("有 x");
            vm.instance.length_of(value).ok_or_else(|| "没有长度".to_owned())
        }
        Err(pyawa_core::ExecError::Raised { exception }) => {
            // SAFETY: 异常对象存活。
            let ty = unsafe { exception.as_ref() }.ty();
            Err(unsafe { ty.as_ref() }.name().to_owned())
        }
        Err(other) => Err(format!("{other:?}")),
    }
}

#[test]
fn list_literals_work_end_to_end() {
    let vm = Vm::new();
    assert_eq!(run_len(&vm, "x = []\n").expect("空列表应当跑得起来"), 0);
    assert_eq!(run_len(&vm, "x = [1, 2]\n").expect("两元素列表应当跑得起来"), 2);
    assert_eq!(run_len(&vm, "x = [1, 2, 3]\n").expect("三元素列表"), 3);
}

#[test]
fn int_of_a_list_reports_the_measured_type_error() {
    // 第 178 轮这条**写不了**（`[]` 当时在编译期就报 `Syntax`）⇒ 第 183 轮列表字面量落地后收回 ✓
    let vm = vm_with_int();
    assert_eq!(
        run(&vm, "x = int([])\n", "x").expect_err("int([]) 应当报错"),
        "TypeError",
        "参照实测：`int([])` ⇒ TypeError（消息由夹具守着）"
    );
}

#[test]
fn dict_literals_work_end_to_end() {
    let vm = Vm::new();
    assert_eq!(run_len(&vm, "x = {}\n").expect("空字典应当跑得起来"), 0);
    assert_eq!(run_len(&vm, "x = {1: 2}\n").expect("一对字典应当跑得起来"), 1);
    assert_eq!(run_len(&vm, "x = {1: 2, 3: 4}\n").expect("两对字典"), 2);
}

/// 装一个最小的 `dict`（测试实例默认**不装内建**）。
///
/// `dict.__init__` 由 `pyawa-stdlib` 的 `builtins_module` 装进类型字典（那层不在本 crate 的
/// 集成测试里）⇒ 这里照同一处接线**手动挂上**：本轮要验的正是它（`type_call.rs` 挂自定义
/// `__init__` 是同一手法）。
fn vm_with_dict() -> Vm {
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    let dict_type = vm.instance.type_named("dict").expect("dict 在内建表里");
    let object = vm.instance.type_value(dict_type);
    vm.instance.dict_set(builtins, "dict", object);
    vm.instance.set_builtins(Some(builtins));
    let type_namespace = vm
        .instance
        .type_namespace(dict_type.cast::<pyawa_core::Header>())
        .expect("dict 有类型字典");
    let function_type = vm
        .instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 已登记");
    let method = vm
        .instance
        .alloc(pyawa_core::BuiltinFunctionObject::new(
            function_type,
            Box::leak("__init__".to_owned().into_boxed_str()),
            core::cell::Cell::new(pyawa_core::dict_init_native as pyawa_core::NativeFn),
        ))
        .into_raw()
        .cast::<pyawa_core::Header>();
    // `dict_set` 是**借用**口径 ✓（自己留一份）⇒ `method` 那份额外的引用还掉。
    vm.instance.dict_set(type_namespace, "__init__", method);
    // SAFETY: `method` 由本函数持有；类型字典已留一份。
    unsafe { vm.instance.release_object(method.as_ptr()) };
    vm
}

/// 编译并执行一段脚本，成功时把命名空间里的 `x` 读成**按插入序**的 `(键, 值)` 表；
/// 失败时回 `(异常类名, 消息)`（消息口径同 [`run_exception`] 的说明）。
fn run_dict_entries(
    vm: &Vm,
    source: &str,
) -> Result<Vec<(String, i64)>, (String, String)> {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
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
            let value = vm.instance.dict_get(namespace, "x").expect("命名空间里应当有 x");
            let entries = vm.instance.dict_entries(value).expect("x 应当是字典");
            Ok(entries
                .into_iter()
                .map(|(key, value)| {
                    (
                        vm.instance.text_of(key).expect("键应当是字符串").to_owned(),
                        vm.instance.int_value(value).expect("值应当是整数"),
                    )
                })
                .collect())
        }
        Err(pyawa_core::ExecError::Raised { exception }) => Err(exception_parts(vm, exception)),
        Err(other) => Err((format!("{other:?}"), String::new())),
    }
}

/// 异常对象的 `(类名, 消息)`（消息走 `ExceptionObject::message_with`）。
fn exception_parts(
    vm: &Vm,
    exception: core::ptr::NonNull<pyawa_core::Header>,
) -> (String, String) {
    // SAFETY: 异常对象由 `ExecError::Raised` 持有，存活；类型身份由 VM 保证。
    let ty = unsafe { exception.as_ref() }.ty();
    // SAFETY: 同上。
    let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
        .message_with(&vm.instance)
        .unwrap_or_default();
    (unsafe { ty.as_ref() }.name().to_owned(), message)
}

/// 跑一段脚本，期望它抛异常，回 `(类名, 消息)`。
fn run_exception(vm: &Vm, source: &str) -> (String, String) {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: `namespace` 由本函数持有，帧接手一份引用。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => panic!("{source:?} 应当抛异常"),
        Err(pyawa_core::ExecError::Raised { exception }) => exception_parts(vm, exception),
        Err(other) => panic!("{source:?} 应当是 Python 异常，实际：{other:?}"),
    }
}

/// `dict(...)` 的关键字实参（task-1）：`_kwargs` 先前整份没读 ⇒ `dict(a=1)` 得 `{}`。
///
/// 参照 3.14.4 实测口径：先填位置实参（映射／成对可迭代），**再填关键字实参、同名键覆盖**。
/// 官方 `Lib/argparse.py` 的 `dict(kwargs, dest=dest, option_strings=[])` 就压在这一格
/// （缺 `dest` ⇒ `unittest.main()` 当场 `KeyError: 'dest'`）。
#[test]
fn dict_init_merges_the_positions_and_the_keywords() {
    fn expected(pairs: &[(&str, i64)]) -> Vec<(String, i64)> {
        pairs.iter().map(|(key, value)| ((*key).to_owned(), *value)).collect()
    }
    let vm = vm_with_dict();
    let entries = |source: &str| {
        run_dict_entries(&vm, source).unwrap_or_else(|(ty, message)| {
            panic!("{source:?} 应当成功，实际 {ty}: {message}")
        })
    };

    // 零实参 ⇒ 空字典
    assert_eq!(entries("x = dict()\n"), expected(&[]), "dict()");
    // 只有关键字
    assert_eq!(entries("x = dict(a=1)\n"), expected(&[("a", 1)]), "dict(a=1)");
    assert_eq!(
        entries("x = dict(**{'a': 1, 'b': 2})\n"),
        expected(&[("a", 1), ("b", 2)]),
        "dict(**kw)"
    );
    // 映射 ＋ 关键字：关键字追加在后面
    assert_eq!(
        entries("x = dict({'a': 1}, b=2)\n"),
        expected(&[("a", 1), ("b", 2)]),
        "dict(mapping, **kw)"
    );
    // 同名键 ⇒ 关键字**覆盖**
    assert_eq!(
        entries("x = dict({'a': 1}, a=2)\n"),
        expected(&[("a", 2)]),
        "dict(mapping, a=2)"
    );
    assert_eq!(
        entries("x = dict({'a': 1}, a=2, b=3)\n"),
        expected(&[("a", 2), ("b", 3)]),
        "dict(mapping, a=2, b=3)"
    );
    // 成对可迭代 ＋ 关键字：同一口径
    assert_eq!(
        entries("x = dict([('a', 1)], a=2)\n"),
        expected(&[("a", 2)]),
        "dict(iterable_of_pairs, a=2)"
    );
    assert_eq!(
        entries("x = dict([('a', 1), ('b', 2)])\n"),
        expected(&[("a", 1), ("b", 2)]),
        "dict(iterable_of_pairs)"
    );
    // `**` 展开的字典与字面关键字同一条路
    assert_eq!(
        entries("x = dict({'a': 1}, **{'b': 2})\n"),
        expected(&[("a", 1), ("b", 2)]),
        "dict(mapping, **{{'b': 2}})"
    );
    // 成对可迭代里的重复键 ⇒ 后者覆盖（update 语义，参照实测 `{'a': 2}`）
    assert_eq!(
        entries("x = dict([('a', 1), ('a', 2)])\n"),
        expected(&[("a", 2)]),
        "dict([('a', 1), ('a', 2)])"
    );
}

/// `dict(...)` 的失败形态：位置实参个数与源不可迭代／元素不是两项的实测文案。
#[test]
fn dict_init_reports_the_measured_argument_errors() {
    let vm = vm_with_dict();
    // 参照实测：`dict({}, {})` ⇒ `dict expected at most 1 argument, got 2`
    assert_eq!(
        run_exception(&vm, "x = dict({}, {})\n"),
        ("TypeError".to_owned(), "dict expected at most 1 argument, got 2".to_owned())
    );
    assert_eq!(
        run_exception(&vm, "x = dict({}, {}, {})\n"),
        ("TypeError".to_owned(), "dict expected at most 1 argument, got 3".to_owned())
    );
    // 源不可迭代
    assert_eq!(
        run_exception(&vm, "x = dict(1)\n"),
        ("TypeError".to_owned(), "'int' object is not iterable".to_owned())
    );
    // 元素不是两项（文案是**本层现有口径**：与参照只差 `#index`／`; 2 is required` 那半句，
    // 不在本轮口径内 ⇒ 原样保留）
    assert_eq!(
        run_exception(&vm, "x = dict([(1, 2, 3)])\n"),
        (
            "ValueError".to_owned(),
            "dictionary update sequence element does not have length 2".to_owned()
        )
    );
}

/// `dict.get` 的键型（task-5）：除 `str`／`int` 外，`None`／tuple／`bool` 等键也要按**值相等**命中。
///
/// 动因：官方 `argparse` 把 `None` 当**注册表键**（`register('action', None, _StoreAction)`）再
/// `.get(None)` 取 ⇒ 修前永远拿到默认值 ⇒ `unittest.main()` 卡在 `TypeError: None is not callable`。
#[test]
fn dict_get_finds_none_and_tuple_keys() {
    let vm = Vm::new();
    let get = |source: &str| {
        run(&vm, source, "x").unwrap_or_else(|error| panic!("{source:?} 应当成功：{error}"))
    };
    assert_eq!(get("d = {None: 1}\nx = d.get(None, 9)\n"), 1, "None 键");
    assert_eq!(get("d = {None: 5}\nx = d.get(None)\n"), 5, "None 键（无默认值）");
    assert_eq!(get("d = {None: 1}\nx = d.get('nope', 9)\n"), 9, "不存在的 str 键给默认值");
    assert_eq!(get("d = {(1, 2): 3}\nx = d.get((1, 2), 9)\n"), 3, "tuple 键");
    assert_eq!(
        get("d = {(1, 2): 3}\nx = d.get((2, 3), 9)\n"),
        9,
        "tuple 键未命中给默认值"
    );
    assert_eq!(get("d = {1: 10, 's': 20, None: 30}\nx = d.get(None)\n"), 30, "混合键里的 None");
    assert_eq!(get("d = {1: 10, 's': 20}\nx = d.get(1)\n"), 10, "int 快路不回归");
    assert_eq!(get("d = {1: 10, 's': 20}\nx = d.get('s')\n"), 20, "str 快路不回归");
    // `bool` 与 `int` 按值相等（参照 `{True: 7}.get(1)` ⇒ `7`）
    assert_eq!(get("d = {True: 7}\nx = d.get(1, 9)\n"), 7, "bool/int 值相等");
    // 未命中且无默认值 ⇒ `None`（用 `is None` 转成布尔来断言）
    assert!(
        run_truth(&vm, "d = {None: 1}\nx = d.get('nope') is None\n").expect("应当成功"),
        "未命中 ⇒ None"
    );
    // 相邻路径不回归（它们本来就走别的实现）
    assert!(
        run_truth(&vm, "d = {None: 1}\nx = None in d\n").expect("应当成功"),
        "`in` 不受影响"
    );
    assert_eq!(get("d = {None: 1}\nx = d.pop(None)\n"), 1, "`pop` 不受影响");
    assert_eq!(get("d = {}\nx = d.setdefault(None, 5)\n"), 5, "`setdefault` 不受影响");
}

/// `dict | dict`（`PEP 584`，task-13）：**新建** dict，先左后右、右侧覆盖同名键；两侧原对象不改。
///
/// 动因：`Lib/sysconfig/__init__.py:427` 的 `_get_sysconfigdata() | vars` ⇒
/// `sysconfig.get_config_var()` ⇒ 官方 `test.support` 导入链（`P3-32` 框架门槛）。
#[test]
fn dict_merge_operator_matches_the_reference() {
    let vm = vm_with_dict();
    let check = |source: &str| {
        run_truth(&vm, source).unwrap_or_else(|error| panic!("{source:?} 应当成功：{error}"))
    };
    // 基本合并 ＋ 右侧覆盖同名键（插入序：左侧的 a 保留位置，c 在后，b 追加在尾）
    assert!(check("x = ({'a': 1} | {'b': 2}) == {'a': 1, 'b': 2}\n"), "基本合并");
    assert!(
        check("x = ({'a': 1, 'c': 3} | {'a': 2, 'b': 4}) == {'a': 2, 'c': 3, 'b': 4}\n"),
        "右侧覆盖同名键"
    );
    assert!(check("x = ({} | {}) == {}\n"), "两侧空 dict");
    assert!(check("x = ({} | {'a': 1}) == {'a': 1}\n"), "左侧空");
    assert!(check("x = ({'a': 1} | {}) == {'a': 1}\n"), "右侧空");
    // 不原地改：两侧原对象保持原值
    assert!(
        check("a = {'a': 1}\nb = {'a': 2}\nc = a | b\nx = (a == {'a': 1}) and (b == {'a': 2})\n"),
        "两侧原对象不改"
    );
    // dict 子类参与（结果仍是普通 dict —— 用 == 与构造比较即可）
    assert!(
        check("class D(dict): pass\nx = (D({'a': 1}) | {'b': 2}) == {'a': 1, 'b': 2}\n"),
        "子类在左侧"
    );
    assert!(
        check("class D(dict): pass\nx = ({'a': 1} | D({'b': 2})) == {'a': 1, 'b': 2}\n"),
        "子类在右侧"
    );
    // 相邻路径不回归：int 位或、`|=`（走 protocol 的 inplace 路径）
    assert!(check("x = (1 | 2) == 3\n"), "int 位或不回归");
    assert!(
        check("a = {'a': 1}\na |= {'b': 2}\nx = a == {'a': 1, 'b': 2}\n"),
        "`|=` 不回归"
    );
    // 非 dict 的对侧照旧报"一处真相"的 TypeError 文案
    assert_eq!(
        run_exception(&vm, "x = {} | []\n"),
        (
            "TypeError".to_owned(),
            "unsupported operand type(s) for |: 'dict' and 'list'".to_owned()
        )
    );
}
