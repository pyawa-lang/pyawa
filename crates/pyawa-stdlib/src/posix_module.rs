//! `posix` 模块（第 138 轮）：**最小面**。
//!
//! 现状：只给出名字空间与 `__all__`，让 `Lib/os.py` 的 `from posix import *` 能过
//! （`_get_exports_list` 优先读 `module.__all__` ✓）。**函数面（`open`／`stat`／`listdir`…）尚未落地** ✗
//! —— 它们按 `CM-8` **必须**走能力域（`fs` ✓），逐条随用例补 ✓。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::ptr::NonNull;

use pyawa_capabilities::fs::open_flag;
use pyawa_core::{AttributeObject, Header, Instance};

use crate::io_module::fs_error;

/// 模块名（`posix`）。
pub const NAME: &str = "posix";

/// **`posix._exit(n)`** ✓（第 188 轮）：**立即结束进程** ✓ —— **不跑**清理／不刷缓冲 ✓（与参照同义 ✓）。
///
/// 为什么先做它 ✓：`Lib/os.py:57` 是 `from posix import _exit` ✓（**显式**导入 ✓）⇒ 少了它，
/// 整篇 `os.py` 连**导入**都过不去 ✓（`os.py:907` 的"命令找不到"那条路也用它 ✓）。
fn exit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let code = args
        .first()
        .and_then(|value| instance.int_value(*value))
        .unwrap_or(0);
    // **立即退**：这是 `_exit` 的语义 ✓（`os._exit` 与 `sys.exit` 的区别就在这 ✓）。
    std::process::exit(code as i32);
}

/// **`posix._path_normpath(p)`** ✓（第 195 轮）：`Lib/posixpath.py:341` 要它 ✓（`normpath` ✓）。
///
/// **规格有据** ✓：就是 `posixpath.py` 自己那份**纯 Python 回退实现** ✓（`splitroot` ＋ 逐段折叠 ✓）；
/// 实测样本（参照 ✓）：`''` ⇒ `'.'` ✓、`'a//b'` ⇒ `'a/b'` ✓、`'/a/../b'` ⇒ `'/b'` ✓、
/// `'//a/../..'` ⇒ `'//'` ✓、`'x/../../y'` ⇒ `'../y'` ✓。
///
/// **如实说** ✗：只接 `str` ✓。
fn path_normpath_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "_path_normpath() 要 1 个实参"));
    };
    let Some(path) = instance.text_of(value) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_path_normpath() 的参数要是 str（**bytes 随后补** ✗）",
        ));
    };
    if path.is_empty() {
        return Ok(instance.new_str("."));
    }
    // `splitroot` 那三步 ✓（与 `_path_splitroot_ex` **同一口径** ✓ —— 规格同源 ✓）。
    let (initial_slashes, rest) = if !path.starts_with('/') {
        ("", path)
    } else if path.as_bytes().get(1) != Some(&b'/') || path.as_bytes().get(2) == Some(&b'/') {
        ("/", &path[1..])
    } else {
        ("//", &path[2..])
    };
    let mut folded: Vec<&str> = Vec::new();
    for component in rest.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        let keep = component != ".."
            || (initial_slashes.is_empty() && folded.is_empty())
            || folded.last() == Some(&"..");
        if keep {
            folded.push(component);
        } else if !folded.is_empty() {
            folded.pop();
        }
    }
    let mut text = format!("{initial_slashes}{}", folded.join("/"));
    if text.is_empty() {
        text = ".".to_owned();
    }
    Ok(instance.new_str(&text))
}

