//! `PLAN §12.12` 步 2：协议 dunder 的**统一解析**（`resolve_dunder`）与**统一调用**
//! （`call_special_method`）的一致性测试。
//!
//! 依据：`SPEC-c-abi` §17 的「语义唯一」。本文件只从**可观察行为**钉住收敛后的口径：
//! 1. `object` 的默认 `__eq__`／`__ne__`／`__lt__` **不算自有** ⇒ 不递归、`<` 如实
//!    `TypeError`；
//! 2. 用户类自有的 `__eq__`／`__ne__`／`__lt__` 命中，`NotImplemented` 交回反射／兜底；
//! 3. 内建容器自己的 `__eq__` 走容器递归（`overrides_compare` 的「跳过」语义）；
//! 4. 子类优先／反射次序（`test_binop.test_comparison_orders` 的最小复现）；
//! 5. `has_custom_eq` 的容器去重路径行为不变（用户 `__hash__`＋`__eq__` 与纯内建快路）。
//!
//! 这些测试**不装** stdlib 的 `builtins`（与 `tests/classes.rs` 同口径）⇒ 只补源码真正用到的
//! 名字（`NotImplemented`、异常类型）。判定尽量在 Rust 侧读命名空间里的值。

mod common;

use core::ptr::NonNull;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use pyawa_core::{ExecError, Header};

use common::Vm;

/// 跑一段**模块源码**，失败原样交回（与 `tests/classes.rs` 的 `try_run_source` 同口径：
/// 模块级 `STORE_NAME` 要命名空间形态的帧）。
fn try_run_source(vm: &Vm, source: &str) -> Result<NonNull<Header>, ExecError> {
    let unit = compile(source, "<protocol-unified>", Mode::PurePython, CheckTier::Shallow, 0)
        .expect("测试源码应当编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // 富比较协议的反向派发要用 `NotImplemented`（stdlib 才把名字挂进 builtins）。
    let not_implemented = vm.instance.not_implemented();
    vm.instance
        .dict_set(namespace, "NotImplemented", not_implemented);
    // `object`／`dict`（源码里显式调 `object.__eq__`、`class DictSub(dict)` 用；core 测试
    // 没有 stdlib builtins）。
    for name in ["object", "dict"] {
        if let Some(ty) = vm.instance.type_named(name) {
            vm.instance.dict_set(namespace, name, ty.cast::<Header>());
        }
    }
    // SAFETY: namespace 由本函数持有；`for_code_with_namespace` 接手一份引用（`OM-16`）。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).map(|_| namespace)
}

/// 同 [`try_run_source`]，失败直接 panic 并把异常类型／消息打出来（测试断言主路用）。
fn run_source(vm: &Vm, source: &str) -> NonNull<Header> {
    match try_run_source(vm, source) {
        Ok(namespace) => namespace,
        Err(error) => panic!(
            "源码跑挂了：{error:?}｜异常：{:?}",
            vm.pending_exception()
        ),
    }
}

fn namespace_lookup(vm: &Vm, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    vm.instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("命名空间里没有 {name}"))
}

/// 命名空间里的布尔值（`True`／`False` 都经真值判定 ⇒ 不依赖具体单例）。
fn source_bool(vm: &Vm, namespace: NonNull<Header>, name: &str) -> bool {
    let value = namespace_lookup(vm, namespace, name);
    vm.instance
        .truthiness_of(value)
        .unwrap_or_else(|error| panic!("{name} 应当可判真：{error:?}"))
}

/// 命名空间里的字符串值。
fn source_text(vm: &Vm, namespace: NonNull<Header>, name: &str) -> String {
    let value = namespace_lookup(vm, namespace, name);
    match vm.instance.text_value(value) {
        Some(text) => text.to_owned(),
        None => panic!("{name} 应当是 str"),
    }
}

/// 命名空间里某个对象携带的 `len()`（本层 `Instance::length_of`，与 `len()` 同源）。
fn source_length(vm: &Vm, namespace: NonNull<Header>, name: &str) -> usize {
    let value = namespace_lookup(vm, namespace, name);
    vm.instance
        .length_of(value)
        .unwrap_or_else(|| panic!("{name} 应当有长度"))
}

/// 跑一段**应当抛 `TypeError`** 的源码，并核对挂起异常的类型。
fn expect_type_error(vm: &Vm, source: &str, what: &str) {
    match try_run_source(vm, source) {
        Ok(_) => panic!("{what}：应当抛 TypeError，实际正常返回"),
        Err(ExecError::Raised { .. }) => {}
        Err(other) => panic!("{what}：应当抛 TypeError，实际是 {other:?}"),
    }
    let (type_name, message) = vm.pending_exception().expect("应当有挂起异常");
    assert_eq!(type_name, "TypeError", "{what}：异常类型（消息：{message:?}）");
}

