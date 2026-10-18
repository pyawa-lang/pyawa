//! **`CP-21`／`CP-32` 的 VM 侧路径解析**：`NaiveFs` 反证（安全**完全**来自 VM）。
//!
//! # 强验收形态
//!
//! [`NaiveFs`] 是刻意**完全不做**越界检查的测试 provider：名字去掉前导 `/` 后**直接**
//! `root.join(name)`（不归一 `..`、不解析符号链接、不核归属）。如果 `../outside.txt` 或指向根外的
//! 符号链接还能读到根外文件，就说明夹根判定漏到了 provider 外面（`CP-7`／`CP-21`／`CP-32` 不成立）。
//!
//! 覆盖：`.`／`..`／多段／绝对路径（chroot）／符号链接（叶子＋中间段＋绝对目标）／环（`ELOOP`）／
//! 超长词法路径／`cwd`（`CP-22` 的实例状态）／叶子语义（`readlink` 不跟随最后一段）。
//!
//! 另一个观察通道：`NaiveFs` 记录**每一次**收到的名字与映射后的宿主路径 ⇒ 断言
//! "provider 收到的名字全是归一后的 guest 绝对名（无 `..`）、映射后永远在 root 之内"。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_abi::status::PA_OK;
use pyawa_abi::{
    pa_create, pa_destroy, pa_host, pa_setcapability, pa_setcapability_async, pa_state,
    PA_ABI_SIZE, PA_ABI_VERSION,
};
use pyawa_capabilities::fs::{CapStatus, CpFsVtable, FileInfo, Handle};
use pyawa_core::{CapabilityCallError, Instance};

/// provider 收到的名字与映射后的宿主路径（反证用）。
#[derive(Default)]
struct Recorded {
    names: Vec<Vec<u8>>,
    mapped: Vec<PathBuf>,
}

/// **故意的朴素 provider**：`root ＋ 名字直接 join`，**零越界检查**（见文件头）。
struct NaiveFs {
    root: PathBuf,
    files: Mutex<HashMap<u64, std::fs::File>>,
    next: Mutex<u64>,
    recorded: Mutex<Recorded>,
    /// `false` 时 `readlink` 槽留 `None`（`CP-3`：只有该操作未实现）。
    with_readlink: bool,
}

impl NaiveFs {
    fn new(root: &Path, with_readlink: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            files: Mutex::new(HashMap::new()),
            next: Mutex::new(3),
            recorded: Mutex::new(Recorded::default()),
            with_readlink,
        }
    }

    /// **朴素映射**：去掉前导 `/` 后直接 `root.join(名字)` —— 不归一、不核归属。
    fn map(&self, name: &[u8]) -> PathBuf {
        let relative = name.strip_prefix(b"/").unwrap_or(name);
        let text = std::str::from_utf8(relative).unwrap_or("");
        let mapped = self.root.join(text);
        let mut recorded = self
            .recorded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        recorded.names.push(name.to_vec());
        recorded.mapped.push(mapped.clone());
        mapped
    }

    fn vtable(&self) -> CpFsVtable {
        CpFsVtable {
            version: pyawa_abi::PA_ABI_VERSION,
            size: core::mem::size_of::<CpFsVtable>() as u32,
            state: (self as *const Self).cast_mut().cast::<core::ffi::c_void>(),
            open: Some(naive_open),
            stat: Some(naive_stat),
            readlink: self.with_readlink.then_some(naive_readlink),
            read: Some(naive_read),
            close: Some(naive_close),
            ..CpFsVtable::UNIMPLEMENTED
        }
    }
}

fn errno_out(slot: *mut i32, errno: i32) {
    // SAFETY: 出参由调用方提供（vtable 契约）。
    unsafe { *slot = errno };
}

fn name_of(state: *mut core::ffi::c_void) -> &'static NaiveFs {
    // SAFETY: `state` 是 `vtable()` 交出的 provider 指针，活到实例销毁之后（调用方保证）。
    unsafe { &*(state as *const NaiveFs) }
}

