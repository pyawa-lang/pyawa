//! **`AB-75`／`AB-78`**：`pa_set_object_ops` 三形态＋投影等价差分（`T-AB-12`／`T-AB-14`）。
//!
//! 覆盖：类型/实例/模块挂钩子（每对象一份、互不外溢、`NULL` 清除）、`pa_set_type_ops` 是
//! `pa_set_object_ops(type)` 的投影、模块注入函数的属性写投影、`pa_tointeger`↔`pa_tointstring`
//! 差分、`pa_raise_host_exception`↔「构造异常并抛出」差分。

use core::ffi::{c_char, c_void};
use core::mem::size_of;

use pyawa_abi::host::{pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
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



fn message_of(state: *mut pa_state) -> String {
    let raw = unsafe { pa_errmsg(state) };
    if raw.is_null() {
        return String::new();
    }
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
            b"objops\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    }
}

fn new_state(install_stdlib: bool) -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    if install_stdlib {
        // SAFETY: state 由 pa_create 交回、尚未销毁。
        unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    }
    state
}

/// 属性回调：`answer` ⇒ 42；`boom` ⇒ 错误；其它 ⇒ 未处理。
unsafe extern "C" fn answer_getattr(
    state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    match name {
        b"answer" => unsafe { pa_pushinteger(state, 42) },
        b"boom" => PA_ERR_INVALID,
        _ => PA_ERR_NOTFOUND,
    }
}

fn answer_ops() -> pa_type_ops {
    pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn: Some(answer_getattr),
        setattr_fn: None,
    }
}

/// 注册一个简单宿主类型并返回它的全局名；类型对象留在栈上（调用方负责收/用）。
fn register_type(state: *mut pa_state, name: &'static [u8], global: &'static [u8]) {
    let signature = empty_signature();
    // SAFETY: 名字/函数/签名都有效；base 0 ⇒ object。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                name.as_ptr().cast(),
                0,
                0,
                    Some(noop_dealloc as PaHostDealloc),
                    Some(noop_traverse as PaHostTraverse),
                &signature,)
        },
        PA_OK,
        "注册类型失败：{}",
        message_of(state)
    );
    // 类型在栈顶 ⇒ 绑定到全局名（消费栈顶）。
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(unsafe { pa_setglobal(state, global.as_ptr().cast()) }, PA_OK);
}

/// 用 `pa_setglobal` 名建一个实例并绑定到 `global`。
fn instance_of(state: *mut pa_state, type_global: &'static [u8], global: &'static [u8]) {
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(
        unsafe { pa_getglobal(state, type_global.as_ptr().cast()) },
        PA_OK
    );
    let mut payload: *mut c_void = core::ptr::null_mut();
    // SAFETY: 类型在栈顶（-1）。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // SAFETY: 实例在栈顶 ⇒ 写全局。
    assert_eq!(unsafe { pa_setglobal(state, global.as_ptr().cast()) }, PA_OK);
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
}