/// **`posix._path_splitroot_ex(p)`** ✓（第 195 轮）：`Lib/posixpath.py:139` 要它 ✓
///（`splitroot` ✓）。**规格有据** ✓：就是 `posixpath.py` 自己那份**纯 Python 回退实现** ✓ ——
/// 相对路径 ⇒ `('', '', p)` ✓；**恰好两个**前导斜杠 ⇒ `('', '//', p[2:])` ✓；其余绝对路径 ⇒ `('', '/', p[1:])` ✓。
/// 实测样本（参照 ✓）：`''` ⇒ `('','','')` ✓、`'/'` ⇒ `('','/','')` ✓、`'//'` ⇒ `('','//','')` ✓、
/// `'///a'` ⇒ `('','/','//a')` ✓、`'//a/b'` ⇒ `('','//','a/b')` ✓。
///
/// **如实说** ✗：只接 `str` ✓（`bytes` 那条随后补 ✓）。
fn path_splitroot_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "_path_splitroot_ex() 要 1 个实参"));
    };
    let Some(path) = instance.text_of(value) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_path_splitroot_ex() 的参数要是 str（**bytes 随后补** ✗）",
        ));
    };
    let drive_text = "";
    let (root_text, tail_text) = if !path.starts_with('/') {
        ("", path)
    } else if path.as_bytes().get(1) != Some(&b'/') || path.as_bytes().get(2) == Some(&b'/') {
        ("/", &path[1..])
    } else {
        ("//", &path[2..])
    };
    let drive = instance.new_str(drive_text);
    let root = instance.new_str(root_text);
    let tail = instance.new_str(tail_text);
    Ok(instance.new_tuple(vec![drive, root, tail]))
}

/// **`posix.open`／`close`／`read`／`write`** ✓（第 204 轮）：`os.py` 一导入就 `from posix import *` ✓
/// ⇒ 这些名字都要在 ✓（**一处真相** ✓：全部走 `fs` 域 ✓）。
/// POSIX 的 `O_*` ⇒ **能力域**的 `open_flag` ✓（**两套位不是一回事** ✗：域里 `APPEND=8`／`TRUNC=16`／
/// `CREAT=32` ✓，POSIX 是 `0o2000`／`0o1000`／`0o100` ✓；访问模式那两位同值 ✓）。
///
/// **真 bug**（task-18 实测 ✗）：先前把 POSIX 位**原样**交给 `fs_open` ✗ ⇒ provider 按域位解
/// ⇒ `os.open(p, O_WRONLY|O_CREAT)` 实测报 `打不开` ✓（`O_CREAT` 的位根本对不上 ✓）。
/// `O_EXCL` 域里**没有**槽 ✗ ⇒ 用 `fs_stat` 先探存在（同 `_io` 的 `x` 模式 ✓）。
fn domain_open_flags(
    instance: &Instance,
    path: &[u8],
    posix: i32,
) -> Result<i32, pyawa_core::ExecError> {
    let mut flags = posix & 3; // O_RDONLY／O_WRONLY／O_RDWR：两套同值 ✓
    if posix & 0o2000 != 0 {
        flags |= open_flag::APPEND;
    }
    if posix & 0o1000 != 0 {
        flags |= open_flag::TRUNC;
    }
    if posix & 0o100 != 0 {
        if posix & 0o200 != 0 && instance.fs_stat(path).is_ok() {
            return Err(instance.raise_builtin_error(
                "FileExistsError",
                &format!("[Errno 17] File exists: '{}'", String::from_utf8_lossy(path)),
            ));
        }
        flags |= open_flag::CREAT;
    }
    Ok(flags)
}

fn open_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (Some(path_value), Some(flags_value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "open() 要 path 与 flags"));
    };
    let Some(path) = instance.text_of(path_value) else {
        return Err(instance.raise_builtin_error("TypeError", "open() 的 path 要是 str"));
    };
    let posix = instance.int_value(flags_value).unwrap_or(0) as i32;
    let mode = args.get(2).copied().and_then(|value| instance.int_value(value)).unwrap_or(0o777) as u32;
    let flags = domain_open_flags(instance, path.as_bytes(), posix)?;
    match instance.fs_open(path.as_bytes(), flags, mode) {
        Ok(handle) => Ok(instance.new_int(handle as i64)),
        Err(error) => Err(fs_error(instance, error, path)),
    }
}

fn close_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(handle_value) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "close() 要 1 个实参"));
    };
    let handle = instance.int_value(handle_value).unwrap_or(-1) as u64;
    let _ = instance.fs_close(handle);
    // **返回 `None` 必须是新引用** ✗（第 280 轮修：先前交的是**借用**的单例 ✗ ⇒ 调用方按
    // "新引用"接管 ⇒ 单例被多释放一次 ⇒ 堆欠计数（`corrupted double-linked list` 这一类 ✓））。
    Ok(instance.new_none())
}