fn text_of(name: *const u8, len: usize) -> Result<Vec<u8>, i32> {
    // SAFETY: 调用方按 vtable 契约给出 `name`／`len`。
    Ok(unsafe { core::slice::from_raw_parts(name, len) }.to_vec())
}

#[allow(unsafe_code)]
extern "C" fn naive_open(
    state: *mut core::ffi::c_void,
    name: *const u8,
    len: usize,
    flags: i32,
    _mode: u32,
    out: *mut Handle,
    errno_out_slot: *mut i32,
) -> CapStatus {
    let provider = name_of(state);
    let name = match text_of(name, len) {
        Ok(name) => name,
        Err(errno) => {
            errno_out(errno_out_slot, errno);
            return CapStatus::Machine;
        }
    };
    let path = provider.map(&name);
    let write = flags & 3 != 0;
    match std::fs::OpenOptions::new().read(true).write(write).open(&path) {
        Ok(file) => {
            let mut next = provider
                .next
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let handle = *next;
            *next += 1;
            provider
                .files
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(handle, file);
            // SAFETY: 出参由调用方提供。
            unsafe { *out = Handle(handle) };
            CapStatus::Ok
        }
        Err(error) => {
            errno_out(errno_out_slot, error.raw_os_error().unwrap_or(5));
            CapStatus::Machine
        }
    }
}

#[allow(unsafe_code)]
extern "C" fn naive_stat(
    state: *mut core::ffi::c_void,
    name: *const u8,
    len: usize,
    out: *mut FileInfo,
    errno_out_slot: *mut i32,
) -> CapStatus {
    let provider = name_of(state);
    let name = match text_of(name, len) {
        Ok(name) => name,
        Err(errno) => {
            errno_out(errno_out_slot, errno);
            return CapStatus::Machine;
        }
    };
    match std::fs::metadata(provider.map(&name)) {
        Ok(meta) => {
            // SAFETY: 出参由调用方提供。
            unsafe {
                *out = FileInfo {
                    size: meta.len(),
                    mode: 0,
                    is_dir: meta.is_dir(),
                    dev: 0,
                    ino: 0,
                }
            };
            CapStatus::Ok
        }
        Err(error) => {
            errno_out(errno_out_slot, error.raw_os_error().unwrap_or(5));
            CapStatus::Machine
        }
    }
}

#[allow(unsafe_code)]
extern "C" fn naive_readlink(
    state: *mut core::ffi::c_void,
    name: *const u8,
    len: usize,
    out: *mut u8,
    capacity: usize,
    out_len: *mut usize,
    errno_out_slot: *mut i32,
) -> CapStatus {
    use std::os::unix::ffi::OsStrExt;
    let provider = name_of(state);
    let name = match text_of(name, len) {
        Ok(name) => name,
        Err(errno) => {
            errno_out(errno_out_slot, errno);
            return CapStatus::Machine;
        }
    };
    match std::fs::read_link(provider.map(&name)) {
        Ok(target) => {
            let bytes = target.as_os_str().as_bytes();
            // 两段式出参：总长给 `out_len`，只写 min(总长, 容量)。
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = bytes.len() };
            let writable = bytes.len().min(capacity);
            // SAFETY: `out` 至少有 `capacity` 字节（vtable 契约）。
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, writable) };
            CapStatus::Ok
        }
        Err(error) => {
            errno_out(errno_out_slot, error.raw_os_error().unwrap_or(5));
            CapStatus::Machine
        }
    }
}

#[allow(unsafe_code)]
extern "C" fn naive_read(
    state: *mut core::ffi::c_void,
    handle: Handle,
    buffer: *mut u8,
    len: usize,
    _capacity: usize,
    out_len: *mut usize,
    errno_out_slot: *mut i32,
) -> CapStatus {
    use std::io::Read;
    let provider = name_of(state);
    let mut files = provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = files.get_mut(&handle.0) else {
        errno_out(errno_out_slot, 9);
        return CapStatus::Machine;
    };
    // SAFETY: 调用方按 vtable 契约给出可写 `buffer`／`len`。
    let slice = unsafe { core::slice::from_raw_parts_mut(buffer, len) };
    match file.read(slice) {
        Ok(read) => {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = read };
            CapStatus::Ok
        }
        Err(error) => {
            errno_out(errno_out_slot, error.raw_os_error().unwrap_or(5));
            CapStatus::Machine
        }
    }
}

