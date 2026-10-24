//! task-43：`builtins.compile()` ＋ `sys._getframe` 的 `f_back` 链。
//!
//! 两格都用**真的编译→执行**来验（不是单测 Rust 助手）：
//! - `compile()` 装进 builtins 原生函数表，脚本里 `compile/exec/eval` 端到端；
//! - `getframe` 同样装成原生函数，脚本里沿 `f_back` 链读 `f_code.co_name`。

mod common;

use common::Vm;
use core::cell::Cell;
use core::ptr::NonNull;
use pyawa_core::{
    BuiltinFunctionObject, CodeObject, ExecError, Frame, Header, Instance, IntObject, NativeFn,
    StrObject,
};

/// 把一个原生函数装进 `builtins` 字典（名字用 `&'static str` ✓）。
fn install(instance: &Instance, builtins: NonNull<Header>, name: &'static str, function: NativeFn) {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(ty, name, Cell::new(function)));
    instance.dict_set(builtins, name, native.into_raw().cast::<Header>());
}

/// 装几个内建**类型对象**当值（脚本里的 `except ValueError`／`str(...)` 要用 ✓）。
fn install_type(instance: &Instance, builtins: NonNull<Header>, name: &str) {
    let ty = instance.type_named(name).expect("内建类型已登记");
    let value = instance.type_value(ty);
    instance.dict_set(builtins, name, value);
    instance.release(value);
}

/// 跑一段源码（模块帧），返回命名空间。`seed` 在开跑前给命名空间放内置（模块级 `LOAD_NAME`
/// 在这条测试组装路径里查命名空间 ✓；`exec`／`eval` 的函数帧 `__globals__` 也是它 ✓）。
fn run_source(
    vm: &Vm,
    source: &str,
    seed: impl FnOnce(&Vm, NonNull<Header>),
) -> NonNull<Header> {
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

fn lookup(options: &Vm, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    options
        .instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("命名空间里缺 {name}"))
}

// `exec(code, namespace)` / `eval(code)` 的测试替身（stdlib 的两个内置在 core 测试里不可用）。
unsafe fn test_exec(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code = args[0];
    let namespace = match args.get(1).copied() {
        Some(value) => value,
        None => instance.new_dict(),
    };
    pyawa_core::exec_code_public(instance, code, None, namespace)
}

unsafe fn test_eval(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let code = args[0];
    let namespace = match args.get(1).copied() {
        Some(value) => value,
        None => instance.new_dict(),
    };
    pyawa_core::eval_code_public(instance, code, None, namespace)
}

#[test]
fn compile_builtin_produces_executable_code_objects() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
code = compile("x = 1\ny = x + 2", "<f>", "exec")
filename = code.co_filename
code_name = code.co_name
ns = {}
exec(code, ns)
x = ns["x"]
y = ns["y"]
ecode = compile("1 + 2", "<ev>", "eval")
ev = eval(ecode)
ev_filename = ecode.co_filename
scode = compile("z = 5", "<sg>", "single")
sns = {}
exec(scode, sns)
single = sns["z"]
bns = {}
exec(compile(b"q = 9", "<b>", "exec"), bns)
bytes_value = bns["q"]
try:
    compile("1", "<f>", "bogus")
except ValueError as error:
    bad_mode = str(error)
try:
    compile("x =", "<bad>", "exec")
except SyntaxError as error:
    syntax_filename = error.filename
    syntax_lineno = error.lineno
