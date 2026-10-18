//! **`T-CP-*` 的可执行验收**（M5 判据①：`SPEC-capabilities.md` §11）。
//!
//! 本文件只验**能力形状层 ＋ ABI 注册 ＋ VM 侧适配层**能独立验的条目：
//! `T-CP-1`／`T-CP-2`／`T-CP-5`（映射的一半）／`T-CP-7`（互不可见＋销毁兜底释放）／
//! `T-CP-8`／`T-CP-9`／`T-CP-12`（通道的一半）／`T-CP-3`（`select` 的 core 分流域）／
//! `T-CP-10`（可取消等待的 core 语义：分片／已取消／未开始／无后续 provider 调用）。
//!
//! **不**在这里验的（有专门的可执行验收，别在本文件重复造第二个真相）：
//! - `T-CP-4`／`T-CP-5`（真 provider）／`T-CP-7`（真 provider 的残留句柄半边）／
//!   `T-CP-11`／`T-CP-12`（真 provider 的一半）
//!   ⇒ `crates/pyawa-runtime/tests/two_providers.rs`（真实 `PosixFs` vs 受限 `RestrictedFs`）；
//! - `T-CP-3` 的 Python 面（`select.select`／`os.dup`／`os.dup2`）⇒ 同文件
//!   `crates/pyawa-runtime/tests/two_providers.rs`（e2e）＋ `crates/pyawa-runtime/tests/select_dup2_shapes.rs`；
//! - `T-CP-10` 的 **e2e**（挂起中另一线程 `pa_interrupt` ⇒ ≤2 s 返回／真 CLI 真睡）⇒
//!   `crates/pyawa-runtime/tests/t_cp10_cancel.rs`（真实例、真 provider、真 CLI）；
//! - `T-CP-6`／`T-CP-13`（静态检查 ⇒ `python3 tests/ci/check.py` 的 `T-CX-4`）。
//!
//! **做法**：自造 vtable（`CP-12`：形状层只含类型与函数指针；测试侧当"宿主"）。这样每条断言都
//! 只钉**VM 侧适配层**的语义，不借真实机器实现——真实档那一半在 `two_providers.rs` 里单独验。
//!
//! **投影到哪条 `CP-`** 逐条写在测试函数的文档里（便于 `CX-1` 反查）。

use core::ffi::c_void;
use std::sync::Mutex;

use pyawa_abi::capability::{PA_ASYNC_NO, PA_ASYNC_OK, PA_DOMAIN_FS, PA_DOMAIN_IPC};
use pyawa_abi::status::{PA_ERR_INVALID, PA_OK};
use pyawa_abi::{
    pa_create, pa_destroy, pa_errmsg, pa_exec_string, pa_host, pa_setcapability,
    pa_setcapability_async, pa_state, PA_ABI_SIZE, PA_ABI_VERSION,
};
use pyawa_capabilities::fs::{CapStatus, CpFsVtable, Handle};
use pyawa_core::CapabilityCallError;

// --------------------------------------------------------------------------- #
// 基建：建实例、注册 vtable、跑一段脚本并取回未捕获异常
// --------------------------------------------------------------------------- #

/// **`CP-30`**：注册路径会读并校验 vtable 公共头 `(version, size)` ⇒ 测试侧当"宿主"的表必须
/// 填**合法头**。`CpFsVtable::UNIMPLEMENTED` 的 `version: 0` 是"未填"哨兵，**不能**直接进注册路径；
/// 这里以它为底、把公共头换成 `PA_ABI_VERSION` ＋ 本结构体字节数（槽位仍全 `None`，语义不变）。
fn valid_fs_table() -> CpFsVtable {
    CpFsVtable {
        version: PA_ABI_VERSION,
        size: core::mem::size_of::<CpFsVtable>() as u32,
        ..CpFsVtable::UNIMPLEMENTED
    }
}

/// 建一个可用实例（`AB-55` 的 C 形态入口；失败即 panic——本文件不测创建失败那条）。
fn new_state() -> *mut pa_state {
    let host = pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按 `AB-8`／`AB-43` 的契约传宿主结构；出参是本函数的局部量。
    let created = unsafe { pa_create(&host, &mut state) };
    assert_eq!(created, PA_OK, "建实例失败");
    assert!(!state.is_null(), "`pa_create` 成功了却不交回实例");
    state
}

/// 按 `AB-33`／`AB-34` 注册一个 `fs` 域实现（表由调用方持有且必须比 state 活得久）。
///
/// # Safety
///
/// `state` 必须由 [`new_state`] 交回且尚未销毁；`table` 的地址在 `state` 存活期内有效。
unsafe fn register_fs(state: *mut pa_state, table: *const CpFsVtable) {
    // SAFETY: 调用方契约（见函数注释）。
    unsafe {
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK,
            "声明 `fs` 域异步分类失败"
        );
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, table.cast()),
            PA_OK,
            "注册 `fs` 域实现失败"
        );
    }
}

/// 测试侧"标准流"：`print` 经 `fs` 域的 `write` 槽落到这里（**不**往测试进程的 fd 1 写）。
static STDOUT_SINK: Mutex<Vec<u8>> = Mutex::new(Vec::new());

extern "C" fn sink_write(
    _state: *mut c_void,
    _handle: Handle,
    bytes: *const u8,
    len: usize,
    out_len: *mut usize,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 契约同 `SPEC-capabilities.md` §9.1（缓冲区可读 `len` 字节）。
    let slice = unsafe { core::slice::from_raw_parts(bytes, len) };
    STDOUT_SINK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .extend_from_slice(slice);
    if !out_len.is_null() {
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = len };
    }
    CapStatus::Ok
}

/// 一次脚本执行的可观察结果（减配版：退出码 ＋ 未捕获异常类型／消息）。
struct Run {
    /// `pa_exec_string == PA_OK` ⇒ 0，否则 1（与 `crates/pyawa-abi/tests/conformance.rs` 同口径）。
    exit: i32,
    /// 未捕获异常（`类型`，`消息`）；正常退出 ⇒ `None`。
    exception: Option<(String, String)>,
}

/// 跑一段纯 Python 脚本（`python` 模式）。
fn run_script(state: *mut pa_state, source: &str) -> Run {
    let mode = b"python\0";
    // SAFETY: 源码指针＋长度由本函数的借用保证；`chunkname`／`options` 传 NULL（`AB-60`／`AB-61`）。
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    };
    let exit = if status == PA_OK { 0 } else { 1 };
    let message = if status == PA_OK {
        String::new()
    } else {
        // SAFETY: `pa_errmsg` 的借用只在本次调用之后有效 ⇒ 当场复制成 String（`AB-48`）。
        unsafe {
            let pointer = pa_errmsg(state);
            if pointer.is_null() {
                String::new()
            } else {
                core::ffi::CStr::from_ptr(pointer).to_string_lossy().into_owned()
            }
        }
    };
    // 未捕获异常的消息形态：`类型: str(异常)`（core 的 `exception_uncaught_text` 一处真相）。
    let exception = if message.is_empty() {
        None
    } else {
        match message.split_once(": ") {
            Some((kind, text)) => Some((kind.trim().to_owned(), text.trim().to_owned())),
            None => Some((message.trim().to_owned(), String::new())),
        }
    };
    Run { exit, exception }
}

