//! **协议 oracle 机检**（用户「对象语义唯一入口＝属性查找」不变量；`PLAN §12.12` 的机检并入）。
//!
//! 纯 ABI、**无 numpy**：给宿主对象挂一个**记录所有被查名字**的 `getattr_fn`（一律
//! `PA_ERR_NOTFOUND`），对 `pyawa_core::PROTOCOL_CASES` 的**每一条**协议跑一个用例，
//! 断言该协议的 dunder **出现在记录里**——没出现 ⇒ 该协议绕过了属性查找（旁路）。
//!
//! **禁止**用"只覆盖已实现协议"让机检变绿：片段表↔清单必须**双向一致**（缺片段红、[多片段]
//! 没有清单条目也红）；新增协议必须两边同时改。

use core::ffi::{c_char, c_void};
use core::mem::size_of;
use std::sync::Mutex;

use pyawa_abi::host::{pa_param, pa_sig, pa_type_ops, PaHostDealloc, PaHostTraverse};
use pyawa_abi::status::*;
use pyawa_abi::*;

/// 记录被 `getattr_fn` 问过的名字（单测逐条清空；测试只有一个 `#[test]`，无并发问题）。
static QUERIED: Mutex<Vec<String>> = Mutex::new(Vec::new());

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

/// **记录式** `getattr_fn`：只记录名字，一律"未处理"（回退默认）——oracle 只看**有没有问**。
unsafe extern "C" fn recording_getattr(
    _state: *mut pa_state,
    _payload: *mut c_void,
    name: *const c_char,
    len: usize,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: 契约保证 name 至少 len 字节可读。
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), len) };
    if let Ok(name) = core::str::from_utf8(name) {
        QUERIED.lock().expect("记录锁").push(name.to_owned());
        // `with` 先查 `__exit__`、再查 `__enter__`：让 `__exit__` "成功"（nil）才能走到
        // `__enter__` 的查找（只为记录；真调用失败也无妨，脚本包了 try/except）。
        if name == "__exit__" || name == "__aexit__" {
            // SAFETY: state 由回调契约交回。
            return unsafe { pa_pushnil(_state) };
        }
        // 让 `async for` 走到 `__anext__` 的查找（`__aiter__` 直接返回 probe 自身）。
        if name == "__aiter__" {
            // `__aiter__` 应该是**可调用**的：返回宿主函数 `host_aiter`（调用后交回 probe 自身）。
            // SAFETY: state 由回调契约交回。
            return unsafe { pa_getglobal(_state, b"host_aiter\0".as_ptr().cast()) };
        }
    }
    PA_ERR_NOTFOUND
}

