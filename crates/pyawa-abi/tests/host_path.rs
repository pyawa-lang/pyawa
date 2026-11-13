//! **`AB-68`：宿主设置每实例 `sys.path`**（`pa_setpath`）的验收。
//!
//! 判据（PLAN「钩子族实现需求」AB-68 五条）：
//! ① 宿主设两个目录 ⇒ 脚本能 import 两处模块 ✓；
//! ② `sys.path` 语义与 `IM-*` 完全一致（不得另立一套 ✗）——本文件用"脚本自己改 `sys.path` 后
//!    语义不变"来钉（`sys.path` 是普通 Python 列表）✓；
//! ③ 路径访问仍受 `fs` 能力夹根（越根 ⇒ `ImportError`，不是读到宿主文件）✓；
//! ④ 脚本自己改 `sys.path` 语义不变 ✓；
//! ⑤ 两实例互不影响 ✓。
//!
//! 测试侧自带一个**最小 `fs` provider**（只实现 `open`／`read`／`close`；根＝临时目录，
//! 越根路径 ⇒ `ENOENT`）。这正是 `AB-68` 要划清的那条线：`sys.path` 只决定"在哪找"，
//! **能不能读**由能力面决定。

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_abi::{
    pa_create, pa_destroy, pa_errmsg, pa_exec_string, pa_getglobal, pa_host, pa_setcapability,
    pa_setcapability_async, pa_setpath, pa_settop, pa_state, pa_tointeger, pa_tostring,
    PA_ABI_SIZE, PA_ABI_VERSION,
};
use pyawa_abi::status::{PA_ERR_INVALID, PA_OK};
use pyawa_capabilities::fs::{CapStatus, CpFsVtable, Handle};

// --------------------------------------------------------------------------- #
// 最小 `fs` provider：根目录内的只读文件；越根 ⇒ ENOENT（2）
// --------------------------------------------------------------------------- #

struct Provider {
    root: PathBuf,
    next: AtomicU64,
    files: Mutex<HashMap<u64, (Vec<u8>, usize)>>,
}

impl Provider {
    fn new(root: &Path) -> Box<Self> {
        Box::new(Self {
            root: root.to_path_buf(),
            next: AtomicU64::new(1),
            files: Mutex::new(HashMap::new()),
        })
    }

    fn vtable(self: &Box<Self>) -> CpFsVtable {
        CpFsVtable {
            version: PA_ABI_VERSION,
            size: core::mem::size_of::<CpFsVtable>() as u32,
            state: self.as_ref() as *const Self as *mut c_void,
            open: Some(provider_open),
            read: Some(provider_read),
            close: Some(provider_close),
            ..CpFsVtable::UNIMPLEMENTED
        }
    }
}

extern "C" fn provider_open(
    state: *mut c_void,
    name: *const u8,
    len: usize,
    _flags: i32,
    _mode: u32,
    out: *mut Handle,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: provider 的 state 由注册方保证指向 `Provider` 且活到实例销毁。
    let provider = unsafe { &*(state as *const Provider) };
    // SAFETY: 名字缓冲区由调用方按 `len` 给出。
    let bytes = unsafe { core::slice::from_raw_parts(name, len) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = 2 };
        return CapStatus::Machine;
    };
    let path = Path::new(text);
    // **根夹紧**：虚拟机解析后的名字必须是本 provider 根下的路径（`..` 已被解析，不做二次拼接）。
    if !path.starts_with(&provider.root) || text.contains("..") {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = 2 };
        return CapStatus::Machine;
    }
    let Ok(content) = std::fs::read(path) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = 2 };
        return CapStatus::Machine;
    };
    let handle = provider.next.fetch_add(1, Ordering::Relaxed);
    provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(handle, (content, 0));
    // SAFETY: 出参由调用方提供。
    unsafe { *out = Handle(handle) };
    CapStatus::Ok
}

