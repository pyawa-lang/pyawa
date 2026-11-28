//! **`next()` 的 `StopIteration.value`**（round 294 旧 bug 修；独立于 A 路线）。
//!
//! 修因：`next(gen)`／`next(iterator)` 在生成器 `return` 收尾时把"推进结果"坍缩成
//! `None`（执行器 `advance_iterator` 的返回形状），`__next__` 抛出的 `StopIteration`
//! （`args[0]` ＝生成器 return 值）被吞掉 ⇒ `next()` 只能造一个**空** `StopIteration`
//! （实测 `.value` 是空串 ✗）。现在 `next()` 走核心的 `next_value`：
//!
//! * `__next__` 协议（生成器／用户迭代器）抛出的 `StopIteration` **原样**抛回
//!   ⇒ `.value`／`args`／子类都不丢（参照就是"传播同一个异常对象"）；
//! * 内建迭代器耗尽 ⇒ **无实参**的 `StopIteration()`（`.value is None`、`args == ()`）；
//! * `next(g, default)` 在 `StopIteration` 时交 `default`（吞异常口径不变）；
//! * `for` 循环仍走 `advance_iterator`（不需要结束值，行为不变）。
//!
//! 期望值由**本机 `python3`（3.14.4）跑同一段脚本**取得（`MS-17` 的参照口径），逐字节一致；
//! ABI 侧用 `paL_newstate` ＋ `pa_exec_string` 把同样的语义再钉一遍（不依赖 CLI／VM 路径）。
#![allow(unused_unsafe)]

