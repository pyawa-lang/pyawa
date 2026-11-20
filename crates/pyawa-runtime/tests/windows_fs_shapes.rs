//! **Windows `fs` provider 形状测试**（`PLAN §12.20` W3；只在 `windows` 目标编译）。
//!
//! 直接测 provider 的 vtable（不依赖 CLI／ABI／VM 路径解析器）：
//!
//! 1. 建临时目录，把宿主路径转成 guest 名（`\` → `/`、加前导 `/`，形如 `/C:/…/temp`）；
//! 2. `open` 建文件 → `write` → `close` → 重新 `open` 读回（含 `stat`／`fstat`／句柄计数）；
//! 3. `mkdir` → `listdir`（解析 NUL 分隔 blob）→ `rmdir`；
//! 4. `rename` 后旧名 `ENOENT`、新名存在（errno 断言）；
//! 5. 不存在路径 `open`／`stat` ⇒ `Machine` ＋ `ENOENT`；
//! 6. 路径映射用 `open`／`stat` **间接**覆盖（不测私有 `host_path`）：宿主文件真被创建、
//!    反斜杠拼写、去掉盘符的"当前盘根相对"拼写都能命中同一个文件；
//! 7. `symlink`：无特权（`EPERM`／`EACCES`／Win32 `1314`）⇒ **如实跳过**，否则 `readlink` 读回
//!    同一个 guest 目标。
//!
//! errno 期望值一律按**名字**从 `pyawa_abi::platform_errno::HOST_ERRNO` 查（`CM-20`：一处真相，
//! 不手写数字）。
#![cfg(windows)]
// vtable 槽位是**安全**的 `extern "C" fn` 指针 ⇒ 调用本身不需要 `unsafe`；按任务约定仍逐次写
// `unsafe {}` ＋ `SAFETY:` 注释（防将来槽位改成 `unsafe fn` 时漏掉）⇒ 关掉"多余 unsafe"提醒。
#![allow(unused_unsafe)]

use pyawa_capabilities::fs::{open_flag, CapStatus, CpFsVtable, FileInfo, Handle};
use pyawa_runtime::fs_windows::WindowsFs;

/// 按名字取宿主 errno（与 provider 的 `fallback_errno` 同一个真相源）。
fn host_errno(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .expect("宿主 errno 表里应当有这个常量")
}

/// 宿主路径 → guest 名（`\` → `/`、加前导 `/`）：`C:\a\b` ⇒ `/C:/a/b`。
fn guest_name(path: &std::path::Path) -> Vec<u8> {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("/{text}").into_bytes()
}

/// guest 目录名下的一个子项（`/C:/a/b` ＋ `name`）。
fn guest_child(dir: &std::path::Path, name: &str) -> Vec<u8> {
    let mut bytes = guest_name(dir);
    bytes.push(b'/');
    bytes.extend_from_slice(name.as_bytes());
    bytes
}

/// 建一个**唯一**的临时目录（进程号 ＋ 纳秒后缀；`tests` 并发跑也不打架）。
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root =
        std::env::temp_dir().join(format!("pyawa-winfs-{}-{tag}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    root
}

/// 当前盘的根（如 `C:`；与 provider 的 `current_drive_root` 同口径，只用于测试第 6 条）。
fn current_drive_root() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    match cwd.components().next()? {
        std::path::Component::Prefix(prefix) => {
            Some(prefix.as_os_str().to_string_lossy().into_owned())
        }
        _ => None,
    }
}

// --------------------------------------------------------------------------- //
// vtable 调用小工具（unsafe 只出现在这里，逐调用带 SAFETY）
// --------------------------------------------------------------------------- //

fn do_open(vtable: &CpFsVtable, name: &[u8], flags: i32) -> Result<Handle, i32> {
    let mut handle = Handle(0);
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有的 provider 给出（state 有效且活到测试结束）；name 字节切片
    // 活到调用结束；出参是活的局部变量。
    let status = unsafe {
        (vtable.open.expect("open 已接线"))(
            vtable.state,
            name.as_ptr(),
            name.len(),
            flags,
            0o666,
            &mut handle,
            &mut errno,
        )
    };
    match status {
        CapStatus::Ok => Ok(handle),
        CapStatus::Machine => Err(errno),
        other => panic!("open 返回了意外状态 {other:?}"),
    }
}

