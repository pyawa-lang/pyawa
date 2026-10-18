//! **挂账 bug 批A（二）**：PEP 604 的 `X | None` 运行期缺失（b5：dataclass union 注解）。
//!
//! 症状（`target/recon/bugs/b5_pep604.py`）：`a: str | None = None` 的 dataclass 报
//! `AttributeError: 'function' object has no attribute '__builtins__'`。
//!
//! 根因链（本轮由 Lead 裁定交给 `executor/` 写手修 `NB_OR` 臂）：
//! ① **主因**：`executor.rs` 的 `NB_OR` 只接"两操作数都是**类型对象**"；`None` 是 `NoneType`
//!    实例 ⇒ `str | None` 落进算术 ⇒ `TypeError: unsupported operand type(s) for |: 'type' and
//!    'NoneType'`（对照：`int | str` 正常）。CPython 里 `X | None` 与 `None | X` 都得到含
//!    `NoneType` 的联合，`None | None` 仍是 `TypeError`。
//! ② **连锁**：该 `TypeError` 让 `annotationlib.get_annotations(FORWARDREF)` 走回退分支、去读
//!    `annotate.__builtins__`（`Lib/annotationlib.py`）——pyawa 函数对象没接 `__builtins__`，
//!    于是表面报那个 AttributeError。**主因修好后** `C.__annotations__`（`Format.VALUE`）能算出来，
//!    FORWARDREF 分支提前 return，不再摸 `__builtins__` ⇒ b5 即绿。
//! ③ **次生缺口（独立挂账，不属本轮）**：函数对象的 `__builtins__` 属性未接
//!    （`builtin/function.rs` 的 `function_getattr`）；**显式** `FORWARDREF`／`STRING` 格式的
//!    `annotationlib.get_annotations` 仍会撞它。
//!
//! **落点说明**：本文件放 core tests（task 写域）——它不需要 `import dataclasses`，直接钉
//! ①与②的**协议面**：联合构造（`int | None`／`None | int`／`int | str | None`）、
//! `None | None` 仍 `TypeError`、以及 dataclass 真正依赖的那步 `类.__annotations__` 可读。
//! dataclass 的**端到端** `C(a=None)` 要 `Lib/` 组合根（`import dataclasses` 只在
//! `pyawa-runtime` 层可达）⇒ 用 CLI 探针 `b5_pep604.py` 做端到端对拍（见 task 汇报）；
//! 若要把 dataclass 用例也进仓库，应放 `crates/pyawa-runtime/tests/`（不在本轮写域）。

mod common;

use core::ptr::NonNull;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use pyawa_core::{Frame, Header, Instance, TupleObject};

use common::Vm;

/// 装一个最小 `builtins`：联合构造要 `int`／`str` 这些**类型对象**的名字（`LOAD_GLOBAL` 回退到
/// builtins，`BC-57`）。
fn vm_with_builtins() -> Vm {
    let vm = Vm::new();
    let builtins = vm.instance.new_dict();
    for name in ["int", "str", "bool", "list", "dict", "float"] {
        if let Some(ty) = vm.instance.type_named(name) {
            let value = vm.instance.type_value(ty);
            vm.instance.dict_set(builtins, name, value);
        }
    }
    vm.instance.set_builtins(Some(builtins));
    vm
}

/// 执行一段模块源码，返回**模块命名空间**（本测试持有一份引用，故返回值在函数返回后仍存活）。
fn run_module(vm: &Vm, source: &str) -> NonNull<Header> {
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("应当编得过 {source:?}，却报了 {error:?}"));
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有，存活到执行结束（下面还有一份给帧）。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    if let Err(error) = pyawa_core::execute(&vm.instance, &frame) {
        panic!("跑挂了：{error:?}（{source:?}）");
    }
    namespace
}

/// 取命名空间里的一个全局。
fn global_of(vm: &Vm, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    vm.instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("命名空间里应当有 {name}"))
}

/// 一个值的**类型名**。
fn type_name(instance: &Instance, value: NonNull<Header>) -> String {
    let ty = instance.type_of(value);
    // SAFETY: ty 是实例持有的类型对象，存活。
    unsafe { ty.as_ref() }.name().to_owned()
}

/// **一个值作为"类型/union 成员"时的名字**：类型对象（`int`／`NoneType`）要取**它自己**的
/// `__name__`（`type_of` 只会给 `type`）；其余值（联合对象／一般值）取类型名。
fn member_type_name(instance: &Instance, value: NonNull<Header>) -> String {
    if instance.is_type_object(value) {
        // SAFETY: 上面刚确认是类型对象。
        return unsafe { value.cast::<pyawa_core::TypeObject>().as_ref() }
            .name()
            .to_owned();
    }
    type_name(instance, value)
}