fn take_stdout() -> String {
    let bytes = {
        let mut sink = STDOUT_SINK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        core::mem::take(&mut *sink)
    };
    String::from_utf8_lossy(&bytes).into_owned()
}

// --------------------------------------------------------------------------- #
// 自造 vtable（宿主是测试自己）
// --------------------------------------------------------------------------- #

const EACCES: i32 = 13;
const ENOENT: i32 = 2;
const EBADF: i32 = 9;

/// 拒绝实现：所有调用都报**真实机器错误**（`EACCES`）——用于 `T-CP-2`（`CP-6`）。
extern "C" fn refuse_open(
    _state: *mut c_void,
    _name: *const u8,
    _len: usize,
    _flags: i32,
    _mode: u32,
    _out: *mut Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 出参由调用方提供。
    unsafe { *errno_out = EACCES };
    CapStatus::Machine
}

/// 拒绝 `stat`（`ENOENT`）：与 `open` 的 `EACCES` 同域不同 errno。
extern "C" fn refuse_stat(
    _state: *mut c_void,
    _name: *const u8,
    _len: usize,
    _out: *mut pyawa_capabilities::fs::FileInfo,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 出参由调用方提供。
    unsafe { *errno_out = ENOENT };
    CapStatus::Machine
}

/// "provider 没有这个路径"（`ENOENT`）——用于 `T-CP-5`（`CP-23` 的一半：不是"未实现"）。
extern "C" fn missing_open(
    _state: *mut c_void,
    _name: *const u8,
    _len: usize,
    _flags: i32,
    _mode: u32,
    _out: *mut Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 出参由调用方提供。
    unsafe { *errno_out = ENOENT };
    CapStatus::Machine
}

/// 一个只接 `write` 的 vtable（`CP-3`：其余槽位保持"未实现"）。
fn write_only_vtable() -> CpFsVtable {
    CpFsVtable { write: Some(sink_write), ..valid_fs_table() }
}

// --------------------------------------------------------------------------- #
// T-CP-1／T-CP-2／T-CP-5（CP-2／CP-3／CP-5／CP-6）
// --------------------------------------------------------------------------- #

/// **`T-CP-1`**：域／槽位未实现时，**创建实例成功**、**调用**才返回"未实现"
/// （`CP-2`：整域 `null`／未注册；`CP-3`：单槽 `None`；`CP-5`：三态通道）。
#[test]
fn unimplemented_domains_and_slots_are_reported_at_call_time() {
    let state = new_state();
    // SAFETY: `state` 由 `new_state` 交回且活到本测试末尾；`instance()` 只借用。
    let instance = unsafe { &*state }.instance();

    // `CP-2`：未注册 ⇒ 创建没有失败（上面断言过 PA_OK），**调用**才报"未实现"。
    assert!(instance.fs_vtable().is_none(), "未注册的域不该有 vtable");
    assert_eq!(
        instance.fs_open(b"x", 0, 0),
        Err(CapabilityCallError::NotRegistered),
        "`CP-2`：整域未实现 ⇒ 调用时报未实现"
    );
    assert_eq!(
        instance.fs_write(1, b"x"),
        Err(CapabilityCallError::NotRegistered),
        "`CP-2`：同一域的所有槽位都经同一条三态通道"
    );

    // `CP-2` 的另一种形态：显式注册 `null` vtable（"提供了这个域但整域为空"）。
    // SAFETY: `state` 活到本测试末尾；`PA_DOMAIN_FS` 在表内。
    unsafe {
        assert_eq!(pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK), PA_OK);
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, core::ptr::null()),
            PA_OK,
            "`CP-2`：null vtable 允许注册（创建期不拒绝）"
        );
    }
    assert!(instance.fs_vtable().is_none(), "`CP-2`：null vtable ⇒ 取不到实现");
    assert_eq!(
        instance.fs_open(b"x", 0, 0),
        Err(CapabilityCallError::NotRegistered),
        "`CP-2`：null vtable 调用时报未实现"
    );

    // `CP-3`：只接 `write`，其余槽位照旧"未实现"——同域其余槽位不受影响。
    let table = write_only_vtable();
    // SAFETY: `table` 活到测试末尾（比 state 长）；地址在 state 存活期内有效。
    unsafe { register_fs(state, &table) };
    STDOUT_SINK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    assert_eq!(instance.fs_write(1, b"ok\n"), Ok(3), "`CP-3`：接了 `write` 就该成功");
    assert_eq!(
        instance.fs_open(b"x", 0, 0),
        Err(CapabilityCallError::NotImplemented),
        "`CP-3`：没接的槽位只有它自己报未实现"
    );
    assert_eq!(
        instance.fs_read(1, &mut [0u8; 4]),
        Err(CapabilityCallError::NotImplemented),
        "`CP-3`：槽位之间互不牵连"
    );
    assert_eq!(take_stdout(), "ok\n", "`write` 槽真的被调到（`CP-5` 的成功态）");

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// **`T-CP-2`**：宿主已实现但**拒绝** ⇒ 真实机器错误（`CP-6`），且与 `T-CP-1` 的"未实现"
/// **可区分**（`CP-5`：两条通道不许混）。
#[test]
fn refusal_is_a_machine_error_distinct_from_unimplemented() {
    // 机器错误：`open` 拒绝（EACCES）、`stat` 报 ENOENT（同一个域里两种真实 errno）。
    let refused = CpFsVtable {
        open: Some(refuse_open),
        stat: Some(refuse_stat),
        write: Some(sink_write),
        ..valid_fs_table()
    };
    let state = new_state();
    // SAFETY: `refused` 活到测试末尾；地址在 state 存活期内有效。
    unsafe { register_fs(state, &refused) };
    // SAFETY: 只借用。
    let instance = unsafe { &*state }.instance();
    assert_eq!(
        instance.fs_open(b"denied", 0, 0),
        Err(CapabilityCallError::Machine(EACCES)),
        "`CP-6`：拒绝必须是真实机器错误（errno 原样带回）"
    );
    assert_eq!(
        instance.fs_stat(b"gone"),
        Err(CapabilityCallError::Machine(ENOENT)),
        "`CP-6`：机器错误按真实 errno 分类，不是「权限不足」这一类别之一"
    );
    // 三条通道**两两可区分**（`CP-5`：禁止把"未实现"编码成 errno／共用一条通道）。
    let machine = instance.fs_open(b"denied", 0, 0).unwrap_err();
    let unimplemented = instance.fs_read(1, &mut [0u8; 1]).unwrap_err();
    let unregistered = pyawa_core::Instance::new().fs_open(b"x", 0, 0).unwrap_err();
    assert_ne!(machine, unimplemented, "机器错误 ≠ 未实现");
    assert_ne!(machine, unregistered, "机器错误 ≠ 未注册");
    assert_ne!(unimplemented, unregistered, "未实现 ≠ 未注册（`CP-2`／`CP-3` 两种形态）");

    // **Python 层**需要组合根装配（`CM-14`）：`open`／`print` 这些内建名由 stdlib 装上
    //（本测试不 import 模块 ⇒ 不需要 `sys.path`）。
    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };

    // **Python 层**：同一段 `open(...)`，拒绝 ⇒ `PermissionError`；未实现 ⇒ 另一种呈现
    // （当前是 `OSError` ＋ "未实现" 文案；`CP-34` 的独立异常等 `SPEC-c-modules.md`，不在此伪造）。
    let refusal = run_script(state, "open(\"denied\")\n");
    assert_eq!(refusal.exit, 1, "未捕获异常应当非零");
    let (kind, message) = refusal.exception.expect("拒绝应当抛未捕获异常");
    assert_eq!(kind, "PermissionError", "`CP-6`：EACCES ⇒ PermissionError，实际 {kind}");
    assert!(message.contains("[Errno 13]"), "消息要带机器错误（errno）：{message}");

    // 另一个实例：域已注册但槽位未实现。
    let unimplemented_table = valid_fs_table();
    let other = new_state();
    // SAFETY: 同上。
    unsafe {
        register_fs(other, &unimplemented_table);
        pyawa_stdlib::install((&*other).instance(), "probe", &[]);
    }
    let missing = run_script(other, "open(\"whatever\")\n");
    assert_eq!(missing.exit, 1);
    let (other_kind, other_message) = missing.exception.expect("未实现应当抛未捕获异常");
    assert_ne!(other_kind, kind, "`T-CP-2`：两种失败在 Python 层必须可区分");
    assert!(
        other_message.contains("未实现"),
        "未实现要走独立通道（不是 errno）：{other_message}"
    );

    // SAFETY: 两个 state 都由 `new_state` 交回、尚未销毁。
    unsafe {
        assert_eq!(pa_destroy(state), PA_OK);
        assert_eq!(pa_destroy(other), PA_OK);
    }
}

