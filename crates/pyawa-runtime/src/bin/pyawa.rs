//! `pyawa` 命令行入口（最小版；`PLAN-milestones.md` 第 4 条第 ① 项）。
//!
//! ```text
//! pyawa <文件.py|文件.pyawa> [参数…]
//! ```
//!
//! # 如实范围（**这是本版的边界，不是待办清单的省略**）
//!
//! - **能**：跑单个源文件、接收参数（**暂不暴露给脚本**，`sys` 未接线）、给退出码、
//!   报未捕获异常的类型／消息。
//! - **不能 `print`**：`CM-26` 要求 `print` 走 `sys.stdout → _io → fs` 域，**禁止**为手感临时绕开
//!   ⇒ 本版**不提供任何"打印"能力**；下面那句未捕获异常的诊断是**宿主侧**报错（与 `sys.stderr` 无关），
//!   落地 `sys`／`_io` 时会换成合规通道。
//! - **源文件经 `fs` 域读入**（`SPEC-capabilities.md` §9.1 的 `open`／`read`／`close`）——
//!   与后面 `print`／`site.py`（`IM-24`）／import 的 I/O（`IM-15`）走**同一条线**；
//!   本 crate 是平台集中点（`DESIGN.md` §7），真实实现见 [`pyawa_runtime::fs_posix`]。
//! - 退出码：`0` 成功；`1` 脚本异常／语法错；`2` 用法或宿主侧错误（读不到文件等）。
//! - 模式：`.pyawa` ⇒ `"pyawa"`（扩展模式）；其余 `.py` ⇒ `"python"`（纯 Python 模式，`IM-1`／`IM-3`）。

#![allow(unsafe_code)] // 按项开许可的替代：本文件全是 FFI 入口胶水（调用 `unsafe extern "C"`）

use core::ffi::{c_char, c_void};
use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_CLOCK, PA_DOMAIN_FS};
use pyawa_abi::{
    pa_setcapability, pa_setcapability_async,
    pa_create, pa_destroy, pa_errmsg, pa_exec_bytecode, pa_exec_string, pa_host, pa_state, status,
    PA_ABI_SIZE,
    PA_ABI_VERSION,
};
use pyawa_capabilities::fs::CapStatus;
use pyawa_runtime::clock_system::SystemClock;
use pyawa_runtime::fs_posix::PosixFs;
use std::ffi::CString;

/// 用法／宿主侧错误的退出码（与脚本异常的 `1` 分开）。
const EXIT_HOST: i32 = 2;
/// 脚本异常／语法错。
const EXIT_SCRIPT: i32 = 1;