extern "C" fn provider_read(
    state: *mut c_void,
    handle: Handle,
    out: *mut u8,
    capacity: usize,
    _wanted: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `provider_open`。
    let provider = unsafe { &*(state as *const Provider) };
    let mut files = provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some((content, position)) = files.get_mut(&handle.0) else {
        // SAFETY: 出参由调用方提供。
        unsafe { *errno_out = 9 };
        return CapStatus::Machine;
    };
    let start = (*position).min(content.len());
    let count = capacity.min(content.len() - start);
    if count > 0 && !out.is_null() {
        // SAFETY: 目标缓冲区由调用方按 `capacity` 给出；源与目标不重叠。
        unsafe { core::ptr::copy_nonoverlapping(content[start..].as_ptr(), out, count) };
    }
    *position = start + count;
    // SAFETY: 出参由调用方提供。
    unsafe { *out_len = count };
    CapStatus::Ok
}

extern "C" fn provider_close(
    state: *mut c_void,
    handle: Handle,
    _errno_out: *mut i32,
) -> CapStatus {
    // SAFETY: 同 `provider_open`。
    let provider = unsafe { &*(state as *const Provider) };
    provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle.0);
    CapStatus::Ok
}

// --------------------------------------------------------------------------- #
// 基建
// --------------------------------------------------------------------------- #

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

fn new_state() -> *mut pa_state {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: host 是局部量、出参可写。
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    assert!(!state.is_null());
    state
}

/// # Safety
///
/// `state` 由 [`new_state`] 交回；`table` 的地址在 state 存活期内有效。
unsafe fn attach_fs(state: *mut pa_state, table: *const CpFsVtable) {
    // SAFETY: 调用方契约。
    unsafe {
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK
        );
        assert_eq!(pa_setcapability(state, PA_DOMAIN_FS, table.cast()), PA_OK);
    }
}

fn run(state: *mut pa_state, source: &str) -> i32 {
    let source = std::ffi::CString::new(source).expect("脚本无 NUL");
    let mode = std::ffi::CString::new("python").expect("mode");
    // SAFETY: state 存活；source／mode 是 NUL 结尾局部串。
    unsafe {
        pa_exec_string(
            state,
            source.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    }
}

fn error_message(state: *mut pa_state) -> String {
    // SAFETY: state 存活；借用只在本函数内用。
    let pointer = unsafe { pa_errmsg(state) };
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 返回 NUL 结尾借用（`AB-48`：下一次 API 前有效）。
    unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

fn global_int(state: *mut pa_state, name: &str) -> i64 {
    let name = std::ffi::CString::new(name).expect("名字无 NUL");
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr()), PA_OK);
        let mut value = 0i64;
        assert_eq!(pa_tointeger(state, -1, &mut value), PA_OK);
        assert_eq!(pa_settop(state, 0), PA_OK);
        value
    }
}

fn global_text(state: *mut pa_state, name: &str) -> String {
    let display = name.to_owned();
    let name = std::ffi::CString::new(name).expect("名字无 NUL");
    // SAFETY: state 存活；视图用完立刻拷走。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr()), PA_OK);
        let mut length = 0usize;
        let pointer = pa_tostring(state, -1, &mut length);
        assert!(!pointer.is_null(), "全局 {display} 应是 str");
        let bytes = core::slice::from_raw_parts(pointer.cast::<u8>(), length);
        let text = String::from_utf8_lossy(bytes).into_owned();
        assert_eq!(pa_settop(state, 0), PA_OK);
        text
    }
}