/// **`T-CP-5`**（映射的一半）：provider 没有 `/proc`／`/dev`／`/sys` 时，访问**必须**表现为
/// `FileNotFoundError`（`CP-23`），**禁止**表现为"未实现"。
///
/// 真实受限 provider 的那一半在 `crates/pyawa-runtime/tests/two_providers.rs`（真 `ENOENT`）。
#[test]
fn missing_special_roots_are_file_not_found_not_unimplemented() {
    // provider 对任何名字都报 ENOENT（＝这座"机器"上没有这些路径）。
    let empty_machine = CpFsVtable {
        open: Some(missing_open),
        write: Some(sink_write),
        ..valid_fs_table()
    };
    let state = new_state();
    // SAFETY: `empty_machine` 活到测试末尾；`state` 由 `new_state` 交回、尚未销毁。
    unsafe {
        register_fs(state, &empty_machine);
        pyawa_stdlib::install((&*state).instance(), "probe", &[]);
    }
    let run = run_script(state, "open(\"/proc/self/status\")\n");
    assert_eq!(run.exit, 1);
    let (kind, message) = run.exception.expect("应当抛未捕获异常");
    assert_eq!(kind, "FileNotFoundError", "`CP-23`：缺 /proc 必须是 FileNotFoundError，实际 {kind}");
    assert!(message.contains("[Errno 2]"), "errno 必须是 ENOENT：{message}");
    assert!(!message.contains("未实现"), "`CP-23`：禁止表现为未实现：{message}");

    // 对照：槽位**未实现**是另一条通道（不是 FileNotFoundError）——这是"两种失败之分"的判据。
    let unimplemented = valid_fs_table();
    let other = new_state();
    // SAFETY: 同上。
    unsafe {
        register_fs(other, &unimplemented);
        pyawa_stdlib::install((&*other).instance(), "probe", &[]);
    }
    let run = run_script(other, "open(\"/proc/self/status\")\n");
    let (other_kind, other_message) = run.exception.expect("应当抛未捕获异常");
    assert_ne!(
        other_kind, "FileNotFoundError",
        "`CP-5`／`CP-23`：槽位未实现不得伪装成 FileNotFoundError"
    );
    assert!(other_message.contains("未实现"), "未实现要有自己的通道：{other_message}");

    // SAFETY: 两个 state 都由 `new_state` 交回、尚未销毁。
    unsafe {
        assert_eq!(pa_destroy(state), PA_OK);
        assert_eq!(pa_destroy(other), PA_OK);
    }
}

// --------------------------------------------------------------------------- #
// T-CP-7（互不可见的一半）／T-CP-8／T-CP-9／T-CP-12（通道的一半）
// --------------------------------------------------------------------------- #

static COUNTER_A: Mutex<u64> = Mutex::new(0);
static COUNTER_B: Mutex<u64> = Mutex::new(0);

extern "C" fn write_counter_a(
    _state: *mut c_void,
    _handle: Handle,
    _bytes: *const u8,
    len: usize,
    out_len: *mut usize,
    _errno_out: *mut i32,
) -> CapStatus {
    *COUNTER_A.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
    if !out_len.is_null() {
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = len };
    }
    CapStatus::Ok
}

extern "C" fn write_counter_b(
    _state: *mut c_void,
    _handle: Handle,
    _bytes: *const u8,
    len: usize,
    out_len: *mut usize,
    _errno_out: *mut i32,
) -> CapStatus {
    *COUNTER_B.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
    if !out_len.is_null() {
        // SAFETY: 出参由调用方提供。
        unsafe { *out_len = len };
    }
    CapStatus::Ok
}

fn counter(table: extern "C" fn(*mut c_void, Handle, *const u8, usize, *mut usize, *mut i32) -> CapStatus) -> CpFsVtable {
    CpFsVtable { write: Some(table), ..valid_fs_table() }
}

