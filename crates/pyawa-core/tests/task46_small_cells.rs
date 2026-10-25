//! task-46 的 core 小格批（端到端：真编译→执行，不是只测 Rust 助手）：
//! ① 异常子类 MRO 面；② `bytes.ljust`／`rjust`／`center`；③ `set.union`（core 版）；
//! ④ 函数 `__dict__`（类体换 `co_qualname` 时不能丢）；⑤ dict 的不可哈希键要报 `TypeError`。

mod common;

use common::Vm;
use core::ptr::NonNull;
use pyawa_core::{ExecError, Frame, Header, Instance, IntObject, StrObject};

/// 跑一段源码（模块帧），返回命名空间；`seed` 给命名空间放内建（模块级 `LOAD_NAME` 查它 ✓）。
fn run_source(vm: &Vm, source: &str, seed: impl FnOnce(&Vm, NonNull<Header>)) -> NonNull<Header> {
    use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("编不过：{error:?}"));
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    seed(vm, namespace);
    // SAFETY: namespace 由本函数持有，帧接手一份引用。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame)
        .unwrap_or_else(|error| panic!("源码应当跑得起来：{error:?}（未决异常 {:?}）", vm.pending_exception()));
    namespace
}

/// 把内建**类型对象**当值放进命名空间（`except TypeError`／`str(...)` 要用）。
fn install_type(instance: &Instance, namespace: NonNull<Header>, name: &str) {
    let ty = instance.type_named(name).expect("内建类型已登记");
    let value = instance.type_value(ty);
    instance.dict_set(namespace, name, value);
    instance.release(value);
}

fn lookup(vm: &Vm, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    vm.instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("命名空间里缺 {name}"))
}

fn int_of(value: NonNull<Header>) -> i64 {
    // SAFETY: 调用方保证是 int。
    unsafe { &*value.as_ptr().cast::<IntObject>() }
        .value
        .to_i64()
        .expect("测试里是小整数")
}

fn text_of(value: NonNull<Header>) -> String {
    // SAFETY: 调用方保证是 str。
    unsafe { &*value.as_ptr().cast::<StrObject>() }.value().to_owned()
}

fn bool_of(vm: &Vm, value: NonNull<Header>) -> bool {
    vm.instance
        .object_repr(value)
        .expect("bool 有 repr")
        == "True"
}

#[test]
fn exception_subclasses_inherit_the_base_exception_surface() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class E(ValueError):
    pass

e = E("x")
message = str(e)
first_arg = e.args[0]
same_frame = e.with_traceback(None) is e
cause_none = e.__cause__ is None
context_none = e.__context__ is None
suppress = e.__suppress_context__
traceback_none = e.__traceback__ is None
"#,
        |vm, namespace| {
            install_type(&vm.instance, namespace, "str");
            install_type(&vm.instance, namespace, "ValueError");
        },
    );
    assert_eq!(text_of(lookup(&vm, namespace, "message")), "x");
    assert_eq!(text_of(lookup(&vm, namespace, "first_arg")), "x");
    assert!(bool_of(&vm, lookup(&vm, namespace, "same_frame")));
    assert!(bool_of(&vm, lookup(&vm, namespace, "cause_none")));
    assert!(bool_of(&vm, lookup(&vm, namespace, "context_none")));
    assert!(!bool_of(&vm, lookup(&vm, namespace, "suppress")));
    assert!(bool_of(&vm, lookup(&vm, namespace, "traceback_none")));
}