#[allow(unsafe_code)]
extern "C" fn naive_close(
    state: *mut core::ffi::c_void,
    handle: Handle,
    _errno_out_slot: *mut i32,
) -> CapStatus {
    let provider = name_of(state);
    provider
        .files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&handle.0);
    CapStatus::Ok
}

// --------------------------------------------------------------------------- #
// 夹具
// --------------------------------------------------------------------------- #

struct Fixture {
    base: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "pyawa-path-resolution-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        std::fs::create_dir_all(root.join("sub")).expect("建根目录");
        std::fs::write(root.join("inside.txt"), "inside\n").expect("写 inside.txt");
        std::fs::write(root.join("sub/inside2.txt"), "sub\n").expect("写 inside2.txt");
        // 根外的机密文件（NaiveFs 零检查 ⇒ 若 VM 漏防就会读到这里）。
        std::fs::write(base.join("outside.txt"), "outside-secret\n").expect("写根外文件");
        std::os::unix::fs::symlink("inside.txt", root.join("link")).expect("建合法链接");
        std::os::unix::fs::symlink("../outside.txt", root.join("escape-link"))
            .expect("建逃逸链接");
        std::os::unix::fs::symlink("/inside.txt", root.join("abs-link")).expect("建绝对目标链接");
        std::os::unix::fs::symlink("sub", root.join("dir-link")).expect("建目录链接");
        std::os::unix::fs::symlink("loop-b", root.join("loop-a")).expect("建环 a");
        std::os::unix::fs::symlink("loop-a", root.join("loop-b")).expect("建环 b");
        Self { base, root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// 建一个只注册了 `fs` 域的实例（`pa_create` 会注入平台 errno 表 ⇒ `ELOOP` 等按平台取）。
fn state_with(table: &CpFsVtable) -> *mut pa_state {
    let host = pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 调用方保证 `table` 活到 state 销毁之后。
    unsafe {
        assert_eq!(pa_create(&host, &mut state), PA_OK, "建实例失败");
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK,
            "声明 fs 域异步分类失败"
        );
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, table as *const CpFsVtable as *const _),
            PA_OK,
            "注册 fs 域实现失败"
        );
    }
    state
}

