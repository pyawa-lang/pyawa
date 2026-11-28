//! **A-1（round 292）**：内建容器 `repr` 改**显式 worklist** 后的深度口径（`PLAN §12.22`）。
//!
//! 本测试跑在 **ABI（`paL_newstate`，不声明预算）** 上 —— 这正是 A-1 之前到不了的宿主形态：
//! 旧实现里内建容器 `repr` 是 Rust 递归，深度上限与宿主声明的原生预算取小（ABI 默认 **64**），
//! 200 层深容器 `repr` 会 `RecursionError`（"未声明预算的宿主"差异，见 `DIV-11`）。
//!
//! A-1 把内建容器嵌套搬进堆上的工作栈 ⇒ 深度只按**语义**额度 `recursion_limit`（默认 1000）算：
//! * ABI 上 200 层深 `list`／`tuple`／`dict`／`frozenset` 的 `repr` **成功**，且文本与参照同形
//!   （`"[" * (n+1) + "]" * (n+1)`）；
//! * `sys.setrecursionlimit(100)` 后同一 200 层用例报**可捕获** `RecursionError`（`setrecursionlimit` 生效）；
//! * 恢复默认后深层（900 层）仍成功；
//! * 同一实例随后仍可用（守卫触发不破坏 VM 状态）。
//!
//! CLI 侧 150,000 层仍报可捕获 `RecursionError`（不中止）由 `repr_loop_crash_shapes.rs` 钉住；
//! 用户 `__repr__` 回调的原生栈仍受 `D1` 预算（`native_callback_depth.rs`／`report` 口径不变）。
#![allow(unused_unsafe)]

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

/// ABI（**未声明原生预算**，默认 64）：200 层深容器 `repr` 现在应成功（A-1 前会 `RecursionError`）。
#[test]
fn abi_builtin_container_repr_reaches_semantic_recursion_limit() {
    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = r#"
import sys

limit_default = sys.getrecursionlimit()

def nest_list(n):
    value = []
    for _ in range(n):
        value = [value]
    return value

def nest_dict(n):
    value = {}
    for _ in range(n):
        value = {1: value}
    return value

def nest_tuple(n):
    value = ()
    for _ in range(n):
        value = (value,)
    return value

def nest_frozen(n):
    value = frozenset()
    for _ in range(n):
        value = frozenset((value,))
    return value

def succeeds(f, n):
    try:
        return f(n)
    except RecursionError:
        return None

# A-1 的核心验收：ABI 不声明预算（旧上限 64）也能到 200 层。
list_200 = succeeds(lambda n: repr(nest_list(n)), 200)
dict_200 = succeeds(lambda n: repr(nest_dict(n)), 200)
tuple_200 = succeeds(lambda n: repr(nest_tuple(n)), 200)
frozen_200 = succeeds(lambda n: repr(nest_frozen(n)), 200)
ok_200 = int(
    list_200 == "[" * 201 + "]" * 201
    and dict_200 == "{1: " * 200 + "{}" + "}" * 200
    and tuple_200 == "(" * 200 + "()" + ",)" * 200
    and frozen_200 == "frozenset({" * 200 + "frozenset()" + "})" * 200
)

# setrecursionlimit 生效：降到 100 后同一 200 层用例 ⇒ 可捕获 RecursionError。
sys.setrecursionlimit(100)
lowered_caught = 0
try:
    repr(nest_list(200))
except RecursionError:
    lowered_caught = 1
sys.setrecursionlimit(limit_default)

# 恢复默认后 900 层仍成功（语义额度 1000 之内）。
deep_900 = succeeds(lambda n: repr(nest_list(n)), 900) is not None

# 守卫触发之后：同一实例仍可用（栈没有损坏）。
after_ok = int(repr([1, [2, {'k': 3}]]) == "[1, [2, {'k': 3}]]")
"#;
    let rc = abi_exec(state, script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    assert_eq!(
        abi_global_int(state, "limit_default"),
        Some(1000),
        "默认语义递归上限应为参照口径 1000：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "ok_200"),
        Some(1),
        "ABI（无原生预算声明）上 200 层深容器 repr 应成功且文本同形：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "lowered_caught"),
        Some(1),
        "setrecursionlimit(100) 后 200 层深容器 repr 应报可捕获 RecursionError：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "deep_900"),
        Some(1),
        "恢复默认后 900 层深容器 repr 应成功：{}",
        abi_errmsg(state)
    );
    assert_eq!(
        abi_global_int(state, "after_ok"),
        Some(1),
        "守卫触发后浅层 repr 应仍正确：{}",
        abi_errmsg(state)
    );
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };
}