#[test]
fn bytes_justify_fills_and_checks_the_fill_character() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
ljust_value = b"a".ljust(3, b"x")
ljust_default = b"ab".ljust(4)
ljust_short = b"abc".ljust(2, b"x")
rjust_value = b"a".rjust(3, b"x")
center_value = b"a".center(4, b"x")
"#,
        |_vm, _namespace| {},
    );
    let bytes_of = |name: &str| {
        vm.instance
            .bytes_value(lookup(&vm, namespace, name))
            .unwrap_or_else(|| panic!("{name} 应当是 bytes"))
            .to_vec()
    };
    assert_eq!(bytes_of("ljust_value"), b"axx");
    assert_eq!(bytes_of("ljust_default"), b"ab  ");
    assert_eq!(bytes_of("ljust_short"), b"abc");
    assert_eq!(bytes_of("rjust_value"), b"xxa");
    assert_eq!(bytes_of("center_value"), b"xaxx");
    // 长度不是 1 的填充字符 ⇒ 参照文案的 TypeError ✓。
    let vm = Vm::new();
    let unit = pyawa_core::compile::compile(
        r#"result = b"a".ljust(3, b"xy")"#,
        "<t>",
        pyawa_core::compile::Mode::PurePython,
        pyawa_core::compile::CheckTier::Shallow,
        0,
    )
    .expect("编得过");
    let code = pyawa_core::compile::instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    // SAFETY: namespace 由本测试持有，帧接手一份引用。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Err(ExecError::Raised { .. }) => {
            let (name, message) = vm.pending_exception().expect("应当有未决异常");
            assert_eq!(name, "TypeError");
            assert_eq!(
                message.unwrap_or_default(),
                "ljust(): argument 2 must be a byte string of length 1, not a bytes object of length 2"
            );
        }
        Err(other) => panic!("应当抛 TypeError，实际 {other:?}"),
        Ok(_) => panic!("应当抛 TypeError，实际成功"),
    }
}

#[test]
fn set_union_is_core_and_accepts_general_iterables() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
plain = {1, 2}.union({2, 3}, [4])
from_string = {1}.union("ab")
from_dict = {1}.union({"a": 2})
unbound = set.union({1}, {2})
frozen = frozenset({1}).union([2])
frozen_type = type(frozen).__name__
"#,
        |vm, namespace| {
            install_type(&vm.instance, namespace, "set");
            install_type(&vm.instance, namespace, "frozenset");
            install_type(&vm.instance, namespace, "type");
        },
    );
    let items = |name: &str| {
        let mut values: Vec<i64> = vm
            .instance
            .set_items(lookup(&vm, namespace, name))
            .unwrap_or_default()
            .into_iter()
            .map(int_of)
            .collect();
        values.sort_unstable();
        values
    };
    assert_eq!(items("plain"), vec![1, 2, 3, 4]);
    assert_eq!(items("unbound"), vec![1, 2]);
    assert_eq!(items("frozen"), vec![1, 2]);
    assert_eq!(
        text_of(lookup(&vm, namespace, "frozen_type")),
        "frozenset"
    );
    // str／dict 作实参：元素是 str ✓（这里只验个数：{1} ∪ {'a','b'} = 3 个）。
    assert_eq!(
        vm.instance
            .set_items(lookup(&vm, namespace, "from_string"))
            .unwrap_or_default()
            .len(),
        3
    );
    assert_eq!(
        vm.instance
            .set_items(lookup(&vm, namespace, "from_dict"))
            .unwrap_or_default()
            .len(),
        2
    );
}

#[test]
fn function_dict_survives_requalification_in_nested_classes() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
def funcattrs(**kwds):
    def decorate(func):
        func.__dict__.update(kwds)
        return func
    return decorate

def make():
    class C:
        @funcattrs(abc=1, xyz="haha")
        @funcattrs(booh=42)
        def foo(self):
            return 42
    return C

C = make()
abc = C.foo.abc
xyz = C.foo.xyz
booh = C.foo.booh
called = C().foo()
"#,
        |_vm, _namespace| {},
    );
    assert_eq!(int_of(lookup(&vm, namespace, "abc")), 1);
    assert_eq!(text_of(lookup(&vm, namespace, "xyz")), "haha");
    assert_eq!(int_of(lookup(&vm, namespace, "booh")), 42);
    assert_eq!(int_of(lookup(&vm, namespace, "called")), 42);
}

#[test]
fn dict_rejects_unhashable_keys_like_the_reference() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
d = {}
try:
    d[[1]]
except TypeError:
    read_error = 1
try:
    d[[1]] = 2
except TypeError:
    write_error = 1
try:
    d.get(([1],))
except TypeError:
    get_error = 1
try:
    [1] in d
except TypeError:
    contains_error = 1
"#,
        |vm, namespace| {
            install_type(&vm.instance, namespace, "TypeError");
        },
    );
    assert_eq!(int_of(lookup(&vm, namespace, "read_error")), 1);
    assert_eq!(int_of(lookup(&vm, namespace, "write_error")), 1);
    assert_eq!(int_of(lookup(&vm, namespace, "get_error")), 1);
    assert_eq!(int_of(lookup(&vm, namespace, "contains_error")), 1);
}