// --------------------------------------------------------------------------- //
// 1. `object` 默认实现：不算自有、不递归、`<` 如实 TypeError
// --------------------------------------------------------------------------- //

#[test]
fn object_default_comparisons_are_not_own_dunders() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Plain:
    pass

same = Plain()
other = Plain()

default_eq_same = (same == same)
default_eq_other = (same == other)
default_ne_same = (same != same)
default_ne_other = (same != other)

# `object.__ne__` 的显式调用也必须走完整相等协议，而不是把默认实现当"自有"再递归。
explicit_object_eq = (object.__eq__(same, same) is NotImplemented)
"#,
    );
    assert!(
        source_bool(&vm, namespace, "default_eq_same"),
        "`object.__eq__` 默认：同一对象相等"
    );
    assert!(
        !source_bool(&vm, namespace, "default_eq_other"),
        "`object.__eq__` 默认：不同对象不等"
    );
    assert!(
        !source_bool(&vm, namespace, "default_ne_same"),
        "`object.__ne__` 默认：同一对象 `!=` 为假"
    );
    assert!(
        source_bool(&vm, namespace, "default_ne_other"),
        "`object.__ne__` 默认：不同对象 `!=` 为真"
    );
    assert!(
        !source_bool(&vm, namespace, "explicit_object_eq"),
        "`object.__eq__(x, x)` 参照返回 True（不是 NotImplemented）"
    );

    // `object.__lt__` 在 core 里没有类型字典条目（stdlib 才挂）：默认实现不得被当成自有而
    // 递归派发，必须如实落到 `TypeError`。
    expect_type_error(
        &vm,
        r#"
class Plain:
    pass
Plain() < Plain()
"#,
        "object 默认 `<`",
    );
}

// --------------------------------------------------------------------------- //
// 2. 用户类自有 dunder 命中；`NotImplemented` 交回反射／兜底
// --------------------------------------------------------------------------- //

#[test]
fn user_own_dunders_are_dispatched_and_not_implemented_falls_back() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class OwnEq:
    def __eq__(self, other):
        return "own.eq"
    def __ne__(self, other):
        return "own.ne"

class OwnLt:
    def __lt__(self, other):
        return "own.lt"

class LeftEq:
    def __eq__(self, other):
        return NotImplemented

class RightEq:
    def __eq__(self, other):
        return "right.eq"

class LeftLt:
    def __lt__(self, other):
        return NotImplemented

class RightGt:
    def __gt__(self, other):
        return "right.gt"

class EqOnly:
    def __eq__(self, other):
        return True

class NeNotImplemented:
    def __ne__(self, other):
        return NotImplemented
    def __eq__(self, other):
        return True

own_eq = (OwnEq() == OwnEq())
own_ne = (OwnEq() != OwnEq())
own_lt = (OwnLt() < OwnLt())

eq_notimplemented_reflected = (LeftEq() == RightEq())
lt_notimplemented_reflected = (LeftLt() < RightGt())

# 自己只有 `__eq__`、没有 `__ne__` ⇒ `object.__ne__` 默认委托 `__eq__` 再取反（bool）。
eq_only_ne = (EqOnly() != EqOnly())

# 两侧 `__ne__` 都 NotImplemented ⇒ 身份兜底：两个不同对象 ⇒ `!=` 为真。
both_ne_notimplemented = (NeNotImplemented() != NeNotImplemented())
"#,
    );
    assert_eq!(source_text(&vm, namespace, "own_eq"), "own.eq");
    assert_eq!(source_text(&vm, namespace, "own_ne"), "own.ne");
    assert_eq!(source_text(&vm, namespace, "own_lt"), "own.lt");
    assert_eq!(
        source_text(&vm, namespace, "eq_notimplemented_reflected"),
        "right.eq",
        "`__eq__` 返回 NotImplemented ⇒ 反射到右侧 `__eq__`"
    );
    assert_eq!(
        source_text(&vm, namespace, "lt_notimplemented_reflected"),
        "right.gt",
        "`__lt__` 返回 NotImplemented ⇒ 反射到右侧 `__gt__`"
    );
    assert!(
        !source_bool(&vm, namespace, "eq_only_ne"),
        "只有 `__eq__` 的类：`!=` 走默认委托再取反 ⇒ False"
    );
    assert!(
        source_bool(&vm, namespace, "both_ne_notimplemented"),
        "两侧 `__ne__` 都 NotImplemented ⇒ 身份兜底（不同对象 ⇒ True）"
    );
}

// --------------------------------------------------------------------------- //
// 3. 内建容器自己的 `__eq__`：走容器递归，不按用户富路径
// --------------------------------------------------------------------------- //

#[test]
fn builtin_container_equality_uses_container_recursion() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class ValueEq:
    def __eq__(self, other):
        return True

