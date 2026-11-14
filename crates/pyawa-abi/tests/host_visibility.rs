//! **`AB-73` 宿主类型属性派发**（`PLAN` §12.11 的 B 阶段第一步；`SPEC-c-abi.md` 的 `AB-73`）。
//!
//! 已覆盖：属性回退读（`getattr_fn` 命中／`PA_ERR_NOTFOUND` 回退到 `AttributeError`／宿主
//! 错误 ⇒ 脚本可捕获的 `RuntimeError`）、`pa_set_type_ops(NULL)` 与"未装 ops"的**零回归**。
//! 未覆盖（下一步 B2）：`setattr_fn`、方法绑定、`AB-74` 异常映射、`len`/`for`/`+` 协议链路。

use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::sync::atomic::{AtomicI64, Ordering};
use std::ptr::NonNull;

use pyawa_abi::host::{pa_param, pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
use pyawa_abi::helpers::{paL_getref, paL_ref};
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

fn empty_signature() -> pa_sig {
    pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    }
}

/// 宿主类型的属性回调：`answer` ⇒ 42；`boom` ⇒ 错误；其它 ⇒ 未处理（回退）。
unsafe extern "C" fn type_getattr(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    match name {
        b"answer" => {
            // SAFETY: state 是当前实例。
            unsafe { pa_pushinteger(state, 42) }
        }
        b"boom" => PA_ERR_INVALID,
        _ => PA_ERR_NOTFOUND,
    }
}

fn ops_with(getattr_fn: Option<pyawa_abi::host::PaTypeGetAttr>) -> pa_type_ops {
    pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn,
        setattr_fn: None,
    }
}

fn message_of(state: *mut pa_state) -> String {
    // SAFETY: state 由调用方保证存活。
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return String::new();
    }
    // SAFETY: pa_errmsg 交回 NUL 结尾的借用串。
    unsafe { core::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

fn exec(state: *mut pa_state, source: &[u8]) -> i32 {
    // SAFETY: 按 `pa_exec_string` 契约传参（显式长度、NUL 结尾块名/模式）。
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"ab73\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

/// 建一个宿主类型实例并绑定到全局名；返回类型在栈上的位置由调用方先 `pa_newtype`。
fn install_instance(state: *mut pa_state, name: &[u8]) {
    let mut payload: *mut c_void = core::ptr::null_mut();
    // 类型在栈顶（pa_newtype 的 +1）；`pa_newhandle` 的 type_index 指它，且不消耗。
    // SAFETY: 契约见 `pa_newhandle`。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // 实例在栈顶 ⇒ 写成全局（消费栈顶）。
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(unsafe { pa_setglobal(state, name.as_ptr().cast()) }, PA_OK);
    // 类型还在栈上 ⇒ 收掉（不消耗属性的检查用）。
    // SAFETY: 上面确认栈上有类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
}

fn setup_type_with_ops(state: *mut pa_state, type_name: &[u8], global: &[u8], callback: pyawa_abi::host::PaTypeGetAttr) {
    let signature = empty_signature();
    // SAFETY: 名字/函数/签名都有效；`pa_newtype` 压入类型对象（+1）。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                type_name.as_ptr().cast(),
                0,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &signature,
            )
        },
        PA_OK,
        "注册宿主类型失败：{}",
        message_of(state)
    );
    let ops = ops_with(Some(callback));
    // SAFETY: 类型在栈顶（-1），ops 有效。
    assert_eq!(
        unsafe { pa_set_type_ops(state, -1, &ops) },
        PA_OK,
        "装 pa_type_ops 失败：{}",
        message_of(state)
    );
    install_instance(state, global);
}

