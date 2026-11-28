//! **A-2a（round 293）**：`yield from` 的 send 向下传 ＋ 委托链改**显式推进**（`PLAN §12.22`）。
//!
//! 旧实现里 `yield from` 每层委托走 `SEND → resume_generator → execute` **Rust 递归** ⇒ 深度吃
//! **D1 原生预算**（ABI／嵌入／wasm 未声明 = **64**；CLI 声明 1000 只是碰巧与语义额度相等），
//! 且 emitter 的循环形状把恢复时送进来的值 `POP_TOP` 掉 ⇒ `gen.send(x)` 到不了被委托的生成器。
//!
//! A-2a 之后：
//! * **send 向下传**（逐层、含返回值/`StopIteration.value` 经 `yield from` 回传）；
//! * 委托链用 `execute_stack` 的**堆上委托帧**推进 ⇒ 不再吃 D1，深度口径＝`recursion_limit`
//!   （默认 1000）；ABI（`paL_newstate`，默认预算 64）**200／900 层成功**、1200 层可捕获
//!   `RecursionError`；CLI 990 层成功、1200／1500 层可捕获 `RecursionError`（不 trap）。
//!
//! **不在 A-2a**（A-2b 待做）：PEP 380 的 `throw` 向最内层传、`close` 关整条链（现状行为保持
//! 不变）；`next(gen)` 的 `StopIteration.value` 旧 bug 也另轮修。
#![allow(unused_unsafe)]

use std::process::Command;

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

/// `yield from` 链：`level(n)` 逐层委托到 `level(0)`（最内层 `yield 77; return "bottom"`）。
/// `drive(n)` 用 `send(None)` 推进（`next()` 的 `StopIteration.value` 是另一个已知旧 bug，
/// 不在 A-2a：这里用 `send` 精确验证"返回值经 `yield from` 回传"）。
const ABI_SCRIPT: &str = r#"
def level(n):
    if n == 0:
        yield 77
        return "bottom"
    r = yield from level(n - 1)
    return r

def drive(n):
    it = level(n)
    count = 0
    while True:
        try:
            it.send(None)
        except StopIteration as e:
            return count, e.value
        count += 1

def ok(n):
    try:
        count, value = drive(n)
        return int(count == 1 and value == "bottom")
    except RecursionError:
        return 0

ok200 = ok(200)
ok900 = ok(900)
try:
    drive(1200)
    deep1200 = 0
except RecursionError:
    deep1200 = 1

# send 逐层向下传：10 进 inner 的 x、20 进 inner 的 y；返回值经 yield from 回 outer。
def inner():
    x = yield "a"
    y = yield ("got", x)
    return ("inner-ret", y)

def outer():
    r = yield from inner()
    return ("outer-ret", r)

g = outer()
s1 = g.send(None)
s2 = g.send(10)
try:
    g.send(20)
    sent_ret = None
except StopIteration as e:
    sent_ret = e.value
send_ok = int(s1 == "a" and s2 == ("got", 10) and sent_ret == ("outer-ret", ("inner-ret", 20)))
"#;

/// ABI（`paL_newstate`，**未声明原生预算**＝默认 64）：200／900 层链成功、1200 层可捕获
/// `RecursionError`，且 send 逐层传递正确（A-2a 前 `ok200/ok900/send_ok` 都是 0）。
#[test]
fn abi_yield_from_chain_and_send_are_heap_based() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let rc = abi_exec(state, ABI_SCRIPT);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(
        abi_global_int(state, "ok200"),
        Some(1),
        "ABI（未声明预算，默认 64）上 200 层 yield from 链应成功：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "ok900"),
        Some(1),
        "ABI 上 900 层 yield from 链应成功（语义额度 `recursion_limit`）：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "deep1200"),
        Some(1),
        "1200 层应报可捕获 RecursionError：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "send_ok"),
        Some(1),
        "send 值应逐层向下传、返回值经 yield from 回传（对照 python3 3.14.4）：{}",
        abi_errmsg(state)
    );
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}

/// CLI：990 层链成功、1200／1500 层可捕获 `RecursionError`（不中止）；send 语义与参照一致。
#[test]
fn cli_yield_from_chain_and_send_match_the_reference() {
    let root = std::env::temp_dir().join(format!(
        "pyawa-yield-from-a2a-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    let script = r#"
def level(n):
    if n == 0:
        yield 77
        return "bottom"
    r = yield from level(n - 1)
    return r

def drive(n):
    it = level(n)
    count = 0
    while True:
        try:
            it.send(None)
        except StopIteration as e:
            return count, e.value
        count += 1

ok990 = int(drive(990) == (1, "bottom"))
try:
    drive(1200)
    deep1200 = 0
except RecursionError:
    deep1200 = 1
try:
    drive(1500)
    deep1500 = 0
except RecursionError:
    deep1500 = 1

def inner():
    x = yield "a"
    y = yield ("got", x)
    return ("inner-ret", y)

def outer():
    r = yield from inner()
    return ("outer-ret", r)

g = outer()
s1 = g.send(None)
s2 = g.send(10)
try:
    g.send(20)
    sent_ret = None
except StopIteration as e:
    sent_ret = e.value
send_ok = int(s1 == "a" and s2 == ("got", 10) and sent_ret == ("outer-ret", ("inner-ret", 20)))
print(ok990, deep1200, deep1500, send_ok)
"#;
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
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
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1 1 1 1\n",
        "CLI 990 层成功／1200·1500 层可捕获／send 语义应与参照一致（python3 3.14.4 同输出）"
    );
    assert!(
        output.stderr.is_empty(),
        "stderr 必须为空：{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