fn do_read(vtable: &CpFsVtable, handle: Handle, capacity: usize) -> Result<Vec<u8>, i32> {
    let mut buffer = vec![0u8; capacity];
    let mut read = 0usize;
    let mut errno = 0i32;
    // SAFETY: vtable／句柄由本测试给出；buffer 是本函数的活缓冲区（指针与长度一致）；出参是
    // 活的局部变量。
    let status = unsafe {
        (vtable.read.expect("read 已接线"))(
            vtable.state,
            handle,
            buffer.as_mut_ptr(),
            capacity,
            capacity,
            &mut read,
            &mut errno,
        )
    };
    match status {
        CapStatus::Ok => {
            buffer.truncate(read);
            Ok(buffer)
        }
        CapStatus::Machine => Err(errno),
        other => panic!("read 返回了意外状态 {other:?}"),
    }
}

fn do_write(vtable: &CpFsVtable, handle: Handle, bytes: &[u8]) -> Result<usize, i32> {
    let mut written = 0usize;
    let mut errno = 0i32;
    // SAFETY: vtable／句柄由本测试给出；bytes 是活切片；出参是活的局部变量。
    let status = unsafe {
        (vtable.write.expect("write 已接线"))(
            vtable.state,
            handle,
            bytes.as_ptr(),
            bytes.len(),
            &mut written,
            &mut errno,
        )
    };
    match status {
        CapStatus::Ok => Ok(written),
        CapStatus::Machine => Err(errno),
        other => panic!("write 返回了意外状态 {other:?}"),
    }
}

fn do_close(vtable: &CpFsVtable, handle: Handle) {
    let mut errno = 0i32;
    // SAFETY: vtable／句柄由本测试给出；errno 是活的局部变量。
    let status = unsafe { (vtable.close.expect("close 已接线"))(vtable.state, handle, &mut errno) };
    assert_eq!(status, CapStatus::Ok, "close 必须成功（CP-17）");
}

fn do_stat(vtable: &CpFsVtable, name: &[u8]) -> Result<FileInfo, i32> {
    let mut info = FileInfo::default();
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有；name 是活切片；出参是活的局部变量。
    let status = unsafe {
        (vtable.stat.expect("stat 已接线"))(
            vtable.state,
            name.as_ptr(),
            name.len(),
            &mut info,
            &mut errno,
        )
    };
    match status {
        CapStatus::Ok => Ok(info),
        CapStatus::Machine => Err(errno),
        other => panic!("stat 返回了意外状态 {other:?}"),
    }
}

fn do_fstat(vtable: &CpFsVtable, handle: Handle) -> Result<FileInfo, i32> {
    let mut info = FileInfo::default();
    let mut errno = 0i32;
    // SAFETY: vtable／句柄由本测试给出；出参是活的局部变量。
    let status =
        unsafe { (vtable.fstat.expect("fstat 已接线"))(vtable.state, handle, &mut info, &mut errno) };
    match status {
        CapStatus::Ok => Ok(info),
        CapStatus::Machine => Err(errno),
        other => panic!("fstat 返回了意外状态 {other:?}"),
    }
}

fn do_mkdir(vtable: &CpFsVtable, name: &[u8]) {
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有；name 是活切片；errno 是活的局部变量。
    let status = unsafe {
        (vtable.mkdir.expect("mkdir 已接线"))(vtable.state, name.as_ptr(), name.len(), 0o755, &mut errno)
    };
    assert_eq!(status, CapStatus::Ok, "mkdir 应当成功（errno {errno}）");
}

fn do_rmdir(vtable: &CpFsVtable, name: &[u8]) {
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有；name 是活切片；errno 是活的局部变量。
    let status = unsafe {
        (vtable.rmdir.expect("rmdir 已接线"))(vtable.state, name.as_ptr(), name.len(), &mut errno)
    };
    assert_eq!(status, CapStatus::Ok, "rmdir 应当成功（errno {errno}）");
}