/// **`T-CP-7`**（前半）：两个实例的**能力表互不可见**（`CP-13`）；销毁其中一个不影响另一个。
///
/// 后半（销毁后无残留句柄，`CP-19`）由 round 11 的 `CP-31` 统一能力表收口：
/// - ABI 面：本文件 [`instance_destroy_closes_every_provider_handle_exactly_once`]；
/// - 真 provider：`crates/pyawa-runtime/tests/two_providers.rs` 的
///   `instance_destroy_releases_every_provider_handle`。
#[test]
fn instance_capability_tables_are_mutually_invisible() {
    let table_a = counter(write_counter_a);
    let table_b = counter(write_counter_b);
    let first = new_state();
    let second = new_state();
    // SAFETY: 两张表都活到测试末尾；地址在两个 state 存活期内有效。
    unsafe {
        register_fs(first, &table_a);
        register_fs(second, &table_b);
    }
    *COUNTER_A.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = 0;
    *COUNTER_B.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = 0;

    // SAFETY: 只借用；两个 state 都还没销毁。
    let first_instance = unsafe { &*first }.instance();
    let second_instance = unsafe { &*second }.instance();
    assert_eq!(first_instance.fs_write(1, b"a"), Ok(1));
    assert_eq!(*COUNTER_A.lock().unwrap(), 1, "第一个实例只该看见自己的 vtable");
    assert_eq!(*COUNTER_B.lock().unwrap(), 0, "`CP-13`：实例间能力表互不可见");

    assert_eq!(second_instance.fs_write(1, b"b"), Ok(1));
    assert_eq!(*COUNTER_A.lock().unwrap(), 1);
    assert_eq!(*COUNTER_B.lock().unwrap(), 1);

    // 销毁第二个实例：第一个实例的能力表不受影响（`CP-13`：禁止进程级能力表）。
    // SAFETY: `second` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(second) }, PA_OK);
    assert_eq!(first_instance.fs_write(1, b"c"), Ok(1));
    assert_eq!(*COUNTER_A.lock().unwrap(), 2, "销毁别的实例不该动本实例的能力表");
    assert_eq!(*COUNTER_B.lock().unwrap(), 1, "已销毁实例的 vtable 不该再被调到");

    // 第三个实例：没注册 ⇒ 各自的表独立（`CP-13`）。
    let third = new_state();
    // SAFETY: 只借用。
    assert!(unsafe { &*third }.instance().fs_vtable().is_none());
    // SAFETY: `first`／`third` 由 `new_state` 交回、尚未销毁。
    unsafe {
        assert_eq!(pa_destroy(first), PA_OK);
        assert_eq!(pa_destroy(third), PA_OK);
    }
}

/// **`T-CP-8`**：未声明异步分类的域／槽位注册失败，**不落默认值**（`CP-25`／`AB-34`）。
#[test]
fn async_classification_is_mandatory_without_defaults() {
    let state = new_state();
    let table = write_only_vtable();
    // SAFETY: `state` 由 `new_state` 交回；`table` 活到测试末尾。
    unsafe {
        // 没声明分类 ⇒ 注册实现必须失败（`CP-25`：缺失即注册失败）。
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, (&table as *const CpFsVtable).cast()),
            PA_ERR_INVALID,
            "`CP-25`：缺失异步分类即注册失败"
        );
        // 失败之后**没有落下默认值**：取不到 vtable、调用报未注册。
        let instance = (&*state).instance();
        assert!(instance.fs_vtable().is_none(), "`CP-25`：注册失败不得留下实现");
        assert_eq!(
            instance.fs_write(1, b"x"),
            Err(CapabilityCallError::NotRegistered),
            "`CP-25`：不落默认值 ⇒ 仍是未注册"
        );

        // 分类只有二值（`CP-37`：按域二值，禁止域内混合）。
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_FS, 42),
            PA_ERR_INVALID,
            "分类取值只有 PA_ASYNC_OK／PA_ASYNC_NO"
        );
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_IPC, PA_ASYNC_NO),
            PA_OK,
            "`CP-37`：ipc 取「不可异步化」是合法值"
        );
        assert_eq!(
            pa_setcapability_async(state, pyawa_abi::capability::DOMAIN_COUNT as i32, PA_ASYNC_OK),
            PA_ERR_INVALID,
            "域编号越界要拒绝"
        );
        // 声明之后才允许注册（顺序判据：分类先于实现）。
        assert_eq!(pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK), PA_OK);
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, (&table as *const CpFsVtable).cast()),
            PA_OK
        );
        assert_eq!(pa_destroy(state), PA_OK);
    }
}

/// **`T-CP-9`**：可异步化槽位的签名**不含 VM 对象类型**（`CP-26`／`CX-10` 的编译期面）。
///
/// 两重证据（都能在编译期／测试期判定）：
/// 1. **精确类型断言**：每个槽位都能写成"只含裸数据与不透明句柄"的 `extern "C" fn` 指针类型；
///    改动槽位签名（例如塞进 `NonNull<Header>`）会**编译失败**；
/// 2. **形状层源码扫描**：`pyawa-capabilities` 的形状文件里不出现核心 VM 类型名。
#[test]
fn async_slot_signatures_contain_only_raw_data() {
    // ① 精确签名（编译期断言）：`fs` 域是"可异步化"（§9.1 全部），`clock` 域也是。
    type FsOpen =
        Option<extern "C" fn(*mut c_void, *const u8, usize, i32, u32, *mut Handle, *mut i32) -> CapStatus>;
    type FsStat = Option<
        extern "C" fn(
            *mut c_void,
            *const u8,
            usize,
            *mut pyawa_capabilities::fs::FileInfo,
            *mut i32,
        ) -> CapStatus,
    >;
    type FsFstat = Option<
        extern "C" fn(
            *mut c_void,
            Handle,
            *mut pyawa_capabilities::fs::FileInfo,
            *mut i32,
        ) -> CapStatus,
    >;
    type FsListdir =
        Option<extern "C" fn(*mut c_void, Handle, *mut u8, usize, *mut usize, *mut i32) -> CapStatus>;
    type FsName = Option<extern "C" fn(*mut c_void, *const u8, usize, *mut i32) -> CapStatus>;
    type FsNameMode = Option<extern "C" fn(*mut c_void, *const u8, usize, u32, *mut i32) -> CapStatus>;
    type FsRename = Option<
        extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, *mut i32) -> CapStatus,
    >;
    type FsReadlink = Option<
        extern "C" fn(*mut c_void, *const u8, usize, *mut u8, usize, *mut usize, *mut i32) -> CapStatus,
    >;
    type FsSymlink = Option<
        extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, *mut i32) -> CapStatus,
    >;
    type FsUtime =
        Option<extern "C" fn(*mut c_void, *const u8, usize, i64, i64, *mut i32) -> CapStatus>;
    type FsRead =
        Option<extern "C" fn(*mut c_void, Handle, *mut u8, usize, usize, *mut usize, *mut i32) -> CapStatus>;
    type FsWrite =
        Option<extern "C" fn(*mut c_void, Handle, *const u8, usize, *mut usize, *mut i32) -> CapStatus>;
    type FsSeek = Option<extern "C" fn(*mut c_void, Handle, i64, i32, *mut u64, *mut i32) -> CapStatus>;
    type FsTell = Option<extern "C" fn(*mut c_void, Handle, *mut u64, *mut i32) -> CapStatus>;
    type FsTruncate = Option<extern "C" fn(*mut c_void, Handle, u64, *mut i32) -> CapStatus>;
    type FsFsync = Option<extern "C" fn(*mut c_void, Handle, *mut i32) -> CapStatus>;
    type FsClose = Option<extern "C" fn(*mut c_void, Handle, *mut i32) -> CapStatus>;

    let table = CpFsVtable::UNIMPLEMENTED;
    let _: FsOpen = table.open;
    let _: FsStat = table.stat;
    let _: FsFstat = table.fstat;
    let _: FsListdir = table.listdir;
    let _: FsNameMode = table.mkdir;
    let _: FsName = table.rmdir;
    let _: FsName = table.unlink;
    let _: FsRename = table.rename;
    let _: FsReadlink = table.readlink;
    let _: FsSymlink = table.symlink;
    let _: FsNameMode = table.chmod;
    let _: FsUtime = table.utime;
    let _: FsRead = table.read;
    let _: FsWrite = table.write;
    let _: FsSeek = table.seek;
    let _: FsTell = table.tell;
    let _: FsTruncate = table.truncate;
    let _: FsFsync = table.fsync;
    let _: FsClose = table.close;

    // `clock` 域（同样"可异步化"）：三个槽位都是裸数据。
    type ClockNow = Option<extern "C" fn(*mut c_void, *mut i64) -> CapStatus>;
    type ClockSleep = Option<extern "C" fn(*mut c_void, i64, *mut i32) -> CapStatus>;
    let clock = pyawa_capabilities::clock::CpClockVtable::UNIMPLEMENTED;
    let _: ClockNow = clock.now_ns;
    let _: ClockNow = clock.monotonic_ns;
    let _: ClockSleep = clock.sleep_ns;

    // ② 形状层源码扫描：能力接口 crate 里不得出现核心 VM 类型（`CP-12`：只含类型与函数指针）。
    let fs_shape = include_str!("../../pyawa-capabilities/src/fs.rs");
    let clock_shape = include_str!("../../pyawa-capabilities/src/clock.rs");
    for (file, source) in [("fs.rs", fs_shape), ("clock.rs", clock_shape)] {
        for forbidden in ["pyawa_core", "Header", "NonNull", "Instance", "Value"] {
            assert!(
                !source.contains(forbidden),
                "`CP-26`／`CP-12`：{file} 的形状里出现了 VM 类型 {forbidden}"
            );
        }
    }
}

