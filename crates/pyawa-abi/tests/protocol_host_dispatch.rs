//! **宿主对象经 `getattr_fn` 获得协议**的行为测试（`PLAN`「对象语义唯一入口＝属性查找」）。
//!
//! oracle（`protocol_oracle.rs`）证明**问到了**；这里证明**问到之后真的生效**（str/repr/format/
//! hash/eq/abs/contains），且未装 ops 的宿主类型仍走默认（零回归）。

use core::ffi::{c_char, c_void};
use core::mem::size_of;

use pyawa_abi::host::{pa_param, pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

unsafe extern "C" fn noop_dealloc(_payload: *mut c_void) {}
unsafe extern "C" fn noop_traverse(
    _payload: *mut c_void,
    _context: *mut c_void,
    _visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
) {
}

fn signature_with(params: &[pa_param]) -> pa_sig {
    pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: params.as_ptr(),
    }
}

fn self_param() -> pa_param {
    pa_param {
        size: size_of::<pa_param>(),
        name: b"self\0".as_ptr().cast(),
        type_expr: core::ptr::null(),
        flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
        default_handle: core::ptr::null_mut(),
    }
}

fn message_of(state: *mut pa_state) -> String {
    // SAFETY: state 存活。
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return String::new();
    }
    // SAFETY: NUL 结尾借用串。
    unsafe { core::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

fn exec(state: *mut pa_state, source: &[u8]) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"host-dispatch\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

fn new_state() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    state
}

// ---- 宿主函数（返回固定值；绑定方法由 ABI 的 HostBoundMethod 补 self）----
unsafe extern "C-unwind" fn host_str(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe { pa_pushstring(state, b"HOST-STR\0".as_ptr().cast(), -1) }
}
unsafe extern "C-unwind" fn host_repr(state: *mut pa_state) -> i32 {
    // SAFETY: 同上。
    unsafe { pa_pushstring(state, b"HOST-REPR\0".as_ptr().cast(), -1) }
}
unsafe extern "C-unwind" fn host_format(state: *mut pa_state) -> i32 {
    // SAFETY: 同上。
    unsafe { pa_pushstring(state, b"HOST-FMT\0".as_ptr().cast(), -1) }
}
unsafe extern "C-unwind" fn host_hash(state: *mut pa_state) -> i32 {
    // SAFETY: 同上。
    unsafe { pa_pushinteger(state, 123) }
}
unsafe extern "C-unwind" fn host_eq(state: *mut pa_state) -> i32 {
    // SAFETY: 同上。
    unsafe { pa_pushboolean(state, 1) }
}
unsafe extern "C-unwind" fn host_abs(state: *mut pa_state) -> i32 {
    // SAFETY: 同上。
    unsafe { pa_pushinteger(state, 7) }
}
unsafe extern "C-unwind" fn host_contains(state: *mut pa_state) -> i32 {
    // SAFETY: 同上。
    unsafe { pa_pushboolean(state, 1) }
}

/// 按名字把对应的宿主函数压栈（`getattr_fn` 返回**已绑定方法**）。
unsafe extern "C" fn dispatch_getattr(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    let global: &[u8] = match name {
        b"__str__" => b"host_str\0",
        b"__repr__" => b"host_repr\0",
        b"__format__" => b"host_format\0",
        b"__hash__" => b"host_hash\0",
        b"__eq__" => b"host_eq\0",
        b"__abs__" => b"host_abs\0",
        b"__contains__" => b"host_contains\0",
        _ => return PA_ERR_NOTFOUND,
    };
    // SAFETY: 名字 NUL 结尾。
    unsafe { pa_getglobal(state, global.as_ptr().cast()) }
}

fn register_dunder_functions(state: *mut pa_state) {
    let one = [self_param()];
    let sig1 = signature_with(&one);
    let two = [self_param(), self_param()];
    let sig2 = signature_with(&two);
    // SAFETY: 名字/函数/签名按契约。
    unsafe {
        assert_eq!(pa_register(state, b"host_str\0".as_ptr().cast(), host_str, &sig1), PA_OK);
        assert_eq!(pa_register(state, b"host_repr\0".as_ptr().cast(), host_repr, &sig1), PA_OK);
        assert_eq!(pa_register(state, b"host_format\0".as_ptr().cast(), host_format, &sig2), PA_OK);
        assert_eq!(pa_register(state, b"host_hash\0".as_ptr().cast(), host_hash, &sig1), PA_OK);
        assert_eq!(pa_register(state, b"host_eq\0".as_ptr().cast(), host_eq, &sig2), PA_OK);
        assert_eq!(pa_register(state, b"host_abs\0".as_ptr().cast(), host_abs, &sig1), PA_OK);
        assert_eq!(
            pa_register(state, b"host_contains\0".as_ptr().cast(), host_contains, &sig2),
            PA_OK
        );
    }
}