fn do_rename(vtable: &CpFsVtable, from: &[u8], to: &[u8]) -> Result<(), i32> {
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有；两个名字都是活切片；errno 是活的局部变量。
    let status = unsafe {
        (vtable.rename.expect("rename 已接线"))(
            vtable.state,
            from.as_ptr(),
            from.len(),
            to.as_ptr(),
            to.len(),
            &mut errno,
        )
    };
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("rename 返回了意外状态 {other:?}"),
    }
}

/// `listdir` 的两段式出参：容量不够就按 `out_len` 回报的总长重来一次。
fn do_listdir(vtable: &CpFsVtable, handle: Handle) -> Result<Vec<Vec<u8>>, i32> {
    let mut capacity = 64usize;
    loop {
        let mut buffer = vec![0u8; capacity];
        let mut needed = 0usize;
        let mut errno = 0i32;
        // SAFETY: vtable／句柄由本测试给出；buffer 是活的、长度与 capacity 一致；两个出参是
        // 活的局部变量。
        let status = unsafe {
            (vtable.listdir.expect("listdir 已接线"))(
                vtable.state,
                handle,
                buffer.as_mut_ptr(),
                capacity,
                &mut needed,
                &mut errno,
            )
        };
        match status {
            CapStatus::Ok => {
                if needed <= capacity {
                    buffer.truncate(needed);
                    return Ok(buffer
                        .split(|byte| *byte == 0)
                        .filter(|name| !name.is_empty())
                        .map(|name| name.to_vec())
                        .collect());
                }
                capacity = needed;
            }
            CapStatus::Machine => return Err(errno),
            other => panic!("listdir 返回了意外状态 {other:?}"),
        }
    }
}

fn do_readlink(vtable: &CpFsVtable, name: &[u8]) -> Result<Vec<u8>, i32> {
    let mut capacity = 64usize;
    let mut errno = 0i32;
    loop {
        let mut buffer = vec![0u8; capacity];
        let mut needed = 0usize;
        // SAFETY: vtable 由本测试持有；name 是活切片；buffer 是活的、长度与 capacity 一致；
        // 两个出参是活的局部变量。
        let status = unsafe {
            (vtable.readlink.expect("readlink 已接线"))(
                vtable.state,
                name.as_ptr(),
                name.len(),
                buffer.as_mut_ptr(),
                capacity,
                &mut needed,
                &mut errno,
            )
        };
        match status {
            CapStatus::Ok => {
                if needed <= capacity {
                    buffer.truncate(needed);
                    return Ok(buffer);
                }
                capacity = needed;
            }
            CapStatus::Machine => return Err(errno),
            other => panic!("readlink 返回了意外状态 {other:?}"),
        }
    }
}

fn do_symlink(vtable: &CpFsVtable, target: &[u8], name: &[u8]) -> Result<(), i32> {
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有；两个名字都是活切片；errno 是活的局部变量。
    let status = unsafe {
        (vtable.symlink.expect("symlink 已接线"))(
            vtable.state,
            target.as_ptr(),
            target.len(),
            name.as_ptr(),
            name.len(),
            &mut errno,
        )
    };
    match status {
        CapStatus::Ok => Ok(()),
        CapStatus::Machine => Err(errno),
        other => panic!("symlink 返回了意外状态 {other:?}"),
    }
}

fn do_unlink(vtable: &CpFsVtable, name: &[u8]) {
    let mut errno = 0i32;
    // SAFETY: vtable 由本测试持有；name 是活切片；errno 是活的局部变量。
    let status = unsafe {
        (vtable.unlink.expect("unlink 已接线"))(vtable.state, name.as_ptr(), name.len(), &mut errno)
    };
    assert_eq!(status, CapStatus::Ok, "unlink 应当成功（errno {errno}）");
}

// --------------------------------------------------------------------------- //
// 测试
// --------------------------------------------------------------------------- //