/// 走 VM 的 `fs` 面读一个文件（`open` → 反复 `read` → `close`）。
fn read_file(instance: &Instance, path: &[u8]) -> Result<String, CapabilityCallError> {
    let fd = instance.fs_open(path, 0, 0)?;
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let mut buffer = [0u8; 64];
        let read = instance.fs_read(fd, &mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    instance.fs_close(fd)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn errno_named(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or_else(|| panic!("宿主 errno 表里没有 {name}"))
}

// --------------------------------------------------------------------------- #
// 强验收：NaiveFs（零检查）下，VM 解析保证读不出根外
// --------------------------------------------------------------------------- #

#[test]
fn naive_provider_cannot_escape_through_dotdot_absolute_paths_or_symlinks() {
    let fixture = Fixture::new("escape");
    let provider = NaiveFs::new(&fixture.root, true);
    let table = provider.vtable();
    // `state_with` 内部已按契约 `unsafe`；这里只拿返回值。
    let state = state_with(&table);
    // SAFETY: 只借用；`state` 尚未销毁。
    let instance = unsafe { &*state }.instance();
    // 受限 provider 的实例 cwd 是 chroot 根 `/`（`CP-22`）。
    instance.set_path_cwd(b"/");

    // ---- 合法读取：相对／多段／`.`／`..` 归一／符号链接（叶子＋中间段＋绝对目标）----
    assert_eq!(read_file(instance, b"inside.txt").unwrap(), "inside\n");
    assert_eq!(read_file(instance, b"sub/inside2.txt").unwrap(), "sub\n");
    assert_eq!(read_file(instance, b"sub/../inside.txt").unwrap(), "inside\n");
    assert_eq!(read_file(instance, b"./inside.txt").unwrap(), "inside\n");
    assert_eq!(read_file(instance, b"link").unwrap(), "inside\n", "合法符号链接必须可读");
    assert_eq!(read_file(instance, b"dir-link/inside2.txt").unwrap(), "sub\n", "中间段链接");
    assert_eq!(read_file(instance, b"abs-link").unwrap(), "inside\n", "绝对目标按 guest 根解释");

    // ---- 逃逸：全部必须在 VM 层就变成"根下不存在"（NaiveFs 完全没做检查）----
    for path in [
        "../outside.txt",
        "sub/../../outside.txt",
        "escape-link",
        "/outside.txt",
        "/etc/passwd",
        "/proc/self/status",
    ] {
        let outcome = read_file(instance, path.as_bytes());
        assert_eq!(
            outcome,
            Err(CapabilityCallError::Machine(errno_named("ENOENT"))),
            "{path}：必须 FileNotFoundError 形状（`CP-23`），不是读到根外"
        );
    }
    // 根外文件本身确实存在（否则"没读到"是空测）。
    assert_eq!(
        std::fs::read_to_string(fixture.base.join("outside.txt")).unwrap(),
        "outside-secret\n"
    );

    // ---- 反证通道：provider 收到的名字全是归一后的 guest 绝对名、映射后永远在 root 内 ----
    let recorded = provider
        .recorded
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(!recorded.names.is_empty(), "provider 应当真的被调用过");
    for name in &recorded.names {
        assert!(name.starts_with(b"/"), "provider 只应收到 guest 绝对名：{name:?}");
        assert!(
            !name.split(|byte| *byte == b'/').any(|part| part == b".."),
            "provider 收到的名字不得含越界 `..`：{name:?}"
        );
    }
    for mapped in &recorded.mapped {
        assert!(
            mapped.starts_with(&fixture.root),
            "provider 映射后的宿主路径必须仍在根内：{mapped:?}"
        );
    }

    // SAFETY: `state` 由 `state_with` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

#[test]
fn symlink_loop_is_an_eloop_shaped_machine_error() {
    let fixture = Fixture::new("loop");
    let provider = NaiveFs::new(&fixture.root, true);
    let table = provider.vtable();
    // `state_with` 内部已按契约 `unsafe`；这里只拿返回值。
    let state = state_with(&table);
    // SAFETY: 只借用。
    let instance = unsafe { &*state }.instance();
    instance.set_path_cwd(b"/");
    assert_eq!(
        read_file(instance, b"loop-a"),
        Err(CapabilityCallError::Machine(errno_named("ELOOP"))),
        "`CP-21`：链接环必须报 `ELOOP` 形状（深度上限 40）"
    );
    // SAFETY: `state` 尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

// --------------------------------------------------------------------------- #
// 解析器口径（纯路径面；`CP-22` cwd ＋ 词法 ＋ 叶子语义）
// --------------------------------------------------------------------------- #

#[test]
fn resolver_normalizes_lexically_and_keeps_cwd_instance_local() {
    let fixture = Fixture::new("cwd");
    let provider = NaiveFs::new(&fixture.root, true);
    let table = provider.vtable();
    // `state_with` 内部已按契约 `unsafe`；这里只拿返回值。
    let state = state_with(&table);
    // SAFETY: 只借用。
    let instance = unsafe { &*state }.instance();

    // `CP-22`：cwd 是实例状态；初值 `None` 时惰性取宿主 cwd。
    instance.set_path_cwd(b"/");
    assert_eq!(instance.path_cwd(), b"/".to_vec());
    assert_eq!(instance.resolve_path(b"a/./b/../c").unwrap(), b"/a/c");
    assert_eq!(instance.resolve_path(b"../../x").unwrap(), b"/x", "根处夹住");
    assert_eq!(instance.resolve_path(b"/").unwrap(), b"/");
    assert_eq!(instance.resolve_path(b"a/b/").unwrap(), b"/a/b");

    instance.set_path_cwd(b"/sub");
    assert_eq!(instance.path_cwd(), b"/sub".to_vec());
    assert_eq!(instance.resolve_path(b"inside2.txt").unwrap(), b"/sub/inside2.txt");
    assert_eq!(instance.resolve_path(b"../inside.txt").unwrap(), b"/inside.txt");
    assert_eq!(instance.resolve_path(b"../../etc/passwd").unwrap(), b"/etc/passwd");

    // 叶子语义：`readlink` 一族**不**跟随最后一段（解析到链接本身）；`open` 一族跟随。
    assert_eq!(instance.resolve_path_parent(b"../link").unwrap(), b"/link");
    assert_eq!(instance.resolve_path(b"../link").unwrap(), b"/inside.txt");
    // 空名字 ⇒ `ENOENT`（不是 cwd）。
    assert_eq!(
        instance.resolve_path(b""),
        Err(CapabilityCallError::Machine(errno_named("ENOENT")))
    );

    // `fs_readlink` 走"不跟随最后一段"的解析面。
    assert_eq!(instance.fs_readlink(b"../link").unwrap(), b"inside.txt".to_vec());

    // SAFETY: `state` 尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// `CP-3`：provider 没有 `readlink` 槽时，只有"符号链接探测"退化（**纯词法**）——
/// `..` 夹根仍然生效；**逃逸链接的探测不到**是如实缺口（要满足 `CP-21` 链接面，provider 必须给槽）。
#[test]
fn provider_without_readlink_still_gets_lexical_confinement() {
    let fixture = Fixture::new("no-readlink");
    let provider = NaiveFs::new(&fixture.root, false);
    let table = provider.vtable();
    assert!(table.readlink.is_none(), "该档刻意不提供 readlink 槽");
    // `state_with` 内部已按契约 `unsafe`；这里只拿返回值。
    let state = state_with(&table);
    // SAFETY: 只借用。
    let instance = unsafe { &*state }.instance();
    instance.set_path_cwd(b"/");
    assert_eq!(
        read_file(instance, b"../outside.txt"),
        Err(CapabilityCallError::Machine(errno_named("ENOENT"))),
        "纯词法夹根仍要挡住 `..` 出根"
    );
    // SAFETY: `state` 尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// **超长**：多段长路径的词法解析不爆栈、不丢段；单段超 `PATH_MAX` 由 provider 报
/// `ENAMETOOLONG`（VM 不替内核编这个错）。
#[test]
fn very_long_paths_are_resolved_lexically_and_leave_name_too_long_to_the_provider() {
    let fixture = Fixture::new("long");
    let provider = NaiveFs::new(&fixture.root, true);
    let table = provider.vtable();
    // `state_with` 内部已按契约 `unsafe`；这里只拿返回值。
    let state = state_with(&table);
    // SAFETY: 只借用。
    let instance = unsafe { &*state }.instance();
    instance.set_path_cwd(b"/");

    // 200 段：解析不丢段（`..` 逐段抵消一半）。
    let mut name = String::new();
    for _ in 0..200 {
        name.push_str("d/");
    }
    name.push_str("leaf");
    let resolved = instance.resolve_path(name.as_bytes()).unwrap();
    assert_eq!(resolved.iter().filter(|byte| **byte == b'/').count(), 201);

    // 单段超长：解析阶段探测（`readlink`）就撞 `ENAMETOOLONG` ⇒ 原样上抛（真实机器错误，
    // `CP-6`：不发明类别）。`open` 一族同样得到这个 errno。
    let long_leaf = "x".repeat(5000);
    assert_eq!(
        instance.resolve_path(long_leaf.as_bytes()),
        Err(CapabilityCallError::Machine(errno_named("ENAMETOOLONG")))
    );
    assert_eq!(
        read_file(instance, long_leaf.as_bytes()),
        Err(CapabilityCallError::Machine(errno_named("ENAMETOOLONG"))),
        "超长名字应报真实机器错误"
    );

    // SAFETY: `state` 尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