/// **`T-CP-12`**（通道的一半）：`close` 幂等；对已关闭句柄的其它操作返回机器错误（`CP-17`／`CP-18`）。
///
/// 真 provider（`PosixFs`）的那一半在 `crates/pyawa-runtime/tests/two_providers.rs`。
#[test]
fn close_is_idempotent_and_operations_after_close_fail_with_machine_errors() {
    static HANDLE_TABLE: Mutex<Vec<u64>> = Mutex::new(Vec::new());
    // provider 句柄从 300 起（**故意**与表 fd 3 不同）⇒ 证明适配层真的做了 fd→provider 句柄的换算。
    static NEXT_HANDLE: Mutex<u64> = Mutex::new(300);

    extern "C" fn open(
        _state: *mut c_void,
        _name: *const u8,
        _len: usize,
        _flags: i32,
        _mode: u32,
        out: *mut Handle,
        _errno_out: *mut i32,
    ) -> CapStatus {
        let mut next = NEXT_HANDLE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = *next;
        *next += 1;
        HANDLE_TABLE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(id);
        if !out.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = Handle(id) };
        }
        CapStatus::Ok
    }

    extern "C" fn read(
        _state: *mut c_void,
        handle: Handle,
        _out: *mut u8,
        _capacity: usize,
        _wanted: usize,
        out_len: *mut usize,
        errno_out: *mut i32,
    ) -> CapStatus {
        let table = HANDLE_TABLE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !table.contains(&handle.0) {
            // SAFETY: 出参由调用方提供。
            unsafe { *errno_out = EBADF };
            return CapStatus::Machine;
        }
        if !out_len.is_null() {
            // SAFETY: 同上。
            unsafe { *out_len = 0 };
        }
        CapStatus::Ok
    }

    extern "C" fn close(
        _state: *mut c_void,
        handle: Handle,
        _errno_out: *mut i32,
    ) -> CapStatus {
        HANDLE_TABLE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|id| *id != handle.0);
        // `CP-17`：幂等 —— 关没关过都成功。
        CapStatus::Ok
    }

    let table = CpFsVtable {
        open: Some(open),
        read: Some(read),
        close: Some(close),
        ..valid_fs_table()
    };
    let state = new_state();
    // SAFETY: `table` 活到测试末尾；地址在 state 存活期内有效。
    unsafe { register_fs(state, &table) };
    // SAFETY: 只借用。
    let instance = unsafe { &*state }.instance();

    let handle = instance.fs_open(b"f", 0, 0).expect("open 应当成功");
    assert_eq!(instance.fs_read(handle, &mut [0u8; 4]), Ok(0));
    assert_eq!(instance.fs_close(handle), Ok(()), "第一次 close 成功");
    assert_eq!(instance.fs_close(handle), Ok(()), "`CP-17`：close 必须幂等");
    assert_eq!(
        instance.fs_read(handle, &mut [0u8; 4]),
        Err(CapabilityCallError::Machine(EBADF)),
        "`CP-18`：关闭后再操作必须是 EBADF 形状的机器错误"
    );

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// --------------------------------------------------------------------------- #
// T-CP-7 后半／CP-19＋CP-31：销毁兜底释放、dup 别名计数、select 分流
// --------------------------------------------------------------------------- #

/// 宿主 errno 表的唯一真相（测试里不写第二个数字）。
fn host_errno(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or_else(|| panic!("宿主 errno 表里没有 {name}"))
}

/// **`T-CP-7` 后半（`CP-19`／`CP-31`）**：`pa_destroy` 必须把实例能力表里的句柄**逐个**
/// 交回 provider 的 `close` 槽（不依赖回收器先跑完），且**每个 provider 句柄只关一次**。
#[test]
fn instance_destroy_closes_every_provider_handle_exactly_once() {
    static CLOSED: Mutex<Vec<u64>> = Mutex::new(Vec::new());
    static NEXT: Mutex<u64> = Mutex::new(100);

    extern "C" fn open(
        _state: *mut c_void,
        _name: *const u8,
        _len: usize,
        _flags: i32,
        _mode: u32,
        out: *mut Handle,
        _errno_out: *mut i32,
    ) -> CapStatus {
        let mut next = NEXT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = *next;
        *next += 1;
        if !out.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = Handle(id) };
        }
        CapStatus::Ok
    }
    extern "C" fn close(
        _state: *mut c_void,
        handle: Handle,
        _errno_out: *mut i32,
    ) -> CapStatus {
        CLOSED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(handle.0);
        CapStatus::Ok
    }

    let table = CpFsVtable {
        open: Some(open),
        close: Some(close),
        ..valid_fs_table()
    };
    let state = new_state();
    // SAFETY: `table` 活到 state 销毁之后。
    unsafe { register_fs(state, &table) };
    // SAFETY: 只借用；`state` 尚未销毁。
    let instance = unsafe { &*state }.instance();
    // 同一 provider 句柄开两次 = 两个表项；dup 再加一个别名。
    let first = instance.fs_open(b"a", 0, 0).expect("open a");
    let second = instance.fs_open(b"b", 0, 0).expect("open b");
    let alias = instance.capability_dup(first as i64).expect("dup");
    assert_eq!(instance.capability_fd_count(), 3, "`CP-31`：三个表项");
    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
    let closed = CLOSED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    assert_eq!(
        closed,
        vec![100, 101],
        "`CP-19`：销毁时按 provider 句柄逐个交回 close；两个表项 = 两次 open 的句柄 ⇒ 各关一次\
         （dup 出来的别名不重复关）；实际 {closed:?}，second={second} alias={alias}"
    );
}