fn run(name: &str, script: &str) -> String {
    let root =
        std::env::temp_dir().join(format!("pyawa-next-stopiter-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 参照输出（`python3` 3.14.4 实跑同一段脚本，逐字节一致）。
const RETURN_VALUE_SCRIPT: &str = r#"def gen():
    yield 1
    return 42


g = gen()
print("first", next(g))
try:
    next(g)
except StopIteration as stop:
    print("return42", stop.value, stop.args)


def no_return():
    yield 1


g = no_return()
next(g)
try:
    next(g)
except StopIteration as stop:
    print("noreturn", stop.value, stop.args, len(stop.args))

g = gen()
next(g)
print("default", next(g, "DEFAULT"))

g = gen()
next(g)
try:
    g.send(None)
except StopIteration as stop:
    print("send-return42", stop.value, stop.args)

g = gen()
next(g)
try:
    next(g)
except StopIteration:
    pass
try:
    next(g)
except StopIteration as stop:
    print("again", stop.value, stop.args, len(stop.args))

collected = []
for value in gen():
    collected.append(value)
print("for", collected)

it = iter([])
try:
    next(it)
except StopIteration as stop:
    print("list", stop.value, stop.args)

d = {}
di = iter(d)
try:
    next(di)
except StopIteration as stop:
    print("dict", stop.value, stop.args)
"#;

/// `python3` 3.14.4 实测（逐行）。
const RETURN_VALUE_EXPECTED: &str = "first 1\nreturn42 42 (42,)\nnoreturn None () 0\ndefault DEFAULT\nsend-return42 42 (42,)\nagain None () 0\nfor [1]\nlist None ()\ndict None ()\n";

/// 参照脚本（`python3` 3.14.4 实跑，逐字节一致）。
const DELEGATION_SCRIPT: &str = r#"def inner():
    yield 1
    return "inner-ret"


def outer():
    got = yield from inner()
    return got + "-outer"


g = outer()
print("yf-first", next(g))
try:
    next(g)
except StopIteration as stop:
    print("yf-return", stop.value, stop.args)


def bad():
    yield 1
    raise StopIteration("inside")


g = bad()
print("bad-first", next(g))
try:
    next(g)
except RuntimeError as exc:
    print("pep479", exc)
try:
    next(g)
except StopIteration as stop:
    print("bad-after", stop.value, stop.args)
"#;

/// `python3` 3.14.4 实测（逐行）。
const DELEGATION_EXPECTED: &str = "yf-first 1\nyf-return inner-ret-outer ('inner-ret-outer',)\nbad-first 1\npep479 generator raised StopIteration\nbad-after None ()\n";

/// 生成器 `return` 值经 `next()` ⇒ `StopIteration.value`；无 `return`／已收尾 ⇒ `None`；
/// `next(g, default)` 与 `for` 循环口径不变；普通（list／dict）迭代器耗尽 ⇒ `value is None`。
#[test]
fn next_of_generator_return_value_matches_the_reference() {
    assert_eq!(run("return-value", RETURN_VALUE_SCRIPT), RETURN_VALUE_EXPECTED);
}

/// `yield from` 的返回值经 `next()` 同样带回；`send()` 的既有口径不变；
/// 生成器内 `raise StopIteration` 仍是 PEP 479 的 `RuntimeError`（不是 `.value` 面）。
#[test]
fn yield_from_send_and_pep479_paths_are_unchanged() {
    assert_eq!(run("delegation", DELEGATION_SCRIPT), DELEGATION_EXPECTED);
}

// --------------------------------------------------------------------------- //
// ABI 侧最小验证（`paL_newstate`；不经过 CLI）
// --------------------------------------------------------------------------- //

fn abi_c_string(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).expect("脚本／名字不含 NUL")
}

fn abi_errmsg(state: *mut pyawa_abi::pa_state) -> String {
    // SAFETY: state 活着（调用方保证）。
    let ptr = unsafe { pyawa_abi::pa_errmsg(state) };
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 交回 NUL 结尾的借用字符串。
    unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

fn abi_exec(state: *mut pyawa_abi::pa_state, source: &str) -> i32 {
    let source = abi_c_string(source);
    let mode = abi_c_string("python");
    // SAFETY: 源码／模式名都是活的 CString；options 传 NULL（`AB-61`）。
    unsafe {
        pyawa_abi::pa_exec_string(
            state,
            source.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    }
}

fn abi_global_int(state: *mut pyawa_abi::pa_state, name: &str) -> Option<i64> {
    let name = abi_c_string(name);
    // SAFETY: state 活着；名字是活的 CString。
    if unsafe { pyawa_abi::pa_getglobal(state, name.as_ptr()) } != 0 {
        return None;
    }
    let mut value = 0i64;
    // SAFETY: 栈顶刚被压入；出参是活的局部变量。
    let rc = unsafe { pyawa_abi::pa_tointeger(state, -1, &mut value) };
    // SAFETY: 归还 `pa_getglobal` 的那份引用。
    unsafe { pyawa_abi::pa_pop(state, 1) };
    (rc == 0).then_some(value)
}

/// ABI（`paL_newstate`）最小验证：生成器 `return 42` 的 `.value`、无 `return`／普通迭代器
/// 的 `value is None` 与空 `args`、`next(g, default)`、`for` 忽略结束值、自定义迭代器的
/// `StopIteration` 子类**原样**传播（`args` 两元组）——全用脚本里的 int 全局量回读。
#[test]
fn abi_next_exposes_the_generator_return_value() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
def gen():
    yield 1
    return 42

g = gen()
next(g)
try:
    next(g)
except StopIteration as stop:
    returned = stop.value
    returned_args_len = len(stop.args)

def no_return():
    yield 1

g = no_return()
next(g)
try:
    next(g)
except StopIteration as stop:
    no_return_is_none = int(stop.value is None)
    no_return_args_len = len(stop.args)

g = gen()
next(g)
default_ok = int(next(g, "D") == "D")

it = iter([])
try:
    next(it)
except StopIteration as stop:
    plain_is_none = int(stop.value is None)
    plain_args_len = len(stop.args)

collected = []
for value in gen():
    collected.append(value)
for_ok = int(collected == [1])

class SubStop(StopIteration):
    pass

class Custom:
    def __iter__(self):
        return self

    def __next__(self):
        raise SubStop(7, 8)

try:
    next(Custom())
except StopIteration as stop:
    custom_type_ok = int(type(stop).__name__ == "SubStop")
    custom_value = stop.value
    custom_args_len = len(stop.args)
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(
        abi_global_int(state, "returned"),
        Some(42),
        "ABI：生成器 return 值应进 StopIteration.value：{}",
        abi_errmsg(state)
    );
    assert_eq!(abi_global_int(state, "returned_args_len"), Some(1));
    assert_eq!(abi_global_int(state, "no_return_is_none"), Some(1));
    assert_eq!(abi_global_int(state, "no_return_args_len"), Some(0));
    assert_eq!(abi_global_int(state, "default_ok"), Some(1));
    assert_eq!(abi_global_int(state, "plain_is_none"), Some(1));
    assert_eq!(abi_global_int(state, "plain_args_len"), Some(0));
    assert_eq!(abi_global_int(state, "for_ok"), Some(1));
    assert_eq!(abi_global_int(state, "custom_type_ok"), Some(1));
    assert_eq!(abi_global_int(state, "custom_value"), Some(7));
    assert_eq!(abi_global_int(state, "custom_args_len"), Some(2));
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
