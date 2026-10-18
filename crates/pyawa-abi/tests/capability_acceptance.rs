//! **`T-CP-*` 的可执行验收**（M5 判据①：`SPEC-capabilities.md` §11）。
//!
//! 本文件只验**能力形状层 ＋ ABI 注册 ＋ VM 侧适配层**能独立验的条目：
//! `T-CP-1`／`T-CP-2`／`T-CP-5`（映射的一半）／`T-CP-7`（互不可见的一半）／
//! `T-CP-8`／`T-CP-9`／`T-CP-12`（通道的一半）。
//!
//! **不**在这里验的（有专门的可执行验收，别在本文件重复造第二个真相）：
//! - `T-CP-4`／`T-CP-5`／`T-CP-7`（残留句柄）／`T-CP-11`／`T-CP-12`（真 provider 的一半）
//!   ⇒ `crates/pyawa-runtime/tests/two_providers.rs`（真实 `PosixFs` vs 受限 `RestrictedFs`）；
//! - `T-CP-3`／`T-CP-10`（当前**红**，缺口见 M5 矩阵，不在此伪造绿）；
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
    CpFsVtable { write: Some(sink_write), ..CpFsVtable::UNIMPLEMENTED }
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
        ..CpFsVtable::UNIMPLEMENTED
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
    let unimplemented_table = CpFsVtable::UNIMPLEMENTED;
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
        ..CpFsVtable::UNIMPLEMENTED
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
    let unimplemented = CpFsVtable::UNIMPLEMENTED;
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
    CpFsVtable { write: Some(table), ..CpFsVtable::UNIMPLEMENTED }
}

/// **`T-CP-7`**（前半）：两个实例的**能力表互不可见**（`CP-13`）；销毁其中一个不影响另一个。
///
/// 后半（销毁后无残留句柄，`CP-19`）**当前红**：VM 侧没有 `fd` 能力表（`CP-31` 未落地）⇒
/// 句柄表在 provider 里 ⇒ 实例销毁不会关闭它打开过的句柄。缺口与证据见
/// `crates/pyawa-runtime/tests/two_providers.rs` 的 `instance_destroy_does_not_close_provider_handles`。
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
    static NEXT_HANDLE: Mutex<u64> = Mutex::new(3);

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
        ..CpFsVtable::UNIMPLEMENTED
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