/// 把联合对象**递归摊平**成成员类型名（CPython 会把 `int | str | None` 摊成三元；
/// 本层的联合实现也可能嵌套二元——两种都摊平后比**成员表**）。
fn union_member_names(instance: &Instance, value: NonNull<Header>) -> Vec<String> {
    // 成员是**类型对象**时要取它自己的名字（`type_of(类型对象) == type`，不能直接用 `type_name`）。
    if instance.is_type_object(value) || type_name(instance, value) != "UnionType" {
        return vec![member_type_name(instance, value)];
    }
    let args = instance
        .attribute_optional_of(value, "__args__")
        .unwrap_or_else(|error| panic!("读 `__args__` 失败：{error:?}"))
        .expect("UnionType 必须有 `__args__`");
    // SAFETY: `__args__` 由联合对象持有，且类型约定是 tuple。
    let tuple = unsafe { &*args.as_ptr().cast::<TupleObject>() };
    tuple
        .items()
        .iter()
        .flat_map(|item| union_member_names(instance, *item))
        .collect()
}

/// ① `int | None`／`None | int` 都能构造，成员与参照（含 `NoneType`）一致。
///
/// 参照（`python3` 3.14.4）：`(int | None).__args__ == (int, NoneType)`、
/// `(None | int).__args__ == (NoneType, int)`，两者 `type(...)` 都是联合类型。
#[test]
fn unions_with_none_are_constructible_on_both_sides() {
    let vm = vm_with_builtins();
    let namespace = run_module(
        &vm,
        "left = int | None\nright = None | int\n",
    );
    let left = global_of(&vm, namespace, "left");
    assert_eq!(
        type_name(&vm.instance, left),
        "UnionType",
        "`int | None` 必须得到联合类型"
    );
    assert_eq!(
        union_member_names(&vm.instance, left),
        ["int", "NoneType"],
        "`int | None` 的成员（参照：int, NoneType）"
    );
    let right = global_of(&vm, namespace, "right");
    assert_eq!(
        type_name(&vm.instance, right),
        "UnionType",
        "`None | int` 必须得到联合类型"
    );
    assert_eq!(
        union_member_names(&vm.instance, right),
        ["NoneType", "int"],
        "`None | int` 的成员（参照：NoneType, int）"
    );
}

/// ① 三元 `int | str | None` 也能构造，摊平后成员与参照一致
/// （参照：`(int | str | None).__args__ == (int, str, NoneType)`）。
#[test]
fn union_with_none_and_a_second_type_keeps_all_members() {
    let vm = vm_with_builtins();
    let namespace = run_module(&vm, "value = int | str | None\n");
    let value = global_of(&vm, namespace, "value");
    assert_eq!(type_name(&vm.instance, value), "UnionType");
    assert_eq!(
        union_member_names(&vm.instance, value),
        ["int", "str", "NoneType"],
        "`int | str | None` 摊平后的成员（顺序照参照）"
    );
}

/// ① `None | None` 仍是 `TypeError`（参照消息：
/// `unsupported operand type(s) for |: 'NoneType' and 'NoneType'`）。
#[test]
fn none_or_none_is_still_a_type_error() {
    let vm = vm_with_builtins();
    let unit = compile(
        "value = None | None\n",
        "<t>",
        Mode::PurePython,
        CheckTier::Shallow,
        0,
    )
    .expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    let outcome = pyawa_core::execute(&vm.instance, &frame);
    assert!(
        matches!(outcome, Err(pyawa_core::ExecError::Raised { .. })),
        "`None | None` 必须抛 TypeError（实际没有 Raised）"
    );
    let raw = vm.instance.pending_exception().expect("应当有异常");
    // SAFETY: raw 由实例持有，存活。
    let ty = unsafe { raw.as_ref() }.ty();
    // SAFETY: ty 由实例持有。
    assert_eq!(
        unsafe { ty.as_ref() }.name(),
        "TypeError",
        "`None | None` 必须是 TypeError"
    );
    // SAFETY: raw 指向异常实例。
    let exception = unsafe { &*raw.as_ptr().cast::<pyawa_core::ExceptionObject>() };
    let message = exception.message_with(&vm.instance).unwrap_or_default();
    assert!(
        message.contains("NoneType"),
        "消息要说明操作数类型（参照含 'NoneType'）：{message}"
    );
}

/// ② dataclass 真正依赖的那一步：含 `str | None` 的**类注解**可读
/// （`C.__annotations__` 走 `Format.VALUE`，主因修好后不再落 FORWARDREF 回退分支）。
///
/// 参照（`python3` 3.14.4）：`C.__annotations__` ⇒ `{'a': str | None}`。
#[test]
fn class_annotations_with_union_and_none_are_readable() {
    let vm = vm_with_builtins();
    let namespace = run_module(
        &vm,
        "class C:\n    a: str | None = None\n\nannotations = C.__annotations__\n",
    );
    let annotations = global_of(&vm, namespace, "annotations");
    assert_eq!(
        type_name(&vm.instance, annotations),
        "dict",
        "`C.__annotations__` 必须给注解字典"
    );
    let field = vm
        .instance
        .dict_get(annotations, "a")
        .expect("注解字典里应当有字段 a");
    assert_eq!(type_name(&vm.instance, field), "UnionType");
    assert_eq!(
        union_member_names(&vm.instance, field),
        ["str", "NoneType"],
        "字段 `a: str | None` 的成员（参照：str, NoneType）"
    );
}
