//! **`AB-80`／`T-AB-20`**：`pa_writetobytes` —— 一次性写回恰好 `bytearray` 的数据区。
//!
//! 覆盖：成功写回＋guest 读到新内容＋栈不变；越界/usize 溢出 ⇒ 拒绝且**不部分写**；`bytes`／
//! 子类／非字节／非法 `idx` 拒绝；`p == NULL && len > 0` 拒绝；`len == 0`（合法范围）no-op；
//! 失败 `pa_errmsg` 非空。无 numpy、纯 ABI。

use core::ffi::{c_char, c_void};
use core::mem::size_of;

use pyawa_abi::host::{pa_sig, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
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
            b"write-tobytes\0".as_ptr().cast(),
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

/// 取 guest 全局名到栈顶（借用 → 栈持有）。
fn push_global(state: *mut pa_state, name: &'static [u8]) {
    // SAFETY: 名字 NUL 结尾。
    assert_eq!(
        unsafe { pa_getglobal(state, name.as_ptr().cast()) },
        PA_OK,
        "取全局失败：{}",
        message_of(state)
    );
}

fn write_bytes(state: *mut pa_state, offset: usize, bytes: &[u8]) -> i32 {
    // SAFETY: `-1` 是刚压入的对象；bytes 在调用期间可读。
    unsafe { pa_writetobytes(state, -1, offset, bytes.as_ptr().cast(), bytes.len()) }
}

#[test]
fn writes_bytearray_data_and_guest_reads_it() {
    let state = new_state();
    assert_eq!(exec(state, b"data = bytearray(b'abcdef')\n"), PA_OK, "{}", message_of(state));
    push_global(state, b"data\0");
    let before = unsafe { pa_gettop(state) };
    // SAFETY: 栈顶是 data；写 "XYZ" 到 offset 1。
    assert_eq!(
        unsafe { pa_writetobytes(state, -1, 1, b"XYZ".as_ptr().cast(), 3) },
        PA_OK,
        "{}",
        message_of(state)
    );
    assert_eq!(unsafe { pa_gettop(state) }, before, "成功调用不得改变栈");
    // SAFETY: 收掉 data。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(
        exec(state, b"assert bytes(data) == b'aXYZef'\n"),
        PA_OK,
        "guest 未读到新内容：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn range_and_overflow_are_rejected_without_partial_write() {
    let state = new_state();
    assert_eq!(exec(state, b"data = bytearray(b'abcdef')\n"), PA_OK);
    push_global(state, b"data\0");
    // 越界：5+2 > 6。
    assert_eq!(write_bytes(state, 5, b"XY"), PA_ERR_INVALID);
    assert!(!message_of(state).is_empty(), "越界必须带消息");
    // usize 溢出：MAX-1 + 2。
    assert_eq!(
        unsafe {
            pa_writetobytes(
                state,
                -1,
                usize::MAX - 1,
                b"XY".as_ptr().cast(),
                2,
            )
        },
        PA_ERR_INVALID
    );
    assert!(!message_of(state).is_empty(), "溢出必须带消息");
    // SAFETY: 收掉 data。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    // 不得部分写。
    assert_eq!(
        exec(state, b"assert bytes(data) == b'abcdef'\n"),
        PA_OK,
        "失败路径不得部分写：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn rejects_bytes_subclass_nonbytes_and_bad_index() {
    let state = new_state();
    let cases: &[(&[u8], &[u8])] = &[
        (b"raw = b'abcdef'\n", b"raw\0"),
        (b"class BA(bytearray):\n    pass\nsub = BA(b'abcdef')\n", b"sub\0"),
        (b"items = [1, 2, 3]\n", b"items\0"),
        (b"text = 'abcdef'\n", b"text\0"),
        (b"number = 42\n", b"number\0"),
    ];
    for (source, name) in cases {
        assert_eq!(exec(state, source), PA_OK, "{}", message_of(state));
        push_global(state, name);
        assert_eq!(
            write_bytes(state, 0, b"Z"),
            PA_ERR_INVALID,
            "{} 不应接受写入",
            String::from_utf8_lossy(name)
        );
        assert!(!message_of(state).is_empty(), "拒绝必须带消息");
        // SAFETY: 收掉该值。
        assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    }
    // 非法 idx（不存在）。
    // SAFETY: 不存在的栈索引不得解引用。
    assert_eq!(
        unsafe { pa_writetobytes(state, 100, 0, b"Z".as_ptr().cast(), 1) },
        PA_ERR_INVALID
    );
    assert!(!message_of(state).is_empty());
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn null_pointer_and_zero_length_rules() {
    let state = new_state();
    assert_eq!(exec(state, b"data = bytearray(b'abcdef')\n"), PA_OK);
    push_global(state, b"data\0");
    // NULL + len>0 ⇒ 拒绝。
    // SAFETY: 空指针且 len>0，函数不读取。
    assert_eq!(
        unsafe { pa_writetobytes(state, -1, 0, core::ptr::null(), 1) },
        PA_ERR_INVALID
    );
    assert!(!message_of(state).is_empty());
    // len==0 且 offset <= 长度 ⇒ no-op PA_OK（p 可为 NULL）。
    // SAFETY: len=0，p 不被读取。
    assert_eq!(
        unsafe { pa_writetobytes(state, -1, 0, core::ptr::null(), 0) },
        PA_OK
    );
    assert_eq!(
        unsafe { pa_writetobytes(state, -1, 6, core::ptr::null(), 0) },
        PA_OK,
        "offset==长度时 len==0 应 OK：{}",
        message_of(state)
    );
    // len==0 但 offset > 长度 ⇒ 范围仍拒绝。
    assert_eq!(
        unsafe { pa_writetobytes(state, -1, 7, core::ptr::null(), 0) },
        PA_ERR_INVALID
    );
    // SAFETY: 收掉 data。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);
    assert_eq!(
        exec(state, b"assert bytes(data) == b'abcdef'\n"),
        PA_OK,
        "no-op/拒绝路径不得改内容：{}",
        message_of(state)
    );
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// 防"未用 import"：保持与其它 ABI 测试相同的宿主结构体构造路径（类型面）。
#[allow(dead_code)]
fn _signature_shape() -> (pa_sig, PaHostDealloc, PaHostTraverse) {
    // 仅编译期形状检查；不参与运行。
    let signature = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 0,
        params: core::ptr::null(),
    };
    unsafe extern "C" fn dealloc(_: *mut c_void) {}
    unsafe extern "C" fn traverse(
        _: *mut c_void,
        _: *mut c_void,
        _: unsafe extern "C" fn(*mut c_void, *mut c_void),
    ) {
    }
    (signature, dealloc as PaHostDealloc, traverse as PaHostTraverse)
}

#[allow(dead_code)]
fn _assert_c_char(_: *const c_char) {}