"#,
        |vm, namespace| {
            install(&vm.instance, namespace, "compile", pyawa_core::compile_native);
            install(&vm.instance, namespace, "exec", test_exec);
            install(&vm.instance, namespace, "eval", test_eval);
            install_type(&vm.instance, namespace, "str");
            install_type(&vm.instance, namespace, "ValueError");
            install_type(&vm.instance, namespace, "SyntaxError");
        },
    );
    let instance = &vm.instance;
    assert_eq!(text_of(lookup(&vm, namespace, "filename")), "<f>");
    assert_eq!(text_of(lookup(&vm, namespace, "code_name")), "<module>");
    assert_eq!(int_of(lookup(&vm, namespace, "x")), 1);
    assert_eq!(int_of(lookup(&vm, namespace, "y")), 3);
    assert_eq!(int_of(lookup(&vm, namespace, "ev")), 3);
    assert_eq!(text_of(lookup(&vm, namespace, "ev_filename")), "<ev>");
    assert_eq!(int_of(lookup(&vm, namespace, "single")), 5);
    assert_eq!(int_of(lookup(&vm, namespace, "bytes_value")), 9);
    assert_eq!(
        text_of(lookup(&vm, namespace, "bad_mode")),
        "compile() mode must be 'exec', 'eval' or 'single'"
    );
    assert_eq!(text_of(lookup(&vm, namespace, "syntax_filename")), "<bad>");
    assert_eq!(int_of(lookup(&vm, namespace, "syntax_lineno")), 1);
    // `compile()` 的返回对象是 code 对象类型（本层注册名 `CodeObject` ✗ 参照叫 `code`）。
    let code = lookup(&vm, namespace, "code");
    assert_eq!(instance.type_name(instance.type_of(code)), "CodeObject");
    // SAFETY: 上面刚确认过类型；这里只读 code 载荷的判断字段。
    let _ = unsafe { &*code.as_ptr().cast::<CodeObject>() };
}

unsafe fn test_getframe(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    pyawa_core::getframe_native(instance, bound, args, kwargs)
}

#[test]
fn frame_f_back_chain_follows_the_caller_like_the_reference() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
def inner():
    return getframe(0)

def outer():
    return inner()

frame = outer()
leaf_name = frame.f_code.co_name
parent_name = frame.f_back.f_code.co_name
grand_name = frame.f_back.f_back.f_code.co_name
top_is_none = frame.f_back.f_back.f_back is None


def depth_outer():
    return getframe(1)

depth_name = depth_outer().f_code.co_name

try:
    getframe(99)
except ValueError as error:
    depth_error = str(error)

try:
    getframe(1.5)
except TypeError as error:
    depth_type_error = str(error)


def make():
    def leaf():
        return getframe(0)
    return leaf()

captured = make()
captured_parent = captured.f_back.f_code.co_name
captured_chain_len = 0
node = captured
while node is not None:
    captured_chain_len += 1
    node = node.f_back
"#,
        |vm, namespace| {
            install(&vm.instance, namespace, "getframe", test_getframe);
            install_type(&vm.instance, namespace, "str");
            install_type(&vm.instance, namespace, "ValueError");
            install_type(&vm.instance, namespace, "TypeError");
        },
    );
    assert_eq!(text_of(lookup(&vm, namespace, "leaf_name")), "inner");
    assert_eq!(text_of(lookup(&vm, namespace, "parent_name")), "outer");
    assert_eq!(text_of(lookup(&vm, namespace, "grand_name")), "<module>");
    assert_eq!(
        vm.instance
            .object_repr(lookup(&vm, namespace, "top_is_none"))
            .expect("bool 有 repr"),
        "True"
    );
    assert_eq!(text_of(lookup(&vm, namespace, "depth_name")), "<module>");
    assert_eq!(
        text_of(lookup(&vm, namespace, "depth_error")),
        "call stack is not deep enough"
    );
    assert_eq!(
        text_of(lookup(&vm, namespace, "depth_type_error")),
        "'float' object cannot be interpreted as an integer"
    );
    // 调用者返回后，捕获的帧仍沿 `f_back` 链读到它（帧持有引用 ⇒ 不悬垂 ✓）。
    assert_eq!(text_of(lookup(&vm, namespace, "captured_parent")), "make");
    assert_eq!(int_of(lookup(&vm, namespace, "captured_chain_len")), 3);
}