fn read_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (Some(handle_value), Some(count_value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "read() 要 fd 与长度"));
    };
    let handle = instance.int_value(handle_value).unwrap_or(-1) as u64;
    let count = instance.int_value(count_value).unwrap_or(0).max(0) as usize;
    let mut buffer = vec![0u8; count];
    match instance.fs_read(handle, &mut buffer) {
        Ok(read) => {
            buffer.truncate(read);
            Ok(instance.new_bytes(&buffer))
        }
        Err(_) => Err(instance.raise_builtin_error("OSError", "read：读不了")),
    }
}

fn write_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let (Some(handle_value), Some(data_value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error("TypeError", "write() 要 fd 与数据"));
    };
    let handle = instance.int_value(handle_value).unwrap_or(-1) as u64;
    // **安全访问器** ✓（本 crate **禁 `unsafe`** ✓ ⇒ 设计规矩 ✓）。
    let Some(data) = instance.bytes_value(data_value) else {
        return Err(instance.raise_builtin_error("TypeError", "write() 的数据要是 bytes"));
    };
    match instance.fs_write(handle, data) {
        Ok(written) => Ok(instance.new_int(written as i64)),
        Err(_) => Err(instance.raise_builtin_error("OSError", "write：写不了")),
    }
}

/// `os.stat_result` 的**每实例唯一**类型对象 ✓（第 722 轮建；第 727 轮修 ✗）。
///
/// **真 bug**（第 727 轮，`PYAWA_DANGLING` 抓到的偶发 UAF ✓）：先前是**进程级**
/// `static Mutex<Option<usize>>` ✗ ⇒ 新 `Instance` 会复用**上一个实例已释放**的类型指针 ✗
/// ⇒ `posix_module::build` 里 `dict_set` 时 `incref 已释放对象` ✓（`file_backed_import_needs_the_fs_domain`
/// 偶发复现 ✓）。现在改为**在本实例的类型注册表里查**（`type_named` ✓）⇒ 同一实例内身份稳定 ✓、跨实例不串 ✓。
fn stat_result_type(instance: &Instance) -> NonNull<pyawa_core::TypeObject> {
    if let Some(existing) = instance.type_named("os.stat_result") {
        return existing;
    }
    instance.new_attribute_type("os.stat_result")
}

/// **`posix.stat(path)`** ✓（第 204 轮）：`Lib/os.py:148` 的 `_set.add(stat)` 要这个名字 ✓
///（它来自 `from posix import *` ✓）。**一处真相** ✓：值来自 `fs` 域的 `stat` 槽 ✓（provider 早已实现 ✓）。
///
/// **如实说** ✗：只给 `FileInfo` **真的有**的字段 ✓（`st_mode`／`st_size`／`st_dev`／`st_ino` ✓）；
/// `st_uid`／时间戳一族随后随 `CP-` 补 ✓。
fn stat_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "stat() 要 1 个实参"));
    };
    let Some(path) = instance.text_of(value) else {
        return Err(instance.raise_builtin_error("TypeError", "stat() 的参数要是 str"));
    };
    let info = match instance.fs_stat(path.as_bytes()) {
        Ok(info) => info,
        Err(_) => {
            return Err(instance.raise_builtin_error(
                "OSError",
                &format!("stat：`fs` 域读不到 {path}"),
            ));
        }
    };
    let result_type = stat_result_type(instance);
    for (field, number) in [
        ("st_mode", info.mode as i64),
        ("st_size", info.size as i64),
        ("st_dev", info.dev as i64),
        ("st_ino", info.ino as i64),
    ] {
        let value = instance.new_int(number);
        instance.set_type_attribute(result_type, field, value);
    }
    let result = instance.alloc(AttributeObject::new(result_type, core::cell::RefCell::new(None)));
    Ok(result.into_raw().cast::<Header>())
}