/// 一个测试独占的临时根（结束时删）。
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pyawa-ab68-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("建沙箱根");
        Self { root }
    }

    fn directory(&self, name: &str) -> PathBuf {
        let directory = self.root.join(name);
        std::fs::create_dir_all(&directory).expect("建目录");
        directory
    }

    fn module(&self, directory: &Path, module: &str, body: &str) {
        std::fs::write(directory.join(format!("{module}.py")), body).expect("写模块");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 把 `dirs` 设成实例的 `sys.path`（`AB-68`）。
///
/// # Safety
///
/// `state` 存活。
unsafe fn set_path(state: *mut pa_state, dirs: &[&Path]) {
    let c_strings: Vec<std::ffi::CString> = dirs
        .iter()
        .map(|dir| std::ffi::CString::new(dir.to_string_lossy().as_bytes()).expect("路径无 NUL"))
        .collect();
    let pointers: Vec<*const std::ffi::c_char> = c_strings.iter().map(|text| text.as_ptr()).collect();
    // SAFETY: 指针数组与各 C 串在调用期间存活（`AB-68`：运行时拷贝）。
    let status = unsafe { pa_setpath(state, pointers.as_ptr(), pointers.len()) };
    assert_eq!(status, PA_OK, "`pa_setpath` 失败：{}", error_message(state));
}

// --------------------------------------------------------------------------- #
// ① 两个目录 ⇒ 两处模块都能 import；`sys.path` 语义可观察
// --------------------------------------------------------------------------- #

#[test]
fn two_host_directories_are_importable() {
    let sandbox = Sandbox::new("two-dirs");
    let first = sandbox.directory("first");
    let second = sandbox.directory("second");
    sandbox.module(&first, "mod_a", "VALUE = \"A\"\n");
    sandbox.module(&second, "mod_b", "VALUE = \"B\"\n");
    let provider = Provider::new(&sandbox.root);
    let table = provider.vtable();
    let state = new_state();
    // SAFETY: state 存活；provider／table 在函数体内活到 destroy 之后（drop 顺序：table 先声明）。
    unsafe {
        attach_fs(state, &table);
        set_path(state, &[&first, &second]);
    }
    assert_eq!(
        run(state, "import mod_a\nimport mod_b\njoined = mod_a.VALUE + mod_b.VALUE\n"),
        PA_OK,
        "两个宿主目录应都能 import：{}",
        error_message(state)
    );
    assert_eq!(global_text(state, "joined"), "AB");
    // `sys.path` 的语义＝普通 Python 列表（`IM-*` 同一份；这里数项＋取首项）。
    assert_eq!(
        run(
            state,
            "import sys\npath_len = 0\nfor item in sys.path:\n    path_len = path_len + 1\nfirst_entry = sys.path[0]\n"
        ),
        PA_OK,
        "`sys.path` 应可观察：{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "path_len"), 2);
    assert_eq!(global_text(state, "first_entry"), first.to_string_lossy());
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// ②④ 脚本自己改 `sys.path` ⇒ 语义不变（普通列表，后续 import 走同一套 `IM-*`）
// --------------------------------------------------------------------------- #

#[test]
fn script_can_extend_sys_path_with_the_same_semantics() {
    let sandbox = Sandbox::new("script-extends");
    let first = sandbox.directory("first");
    let second = sandbox.directory("second");
    sandbox.module(&first, "mod_a", "VALUE = \"A\"\n");
    sandbox.module(&second, "mod_b", "VALUE = \"B\"\n");
    let provider = Provider::new(&sandbox.root);
    let table = provider.vtable();
    let state = new_state();
    // SAFETY: 同上一测试。
    unsafe {
        attach_fs(state, &table);
        set_path(state, &[&first]);
    }
    let script = format!(
        "import sys\nsys.path.append({second:?})\nimport mod_b\ngot = mod_b.VALUE\n",
        second = second.to_string_lossy()
    );
    assert_eq!(
        run(state, &script),
        PA_OK,
        "脚本改 `sys.path` 后应能 import：{}",
        error_message(state)
    );
    assert_eq!(global_text(state, "got"), "B");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// ③ 路径仍受 `fs` 能力夹根：越根目录 import ⇒ ImportError（脚本可捕获）
// --------------------------------------------------------------------------- #

#[test]
fn import_stays_behind_the_fs_capability_root() {
    let sandbox = Sandbox::new("clamp");
    let inside = sandbox.directory("inside");
    let outside = sandbox.directory("outside");
    sandbox.module(&outside, "secret", "VALUE = \"secret\"\n");
    // provider 的根只有 `inside` ⇒ `outside` 里的模块读不到（`sys.path` 里有它也没用 ✓）。
    let provider = Provider::new(&inside);
    let table = provider.vtable();
    let state = new_state();
    // SAFETY: 同上一测试。
    unsafe {
        attach_fs(state, &table);
        set_path(state, &[&inside, &outside]);
    }
    assert_eq!(
        run(
            state,
            "blocked = 0\ntry:\n    import secret\nexcept:\n    blocked = 1\n"
        ),
        PA_OK,
        "ImportError 应可被脚本捕获：{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "blocked"), 1, "越根目录的模块不得导入");
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}

// --------------------------------------------------------------------------- #
// ⑤ 两实例互不影响（各自的 `sys.path` 与模块表）
// --------------------------------------------------------------------------- #

#[test]
fn two_instances_have_independent_paths() {
    let sandbox = Sandbox::new("instances");
    let first = sandbox.directory("first");
    let second = sandbox.directory("second");
    sandbox.module(&first, "mod_a", "VALUE = \"A\"\n");
    sandbox.module(&second, "mod_b", "VALUE = \"B\"\n");
    let provider = Provider::new(&sandbox.root);
    let table = provider.vtable();
    let state_a = new_state();
    let state_b = new_state();
    // SAFETY: 两个 state 都存活；同一 provider 被两边注册（只读，无共享 VM 对象）。
    unsafe {
        attach_fs(state_a, &table);
        attach_fs(state_b, &table);
        set_path(state_a, &[&first]);
        set_path(state_b, &[&second]);
    }
    assert_eq!(
        run(state_a, "import mod_a\nok_a = mod_a.VALUE\n"),
        PA_OK,
        "实例 A 应能 import 自己的目录：{}",
        error_message(state_a)
    );
    assert_eq!(
        run(state_b, "import mod_b\nok_b = mod_b.VALUE\n"),
        PA_OK,
        "实例 B 应能 import 自己的目录：{}",
        error_message(state_b)
    );
    // 互不可见：A 看不到 B 的目录，B 也看不到 A 的（`AB-64` 的按实例边界 ✓）。
    assert_eq!(
        run(state_a, "visible = 0\ntry:\n    import mod_b\nexcept:\n    visible = 1\n"),
        PA_OK,
        "{}",
        error_message(state_a)
    );
    assert_eq!(global_int(state_a, "visible"), 1, "实例 A 不该看到实例 B 的目录");
    assert_eq!(
        run(state_b, "visible = 0\ntry:\n    import mod_a\nexcept:\n    visible = 1\n"),
        PA_OK,
        "{}",
        error_message(state_b)
    );
    assert_eq!(global_int(state_b, "visible"), 1, "实例 B 不该看到实例 A 的目录");
    // SAFETY: 两个 state 都由 pa_create 交回、尚未销毁。
    unsafe {
        pa_destroy(state_a);
        pa_destroy(state_b);
    }
}

// --------------------------------------------------------------------------- #
// 参数边界：NULL／n==0 清空／NULL state／NULL 数组元素
// --------------------------------------------------------------------------- #

#[test]
fn path_argument_validation_and_clearing() {
    // SAFETY: 显式 NULL 是契约允许的用法错误（不得崩）。
    assert_eq!(unsafe { pa_setpath(core::ptr::null_mut(), core::ptr::null(), 0) }, PA_ERR_INVALID);
    let sandbox = Sandbox::new("clear");
    let first = sandbox.directory("first");
    sandbox.module(&first, "mod_a", "VALUE = \"A\"\n");
    let provider = Provider::new(&sandbox.root);
    let table = provider.vtable();
    let state = new_state();
    // SAFETY: state 存活。
    unsafe {
        attach_fs(state, &table);
        set_path(state, &[&first]);
        // `dirs == NULL, n == 0` ⇒ 清空（`AB-68`）。
        assert_eq!(pa_setpath(state, core::ptr::null(), 0), PA_OK);
    }
    // 清空后找不到模块（`sys.path` 空表 ⇒ `ImportError` 可捕获）。
    assert_eq!(
        run(state, "blocked = 0\ntry:\n    import mod_a\nexcept:\n    blocked = 1\n"),
        PA_OK,
        "{}",
        error_message(state)
    );
    assert_eq!(global_int(state, "blocked"), 1, "清空 `sys.path` 后不得再 import");
    // `dirs == NULL` 但 `n > 0` ⇒ 用法错误。
    // SAFETY: state 存活；数组指针显式 NULL。
    assert_eq!(unsafe { pa_setpath(state, core::ptr::null(), 1) }, PA_ERR_INVALID);
    // 数组里某一项是 NULL ⇒ 用法错误（不解引用野指针）。
    let null_dirs: [*const std::ffi::c_char; 1] = [core::ptr::null()];
    // SAFETY: 指针数组本身可读，元素显式 NULL。
    assert_eq!(unsafe { pa_setpath(state, null_dirs.as_ptr(), 1) }, PA_ERR_INVALID);
    // SAFETY: state 由 pa_create 交回、尚未销毁。
    unsafe { pa_destroy(state) };
}