use pyawa_core::compile::{CheckTier, Mode};
use pyawa_runtime::pyac;

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let Some(path) = arguments.next() else {
        usage("缺文件参数");
        std::process::exit(EXIT_HOST);
    };
    let path = std::path::PathBuf::from(path);
    let script_arguments: Vec<std::ffi::OsString> = arguments.collect();
    let mode = match path.extension().and_then(|extension| extension.to_str()) {
        Some("pyawa") => "pyawa",
        Some("py") => "python",
        _ => {
            usage("只接 `.py`／`.pyawa`");
            std::process::exit(EXIT_HOST);
        }
    };

    // 源文件**经 `fs` 域**读入（与 `print`／`site.py`／import 同一条线 ✓）
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    // **`clock` 域**（第 317 轮）：`time` 模块的 `time()`／`monotonic()` 一族经它取 ✓
    //（`SPEC-capabilities.md` §4 表第四项；分类是"可异步化" ✓）。
    let clock_provider = SystemClock::new();
    let clock_vtable = clock_provider.vtable();
    let source = match read_through_fs(&vtable, &path) {
        Ok(source) => source,
        Err(message) => {
            eprintln!("pyawa: {message}");
            std::process::exit(EXIT_HOST);
        }
    };

    // **`pa_host.capabilities` 只传空**：那个指针**不带长度** ✗ ⇒ 宿主若放一个短数组，
    // 任何"按 `DOMAIN_COUNT` 定长读"的实现都会越界（第 90 轮实测：C 示例宿主 `m1.c` 上
    // `SIGABRT` ✓）。注册一律走**按域**的 `pa_setcapability*`（`AB-33`／`AB-34` ✓）。
    let host = pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按 `AB-8`／`AB-43` 的契约传宿主结构；`slots`／`vtable`／`provider` 都活到本函数末尾。
    let created = unsafe { pa_create(&host, &mut state) };
    if created != status::PA_OK {
        eprintln!("pyawa: 建实例失败（状态 {created}）");
        std::process::exit(EXIT_HOST);
    }

    // 能力注册（`AB-32`／`AB-33`／`AB-34`：按域注册、必须带异步分类 ✓）
    let implementation =
        (&vtable as *const pyawa_capabilities::fs::CpFsVtable).cast::<c_void>();
    // SAFETY: `state` 刚建成功；域号／分类取值都在表内。
    let async_status =
        unsafe { pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK) };
    // SAFETY: 同上；`implementation` 指向活到本函数末尾的 vtable。
    let registered = unsafe { pa_setcapability(state, PA_DOMAIN_FS, implementation) };
    // **`clock` 域注册**（同一手法 ✓）
    let clock_implementation =
        (&clock_vtable as *const pyawa_capabilities::clock::CpClockVtable).cast::<c_void>();
    // SAFETY: `state` 已建成功；域号／分类取值都在表内。
    let clock_async = unsafe { pa_setcapability_async(state, PA_DOMAIN_CLOCK, PA_ASYNC_OK) };
    // SAFETY: 同上；`clock_implementation` 指向活到本函数末尾的 vtable。
    let clock_registered =
        unsafe { pa_setcapability(state, PA_DOMAIN_CLOCK, clock_implementation) };
    if clock_async != status::PA_OK || clock_registered != status::PA_OK {
        eprintln!("pyawa: `clock` 域注册失败");
        std::process::exit(EXIT_HOST);
    }
    if async_status != status::PA_OK || registered != status::PA_OK {
        eprintln!("pyawa: 能力注册失败（分类 {async_status}／实现 {registered}）");
        std::process::exit(EXIT_HOST);
    }

    // **组合根装配**（`CM-14`）：内建名字空间 ＋ `sys`（含 `stdout`／`stderr`）；并把那个
    // `sys.stdout` 对象放进内建名字空间 ⇒ `print` 的目的地就是它 ✓
    // （`CM-26`：`print` ⇒ `sys.stdout` ⇒ `_io` 文本层 ⇒ `fs` 域的 `write` ✓，**不设临时 sink** ✓）。
    // SAFETY: `state` 由 `pa_create` 交回，活到本函数末尾 ✓。
    let state_ref = unsafe { &*state };
    let instance = state_ref.instance();
    let program_name = path.to_string_lossy().into_owned();
    let script_arguments_text: Vec<String> = script_arguments
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    // **平台常量注入**（第 134 轮）：`paL_newstate`／`Pyawa::new` 都注入了 ✓，但**本 CLI 走 `pa_create`** ✗
    // ⇒ 这条路上表是空的 ✗（`import errno` 能过但**取不到常量** ✗ ⇒ 对拍当场抓住 ✓）。
    // `CM-20`：常量由宿主注入、映射按名字匹配 ✓ —— 这里就是那个"宿主" ✓。
    instance.set_platform_constants(pyawa_runtime::platform_errno::HOST_ERRNO);
    pyawa_stdlib::install(instance, &program_name, &script_arguments_text);

    // **`.pyac` 容器 ＋ 两步陈旧判定接进运行路径**（第 405 轮；`IM-18`…`IM-21`）：
    //   ① **按名字精确查找**产物（`artifact_path`：别的版本／模式留下的产物**连读都不读** ✓）；
    //   ② 命中就比头部（长度＋指纹＋模式／优化级／档位）⇒ `Fresh` 直接装载 ✓；
    //      缺失／`Stale`／读不出来 ⇒ 编译并写产物 ✓。
    // 两条路都**经 `pa_exec_bytecode`** 执行 ✓ ⇒ 容器与陈旧判定在**运行路径**上真的被用 ✓
    //（这是 `P3-12` 的交付物 ✓；与 `CM-15` 的 import 比例无关 ✓）。
    let version = pyawa_core::opcode_metadata::INSTRUCTION_SET_VERSION;
    let mode_byte = if mode == "python" {
        pyac::MODE_PURE
    } else {
        pyac::MODE_EXTENDED
    };
    let (tier, optimization) = (0u8, 0u8);
    let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "script.py".to_owned());
    let artifact = pyac::artifact_path(directory, &file_name, version);
    let source_bytes = source.as_bytes();
    let product = match pyac::staleness(
        &artifact,
        mode_byte,
        optimization,
        tier,
        source_bytes,
        version,
    ) {
        pyac::Staleness::Fresh => std::fs::read(&artifact).ok(),
        _ => pyawa_core::compile::compile(
            &source,
            &program_name,
            if mode_byte == pyac::MODE_PURE {
                Mode::PurePython
            } else {
                Mode::Extension
            },
            CheckTier::Shallow,
            optimization,
        )
        .ok()
        .map(|unit| {
            let code = pyawa_core::pyac::encode_unit(&unit);
            // 写产物：**失败不致命** ✓（下一轮重编；只影响性能，不影响语义 ✓）
            let _ = pyac::write(
                directory,
                &file_name,
                version,
                mode_byte,
                optimization,
                tier,
                source_bytes,
                &code,
            );
            pyawa_core::pyac::encode(mode_byte, optimization, tier, source_bytes, &code, version)
        }),
    };
    let executed = match product {
        // SAFETY: 产物字节在本栈上活着；`state` 由 `pa_create` 交回 ✓。
        Some(bytes) => unsafe {
            pa_exec_bytecode(state, bytes.as_ptr().cast::<c_void>(), bytes.len() as isize)
        },
        // 产物读不出来／编译失败 ⇒ 交回字符串入口：**状态码与诊断口径不变** ✓
        None => {
            let source_c =
                CString::new(source).unwrap_or_else(|_| CString::new("").expect("空串可用"));
            let mode_c = CString::new(mode).expect("模式名是 ASCII");
            // SAFETY: 源码与模式名都是 NUL 结尾的 `CString`；`opts` 传 NULL（`AB-60`／`AB-61`）。
            unsafe {
                pa_exec_string(
                    state,
                    source_c.as_ptr(),
                    -1,
                    core::ptr::null(),
                    mode_c.as_ptr().cast::<c_char>(),
                    core::ptr::null(),
                )
            }
        }
    };

    let exit = if executed == status::PA_OK {
        0
    } else {
        // 宿主侧诊断（**不是** `sys.stderr`；`print`／`sys` 落地后替换为合规通道）
        let pointer = unsafe { pa_errmsg(state) };
        let message = if pointer.is_null() {
            String::new()
        } else {
            unsafe { core::ffi::CStr::from_ptr(pointer) }.to_string_lossy().into_owned()
        };
        eprintln!("pyawa: 未捕获（状态 {executed}）：{message}");
        EXIT_SCRIPT
    };
    // 实参已经装进 `sys.argv` ✓（第 97 轮起不再是「收了没用」✓；`argv[0]` 是脚本路径 ✓）
    unsafe { pa_destroy(state) };
    std::process::exit(exit);
}