/// **`CP-31` 的 `dup`／`dup2` 别名计数**（自造 vtable 上验 close 槽的调用次数）。
#[test]
fn dup_aliases_close_the_provider_handle_only_when_the_count_hits_zero() {
    static CLOSED: Mutex<Vec<u64>> = Mutex::new(Vec::new());

    extern "C" fn open(
        _state: *mut c_void,
        name: *const u8,
        len: usize,
        _flags: i32,
        _mode: u32,
        out: *mut Handle,
        errno_out: *mut i32,
    ) -> CapStatus {
        // SAFETY: 契约同 `SPEC-capabilities.md` §9.1。
        let bytes = unsafe { core::slice::from_raw_parts(name, len) };
        // **`CP-21`／`CP-32`**：`fs` 名字类方法先经 VM 解析器 ⇒ provider 收到的是 **guest 绝对名**
        // （如 `/…/one`）⇒ 按**最后一段**匹配（测试自己的句柄映射，不是"绝对路径再解释"）。
        let last = bytes.rsplit(|byte| *byte == b'/').next().unwrap_or(bytes);
        let id = match last {
            b"one" => 500,
            b"two" => 501,
            _ => {
                // SAFETY: 出参由调用方提供。
                unsafe { *errno_out = host_errno("ENOENT") };
                return CapStatus::Machine;
            }
        };
        if !out.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = Handle(id) };
        }
        CapStatus::Ok
    }
    extern "C" fn close(
        _state: *mut c_void,
        handle: Handle,
        _errno_out: *mut i32,
    ) -> CapStatus {
        CLOSED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(handle.0);
        CapStatus::Ok
    }

    extern "C" fn read(
        _state: *mut c_void,
        _handle: Handle,
        _out: *mut u8,
        _capacity: usize,
        _wanted: usize,
        out_len: *mut usize,
        _errno_out: *mut i32,
    ) -> CapStatus {
        if !out_len.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = 0 };
        }
        CapStatus::Ok
    }

    let table = CpFsVtable {
        open: Some(open),
        read: Some(read),
        close: Some(close),
        ..valid_fs_table()
    };
    let state = new_state();
    // SAFETY: `table` 活到 state 销毁之后。
    unsafe { register_fs(state, &table) };
    // SAFETY: 只借用；`state` 尚未销毁。
    let instance = unsafe { &*state }.instance();

    // `dup`：两个 fd 一个 provider 句柄。
    let fd = instance.fs_open(b"one", 0, 0).expect("open one");
    let alias = u64::try_from(instance.capability_dup(fd as i64).expect("dup")).expect("fd");
    assert_ne!(fd, alias);
    assert_eq!(instance.fs_close(fd), Ok(()));
    assert!(
        CLOSED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_empty(),
        "`CP-17`：还有别名 ⇒ provider close 不该被调"
    );
    assert_eq!(
        instance.fs_read(fd, &mut [0u8; 1]),
        Err(CapabilityCallError::Machine(host_errno("EBADF"))),
        "`CP-18`：关掉的 fd ⇒ EBADF"
    );
    assert_eq!(instance.fs_close(alias), Ok(()));
    assert_eq!(
        CLOSED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_slice(),
        &[500u64],
        "最后一个别名关闭 ⇒ provider close 恰好一次"
    );

    // `dup2`：目标原有表项按 close 语义释放（目标若是最后别名 ⇒ 真关）。
    let one = instance.fs_open(b"one", 0, 0).expect("open one");
    let two = instance.fs_open(b"two", 0, 0).expect("open two");
    assert_eq!(
        instance.capability_dup2(one as i64, two as i64),
        Ok(two as i64),
        "dup2 返回 new"
    );
    assert_eq!(
        CLOSED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_slice(),
        &[500u64, 501],
        "dup2 释放目标旧表项（501 是最后一个别名 ⇒ 真关）"
    );
    assert_eq!(instance.fs_close(one), Ok(()));
    assert_eq!(
        CLOSED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_slice(),
        &[500u64, 501],
        "500 还有 dup2 出来的别名 ⇒ 不重复关"
    );
    assert_eq!(instance.fs_close(two), Ok(()));
    assert_eq!(
        CLOSED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_slice(),
        &[500u64, 501, 500],
        "最后一个别名关闭 500"
    );

    // 非法/已关 fd ⇒ EBADF（CP-18）；close 自身幂等（CP-17）。
    assert_eq!(
        instance.fs_read(9_999, &mut [0u8; 1]),
        Err(CapabilityCallError::Machine(host_errno("EBADF")))
    );
    assert_eq!(instance.fs_close(9_999), Ok(()), "`CP-17`：close 幂等（表里没有也成功）");
    assert_eq!(
        instance.capability_dup(9_999),
        Err(CapabilityCallError::Machine(host_errno("EBADF")))
    );

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// `select` 的**分流**（`T-CP-3`／`CP-31`）：`fs` 域 fd 恒就绪、`net` 域 fd 走 `poll` 槽，
/// 两域混合按此合并；无效 fd ⇒ `EBADF`；`poll` 槽 `None` ⇒ 如实 `NotImplemented`。
#[test]
fn select_routes_fs_fds_always_ready_and_net_fds_through_poll() {
    use pyawa_capabilities::net::{
        CapStatus as NetStatus, CpNetVtable, PollFd, SocketHandle,
    };

    /// **`CP-30`**：net 域测试表同样要带合法公共头（理由见文件顶部 `valid_fs_table`）。
    fn valid_net_table() -> CpNetVtable {
        CpNetVtable {
            version: PA_ABI_VERSION,
            size: core::mem::size_of::<CpNetVtable>() as u32,
            ..CpNetVtable::UNIMPLEMENTED
        }
    }

    static POLL_CALLS: Mutex<Vec<(usize, i64)>> = Mutex::new(Vec::new());

    extern "C" fn net_socketpair(
        _state: *mut c_void,
        _family: i32,
        _kind: i32,
        _protocol: i32,
        a: *mut SocketHandle,
        b: *mut SocketHandle,
        _errno_out: *mut i32,
    ) -> NetStatus {
        if !a.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *a = SocketHandle(700) };
        }
        if !b.is_null() {
            // SAFETY: 同上。
            unsafe { *b = SocketHandle(701) };
        }
        NetStatus::Ok
    }

    /// **回显** `events`：请求关心的就绪事件全置上（便于验证 core 的位映射与分流）。
    extern "C" fn net_poll(
        _state: *mut c_void,
        fds: *const PollFd,
        len: usize,
        timeout_ns: i64,
        out: *mut PollFd,
        capacity: usize,
        out_len: *mut usize,
        _errno_out: *mut i32,
    ) -> NetStatus {
        POLL_CALLS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push((len, timeout_ns));
        if capacity < len || out.is_null() {
            // 两段式出参：容量不够就只报总数（core 会扩缓冲重调）。
            if !out_len.is_null() {
                // SAFETY: 出参由调用方提供。
                unsafe { *out_len = len };
            }
            return NetStatus::Ok;
        }
        // SAFETY: 调用方按 `len` 给出输入、按 `capacity` 给出输出。
        let inputs = unsafe { core::slice::from_raw_parts(fds, len) };
        for (index, item) in inputs.iter().enumerate() {
            // SAFETY: `index < capacity` 由上面的检查保证。
            unsafe {
                *out.add(index) = PollFd {
                    handle: item.handle,
                    events: item.events,
                    revents: item.events,
                };
            }
        }
        if !out_len.is_null() {
            // SAFETY: 同上。
            unsafe { *out_len = len };
        }
        NetStatus::Ok
    }

    extern "C" fn net_close(
        _state: *mut c_void,
        _handle: SocketHandle,
        _errno_out: *mut i32,
    ) -> NetStatus {
        NetStatus::Ok
    }
    extern "C" fn net_send(
        _state: *mut c_void,
        _handle: SocketHandle,
        _bytes: *const u8,
        _len: usize,
        out_sent: *mut usize,
        _errno_out: *mut i32,
    ) -> NetStatus {
        if !out_sent.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_sent = 0 };
        }
        NetStatus::Ok
    }

    // `fs` 域只要 `open`（表 fd 由 core 分配）＋`write`（stdlib 的 stdout 通道）。
    extern "C" fn fs_open_stub(
        _state: *mut c_void,
        _name: *const u8,
        _len: usize,
        _flags: i32,
        _mode: u32,
        out: *mut Handle,
        _errno_out: *mut i32,
    ) -> CapStatus {
        if !out.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out = Handle(42) };
        }
        CapStatus::Ok
    }
    extern "C" fn fs_read_stub(
        _state: *mut c_void,
        _handle: Handle,
        _out: *mut u8,
        _capacity: usize,
        _wanted: usize,
        out_len: *mut usize,
        _errno_out: *mut i32,
    ) -> CapStatus {
        if !out_len.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = 0 };
        }
        CapStatus::Ok
    }
    let fs_table = CpFsVtable {
        open: Some(fs_open_stub),
        read: Some(fs_read_stub),
        write: Some(sink_write),
        ..valid_fs_table()
    };
    let net_table = CpNetVtable {
        socketpair: Some(net_socketpair),
        send: Some(net_send),
        poll: Some(net_poll),
        close: Some(net_close),
        ..valid_net_table()
    };
    let state = new_state();
    // SAFETY: `state` 活到本测试末尾；两张表也比 state 活得久。
    unsafe {
        register_fs(state, &fs_table);
        assert_eq!(
            pa_setcapability_async(state, pyawa_abi::capability::PA_DOMAIN_NET, PA_ASYNC_OK),
            PA_OK
        );
        assert_eq!(
            pa_setcapability(
                state,
                pyawa_abi::capability::PA_DOMAIN_NET,
                (&net_table as *const CpNetVtable).cast(),
            ),
            PA_OK
        );
    }
    // SAFETY: 只借用；`state` 尚未销毁。
    let instance = unsafe { &*state }.instance();
    let fs_fd = instance.fs_open(b"x", 0, 0).expect("open");
    let (socket_a, _socket_b) = instance.net_socketpair(1, 1, 0).expect("socketpair");
    assert_eq!(
        instance.capability_domain(fs_fd as i64),
        Some(pyawa_capabilities::DOMAIN_FS)
    );
    assert_eq!(
        instance.capability_domain(socket_a as i64),
        Some(pyawa_capabilities::DOMAIN_NET)
    );
    assert_ne!(fs_fd, socket_a, "`CP-31`：fs／net 同一个 fd 编号空间");

    // 只有 fs fd ⇒ 恒就绪，且**不**调 net poll。
    POLL_CALLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    let ready = instance.capability_select(&[fs_fd as i64], &[], &[], None).expect("select");
    assert_eq!(ready, (vec![fs_fd as i64], Vec::new(), Vec::new()));
    assert!(
        POLL_CALLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_empty(),
        "`CP-31`：fs 域不进 net poll"
    );

    // 只有 net fd ⇒ 走 poll（events 按列表映射：read→POLLIN、write→POLLOUT）。
    POLL_CALLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    let ready = instance
        .capability_select(&[socket_a as i64], &[], &[], None)
        .expect("select net read");
    assert_eq!(ready.0, vec![socket_a as i64], "net 可读");
    assert_eq!(
        POLL_CALLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        vec![(1, -1)],
        "`timeout_ns=None` ⇒ 无限等待传给 poll（-1）"
    );

    // 混合：fs 就绪 ⇒ poll 的 timeout 必须归 0（有就绪项就不阻塞）。
    POLL_CALLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    let ready = instance
        .capability_select(&[fs_fd as i64, socket_a as i64], &[], &[], Some(999_999_999))
        .expect("select mixed");
    assert_eq!(ready.0.len(), 2, "混合列表：fs 恒就绪＋net 回显就绪");
    assert_eq!(
        POLL_CALLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone(),
        vec![(1, 0)],
        "有 fs 就绪项 ⇒ timeout 归 0；只有 net fd 进 poll"
    );

    // 写就绪：write 列表 → POLLOUT。
    let ready = instance
        .capability_select(&[], &[socket_a as i64], &[], Some(0))
        .expect("select write");
    assert_eq!(ready.1, vec![socket_a as i64]);

    // 无效 fd ⇒ EBADF（CP-18）。
    assert_eq!(
        instance.capability_select(&[9_999], &[], &[], Some(0)),
        Err(CapabilityCallError::Machine(host_errno("EBADF")))
    );
    // 域不符：net 句柄当 fs fd 用 ⇒ EBADF；fs fd 当 net fd 用 ⇒ EBADF。
    assert_eq!(
        instance.fs_read(socket_a, &mut [0u8; 1]),
        Err(CapabilityCallError::Machine(host_errno("EBADF")))
    );
    assert_eq!(
        instance.net_send(fs_fd, b"x"),
        Err(CapabilityCallError::Machine(host_errno("EBADF")))
    );

    // `poll` 槽为 `None` ⇒ 如实 `NotImplemented`（CP-3／CP-5）。
    let net_table_without_poll = CpNetVtable {
        socketpair: Some(net_socketpair),
        ..valid_net_table()
    };
    let other = new_state();
    let other_fs = CpFsVtable {
        open: Some(fs_open_stub),
        write: Some(sink_write),
        ..valid_fs_table()
    };
    // SAFETY: 两张表都活到 `other` 销毁之后。
    unsafe {
        register_fs(other, &other_fs);
        assert_eq!(
            pa_setcapability_async(other, pyawa_abi::capability::PA_DOMAIN_NET, PA_ASYNC_OK),
            PA_OK
        );
        assert_eq!(
            pa_setcapability(
                other,
                pyawa_abi::capability::PA_DOMAIN_NET,
                (&net_table_without_poll as *const CpNetVtable).cast(),
            ),
            PA_OK
        );
    }
    // SAFETY: 只借用；`other` 尚未销毁。
    let other_instance = unsafe { &*other }.instance();
    let (socket_c, _socket_d) = other_instance.net_socketpair(1, 1, 0).expect("socketpair");
    assert_eq!(
        other_instance.capability_select(&[socket_c as i64], &[], &[], Some(0)),
        Err(CapabilityCallError::NotImplemented),
        "`CP-3`：poll 槽未实现 ⇒ NotImplemented（不是假就绪）"
    );

    // SAFETY: 两个 state 都由 `new_state` 交回、尚未销毁。
    unsafe {
        assert_eq!(pa_destroy(state), PA_OK);
        assert_eq!(pa_destroy(other), PA_OK);
    }
}