fn exec(state: *mut pa_state, source: &str) -> i32 {
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            b"protocol-oracle\0".as_ptr().cast(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
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

/// 一条用例的**可执行片段**（键＝`ProtocolCase::oracle`；值＝脚本正文，可多行）。
fn snippet(oracle: &str) -> Option<&'static str> {
    Some(match oracle {
        // 文本/格式化
        "str" => "str(probe)",
        "repr" => "repr(probe)",
        "format" => "format(probe, \"\")",
        "format_fstring" => "f\"{probe}\"",
        // 真值/哈希
        "bool" => "bool(probe)",
        "hash" => "hash(probe)",
        // 富比较
        "eq" => "probe == 1",
        "ne" => "probe != 1",
        "lt" => "probe < 1",
        "le" => "probe <= 1",
        "gt" => "probe > 1",
        "ge" => "probe >= 1",
        // 容器/迭代
        "len" => "len(probe)",
        "contains" => "1 in probe",
        "getitem" => "probe[0]",
        "setitem" => "probe[0] = 1",
        "delitem" => "del probe[0]",
        "iter" => "iter(probe)",
        "next" => "next(probe)",
        "reversed" => "reversed(probe)",
        // 调用/上下文
        "call" => "probe()",
        "enter" => "with probe:\n    pass",
        "exit" => "with probe:\n    pass",
        // 异步协议：`async def` 在本层是生成器 ⇒ 要 **send(None) 驱动**到协议点。
        "await" => "async def _c():\n    await probe\ng = _c()\ntry:\n    g.send(None)\nexcept Exception:\n    pass",
        "aiter" => "async def _c():\n    async for _ in probe:\n        pass\ng = _c()\ntry:\n    g.send(None)\nexcept Exception:\n    pass",
        "anext" => "async def _c():\n    async for _ in probe:\n        pass\ng = _c()\ntry:\n    g.send(None)\nexcept Exception:\n    pass",
        "aenter" => "async def _c():\n    async with probe:\n        pass\ng = _c()\ntry:\n    g.send(None)\nexcept Exception:\n    pass",
        "aexit" => "async def _c():\n    async with probe:\n        pass\ng = _c()\ntry:\n    g.send(None)\nexcept Exception:\n    pass",
        // 算术（正向）
        "add" => "probe + 1",
        "sub" => "probe - 1",
        "mul" => "probe * 1",
        "truediv" => "probe / 1",
        "floordiv" => "probe // 1",
        "mod" => "probe % 1",
        "pow" => "probe ** 1",
        "matmul" => "probe @ 1",
        "lshift" => "probe << 1",
        "rshift" => "probe >> 1",
        "and" => "probe & 1",
        "or" => "probe | 1",
        "xor" => "probe ^ 1",
        // 算术（反射）
        "radd" => "1 + probe",
        "rsub" => "1 - probe",
        "rmul" => "1 * probe",
        "rtruediv" => "1 / probe",
        "rfloordiv" => "1 // probe",
        "rmod" => "1 % probe",
        "rpow" => "1 ** probe",
        "rmatmul" => "1 @ probe",
        "rlshift" => "1 << probe",
        "rrshift" => "1 >> probe",
        "rand" => "1 & probe",
        "ror" => "1 | probe",
        "rxor" => "1 ^ probe",
        // 原位算术
        "iadd" => "probe += 1",
        "isub" => "probe -= 1",
        "imul" => "probe *= 1",
        "itruediv" => "probe /= 1",
        "ifloordiv" => "probe //= 1",
        "imod" => "probe %= 1",
        "ipow" => "probe **= 1",
        "imatmul" => "probe @= 1",
        "ilshift" => "probe <<= 1",
        "irshift" => "probe >>= 1",
        "iand" => "probe &= 1",
        "ior" => "probe |= 1",
        "ixor" => "probe ^= 1",
        // 一元
        "neg" => "-probe",
        "pos" => "+probe",
        "abs" => "abs(probe)",
        "invert" => "~probe",
        // 转换
        "int" => "int(probe)",
        "float" => "float(probe)",
        "complex" => "complex(probe)",
        "index" => "[0, 1][probe]",
        _ => return None,
    })
}

/// 把片段包成"异常也不打断"的脚本（oracle 只关心**问没问**，不关心回退结果）。
fn script_for(body: &str) -> String {
    let indented = body
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("try:\n{indented}\nexcept Exception:\n    pass\n")
}

#[test]
fn every_protocol_case_queries_host_getattr() {
    // **清单↔片段表双向一致**（防止"只覆盖已实现协议"让机检变绿）。
    let mut cases = std::collections::BTreeMap::new();
    for case in pyawa_core::PROTOCOL_CASES {
        assert!(
            cases.insert(case.oracle, case.dunder).is_none(),
            "清单里 oracle 键重复：{}",
            case.oracle
        );
        assert!(
            snippet(case.oracle).is_some(),
            "清单协议 `{}/{}` 没有 oracle 片段",
            case.protocol,
            case.dunder
        );
    }
    let known = [
        "str", "repr", "format", "format_fstring", "bool", "hash", "eq", "ne", "lt", "le", "gt", "ge", "len",
        "contains", "getitem", "setitem", "delitem", "iter", "next", "reversed", "call", "enter",
        "exit", "await", "aiter", "anext", "aenter", "aexit", "add", "sub", "mul", "truediv", "floordiv", "mod", "pow", "matmul", "lshift",
        "rshift", "and", "or", "xor", "radd", "rsub", "rmul", "rtruediv", "rfloordiv", "rmod",
        "rpow", "rmatmul", "rlshift", "rrshift", "rand", "ror", "rxor", "iadd", "isub", "imul",
        "itruediv", "ifloordiv", "imod", "ipow", "imatmul", "ilshift", "irshift", "iand", "ior",
        "ixor", "neg", "pos", "abs", "invert", "int", "float", "complex", "index",
    ];
    for key in known {
        assert!(
            cases.contains_key(key),
            "oracle 片段 `{key}` 没有清单条目（新增片段必须同时登记协议清单）"
        );
    }

    // **`host_aiter()`**：oracle 的 `__aiter__` 交回 probe 自身（好让流程走到 `__anext__`）。
    unsafe extern "C-unwind" fn host_aiter(state: *mut pa_state) -> i32 {
        // SAFETY: state 由调度器交回；probe 是测试装的全局。
        unsafe { pa_getglobal(state, b"probe\0".as_ptr().cast()) }
    }

    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按契约传参。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "oracle", &[]) };
    let one = pa_param {
        size: size_of::<pa_param>(),
        name: b"self\0".as_ptr().cast(),
        type_expr: core::ptr::null(),
        flags: pyawa_abi::host::param_flags::PA_PARAM_POSITIONAL,
        default_handle: core::ptr::null_mut(),
    };
    let sig = pa_sig {
        size: size_of::<pa_sig>(),
        flags: 0,
        ret_expr: core::ptr::null(),
        nparams: 1,
        params: &one,
    };
    // SAFETY: 名字/函数/签名按契约。
    assert_eq!(
        unsafe { pa_register(state, b"host_aiter\0".as_ptr().cast(), host_aiter, &sig) },
        PA_OK,
        "{}",
        message_of(state)
    );

    // 宿主类型 + 记录式 getattr；实例挂到全局 `probe`。
    let signature = empty_signature();
    // SAFETY: 名字/回调/签名按契约；base 0 ⇒ object。
    assert_eq!(
        unsafe {
            pa_newtype(
                state,
                b"OracleProbe\0".as_ptr().cast(),
                0,
                0,
                Some(noop_dealloc as PaHostDealloc),
                Some(noop_traverse as PaHostTraverse),
                &signature,
            )
        },
        PA_OK,
        "注册宿主类型失败：{}",
        message_of(state)
    );
    // 类型在栈顶：先建实例再装 ops（`pa_set_type_ops` 不消耗栈顶）。
    let mut payload: *mut c_void = core::ptr::null_mut();
    // SAFETY: 类型在 -1。
    assert_eq!(unsafe { pa_newhandle(state, -1, &mut payload) }, PA_OK);
    // SAFETY: 实例在 -1 ⇒ 写全局 `probe`。
    assert_eq!(unsafe { pa_setglobal(state, b"probe\0".as_ptr().cast()) }, PA_OK);
    // SAFETY: 类型仍在栈顶（-1，不消耗）。
    let ops = pa_type_ops {
        size: size_of::<pa_type_ops>(),
        context: core::ptr::null_mut(),
        getattr_fn: Some(recording_getattr),
        setattr_fn: None,
    };
    assert_eq!(
        unsafe { pa_set_type_ops(state, -1, &ops) },
        PA_OK,
        "{}",
        message_of(state)
    );
    // SAFETY: 收掉类型。
    assert_eq!(unsafe { pa_pop(state, 1) }, PA_OK);

    let mut failures = Vec::new();
    for case in pyawa_core::PROTOCOL_CASES {
        let body = snippet(case.oracle).expect("双向一致已在上面检查");
        QUERIED.lock().expect("记录锁").clear();
        let code = exec(state, &script_for(body));
        let queried = QUERIED.lock().expect("记录锁").clone();
        if !queried.iter().any(|name| name == case.dunder) {
            failures.push(format!(
                "{}/{}（片段 `{}`，状态 {code}）：getattr 记录里没有 `{}`；实际问过 {:?}",
                case.protocol, case.dunder, case.oracle, case.dunder, queried
            ));
        }
    }
    // SAFETY: 同上。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
    assert!(
        failures.is_empty(),
        "以下协议绕过了属性查找（宿主 getattr_fn 未被询问）：\n  - {}",
        failures.join("\n  - ")
    );
}