#[test]
fn host_attribute_dispatch_and_fallback() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    setup_type_with_ops(state, b"Thing\0", b"obj\0", type_getattr);

    // ① 命中：`obj.answer` ⇒ 42。
    assert_eq!(exec(state, b"value = obj.answer\ntaken = value == 42\n"), PA_OK);
    // ② 未处理 ⇒ VM 回退到默认 `AttributeError`（裸宿主没有内建名字可 `except` ⇒ 直接看未捕获消息）。
    assert_eq!(exec(state, b"obj.missing\n"), PA_ERR_RUNTIME);
    assert!(
        message_of(state).contains("AttributeError"),
        "回退应当是 AttributeError：{}",
        message_of(state)
    );
    // ③ 宿主错误 ⇒ 不得静默回退，必须转成脚本异常（`RuntimeError`）。
    assert_eq!(exec(state, b"obj.boom\n"), PA_ERR_RUNTIME);
    assert!(
        message_of(state).contains("RuntimeError"),
        "宿主错误应当是 RuntimeError：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn type_without_ops_is_unchanged() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // 未装 ops（`pa_set_type_ops` 的 `NULL` 路径）⇒ 行为与旧类型完全一致。
    let signature = empty_signature();
    // SAFETY: 同上。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Plain\0".as_ptr().cast(),
                0,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &signature,
            )
        },
        PA_OK
    );
    // SAFETY: 类型在栈顶；NULL ⇒ 清空/不派发。
    assert_eq!(unsafe { pa_set_type_ops(state, -1, core::ptr::null()) }, PA_OK);
    install_instance(state, b"plain\0");

    // 未装 ops ⇒ 行为与旧类型一致：属性缺失走默认 `AttributeError`（未捕获 ⇒ 状态码＋消息）。
    assert_eq!(exec(state, b"plain.answer\n"), PA_ERR_RUNTIME);
    assert!(
        message_of(state).contains("AttributeError"),
        "未装 ops 的类型不应有属性：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// ---- B2：`setattr_fn`、方法绑定、`len`/`+` 协议链路 ----

static ATTR_STORAGE: AtomicI64 = AtomicI64::new(0);

/// 两位置形参签名（`self` ＋ 实参；装宿主方法时用）。
fn signature_with_two_positional(left: &'static [u8], right: &'static [u8]) -> (pa_sig, [pa_param; 2]) {
    let params = [
        pa_param {
            size: size_of::<pa_param>(),
            name: left.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
        pa_param {
            size: size_of::<pa_param>(),
            name: right.as_ptr().cast(),
            type_expr: core::ptr::null(),
            flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
            default_handle: core::ptr::null_mut(),
        },
    ];
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: core::ptr::null(),
    };
    (signature, params)
}

fn signature_with_one_positional(name: &'static [u8]) -> (pa_sig, [pa_param; 1]) {
    let params = [pa_param {
        size: size_of::<pa_param>(),
        name: name.as_ptr().cast(),
        type_expr: core::ptr::null(),
        flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
        default_handle: core::ptr::null_mut(),
    }];
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: params.len(),
        params: core::ptr::null(),
    };
    (signature, params)
}

/// 宿主方法：`self.method(x)` ⇒ `x + 100`（`self` 在 `-2`，`x` 在 `-1`）。
unsafe extern "C-unwind" fn host_method(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe {
        let mut x = 0i64;
        if pa_tointeger(state, -1, &mut x) != PA_OK {
            return PA_ERR_INVALID;
        }
        pa_pop(state, 2);
        pa_pushinteger(state, x + 100)
    }
}

/// 宿主 `__len__` ⇒ 7。
unsafe extern "C-unwind" fn host_len(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe {
        pa_pop(state, 1);
        pa_pushinteger(state, 7)
    }
}

/// 宿主 `__add__` ⇒ 9（忽略两侧）。
unsafe extern "C-unwind" fn host_add(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回。
    unsafe {
        pa_pop(state, 2);
        pa_pushinteger(state, 9)
    }
}

/// 宿主 `__iter__`：造一个 `[0, 10, 20]` 列表并返回它的**迭代器**（`for` 要求迭代器）。
unsafe extern "C-unwind" fn host_iter(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回、实例存活。
    let instance = unsafe { &*state }.instance();
    let items: Vec<NonNull<pyawa_core::Header>> =
        (0..3i64).map(|index| instance.new_int(index * 10)).collect();
    let list = instance.new_list(items);
    let iterator = match instance.iter_object(list) {
        Ok(iterator) => iterator,
        Err(_) => {
            // SAFETY: list 是本函数持有的新引用。
            unsafe { instance.release_object(list.as_ptr()) };
            return PA_ERR_RUNTIME;
        }
    };
    // SAFETY: list 的那份交给 `iter_object` 之后归还（迭代器自己持有一份）。
    unsafe { instance.release_object(list.as_ptr()) };
    // `pa_pushhandle` 会再加一份给栈 ⇒ 本函数这份随后归还。
    let status = unsafe { pa_pushhandle(state, iterator.as_ptr().cast()) };
    // SAFETY: iterator 是本函数持有的新引用（栈已另持一份）。
    unsafe { instance.release_object(iterator.as_ptr()) };
    status
}

/// 回调上下文：四个注册表键（`__len__`／`__add__`／`method`／`__iter__`）。
struct MethodRefs {
    refs: [i32; 4],
}

/// 宿主 `getattr`：`__len__`/`__add__`/`method` 返回已保存的宿主函数；`attr` 读存储；`boom` 报错。
unsafe extern "C" fn type_getattr_with_methods(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    // SAFETY: context 是测试里 `Box::leak` 出来的 `MethodRefs`，生命周期覆盖调用。
    let refs = unsafe { &*(context as *const MethodRefs) };
    match name {
        b"__len__" => unsafe { paL_getref(state, refs.refs[0]) },
        b"__add__" => unsafe { paL_getref(state, refs.refs[1]) },
        b"method" => unsafe { paL_getref(state, refs.refs[2]) },
        b"__iter__" => unsafe { paL_getref(state, refs.refs[3]) },
        b"attr" => {
            let value = ATTR_STORAGE.load(Ordering::SeqCst);
            unsafe { pa_pushinteger(state, value) }
        }
        b"boom" => PA_ERR_INVALID,
        _ => PA_ERR_NOTFOUND,
    }
}