class DictSub(dict):
    pass

class CustomDict(dict):
    def __eq__(self, other):
        return "custom.dict.eq"

list_equal = ([1, 2] == [1, 2])
list_unequal = ([1, 2] == [2, 1])
tuple_equal = ((1, 2) == (1, 2))
set_equal = ({1, 2} == {2, 1})
dict_equal = ({'a': 1} == {'a': 1})
dict_value_eq = ({'a': ValueEq()} == {'a': ValueEq()})
dict_subclass_equal = (DictSub() == DictSub())
dict_subclass_plain_equal = (DictSub() == {})
dict_subclass_ne = (DictSub() != DictSub())
custom_dict_override = (CustomDict() == CustomDict())
"#,
    );
    assert!(
        source_bool(&vm, namespace, "list_equal"),
        "list 逐元素递归相等"
    );
    assert!(
        !source_bool(&vm, namespace, "list_unequal"),
        "list 逐元素递归不等"
    );
    assert!(
        source_bool(&vm, namespace, "tuple_equal"),
        "tuple 逐元素递归相等"
    );
    assert!(source_bool(&vm, namespace, "set_equal"), "set 集合相等");
    assert!(source_bool(&vm, namespace, "dict_equal"), "dict 键值递归相等");
    assert!(
        source_bool(&vm, namespace, "dict_value_eq"),
        "dict 的值按 `__eq__` 递归（容器递归而非身份）"
    );
    assert!(
        source_bool(&vm, namespace, "dict_subclass_equal"),
        "dict 子类继承的 `__eq__` 走容器语义（overrides_compare 的跳过判据）"
    );
    assert!(
        source_bool(&vm, namespace, "dict_subclass_plain_equal"),
        "dict 子类与纯 dict 的容器相等语义一致"
    );
    assert!(
        !source_bool(&vm, namespace, "dict_subclass_ne"),
        "dict 子类 `!=` 与容器相等语义一致"
    );
    assert_eq!(
        source_text(&vm, namespace, "custom_dict_override"),
        "custom.dict.eq",
        "用户子类自己的 `__eq__` 必须优先于继承的容器实现"
    );
}

// --------------------------------------------------------------------------- //
// 4. 子类优先／反射次序（`test_binop.test_comparison_orders` 的最小复现）
// --------------------------------------------------------------------------- //

#[test]
fn subclass_priority_and_reflected_order_are_preserved() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Base:
    def __eq__(self, other):
        return "Base.eq"
    def __lt__(self, other):
        return "Base.lt"

class Derived(Base):
    def __eq__(self, other):
        return "Derived.eq"
    def __gt__(self, other):
        return "Derived.gt"

# 右侧类型是左侧的**严格子类** ⇒ 先问右侧自己的方法。
eq_subclass_first = (Base() == Derived())
lt_reflected_first = (Base() < Derived())
"#,
    );
    assert_eq!(
        source_text(&vm, namespace, "eq_subclass_first"),
        "Derived.eq",
        "子类优先：`Base() == Derived()` 先问 `Derived.__eq__`"
    );
    assert_eq!(
        source_text(&vm, namespace, "lt_reflected_first"),
        "Derived.gt",
        "子类优先：`Base() < Derived()` 先问反射的 `Derived.__gt__`"
    );
}

// --------------------------------------------------------------------------- //
// 5. `has_custom_eq`：容器去重路径行为不变
// --------------------------------------------------------------------------- //

#[test]
fn custom_eq_container_dedup_path_is_unchanged() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Key:
    def __init__(self, value):
        self.value = value
    def __hash__(self):
        return 7
    def __eq__(self, other):
        return self.value == other.value

# 用户 `__hash__`＋`__eq__`：同哈希同值 ⇒ 覆盖而不是新增（has_custom_eq ⇒ equality_public）。
mapping = {}
mapping[Key(1)] = "first"
mapping[Key(1)] = "second"
mapping_value = mapping[Key(1)]

# 集合去重走同一条 has_custom_eq 判据。
items = {Key(1), Key(1)}

# 纯内建值仍走快路：`1` 与 `True` 是同一个键/元素。
plain = {1: "a"}
plain[True] = "b"
"#,
    );
    assert_eq!(
        source_length(&vm, namespace, "mapping"),
        1,
        "同哈希同值的用户键应当覆盖（容器去重）"
    );
    assert_eq!(
        source_text(&vm, namespace, "mapping_value"),
        "second",
        "覆盖后读回的是后写入的值"
    );
    assert_eq!(
        source_length(&vm, namespace, "items"),
        1,
        "集合对同哈希同值用户元素去重"
    );
    assert_eq!(
        source_length(&vm, namespace, "plain"),
        1,
        "纯内建值（1／True）仍按快路去重"
    );
}