// --------------------------------------------------------------------------- #
// T-CP-10（`CP-27`／`CP-28`）：可取消等待的 core 语义
// --------------------------------------------------------------------------- #

/// **`T-CP-10`**（VM 侧机制面；e2e 见 `crates/pyawa-runtime/tests/t_cp10_cancel.rs`）。
///
/// 判据（`CP-27`／`CP-28`）：
/// - **`CP-27`** 长等待被切成**有界片**（每片 ≤ 10 ms）⇒ 不会无限阻塞、片间可取消；
/// - **`CP-28`** 三值：已完成（`Ok(true)`）／已取消（`Ok(false)`）；"未开始"＝进入时已中断
///   ⇒ 直接已取消且**不发起** provider 调用；
/// - 取消之后**不再有该调用的后续副作用**（不调 provider、不再睡眠）；
/// - `select` 的空列表＋正 timeout 走**同一条**可取消等待（round 11 前是立即返回）。
#[test]
fn cancellable_sleep_is_sliced_and_cancellable() {
    use pyawa_capabilities::clock::{CapStatus as ClockStatus, CpClockVtable};
    use std::time::{Duration, Instant};

    /// provider 实际收到的片长（ns）。
    static SLICES: Mutex<Vec<i64>> = Mutex::new(Vec::new());
    /// 片长上限（与 `instance/wait.rs` 的 `CANCEL_SLICE_NS` 同值；形状断言用）。
    const MAX_SLICE_NS: i64 = 10_000_000;

    extern "C" fn sleep_slice(
        _state: *mut c_void,
        ns: i64,
        _errno_out: *mut i32,
    ) -> ClockStatus {
        SLICES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(ns);
        if ns > 0 {
            std::thread::sleep(Duration::from_nanos(ns as u64));
        }
        ClockStatus::Ok
    }

    let table = CpClockVtable {
        version: PA_ABI_VERSION,
        size: core::mem::size_of::<CpClockVtable>() as u32,
        state: core::ptr::null_mut(),
        now_ns: None,
        monotonic_ns: None,
        sleep_ns: Some(sleep_slice),
    };
    let instance = pyawa_core::Instance::new();
    assert!(
        instance.set_capability(
            pyawa_capabilities::DOMAIN_CLOCK,
            (&table as *const CpClockVtable).cast(),
            Some(PA_ASYNC_OK),
        ),
        "注册 `clock` 域（可异步化）应当成功"
    );
    let recorded = || {
        SLICES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    };

    // ① 已完成：25 ms ⇒ 至少 3 片、每片 (0, 10ms]、墙钟不少于请求量（CP-27 分片）。
    SLICES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    let started = Instant::now();
    assert_eq!(instance.cancellable_sleep_ns(25_000_000), Ok(true), "睡满 ⇒ 已完成");
    let elapsed = started.elapsed();
    let slices = recorded();
    assert!(slices.len() >= 3, "25 ms 应被切成 ≥3 片：{slices:?}");
    assert!(
        slices.iter().all(|ns| *ns > 0 && *ns <= MAX_SLICE_NS),
        "CP-27：每片必须有界（≤10 ms）且为正：{slices:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(25),
        "CP-27：合计等待不得短于请求量（实测 {elapsed:?}；片长 {slices:?}）"
    );

    // ② 未开始：进入前已中断 ⇒ 已取消，且**不发起任何 provider 调用**（CP-28 的"未开始"）。
    instance.request_interrupt();
    SLICES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    assert_eq!(
        instance.cancellable_sleep_ns(10_000_000_000),
        Ok(false),
        "CP-28：已中断 ⇒ 直接已取消"
    );
    assert!(recorded().is_empty(), "CP-28 未开始：不许调 provider 的 sleep 槽");
    instance.clear_interrupt();

    // ③ 片间取消：另一线程 ~6 ms 后请求中断 ⇒ 快速返回"已取消"，返回后不再发起新片。
    SLICES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    let address = &instance as *const pyawa_core::Instance as usize;
    let interrupter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(6));
        // SAFETY: `instance` 活到本测试末尾（`join` 在它 drop 之前）；`request_interrupt` 只写
        // 原子标志（`AB-5`① 的"执行中从另一线程请求中断"正是契约要求的用法）。
        unsafe { (*(address as *const pyawa_core::Instance)).request_interrupt() };
    });
    let started = Instant::now();
    assert_eq!(
        instance.cancellable_sleep_ns(2_000_000_000),
        Ok(false),
        "CP-28：片间被中断 ⇒ 已取消"
    );
    let elapsed = started.elapsed();
    interrupter.join().expect("中断线程不该 panic");
    assert!(
        elapsed < Duration::from_millis(500),
        "CP-27：取消延迟应在片长量级（实测 {elapsed:?}）"
    );
    let after_cancel = recorded().len();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        recorded().len(),
        after_cancel,
        "CP-28：取消之后不得再发起 provider 调用（无后续副作用）"
    );
    instance.clear_interrupt();

    // ④ `select` 的空列表＋正 timeout 走同一可取消等待：先"已完成"，再"已取消"。
    assert!(
        instance
            .capability_select_cancellable(&[], &[], &[], Some(20_000_000))
            .expect("select 可取消等待应成功")
            .is_some(),
        "空列表＋20 ms ⇒ 睡满后返回空就绪集（round 11 前是立即返回）"
    );
    instance.request_interrupt();
    assert_eq!(
        instance.capability_select_cancellable(&[], &[], &[], Some(10_000_000_000)),
        Ok(None),
        "CP-28：预置中断 ⇒ select 等待直接已取消"
    );
    instance.clear_interrupt();
}