/// 经 `fs` 域 vtable 读一个文件（`open` → `read` → `close`）。
fn read_through_fs(
    vtable: &pyawa_capabilities::fs::CpFsVtable,
    path: &std::path::Path,
) -> Result<String, String> {
    let (Some(open), Some(read), Some(close)) = (vtable.open, vtable.read, vtable.close) else {
        return Err("`fs` 域的读槽位未提供".to_owned());
    };
    let name = path.to_string_lossy().into_owned().into_bytes();
    let mut handle = pyawa_capabilities::fs::Handle(0);
    let mut errno = 0i32;
    let status = open(
        vtable.state,
        name.as_ptr(),
        name.len(),
        pyawa_capabilities::fs::open_flag::RDONLY,
        0,
        &mut handle,
        &mut errno,
    );
    if status != CapStatus::Ok {
        return Err(format!("打不开 {}（状态 {status:?}，errno {errno}）", path.display()));
    }
    let mut contents: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let mut got = 0usize;
        let status = read(
            vtable.state,
            handle,
            buffer.as_mut_ptr(),
            buffer.len(),
            0,
            &mut got,
            &mut errno,
        );
        if status != CapStatus::Ok {
            let _ = close(vtable.state, handle, &mut errno);
            return Err(format!("读 {} 失败（状态 {status:?}，errno {errno}）", path.display()));
        }
        if got == 0 {
            break;
        }
        contents.extend_from_slice(&buffer[..got]);
    }
    let _ = close(vtable.state, handle, &mut errno);
    String::from_utf8(contents).map_err(|_| format!("{} 不是 UTF-8", path.display()))
}

/// 用法提示（宿主侧）。
fn usage(reason: &str) {
    eprintln!("pyawa: {reason}");
    eprintln!("用法：pyawa <文件.py|文件.pyawa> [参数…]");
    eprintln!("范围：跑单文件／给退出码／报未捕获异常；**本版还不能 `print`**（`CM-26`：`print` 须走 `sys.stdout → _io → fs`）。");
}