#[test]
fn vtable_shape_matches_the_capability_contract() {
    let provider = WindowsFs::new();
    let vtable = provider.vtable();
    assert_eq!(vtable.version, pyawa_abi::PA_ABI_VERSION);
    assert_eq!(vtable.size as usize, core::mem::size_of::<CpFsVtable>());
    assert!(!vtable.state.is_null());
    // 除 `pipe` 外全部接线（`CP-3`：只有未实现的那一格是 `None`）。
    assert!(vtable.open.is_some());
    assert!(vtable.stat.is_some());
    assert!(vtable.fstat.is_some());
    assert!(vtable.listdir.is_some());
    assert!(vtable.mkdir.is_some());
    assert!(vtable.rmdir.is_some());
    assert!(vtable.unlink.is_some());
    assert!(vtable.rename.is_some());
    assert!(vtable.readlink.is_some());
    assert!(vtable.symlink.is_some());
    assert!(vtable.chmod.is_some());
    assert!(vtable.utime.is_some());
    assert!(vtable.read.is_some());
    assert!(vtable.write.is_some());
    assert!(vtable.seek.is_some());
    assert!(vtable.tell.is_some());
    assert!(vtable.truncate.is_some());
    assert!(vtable.fsync.is_some());
    assert!(vtable.close.is_some());
    assert!(vtable.stat_times.is_some());
    assert!(vtable.lutime.is_some());
    assert!(vtable.utime_ns.is_some());
    assert!(vtable.pipe.is_none(), "CP-3：Windows 匿名管道尚未接线");
    assert_eq!(provider.open_handle_count(), 0);
}