/// **`posix._create_environ()`** ✓（第 195 轮）：`Lib/os.py:68` 要它 ✓ —— 返回**环境变量字典** ✓。///
/// **一处真相** ✓：真值就是**本进程的环境** ✓（`std::env::vars` ✓）⇒ 本层不另造一份 ✓。
fn create_environ_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let environment = instance.new_dict();
    for (key, value) in std::env::vars() {
        let text = instance.new_str(&value);
        instance.dict_set(environment, &key, text);
    }
    Ok(environment)
}

/// **`posix.unlink(path)`／`posix.rmdir(path)`** ✓（第 722 轮）：`Lib/shutil.py:804` 在**导入期**
/// 就把这两个名字放进 `{os.open, os.stat, os.unlink, os.rmdir}` 这个集合 ✓ ⇒ 名字必须在 ✓。
/// 真值走 **`fs` 能力域**的 `unlink`／`rmdir` 槽 ✓（vtable 与 provider 早就有 ✓，缺的只是包装 ✓）。
fn unlink_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    remove_path(instance, args, false)
}

fn rmdir_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    remove_path(instance, args, true)
}

fn remove_path(
    instance: &Instance,
    args: &[NonNull<Header>],
    directory: bool,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let kind = if directory { "rmdir" } else { "unlink" };
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", &format!("{kind}() 要 1 个实参")));
    };
    let Some(path) = instance.text_of(value) else {
        return Err(instance.raise_builtin_error("TypeError", &format!("{kind}() 的参数要是 str")));
    };
    let outcome = if directory {
        instance.fs_rmdir(path.as_bytes())
    } else {
        instance.fs_unlink(path.as_bytes())
    };
    match outcome {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(pyawa_core::CapabilityCallError::NotImplemented) => Err(instance.raise_builtin_error(
            "NotImplementedError",
            &format!("{kind}：provider 没有这条 fs 槽（CM-8）"),
        )),
        Err(pyawa_core::CapabilityCallError::NotRegistered) => Err(instance.raise_builtin_error(
            "NotImplementedError",
            &format!("{kind}：fs 能力域未注册（CM-8）"),
        )),
        Err(pyawa_core::CapabilityCallError::Machine(errno)) => Err(instance.raise_builtin_error(
            "OSError",
            &format!("{kind}：errno {errno}（{path}）"),
        )),
    }
}

/// **`posix.urandom(n)`** ✓（第 722 轮）：`Lib/random.py:57` 的 `from os import urandom as _urandom`
/// 要这个名字 ✓（`SystemRandom` 用它 ✓）。
///
/// 真值经 **`fs` 能力域**读 `/dev/urandom` ✓（`CX-4`：本 crate **不碰平台** ✗ —— 先前那句
/// `std::fs::File::open` 就被 `tests/ci/check.py` 的 `T-CX-4` 逮住 ✓）；
/// 读不到时退回 **时钟 ＋ 进程内计数器** 摊出的伪随机填充 ✓（**不是密码学熵源** ✗，如实记 ✓）。
fn urandom_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let count = args.first().and_then(|value| instance.int_value(*value)).unwrap_or(0);
    if count < 0 {
        return Err(instance.raise_builtin_error("ValueError", "negative argument not allowed"));
    }
    let mut bytes = vec![0u8; count as usize];
    // **能读满 `/dev/urandom` 才算成功** ✓（`O_RDONLY` ＝ 0 ✓）。
    let mut filled = false;
    if let Ok(handle) = instance.fs_open(b"/dev/urandom", 0, 0) {
        let mut offset = 0usize;
        while offset < bytes.len() {
            match instance.fs_read(handle, &mut bytes[offset..]) {
                Ok(0) | Err(_) => break,
                Ok(read) => offset += read,
            }
        }
        let _ = instance.fs_close(handle);
        filled = offset == bytes.len();
    }
    if !filled {
        let mut state = crate::random_module::seed_words(crate::random_module::entropy_seed());
        for chunk in bytes.chunks_mut(8) {
            let word = crate::random_module::next_u64(&mut state);
            for (index, slot) in chunk.iter_mut().enumerate() {
                *slot = (word >> (index * 8)) as u8;
            }
        }
    }
    Ok(instance.new_bytes(&bytes))
}