/// 宿主 `setattr`：`attr` ⇒ 存进 `ATTR_STORAGE`（值在 `-1`，借用不消耗）。
unsafe extern "C" fn type_setattr(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    value_index: i32,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    match name {
        b"attr" => {
            let mut value = 0i64;
            // SAFETY: state 由回调契约交回。
            if unsafe { pa_tointeger(state, value_index, &mut value) } != PA_OK {
                return PA_ERR_INVALID;
            }
            ATTR_STORAGE.store(value, Ordering::SeqCst);
            PA_OK
        }
        _ => PA_ERR_NOTFOUND,
    }
}

/// 注册一个宿主函数并 `paL_ref` 保存，返回注册表键。
fn register_and_ref(state: *mut pa_state, name: &'static [u8], function: pyawa_abi::host::PaHostFn, sig: pa_sig) -> i32 {
    // SAFETY: 名字/函数/签名都有效。
    assert_eq!(unsafe { pa_register(state, name.as_ptr().cast(), function, &sig) }, PA_OK);
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(unsafe { pa_getglobal(state, name.as_ptr().cast()) }, PA_OK);
    let mut reference = 0i32;
    // SAFETY: 栈顶是刚取出的函数对象。
    assert_eq!(unsafe { paL_ref(state, -1, &mut reference) }, PA_OK);
    // SAFETY: 收掉栈上的函数对象。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    reference
}

#[test]
fn setattr_and_method_binding_and_protocols() {
    ATTR_STORAGE.store(0, Ordering::SeqCst);
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // Python 层内建（`len` 等）由组合根装上（`CM-14`；裸 ABI 实例只有语言骨架）。
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };

    // 注册三个宿主方法函数并保存键。
    let (mut len_sig, len_params) = signature_with_one_positional(b"self\0");
    len_sig.params = len_params.as_ptr();
    let len_ref = register_and_ref(state, b"host_len\0", host_len, len_sig);
    let (mut add_sig, add_params) = signature_with_two_positional(b"self\0", b"other\0");
    add_sig.params = add_params.as_ptr();
    let add_ref = register_and_ref(state, b"host_add\0", host_add, add_sig);
    let (mut method_sig, method_params) = signature_with_two_positional(b"self\0", b"value\0");
    method_sig.params = method_params.as_ptr();
    let method_ref = register_and_ref(state, b"host_method\0", host_method, method_sig);
    let (mut iter_sig, iter_params) = signature_with_one_positional(b"self\0");
    iter_sig.params = iter_params.as_ptr();
    let iter_ref = register_and_ref(state, b"host_iter\0", host_iter, iter_sig);

    let context = Box::leak(Box::new(MethodRefs {
        refs: [len_ref, add_ref, method_ref, iter_ref],
    }));

    // 建宿主类型并装两个回调（`context` 透传）。
    let signature = empty_signature();
    // SAFETY: 名字/函数/签名都有效；`pa_newtype` 压入类型对象（+1）。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"Thing2\0".as_ptr().cast(),
                0,
                noop_dealloc as PaHostDealloc,
                noop_traverse as PaHostTraverse,
                &signature,
            )
        },
        PA_OK,
        "注册宿主类型失败：{}",
        message_of(state)
    );
    let ops = pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: context as *mut MethodRefs as *mut c_void,
        getattr_fn: Some(type_getattr_with_methods),
        setattr_fn: Some(type_setattr),
    };
    // SAFETY: 类型在栈顶（-1），ops 有效。
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK, "{}", message_of(state));
    install_instance(state, b"obj2\0");

    // ① 属性写 + 读（宿主回调往返）。
    let script = b"obj2.attr = 5\nassert obj2.attr == 5\n";
    assert_eq!(exec(state, script), PA_OK, "setattr/getattr 往返失败：{}", message_of(state));
    // ② 方法绑定：`obj2.method(5)` ⇒ `self + x` ⇒ 105（`pa_sig` 含 self）。
    let script = b"v = obj2.method(5)\nassert v == 105\n";
    assert_eq!(exec(state, script), PA_OK, "方法绑定失败：{}", message_of(state));
    // ③ `len(obj2)` 经 `__len__` ⇒ 7。
    let script = b"n = len(obj2)\nassert n == 7\n";
    assert_eq!(exec(state, script), PA_OK, "len 协议失败：{}", message_of(state));
    // ④ `obj2 + 1` 经 `__add__` ⇒ 9。
    let script = b"s = obj2 + 1\nassert s == 9\n";
    assert_eq!(exec(state, script), PA_OK, "+ 协议失败：{}", message_of(state));
    // ⑤ `for x in obj2` 经 `__iter__`（宿主造列表）⇒ 0+10+20 = 30。
    let script = b"total = 0\nfor x in obj2:\n    total += x\nassert total == 30\n";
    assert_eq!(exec(state, script), PA_OK, "for 协议失败：{}", message_of(state));
    // ⑥ 宿主错误仍不得静默回退。
    assert_eq!(exec(state, b"obj2.boom\n"), PA_ERR_RUNTIME);
    assert!(message_of(state).contains("RuntimeError"), "{}", message_of(state));
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