#[test]
fn write_close_reopen_read_roundtrip() {
    let root = temp_dir("roundtrip");
    let provider = WindowsFs::new();
    let vtable = provider.vtable();
    let name = guest_child(&root, "data.txt");

    // 建 + 写（`WRONLY | CREAT | TRUNC`）。
    let created = do_open(
        &vtable,
        &name,
        open_flag::WRONLY | open_flag::CREAT | open_flag::TRUNC,
    )
    .expect("建文件");
    assert_eq!(do_write(&vtable, created, b"hello windows\n").expect("写文件"), 14);
    assert_eq!(do_fstat(&vtable, created).expect("fstat").size, 14);
    do_close(&vtable, created);
    assert_eq!(provider.open_handle_count(), 0, "close 后句柄表必须清空");

    // 重新打开读回。
    let reopened = do_open(&vtable, &name, open_flag::RDONLY).expect("重开文件");
    assert_eq!(do_read(&vtable, reopened, 64).expect("读文件"), b"hello windows\n");
    do_close(&vtable, reopened);
    assert_eq!(provider.open_handle_count(), 0);

    // `stat`：size／is_dir。
    let info = do_stat(&vtable, &name).expect("stat 文件");
    assert_eq!(info.size, 14);
    assert!(!info.is_dir);
    let root_info = do_stat(&vtable, &guest_name(&root)).expect("stat 目录");
    assert!(root_info.is_dir);

    // 宿主侧真值：provider 确实在映射出的宿主路径上创建了文件。
    assert_eq!(std::fs::read(root.join("data.txt")).expect("宿主读回"), b"hello windows\n");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mkdir_listdir_rmdir() {
    let root = temp_dir("listdir");
    let provider = WindowsFs::new();
    let vtable = provider.vtable();

    let sub = guest_child(&root, "sub");
    do_mkdir(&vtable, &sub);

    // `listdir` 收**目录句柄**（与 `fs_posix` 同口径：先 `open` 目录）。
    let dir_handle = do_open(&vtable, &guest_name(&root), open_flag::RDONLY).expect("打开目录");
    let names = do_listdir(&vtable, dir_handle).expect("列目录");
    assert!(
        names.iter().any(|name| name.as_slice() == b"sub"),
        "listdir 结果里应当有 sub，实际 {names:?}"
    );
    do_close(&vtable, dir_handle);
    assert_eq!(provider.open_handle_count(), 0);

    do_rmdir(&vtable, &sub);
    assert_eq!(
        do_stat(&vtable, &sub).expect_err("删掉的目录不该再存在"),
        host_errno("ENOENT")
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rename_moves_the_name() {
    let root = temp_dir("rename");
    let provider = WindowsFs::new();
    let vtable = provider.vtable();
    let old = guest_child(&root, "old.txt");
    let new = guest_child(&root, "new.txt");

    let handle = do_open(&vtable, &old, open_flag::WRONLY | open_flag::CREAT | open_flag::TRUNC)
        .expect("建文件");
    do_write(&vtable, handle, b"x").expect("写文件");
    do_close(&vtable, handle);

    do_rename(&vtable, &old, &new).expect("rename 应当成功");
    assert_eq!(
        do_stat(&vtable, &old).expect_err("旧名应当已不存在"),
        host_errno("ENOENT"),
        "rename 后旧名必须报 ENOENT"
    );
    assert_eq!(do_stat(&vtable, &new).expect("新名应当存在").size, 1);
    // 宿主侧同样成立（映射确实作用在宿主文件系统上）。
    assert!(!root.join("old.txt").exists());
    assert!(root.join("new.txt").exists());
    // rename 一个不存在的名字 ⇒ ENOENT（机器错误，不是未实现）。
    let missing = guest_child(&root, "missing.txt");
    assert_eq!(
        do_rename(&vtable, &missing, &new).expect_err("不存在的源应当报错"),
        host_errno("ENOENT")
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_names_are_machine_errors_with_enoent() {
    let root = temp_dir("missing");
    let provider = WindowsFs::new();
    let vtable = provider.vtable();
    let missing = guest_child(&root, "does-not-exist.txt");

    // `open`：`Machine` ＋ `ENOENT`（`CP-5`：两条通道，未实现**不**走 errno）。
    assert_eq!(
        do_open(&vtable, &missing, open_flag::RDONLY).expect_err("不存在的路径应当报错"),
        host_errno("ENOENT")
    );
    assert_eq!(
        do_stat(&vtable, &missing).expect_err("不存在的路径 stat 应当报错"),
        host_errno("ENOENT")
    );
    assert_eq!(provider.open_handle_count(), 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn guest_names_map_to_host_paths() {
    let root = temp_dir("mapping");
    let provider = WindowsFs::new();
    let vtable = provider.vtable();
    let host_file = root.join("mapped.txt");

    // 1. 盘符形态的 guest 名（`/C:/…`）⇒ 宿主文件真的被创建。
    let guest = guest_name(&host_file);
    let handle = do_open(&vtable, &guest, open_flag::WRONLY | open_flag::CREAT | open_flag::TRUNC)
        .expect("按 guest 名建文件");
    do_write(&vtable, handle, b"mapped").expect("写文件");
    do_close(&vtable, handle);
    assert_eq!(std::fs::read(&host_file).expect("宿主读回"), b"mapped");

    // 2. 反斜杠拼写（无前导 `/`）⇒ 同一个文件（映射把 `\` 等价归一）。
    let backslash = host_file.to_string_lossy().replace('/', "\\").into_bytes();
    assert_eq!(do_stat(&vtable, &backslash).expect("反斜杠拼写应当命中").size, 6);

    // 3. 去掉盘符的"当前盘根相对"拼写（映射走 `当前盘根 ＋ \ ＋ rest` 那一支）。
    if let Some(root_prefix) = current_drive_root() {
        let host_text = host_file.to_string_lossy().replace('\\', "/");
        if host_text.len() > root_prefix.len()
            && host_text.is_char_boundary(root_prefix.len())
            && host_text[..root_prefix.len()].eq_ignore_ascii_case(&root_prefix)
        {
            let rest = host_text[root_prefix.len()..].trim_start_matches('/');
            let relative_guest = format!("/{rest}").into_bytes();
            assert_eq!(
                do_stat(&vtable, &relative_guest).expect("当前盘根相对拼写应当命中").size,
                6
            );
        }
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn symlink_and_readlink_when_privileged() {
    let root = temp_dir("symlink");
    let provider = WindowsFs::new();
    let vtable = provider.vtable();
    let target_host = root.join("target.txt");
    std::fs::write(&target_host, b"payload").expect("写链接目标");
    let guest_target = guest_name(&target_host);
    let guest_link = guest_child(&root, "link.txt");

    match do_symlink(&vtable, &guest_target, &guest_link) {
        Ok(()) => {}
        Err(errno) => {
            // CI 无特权（未开开发者模式）时 Win32 `ERROR_PRIVILEGE_NOT_HELD = 1314` ⇒ `EPERM`；
            // 按 task 说明**如实跳过**，不伪造成功。
            if errno == host_errno("EPERM") || errno == host_errno("EACCES") || errno == 1314 {
                eprintln!("跳过：本机无创建符号链接的特权（errno {errno}）");
                let _ = std::fs::remove_dir_all(&root);
                return;
            }
            panic!("symlink 失败，errno = {errno}");
        }
    }

    // `readlink` 两段式读回：绝对目标 ⇒ guest 形态（`/C:/…`），与建链接时给的 guest 目标一致。
    let read_back = do_readlink(&vtable, &guest_link).expect("读回链接目标");
    assert_eq!(
        read_back, guest_target,
        "readlink 读回的目标应当与建链接时的 guest 目标一致，实际 {:?}",
        String::from_utf8_lossy(&read_back)
    );

    do_unlink(&vtable, &guest_link);
    assert_eq!(
        do_stat(&vtable, &guest_link).expect_err("删掉的链接不该再存在"),
        host_errno("ENOENT")
    );

    let _ = std::fs::remove_dir_all(&root);
}

// --------------------------------------------------------------------------- //
// W3 的端到端验收：`paL_newstate`（Windows 组合根已注册 fs）→ 脚本 open 写文件／读缺失文件
// --------------------------------------------------------------------------- //

fn abi_c_string(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).expect("脚本不含 NUL")
}

fn abi_errmsg(state: *mut pyawa_abi::pa_state) -> String {
    // SAFETY: state 活着（调用方保证）。
    let ptr = unsafe { pyawa_abi::pa_errmsg(state) };
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 交回 NUL 结尾的借用字符串。
    unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

fn abi_exec(state: *mut pyawa_abi::pa_state, source: &str) -> i32 {
    let source = abi_c_string(source);
    let mode = abi_c_string("python");
    // SAFETY: 源码／模式名都是活的 CString；options 传 NULL（`AB-61`）。
    unsafe {
        pyawa_abi::pa_exec_string(
            state,
            source.as_ptr(),
            -1,
            core::ptr::null(),
            mode.as_ptr(),
            core::ptr::null(),
        )
    }
}

fn abi_global_int(state: *mut pyawa_abi::pa_state, name: &str) -> Option<i64> {
    let name = abi_c_string(name);
    // SAFETY: state 活着；名字是活的 CString。
    if unsafe { pyawa_abi::pa_getglobal(state, name.as_ptr()) } != 0 {
        return None;
    }
    let mut value = 0i64;
    // SAFETY: 栈顶刚被压入；出参是活的局部变量。
    let rc = unsafe { pyawa_abi::pa_tointeger(state, -1, &mut value) };
    // SAFETY: 归还 `pa_getglobal` 的那份引用。
    unsafe { pyawa_abi::pa_pop(state, 1) };
    (rc == 0).then_some(value)
}

#[test]
fn open_write_and_enonent_classification_end_to_end_through_the_abi() {
    let root = std::env::temp_dir().join(format!(
        "pyawa-w3-abi-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&root).expect("建临时目录");
    let file_host = root.join("written.txt");
    let missing_host = root.join("missing.txt");
    let file_guest = guest_name(&file_host);
    let missing_guest = guest_name(&missing_host);
    let file_guest = String::from_utf8(file_guest).expect("UTF-8 路径");
    let missing_guest = String::from_utf8(missing_guest).expect("UTF-8 路径");

    let state = pyawa_runtime::paL_newstate();
    assert!(!state.is_null(), "paL_newstate 不得交出 NULL");
    let script = format!(
        "f = open('{file_guest}', 'w')\nf.write('hello')\nf.close()\ntry:\n    open('{missing_guest}')\nexcept FileNotFoundError:\n    ok = 1\n"
    );
    let rc = abi_exec(state, &script);
    assert_eq!(rc, 0, "exec 失败：{}", abi_errmsg(state));
    let missing_classified = abi_global_int(state, "ok");
    assert_eq!(
        missing_classified,
        Some(1),
        "缺失文件应映射成 FileNotFoundError（errno → 异常类的端到端口径）"
    );
    // SAFETY: state 由 paL_newstate 交出、只销毁一次。
    unsafe { pyawa_abi::pa_destroy(state) };

    let written = std::fs::read_to_string(&file_host).expect("provider 应当真的写了宿主文件");
    assert_eq!(written, "hello");

    let _ = std::fs::remove_dir_all(&root);
}