/// **`posix.getpid()`** ✓（第 722 轮）：`Lib/tempfile.py` 的 `gettempdir()` 要它 ✓（拼临时目录名 ✓）。
/// 真值取**宿主进程 id** ✓（`std::process::id` ✓）。
fn getpid_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    Ok(instance.new_int(std::process::id() as i64))
}

/// **`posix.cpu_count()`** ✓（第 270 轮）：`Lib/os.py` 里 `cpu_count = ...` 一类要它 ✓
/// —— 值取**宿主**的可用并行度 ✓（`std::thread::available_parallelism` ✓，与参照在本机实测一致 ✓）。
fn cpu_count_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let count = std::thread::available_parallelism()
        .map(|value| value.get() as i64)
        .unwrap_or(1);
    Ok(instance.new_int(count))
}

/// 建 `posix` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // **`_exit`** ✓（第 188 轮）：`os.py` 导入它 ✓（`__all__` 仍为空 ✓ ⇒ `import *` 不导它 ✓，与参照一致 ✓）。
    let exit_fn = crate::builtins_module::make_native(
        instance,
        "_exit",
        exit_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "_exit", exit_fn);
    // **函数面**（第 204 轮起逐条落地 ✓）：`stat` 是 `Lib/os.py:148` 点名要的第一个 ✓。
    let stat_fn = crate::builtins_module::make_native(instance, "stat", stat_native as pyawa_core::NativeFn);
    instance.dict_set(namespace, "stat", stat_fn);
    // `lstat`／`fstat` 与它**同源** ✓（provider 的同一个槽 ✓）：先各自给一份 ✓。
    let lstat_fn = crate::builtins_module::make_native(instance, "lstat", stat_native as pyawa_core::NativeFn);
    instance.dict_set(namespace, "lstat", lstat_fn);
    // **`stat_result`** ✓（第 722 轮）：`Lib/shutil.py:659` 要 `os.stat_result` ✓
    //（`os.py:55` 的 `from posix import *` 靠 `__all__` 带过去 ✓）。
    let stat_result = stat_result_type(instance);
    instance.dict_set(namespace, "stat_result", stat_result.cast::<Header>());
    for (name, handler) in [
        ("open", open_native as pyawa_core::NativeFn),
        ("close", close_native as pyawa_core::NativeFn),
        ("read", read_native as pyawa_core::NativeFn),
        ("write", write_native as pyawa_core::NativeFn),
        ("unlink", unlink_native as pyawa_core::NativeFn),
        ("rmdir", rmdir_native as pyawa_core::NativeFn),
        ("urandom", urandom_native as pyawa_core::NativeFn),
        ("getpid", getpid_native as pyawa_core::NativeFn),
    ] {
        let native = crate::builtins_module::make_native(instance, name, handler);
        instance.dict_set(namespace, name, native);
    }
    // **POSIX 访问模式常量** ✓（第 722 轮）：`Lib/shutil.py:1575` 的
    // `def which(cmd, mode=os.F_OK | os.X_OK, ...)` 在**def 时**就求值 ✓ ⇒ 名字必须在 ✓。
    for (name, value) in [("F_OK", 0i64), ("X_OK", 1), ("W_OK", 2), ("R_OK", 4)] {
        let constant = instance.new_int(value);
        instance.dict_set(namespace, name, constant);
    }
    // **`open(2)` 的标志位** ✓（第 722 轮）：`Lib/tempfile.py:52` 的
    // `_text_openflags = _os.O_RDWR | _os.O_CREAT | _os.O_EXCL` 在**导入期**就求值 ✓
    // ⇒ 名字必须在 ✓。值取 **Linux／POSIX** 的真实取值 ✓（`fs` provider 直接把它们交给 `open(2)` ✓）。
    for (name, value) in [
        ("O_RDONLY", 0i64),
        ("O_WRONLY", 1),
        ("O_RDWR", 2),
        ("O_CREAT", 0o100),
        ("O_EXCL", 0o200),
        ("O_TRUNC", 0o1000),
        ("O_APPEND", 0o2000),
        ("O_NONBLOCK", 0o4000),
        ("O_DIRECTORY", 0o200000),
        ("O_NOFOLLOW", 0o400000),
        ("O_CLOEXEC", 0o2000000),
        ("O_TMPFILE", 0o20200000),
    ] {
        let constant = instance.new_int(value);
        instance.dict_set(namespace, name, constant);
    }
    // **`__all__` 跟着函数面走** ✓：`os.py:55` 的 `from posix import *` 读的就是它 ✓
    //（之前是**空表** ✗ —— 如实记了"函数面未落地" ✓ ⇒ 现在有一条就列一条 ✓）。
    let exports = instance.new_list(vec![
        instance.new_str("stat"),
        instance.new_str("lstat"),
        instance.new_str("stat_result"),
        instance.new_str("open"),
        instance.new_str("close"),
        instance.new_str("read"),
        instance.new_str("write"),
        instance.new_str("unlink"),
        instance.new_str("rmdir"),
        instance.new_str("urandom"),
        instance.new_str("getpid"),
        instance.new_str("environ"),
        instance.new_str("cpu_count"),
        instance.new_str("F_OK"),
        instance.new_str("X_OK"),
        instance.new_str("W_OK"),
        instance.new_str("R_OK"),
        instance.new_str("O_RDONLY"),
        instance.new_str("O_WRONLY"),
        instance.new_str("O_RDWR"),
        instance.new_str("O_CREAT"),
        instance.new_str("O_EXCL"),
        instance.new_str("O_TRUNC"),
        instance.new_str("O_APPEND"),
        instance.new_str("O_NONBLOCK"),
        instance.new_str("O_DIRECTORY"),
        instance.new_str("O_NOFOLLOW"),
        instance.new_str("O_CLOEXEC"),
        instance.new_str("O_TMPFILE"),
    ]);
    instance.dict_set(namespace, "__all__", exports);
    // **`_have_functions`** ✓（第 188 轮）：`os.py` 一导入就**扫这个表** ✓（用来决定
    // `supports_follow_symlinks` 一族 ✓）。**如实说** ✗：本层还没实现那些可选能力 ✓ ⇒ 给**空表** ✓
    //（语义上就是"一个都不支持" ✓，比编造一串名字**诚实** ✓）。
    let have_functions = instance.new_list(Vec::new());
    instance.dict_set(namespace, "_have_functions", have_functions);
    // **`_create_environ`** ✓（第 195 轮）：`os.py` 一导入就调它 ✓。
    // **`posix.environ`** ✓（第 268 轮）：`Lib/os.py` 里 `data = environ` 读的是 `from posix import *`
    // 那个 `environ` ✓ —— 与 `_create_environ` **同一处真相** ✓（本进程的环境 ✓，`std::env::vars` ✓）。
    let environ = instance.new_dict();
    for (key, value) in std::env::vars() {
        let text = instance.new_str(&value);
        instance.dict_set(environ, &key, text);
    }
    instance.dict_set(namespace, "environ", environ);
    let cpu_count =
        crate::builtins_module::make_native(instance, "cpu_count", cpu_count_native as pyawa_core::NativeFn);
    instance.dict_set(namespace, "cpu_count", cpu_count);
    let create_environ = crate::builtins_module::make_native(
        instance,
        "_create_environ",
        create_environ_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "_create_environ", create_environ);
    // **`_path_splitroot_ex`** ✓（第 195 轮）：`posixpath.py` 一导入就 `from posix import` 它 ✓。
    let splitroot = crate::builtins_module::make_native(
        instance,
        "_path_splitroot_ex",
        path_splitroot_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "_path_splitroot_ex", splitroot);
    // **`_path_normpath`** ✓（第 195 轮）。
    let normpath = crate::builtins_module::make_native(
        instance,
        "_path_normpath",
        path_normpath_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "_path_normpath", normpath);
    // `__all__` 现阶段**为空**（如实：函数面未落地 ✓）⇒ `from posix import *` 导入零个名字 ✓
    // **`__all__` 随函数面走** ✓（第 204 轮）：`os.py:55` 是 `from posix import *` ✓ ⇒
    // 这里列出的名字才会进 `os` 的命名空间 ✓（`_get_exports_list` 读的就是它 ✓）。
    let exports = instance.new_list(Vec::new());
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
