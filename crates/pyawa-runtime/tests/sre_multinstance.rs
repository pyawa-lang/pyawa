//! **同进程多实例**：`import re`／`import json`／`re.compile` 的对象不得跨实例串。
//!
//! 缺口（task-10）：`_sre` 的五张表曾经是**进程级** `static Mutex`（含 `re.Pattern`／`re.Match`
//! 的**类型对象地址**）⇒ 同进程第二个 `Instance` 首次编译正则时拿到**第一个实例**的类型
//! （MRO 指向第一个实例的 `object`）⇒ `call_callable` 无限递归／栈溢出。
//!
//! 判据：同一进程建 A／B 两个实例，各自装 stdlib 并 `import json`（导入期就 `re.compile`）
//! 与 `re.compile`／`re.search`；A／B 结果各自正确，且**类型对象不是同一个**（不串对象）。

use core::ffi::c_void;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_runtime::fs_posix::PosixFs;
use pyawa_runtime::PaState;

/// 装实例：stdlib ＋ `fs` 域（`import` 要读 `Lib/` 模块文件；`install` 自己把 `Lib/` 放进 `sys.path`）。
fn make_state(vtable: &pyawa_capabilities::fs::CpFsVtable) -> PaState {
    let state = PaState::new().expect("建实例");
    let raw = state.as_ptr();
    // SAFETY: `raw` 由 `PaState::new` 交回、活到本函数返回值被 drop。
    let instance = unsafe { &*raw }.instance();
    pyawa_stdlib::install(instance, "sre-multinstance", &[]);
    let implementation = (vtable as *const pyawa_capabilities::fs::CpFsVtable).cast::<c_void>();
    // SAFETY: `raw` 刚建成功；表由调用方持有（活到 state 之后）。
    unsafe {
        assert_eq!(
            pyawa_abi::pa_setcapability_async(raw, PA_DOMAIN_FS, PA_ASYNC_OK),
            pyawa_abi::status::PA_OK,
            "`fs` 域分类注册应当成功"
        );
        assert_eq!(
            pyawa_abi::pa_setcapability(raw, PA_DOMAIN_FS, implementation),
            pyawa_abi::status::PA_OK,
            "`fs` 域注册应当成功"
        );
    }
    state
}

fn error_message(state: *mut pyawa_abi::pa_state) -> String {
    // SAFETY: state 存活；借用只在本函数内用。
    let pointer = unsafe { pyawa_abi::pa_errmsg(state) };
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 返回 NUL 结尾借用（`AB-48`）。
    unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

/// 跑一段脚本；失败即带错误信息 panic。
fn exec(state: *mut pyawa_abi::pa_state, source: &str) {
    // SAFETY: state 存活；源码借用活到调用内；opts 传 NULL（`AB-60`／`AB-61`）。
    let status = unsafe {
        pyawa_abi::pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, pyawa_abi::status::PA_OK, "脚本执行失败：{}", error_message(state));
}

/// 取脚本写进全局的名字（字符串）。
fn global_string(state: *mut pyawa_abi::pa_state, name: &str) -> String {
    let name = std::ffi::CString::new(name).expect("测试用名字不含 NUL");
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pyawa_abi::pa_getglobal(state, name.as_ptr()), pyawa_abi::status::PA_OK);
        let mut length = 0usize;
        let pointer = pyawa_abi::pa_tostring(state, -1, &mut length);
        assert!(!pointer.is_null(), "全局不是字符串");
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), length);
        let text = String::from_utf8(bytes.to_vec()).expect("宿主签名文本是 UTF-8");
        assert_eq!(pyawa_abi::pa_settop(state, 0), pyawa_abi::status::PA_OK);
        text
    }
}

/// 取脚本写进全局的名字（整数）。
fn global_int(state: *mut pyawa_abi::pa_state, name: &str) -> i64 {
    let name = std::ffi::CString::new(name).expect("测试用名字不含 NUL");
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pyawa_abi::pa_getglobal(state, name.as_ptr()), pyawa_abi::status::PA_OK);
        let mut value = 0i64;
        assert_eq!(pyawa_abi::pa_tointeger(state, -1, &mut value), pyawa_abi::status::PA_OK);
        assert_eq!(pyawa_abi::pa_settop(state, 0), pyawa_abi::status::PA_OK);
        value
    }
}

const SCRIPT: &str = r#"
import json
import re
pat = re.compile(r'(a)(b)?')
m = pat.search('xaby')
result = '|'.join([str(m.span()), m.group(1), str(m.group(2)), json.dumps({"k": [1, 2]})])
# 类型对象的**地址**（同进程内比较两实例：必须不同 —— 各实例各一份类型对象）
pattern_type_id = id(type(pat))
match_type_id = id(type(m))
"#;

const EXPECTED: &str = "(1, 3)|a|b|{\"k\": [1, 2]}";

#[test]
fn two_instances_do_not_share_sre_objects() {
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    // **A 与 B 都活着**（A 先建后释放会掩盖"跨实例串"的地址复用，这里刻意都活着）。
    let a = make_state(&vtable);
    let b = make_state(&vtable);
    let (a_raw, b_raw) = (a.as_ptr(), b.as_ptr());

    exec(a_raw, SCRIPT);
    exec(b_raw, SCRIPT);

    let a_result = global_string(a_raw, "result");
    let b_result = global_string(b_raw, "result");
    assert_eq!(a_result, EXPECTED, "实例 A 的 re/json 结果应当正确");
    assert_eq!(b_result, EXPECTED, "实例 B 的 re/json 结果应当正确（不得串到 A 的类型对象）");

    // **类型对象互不串**：两实例的 `re.Pattern`／`re.Match` 必须是**不同**对象。
    assert_ne!(
        global_int(a_raw, "pattern_type_id"),
        global_int(b_raw, "pattern_type_id"),
        "`re.Pattern` 类型对象跨实例串了（同进程第二个实例拿到了第一个实例的类型）"
    );
    assert_ne!(
        global_int(a_raw, "match_type_id"),
        global_int(b_raw, "match_type_id"),
        "`re.Match` 类型对象跨实例串了"
    );

    // **先释放再建第三个**：地址会被复用 ⇒ 若还有任何进程级表钉着旧对象/类型，这里会露馅
    // （`PYAWA_DANGLING=1` 下尤其敏感 ✓）。
    drop(a);
    drop(b);
    let c = make_state(&vtable);
    let c_raw = c.as_ptr();
    exec(c_raw, SCRIPT);
    assert_eq!(
        global_string(c_raw, "result"),
        EXPECTED,
        "A／B 释放后新建的实例 C 也应当独立正确（不得复用已释放实例的类型/模式）"
    );
    drop(c);
}