fn register_probe(state: *mut pa_state, ops: Option<pa_type_ops>) {
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    // SAFETY: 名字/回调/签名按契约。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"HostDispatchProbe\0".as_ptr().cast(),
                0,
                0,
                Some(noop_dealloc as PaHostDealloc),
                Some(noop_traverse as PaHostTraverse),
                &signature,
            )
        },
        PA_OK,
        "{}",
        message_of(state)
    );
    let mut payload: *mut c_void = core::ptr::null_mut();
    // SAFETY: 类型在 -1。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // SAFETY: 实例在 -1 ⇒ 全局 `probe`。
    assert_eq!(unsafe { pa_setglobal(state, b"probe\0".as_ptr().cast()) }, PA_OK);
    if let Some(ops) = ops {
        // SAFETY: 类型仍在 -1（不消耗）。
        assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK, "{}", message_of(state));
    }
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
}

#[test]
fn host_getattr_dunders_take_effect() {
    let state = new_state();
    register_dunder_functions(state);
    let ops = pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn: Some(dispatch_getattr),
        setattr_fn: None,
    };
    register_probe(state, Some(ops));
    for (line, what) in [
        (r#"assert str(probe) == "HOST-STR", str(probe)"#, "str"),
        (r#"assert repr(probe) == "HOST-REPR", repr(probe)"#, "repr"),
        (r#"assert format(probe, "") == "HOST-FMT", format(probe, "")"#, "format-empty"),
        (r#"assert format(probe, "d") == "HOST-FMT""#, "format-d"),
        (r#"assert f"{probe}" == "HOST-FMT""#, "fstring"),
        (r#"assert hash(probe) == 123"#, "hash"),
        (r#"assert (probe == 1) is True"#, "eq"),
        (r#"assert (probe != 1) is False, probe != 1"#, "ne"),
        (r#"assert abs(probe) == 7"#, "abs"),
        (r#"assert (1 in probe) is True"#, "contains"),
    ] {
        let code = exec(state, line.as_bytes());
        assert_eq!(code, PA_OK, "{what} 失败：{}", message_of(state));
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn without_ops_host_type_keeps_default_behavior() {
    let state = new_state();
    register_dunder_functions(state);
    register_probe(state, None);
    // 未装 ops ⇒ 没有任何 dunder 命中；默认语义必须保持（零回归）。
    let script = br#"
assert isinstance(str(probe), str)
assert isinstance(repr(probe), str)
assert hash(probe) == hash(probe)
assert (probe == probe) is True
assert (probe == 1) is False
"#;
    assert_eq!(exec(state, script), PA_OK, "无 ops 默认行为失败：{}", message_of(state));
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn builtin_and_user_fast_paths_unchanged() {
    let state = new_state();
    let script = br#"
# builtin fast paths
assert str(1) == "1"
assert repr([1]) == "[1]"
assert format(3, "03d") == "003"
assert format(1.5, ".1f") == "1.5"
assert f"{1.5}" == "1.5"
assert f"{'x'}" == "x"
assert hash(1) == 1
assert len([1, 2]) == 2
assert 1 + 2 == 3
assert 1 == 1
assert abs(-3) == 3
assert (1 in [1, 2]) is True
assert bool([]) is False
# user-class overrides
class U:
    def __str__(self):
        return "U-str"
    def __repr__(self):
        return "U-repr"
    def __format__(self, spec):
        return "U-fmt:" + spec
u = U()
assert str(u) == "U-str"
assert repr(u) == "U-repr"
assert format(u, "x") == "U-fmt:x"
assert f"{u}" == "U-fmt:"
"#;
    assert_eq!(exec(state, script), PA_OK, "快路/用户类等价失败：{}", message_of(state));
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