#[test]
fn type_and_instance_ops_are_isolated_and_clearable() {
    let state = new_state(false);
    register_type(state, b"OpsType\0", b"OpsType\0");
    instance_of(state, b"OpsType\0", b"ops_a\0");
    instance_of(state, b"OpsType\0", b"ops_b\0");
    let ops = answer_ops();

    // 类型态：一个类型的所有实例都派发。
    // SAFETY: 取回类型压栈。
    assert_eq!(
        unsafe { pa_getglobal(state, b"OpsType\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: 类型在栈顶（-1，不消耗）。
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK, "{}", message_of(state));
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(
        exec(state, b"assert ops_a.answer == 42 and ops_b.answer == 42\n"),
        PA_OK,
        "类型态派发失败：{}",
        message_of(state)
    );

    // 清除类型态 ⇒ 实例都不再有；随后给**单个实例**装 ⇒ 只影响它。
    // SAFETY: 取回类型压栈。
    assert_eq!(
        unsafe { pa_getglobal(state, b"OpsType\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: NULL 清除。
    assert_eq!(unsafe { pa_set_type_ops(state, -1, core::ptr::null()) }, PA_OK);
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(exec(state, b"ops_a.answer\n"), PA_ERR_RUNTIME);
    assert!(message_of(state).contains("AttributeError"), "{}", message_of(state));

    // SAFETY: 取回实例 a 压栈。
    assert_eq!(
        unsafe { pa_getglobal(state, b"ops_a\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: 实例在栈顶（-1，不消耗）。
    assert_eq!(unsafe { pa_set_object_ops(state, -1, &ops) }, PA_OK, "{}", message_of(state));
    // SAFETY: 收掉实例。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(exec(state, b"assert ops_a.answer == 42\n"), PA_OK);
    // 兄弟实例不受影响。
    assert_eq!(exec(state, b"ops_b.answer\n"), PA_ERR_RUNTIME);
    assert!(message_of(state).contains("AttributeError"), "{}", message_of(state));

    // `NULL` 清除实例态。
    // SAFETY: 取回实例 a 压栈。
    assert_eq!(
        unsafe { pa_getglobal(state, b"ops_a\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: NULL 清除。
    assert_eq!(unsafe { pa_set_object_ops(state, -1, core::ptr::null()) }, PA_OK);
    // SAFETY: 收掉实例。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(exec(state, b"ops_a.answer\n"), PA_ERR_RUNTIME);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn set_type_ops_is_a_projection_of_set_object_ops() {
    let state = new_state(false);
    register_type(state, b"ProjType\0", b"ProjType\0");
    instance_of(state, b"ProjType\0", b"proj_obj\0");
    let ops = answer_ops();

    // 投影路径：`pa_set_type_ops(type)`。
    // SAFETY: 取回类型压栈。
    assert_eq!(
        unsafe { pa_getglobal(state, b"ProjType\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: 类型在栈顶。
    assert_eq!(unsafe { pa_set_type_ops(state, -1, &ops) }, PA_OK);
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(exec(state, b"projected = proj_obj.answer\n"), PA_OK);

    // 等价路径：`pa_set_object_ops(type)`。
    // SAFETY: 取回类型压栈。
    assert_eq!(
        unsafe { pa_getglobal(state, b"ProjType\0".as_ptr().cast()) },
        PA_OK
    );
    // SAFETY: 类型在栈顶。
    assert_eq!(unsafe { pa_set_object_ops(state, -1, &ops) }, PA_OK);
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(
        exec(state, b"direct = proj_obj.answer\nassert projected == direct == 42\n"),
        PA_OK,
        "投影等价失败：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn module_object_ops_and_injection_are_attribute_lookups() {
    let state = new_state(true);
    let ops = answer_ops();

    // 模块对象态：测试直接造一个 `AttributeObject` 模块对象（带挂载命名空间），
    // 宿主给**模块对象**装回调；guest 只用普通属性语法读未枚举名字。
    let module_object = {
        // SAFETY: state 由调用方保证存活。
        let instance = unsafe { &*state }.instance();
        let module_type = instance.type_named("module").expect("module 类型已登记");
        let namespace = instance.new_dict();
        instance
            .alloc(pyawa_core::AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<pyawa_core::Header>()
    };
    // SAFETY: 模块对象持有新引用；`pa_pushhandle` 会再加一份给栈。
    assert_eq!(unsafe { pa_pushhandle(state, module_object.as_ptr().cast()) }, PA_OK);
    // SAFETY: 模块对象在栈顶（-1，不消耗）。
    assert_eq!(
        unsafe { pa_set_object_ops(state, -1, &ops) },
        PA_OK,
        "模块态挂钩子失败：{}",
        message_of(state)
    );
    // 模块对象写到全局名（消费栈顶）。
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(unsafe { pa_setglobal(state, b"modprobe\0".as_ptr().cast()) }, PA_OK);
    // SAFETY: 本函数持有的那份引用交回。
    unsafe { (&*state).instance().release_object(module_object.as_ptr()) };
    assert_eq!(
        exec(state, b"assert modprobe.answer == 42\n"),
        PA_OK,
        "模块态属性回退失败：{}",
        message_of(state)
    );

    // 模块注入函数＝向模块对象写属性的投影：`pa_module_*` 装的名字经同一属性查找可见。
    let mut module: *mut c_void = core::ptr::null_mut();
    // SAFETY: 名字 NUL 结尾；out 可写。
    assert_eq!(
        unsafe { pa_add_host_module(state, b"projmod\0".as_ptr().cast(), &mut module) },
        PA_OK,
        "{}",
        message_of(state)
    );
    let signature = empty_signature();
    // SAFETY: 模块句柄/名字/函数/签名按契约。
    assert_eq!(
        unsafe {
            pa_module_addfunction(
                state,
                module,
                b"fn\0".as_ptr().cast(),
                host_zero,
                &signature,
            )
        },
        PA_OK,
        "模块函数注入失败：{}",
        message_of(state)
    );
    // SAFETY: 栈上放 42，装成模块常量。
    assert_eq!(unsafe { pa_pushinteger(state, 42) }, PA_OK);
    // SAFETY: 模块句柄/名字按契约。
    assert_eq!(
        unsafe { pa_module_setconst(state, module, b"answer\0".as_ptr().cast()) },
        PA_OK,
        "模块常量注入失败：{}",
        message_of(state)
    );
    for (script, what) in [
        (&b"import projmod\n"[..], "import"),
        (b"assert projmod.answer == 42\n", "常量属性"),
        (b"assert projmod.fn() is None\n", "函数属性"),
    ] {
        assert_eq!(exec(state, script), PA_OK, "{what} 失败：{}", message_of(state));
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

unsafe extern "C-unwind" fn host_zero(_state: *mut pa_state) -> i32 {
    PA_OK
}

#[test]
fn tointeger_projection_matches_tointstring() {
    let state = new_state(false);
    for value in [0i64, 1, -1, i64::MAX, i64::MIN + 1] {
        // SAFETY: 栈操作按契约。
        assert_eq!(unsafe { pa_pushinteger(state, value) }, PA_OK);
        let mut integer = 0i64;
        // SAFETY: idx 在栈上。
        assert_eq!(unsafe { pa_tointeger(state, -1, &mut integer) }, PA_OK);
        assert_eq!(integer, value);
        let mut len = 0usize;
        // SAFETY: idx 在栈上；len 可写。
        let text = unsafe { pa_tointstring(state, -1, &mut len) };
        assert!(!text.is_null(), "`pa_tointstring` 必须给视图");
        // SAFETY: 借用视图长度由 len 给出。
        let bytes = unsafe { core::slice::from_raw_parts(text.cast::<u8>(), len) };
        assert_eq!(
            String::from_utf8_lossy(bytes),
            value.to_string(),
            "i64 内两投影必须一致"
        );
        // SAFETY: 收栈。
        assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    }
    // i64 外：`pa_tointeger` 如实失败，`pa_tointstring` 仍给十进制串（公开差异，非两个真相）。
    // SAFETY: 十进制串压栈。
    assert_eq!(
        unsafe { pa_pushintstring(state, b"9223372036854775808\0".as_ptr().cast(), -1) },
        PA_OK
    );
    let mut integer = 0i64;
    // SAFETY: idx 在栈上。
    let out_of_range = unsafe { pa_tointeger(state, -1, &mut integer) };
    // 越 i64 ⇒ **如实失败**（大整数通道未接线：`PA_ERR_NOTIMPLEMENTED`，`TS-45`）。
    assert_ne!(out_of_range, PA_OK, "越 i64 必须如实失败");
    let mut len = 0usize;
    // SAFETY: idx 在栈上。
    let text = unsafe { pa_tointstring(state, -1, &mut len) };
    assert!(!text.is_null());
    // SAFETY: 借用视图。
    assert_eq!(
        String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(text.cast::<u8>(), len) }),
        "9223372036854775808"
    );
    // SAFETY: 收栈。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// 宿主函数：取 guest 的 `MyErr` 类并以 `AB-74` 抛 `"boom"`。
unsafe extern "C-unwind" fn host_raise_my_err(state: *mut pa_state) -> i32 {
    // SAFETY: state 由调度器交回；名字 NUL 结尾。
    unsafe {
        if pa_getglobal(state, b"MyErr\0".as_ptr().cast()) != PA_OK {
            return PA_ERR_RUNTIME;
        }
        pa_raise_host_exception(state, -1, b"boom\0".as_ptr().cast(), -1)
    }
}

#[test]
fn host_exception_projection_matches_constructed_raise() {
    let state = new_state(true);
    // SAFETY: 注册宿主函数（0 形参）。
    let signature = empty_signature();
    assert_eq!(
        unsafe {
            pa_register(
                state,
                b"host_raise\0".as_ptr().cast(),
                host_raise_my_err,
                &signature,
            )
        },
        PA_OK
    );
    let script = br#"
class MyErr(Exception):
    pass

def raise_naturally():
    raise MyErr("boom")

natural = None
try:
    raise_naturally()
except MyErr as error:
    natural = (type(error).__name__, error.args[0])

projected = None
try:
    host_raise()
except MyErr as error:
    projected = (type(error).__name__, error.args[0])

assert natural == projected == ("MyErr", "boom")
"#;
    assert_eq!(
        exec(state, script),
        PA_OK,
        "AB-74 投影差分失败：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
