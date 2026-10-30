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

use crate::io_module::{fs_error, fs_error_with_display};


/// **机器错误 ⇒ `OSError` 子类**（`CM-19`／`CM-20`：按 **errno 名**查表 ✓）。
///
/// 与 `io_module::fs_error` 同一口径，区别只是**没有路径后缀**（`os.dup`／`select` 这类调用没有
/// "路径"可显示 ✓）。`strerror` 文案本地一小张（`io_module` 那张是私有的，不跨模块加宽可见性 ✓）。
pub(crate) fn capability_error(
    instance: &Instance,
    error: pyawa_core::CapabilityCallError,
) -> pyawa_core::ExecError {
    match error {
        pyawa_core::CapabilityCallError::NotRegistered => instance.raise_builtin_error(
            "NotImplementedError",
            "能力域未注册（CP-2），本轮未接线",
        ),
        pyawa_core::CapabilityCallError::NotImplemented => instance.raise_builtin_error(
            "NotImplementedError",
            "能力槽未实现（CP-5），本轮未接线",
        ),
        pyawa_core::CapabilityCallError::Machine(errno) => {
            let name = crate::errno_map::ERRNO_ERRORCODE
                .iter()
                .find(|(number, _)| *number == errno as i64)
                .map(|(_, name)| *name);
            let class = name
                .and_then(|name| {
                    crate::errno_map::ERRNO_TO_CLASS
                        .iter()
                        .find(|(key, _)| *key == name)
                        .map(|(_, class)| *class)
                })
                .unwrap_or("OSError");
            let text = name.and_then(errno_text).unwrap_or("unknown error");
            instance.raise_builtin_error(class, &format!("[Errno {errno}] {text}"))
        }
    }
}

fn errno_text(name: &str) -> Option<&'static str> {
    Some(match name {
        "EBADF" => "Bad file descriptor",
        "EINVAL" => "Invalid argument",
        "ENOENT" => "No such file or directory",
        "EACCES" => "Permission denied",
        "EMFILE" => "Too many open files",
        "ENFILE" => "Too many open files in system",
        "EBUSY" => "Device or resource busy",
        "ENOMEM" => "Cannot allocate memory",
        "EPIPE" => "Broken pipe",
        "EAGAIN" => "Resource temporarily unavailable",
        "EOPNOTSUPP" => "Operation not supported",
        "ENOTCONN" => "Transport endpoint is not connected",
        "ECONNRESET" => "Connection reset by peer",
        _ => return None,
    })
}

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

/// **guest 路径实参**（P3-38 S3b-2）：str／bytes → fs 域的**原始字节** ＋ 报错用的显示形。
///
/// PEP 383 口径：
/// - `str` 经 **`utf-8` ＋ `surrogateescape`** 编码 ⇒ 合法 UTF-8 字节**零变化**；
///   U+DC80–DCFF 还原成原始单字节（`'\\udcff'` ⇒ `0xff`）；其它孤立代理照参照抛
///   `UnicodeEncodeError`（`os.fsencode` 同口径）。
/// - `bytes` 原样。
/// - 显示形（`repr` 形，含引号）：str 路径按同一 `surrogateescape` 解回 WTF-8 `str` 再 `repr`
///   ⇒ 报错文案里 `'\\udcff'` 与参照一致；bytes 路径给 `b'…'`。
#[derive(Clone)]
pub(crate) struct PathArgument {
    pub(crate) bytes: Vec<u8>,
    pub(crate) display: String,
    pub(crate) want_bytes: bool,
}

/// 把一份 `str`（WTF-8 字节）按 UTF-8＋surrogateescape 编成 fs 字节（合法 UTF-8 零变化）。
fn encode_path_text(instance: &Instance, text: &[u8]) -> Result<Vec<u8>, pyawa_core::ExecError> {
    let encoded = instance.encode_wtf8_with_encoding_and_errors(text, "utf-8", "surrogateescape")?;
    let bytes = instance.bytes_value(encoded).map(|raw| raw.to_vec()).unwrap_or_default();
    instance.release(encoded);
    Ok(bytes)
}

/// 把 fs 字节按 UTF-8＋surrogateescape 解成 WTF-8 `str`（**新引用**）。
pub(crate) fn decode_path_text(
    instance: &Instance,
    bytes: &[u8],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    instance.decode_with_encoding_and_errors(bytes, "utf-8", "surrogateescape")
}

/// 取一个 guest 路径实参（见 [`PathArgument`] 的 PEP 383 口径）。
pub(crate) fn path_argument(
    instance: &Instance,
    value: NonNull<Header>,
    function: &str,
) -> Result<PathArgument, pyawa_core::ExecError> {
    if let Some(text) = instance.wtf8_of(value) {
        let bytes = encode_path_text(instance, text)?;
        let decoded = decode_path_text(instance, &bytes)?;
        let display = instance.object_repr(decoded)?;
        instance.release(decoded);
        return Ok(PathArgument { bytes, display, want_bytes: false });
    }
    if let Some(raw) = instance.bytes_value(value) {
        let display = format!("b'{}'", String::from_utf8_lossy(raw));
        return Ok(PathArgument { bytes: raw.to_vec(), display, want_bytes: true });
    }
    let name = instance.type_name(instance.type_of(value));
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("{function}: path should be string, bytes or os.PathLike, not {name}"),
    ))
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
    display: &str,
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
                &format!("[Errno 17] File exists: {display}"),
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
    let path = path_argument(instance, path_value, "open")?;
    let posix = instance.int_value(flags_value).unwrap_or(0) as i32;
    let mode = args.get(2).copied().and_then(|value| instance.int_value(value)).unwrap_or(0o777) as u32;
    let flags = domain_open_flags(instance, &path.bytes, &path.display, posix)?;
    match instance.fs_open(&path.bytes, flags, mode) {
        Ok(handle) => Ok(instance.new_int(handle as i64)),
        Err(error) => Err(fs_error_with_display(instance, error, &path.display)),
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


/// **`posix.dup(fd)`**（`CP-31` 的 `dup`）：复制能力表项 ⇒ 新 `fd`（两个 fd 指向**同一**能力对象）。
///
/// 形状照参照实测：关键字一律拒（`posix.dup() takes no keyword arguments`）、恰好 1 参、
/// 实参要能当整数（`'str' object cannot be interpreted as an integer`）；失败 ⇒ `OSError` 形状。
fn dup_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "posix.dup() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("posix.dup() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let fd = descriptor_argument(instance, args[0])?;
    match instance.capability_dup(fd) {
        Ok(duplicate) => Ok(instance.new_int(duplicate)),
        Err(error) => Err(capability_error(instance, error)),
    }
}

/// **`posix.dup2(fd, fd2, inheritable=True)`**（`CP-31` 的表项复制）：让 `fd2` 指向 `fd` 的能力对象；
/// `fd2` 原表项先按 `close` 语义释放。**返回 `fd2`**（参照 3.14 实测就是返回新 fd，不是 `None` ✓）。
///
/// 形状照参照 clinic：位置 2–3 个、关键字 `fd`／`fd2`／`inheritable`；`inheritable` **接受但不生效**
/// （本层的 fd 不建模 `CLOEXEC`／`fork`／`exec`，`AB-` 面也没有继承语义 ⇒ 如实记缺口，不假装 ✓）。
fn dup2_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if args.len() > 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("dup2() takes at most 3 arguments ({} given)", args.len()),
        ));
    }
    let mut slots: [Option<NonNull<Header>>; 3] = [None; 3];
    const KEYWORDS: [&str; 3] = ["fd", "fd2", "inheritable"];
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            return Err(instance.raise_builtin_error("TypeError", "keywords must be strings"));
        };
        let Some(index) = KEYWORDS.iter().position(|candidate| *candidate == name) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("dup2() got an unexpected keyword argument '{name}'"),
            ));
        };
        if slots[index].is_some() || args.get(index).is_some() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("dup2() got multiple values for argument '{name}'"),
            ));
        }
        slots[index] = Some(*value);
    }
    let argument = |index: usize| slots[index].or_else(|| args.get(index).copied());
    let Some(fd_value) = argument(0) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "dup2() missing required argument 'fd' (pos 1)",
        ));
    };
    let Some(fd2_value) = argument(1) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "dup2() missing required argument 'fd2' (pos 2)",
        ));
    };
    let fd = descriptor_argument(instance, fd_value)?;
    let fd2 = descriptor_argument(instance, fd2_value)?;
    if let Some(inheritable) = argument(2) {
        // **接受但不生效**：本层不建模 `inherit` 位（如实记，不假装 ✓）。
        let _ = instance.truth_of(inheritable);
    }
    match instance.capability_dup2(fd, fd2) {
        Ok(duplicate) => Ok(instance.new_int(duplicate)),
        Err(error) => Err(capability_error(instance, error)),
    }
}

/// 整数实参（照参照：不是整数 ⇒ `'<类型>' object cannot be interpreted as an integer` ✓）。
fn descriptor_argument(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<i64, pyawa_core::ExecError> {
    if let Some(number) = instance.int_value(value) {
        return Ok(number);
    }
    let name = instance.type_name(instance.type_of(value));
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("'{name}' object cannot be interpreted as an integer"),
    ))
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

/// **`posix.stat(path)`** ✓（第 204 轮；task-19 补时间戳族）：`Lib/os.py:148` 的 `_set.add(stat)`
/// 要这个名字 ✓（它来自 `from posix import *` ✓）。**一处真相** ✓：值来自 `fs` 域的
/// `stat` 槽 ＋ **追加的** `stat_times` 槽 ✓。
///
/// **结果对象逐次独立** ✓（task-19 修既有 bug ✗）：先前把字段写到**共享类型**上（`set_type_attribute`）
/// ⇒ 第二次 `os.stat` 会覆盖第一次的 `st_size` 等 ✗（参照里每个结果各自独立 ✓）；现在写到**逐结果
/// 对象**的实例属性上，类型仍是同一个 `os.stat_result` ✓。
///
/// 时间戳（`st_atime`／`st_mtime`／`st_ctime` 是**浮点秒**、`*_ns` 是**整数纳秒**，与参照同口径）：
/// `stat_times` 槽**成功才补**；槽缺失／`NotImplemented`（如旧宿主、conformance harness 没提供该槽）
/// ⇒ **不补**，保持"如实缺名"（`CP-3`／`CP-5`）。
///
/// **如实说** ✗：`st_uid` 一族仍未接；`st_dev`／`st_ino` 仍来自 `FileInfo` 的既有值（provider 目前给
/// 0，属 `CP-23` 的保留面，不在本格）。
fn stat_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "stat() 要 1 个实参"));
    };
    let path = path_argument(instance, value, "stat")?;
    let info = match instance.fs_stat(&path.bytes) {
        Ok(info) => info,
        Err(error) => {
            return Err(fs_error_with_display(instance, error, &path.display));
        }
    };
    // **时间戳是追加槽**：取不到（旧宿主／槽未实现）就不补名 —— 不能把"读不到"编成 0 ✗。
    let times = instance.fs_stat_times(&path.bytes).ok();
    // **先分配结果对象，再往它自己的实例属性上写**（task-19 的共享类型 bug 修 ✗）。
    let result_type = stat_result_type(instance);
    let result = instance
        .alloc(AttributeObject::new(result_type, core::cell::RefCell::new(None)))
        .into_raw()
        .cast::<Header>();
    // `set_attribute_value` 是**借用语义**（它自己给属性表 `retain` 一份）⇒ 本函数那份要归还 ✓
    // （与 `_socket_module` 的 `set_socket_int` 同一手法 ✓）。
    for (field, number) in [
        ("st_mode", info.mode as i64),
        ("st_size", info.size as i64),
        ("st_dev", info.dev as i64),
        ("st_ino", info.ino as i64),
    ] {
        let object = instance.new_int(number);
        let outcome = instance.set_attribute_value(result, field, object);
        instance.release(object);
        let _ = outcome;
    }
    if let Some(times) = times {
        // `st_*time`：**浮点秒**（参照实测就是 `ns / 1e9` 同一个浮点值）。
        for (field, nanoseconds) in [
            ("st_atime", times.atime_ns),
            ("st_mtime", times.mtime_ns),
            ("st_ctime", times.ctime_ns),
        ] {
            let object = instance.new_float(nanoseconds as f64 / 1e9);
            let outcome = instance.set_attribute_value(result, field, object);
            instance.release(object);
            let _ = outcome;
        }
        // `st_*time_ns`：**整数纳秒**（`linecache` 一族按它与缓存比对）。
        for (field, nanoseconds) in [
            ("st_atime_ns", times.atime_ns),
            ("st_mtime_ns", times.mtime_ns),
            ("st_ctime_ns", times.ctime_ns),
        ] {
            let object = instance.new_int(nanoseconds);
            let outcome = instance.set_attribute_value(result, field, object);
            instance.release(object);
            let _ = outcome;
        }
    }
    Ok(result)
}

/// **`os.terminal_size` 的每实例唯一类型对象**（task-8）：与 [`stat_result_type`] 同一口径
/// （查本实例注册表 ⇒ 同实例内身份稳定、跨实例不串 ✓）。
fn terminal_size_type(instance: &Instance) -> NonNull<pyawa_core::TypeObject> {
    if let Some(existing) = instance.type_named("os.terminal_size") {
        return existing;
    }
    instance.new_attribute_type("os.terminal_size")
}

/// 取一个**序列**的元素（tuple 族／list 族；借用视图）。`terminal_size` 的实参用。
fn sequence_items(instance: &Instance, value: NonNull<Header>) -> Option<Vec<NonNull<Header>>> {
    instance
        .tuple_items(value)
        .or_else(|| instance.list_items(value))
}

/// **`os.terminal_size(sequence, /, **kwargs)`**（task-8；task-16 补 `sequence=` 关键字）：
/// `os.terminal_size` 的最小可用形态。
///
/// 动因 ✓：`Lib/shutil.py:1548`／`1554` 的 `get_terminal_size()` 在**没有终端**时用
/// `os.terminal_size(fallback)` 包回退值；`Lib/argparse.py:190` 读它的 `.columns` 定帮助宽度
/// ⇒ 缺这个类型，官方 `unittest.main()` 走 argparse 时就
/// `AttributeError: 'module' object has no attribute 'terminal_size'` ✗（P3-32 验收②的最后一格 ✓）。
///
/// **关键字口径** ✓（task-16，参照 3.14.4 实测；`MS-19`：合法 Python 不得被拒）：参照的
/// `structseq_new` 接受 `sequence=`（以及 `dict=` 这个名字）关键字 ⇒ 本层照做，不再"一律拒关键字"。
/// 位置与同名关键字同给、未知关键字、个数上限、缺参的**文案与优先级**都照实测（见函数内注释）。
///
/// **如实登记的偏差** ✗：参照里它是 **structseq**（`tuple` 子类，`len()`／下标／`== (80,24)`
/// 都成立）；本层用**属性命名空间**（`new_attribute_type` ＋ `AttributeObject`）⇒ 只有
/// `.columns`／`.lines` 两个点号属性，元组行为**未接线**（不阻塞 argparse／shutil 这条链 ✓；
/// 与 `os.stat_result` 的既有口径一致 ✓）。构造的序列只接 tuple 族／list 族（参照接任意序列，
/// 如 `'ab'` 按下标取字符——本条已登记，不在 task-16 的范围 ✓）。
fn terminal_size_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // **参数解析照参照 3.14.4 逐条实测**（task-16；优先级就是下面的次序）：
    // ① 个数上限：有位置实参 ⇒ 按「位置＋关键字」总数报 `takes at most 2 arguments (N given)`；
    //    纯关键字 ⇒ 按关键字个数报 `takes at most 2 keyword arguments (N given)`；
    // ② 位置 `sequence` 与关键字 `sequence=` 同给 ⇒
    //    `argument for structseq() given by name ('sequence') and position (1)`；
    //    （`dict` 同名冲突**不可达**：位置 ≥ 2 再加关键字总数已 > 2 ⇒ ① 先报。）
    // ③ 两个来源都没有 ⇒ `structseq() missing required argument 'sequence' (pos 1)`
    //    ——**先于**未知关键字判定（实测 `foo=1` ⇒ missing，不是 unexpected）；
    // ④ 未知关键字 ⇒ `structseq() got an unexpected keyword argument '<名>'`
    //    ——**先于**序列形状校验（实测 `(1,), foo=1` ⇒ unexpected，不是长度错）；
    // ⑤ 校验次序：序列类型 → 第二实参必须是 dict → dict 字段（terminal_size 没有未命名字段
    //    ⇒ **非空** dict 一律 `got duplicate or unexpected field name(s)`）→ 序列长度。
    let positional_sequence = args.first().copied();
    let positional_dict = args.get(1).copied();
    let keyword_of = |name: &str| {
        kwargs
            .iter()
            .find(|(key, _)| instance.text_of(*key) == Some(name))
            .map(|(_, value)| *value)
    };
    let keyword_sequence = keyword_of("sequence");
    let keyword_dict = keyword_of("dict");
    let total = args.len() + kwargs.len();
    if !args.is_empty() && total > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("structseq() takes at most 2 arguments ({total} given)"),
        ));
    }
    if args.is_empty() && kwargs.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "structseq() takes at most 2 keyword arguments ({} given)",
                kwargs.len()
            ),
        ));
    }
    if positional_sequence.is_some() && keyword_sequence.is_some() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "argument for structseq() given by name ('sequence') and position (1)",
        ));
    }
    let Some(sequence) = positional_sequence.or(keyword_sequence) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "structseq() missing required argument 'sequence' (pos 1)",
        ));
    };
    for (key, _) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        if name != "sequence" && name != "dict" {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("structseq() got an unexpected keyword argument '{name}'"),
            ));
        }
    }
    let Some(items) = sequence_items(instance, sequence) else {
        return Err(instance.raise_builtin_error("TypeError", "constructor requires a sequence"));
    };
    if let Some(dictionary) = positional_dict.or(keyword_dict) {
        let Some(entries) = instance.dict_entries(dictionary) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "os.terminal_size() takes a dict as second arg, if any",
            ));
        };
        // `terminal_size` 的两个字段都由序列填满、且没有未命名字段 ⇒ 任何条目都是多余/重复
        // （照参照实测：`{'columns': 1}`／`{1: 2}` 都是同一条文案）。
        if !entries.is_empty() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "os.terminal_size() got duplicate or unexpected field name(s)",
            ));
        }
    }
    if items.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "os.terminal_size() takes a 2-sequence ({}-sequence given)",
                items.len()
            ),
        ));
    }
    let result_type = terminal_size_type(instance);
    let result = instance
        .alloc(AttributeObject::new(result_type, core::cell::RefCell::new(None)))
        .into_raw()
        .cast::<Header>();
    // 属性挂在**实例**上（类型只有一个、值逐次不同）；`set_attribute_value` 自己 `retain` ✓
    for (field, item) in [("columns", items[0]), ("lines", items[1])] {
        instance.set_attribute_value(result, field, item)?;
    }
    Ok(result)
}

/// **`posix.get_terminal_size(fd=1)`**（task-8）：终端宽度探测的**查询口**。
///
/// **如实登记的偏差** ✗：本层没有 `ioctl`（`CX-4`：stdlib 不碰平台接口）⇒ **恒抛 `OSError`**，
/// 口径照参照"目标不是终端"那一档（`[Errno 25] Inappropriate ioctl for device`）——
/// `Lib/shutil.py:1544-1548` 正是**捕获 `OSError` 后回退**到 `fallback` ✓ ⇒ 这条链照常成立 ✓。
/// 参数校验照参照 3.14.4 实测：关键字 ⇒ `posix.get_terminal_size() takes no keyword arguments`
/// （参照的 `fd` 是**位置参数** ✓）；多于 1 个位置实参 ⇒ `get_terminal_size expected at most 1
/// argument, got N`；非整数 ⇒ `'<类型>' object cannot be interpreted as an integer`。
fn get_terminal_size_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "posix.get_terminal_size() takes no keyword arguments",
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("get_terminal_size expected at most 1 argument, got {}", args.len()),
        ));
    }
    if let Some(fd) = args.first() {
        if instance.int_value(*fd).is_none() {
            let name = instance.type_name(instance.type_of(*fd));
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("'{name}' object cannot be interpreted as an integer"),
            ));
        }
    }
    // 没有 ioctl：如实报"不是终端"（`shutil` 捕获 `OSError` 后回退 ✓）
    Err(instance.raise_builtin_error(
        "OSError",
        "[Errno 25] Inappropriate ioctl for device",
    ))
}

/// **`posix.getcwd()`** ✓（task-10）：`sysconfig` 一族的**第一道 import 闸** ✓ —— 它不是直接调
/// `os.getcwd()`，而是 `Lib/posixpath.py` 的 `realpath` 对**绝对路径**也会调 `os.getcwd()`
/// （`posixpath.py:381`／`426`；`sysconfig/__init__.py:219` 的 `_safe_realpath(sys.executable)` 经它 ✓）。
/// `Lib/test/support/os_helper.py:115` 的 `SAVEDCWD = os.getcwd()` 是**模块级**调用 ✓。
///
/// **实现路线（如实）** ✗：`fs` 能力域**没有** `getcwd` 槽 ⇒ 走 `std::env::current_dir()` 最小路径 ✓
/// （`std::env` 不在 `T-CX-4` 的禁止面里 ✓；同文件 `create_environ_native` 已有 `std::env::vars` 先例 ✓）。
/// **原则路线**（给 `CpFsVtable` 加 `getcwd` 槽、走能力域）面更大 ⇒ 留后续轮 ✓。
fn getcwd_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // 参照：`posix.getcwd() takes no arguments (1 given)`／`… takes no keyword arguments` ✓（实测）
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("posix.getcwd() takes no arguments ({} given)", args.len()),
        ));
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "posix.getcwd() takes no keyword arguments",
        ));
    }
    match current_dir_bytes() {
        // **PEP 383**：`getcwd()` = `fsdecode(getcwdb())` —— 原始字节按
        // utf-8＋surrogateescape 解成 WTF-8 str（含代理的目录名 ⇒ `'\udcff'` 一类）✓。
        Ok(bytes) => decode_path_text(instance, &bytes),
        Err(error) => Err(fs_error(
            instance,
            pyawa_core::CapabilityCallError::Machine(error.raw_os_error().unwrap_or(0)),
            "",
        )),
    }
}

/// **当前工作目录的原始字节**（P3-38 S3b-3）：`getcwdb` 的真值、`getcwd` 的输入。
///
/// `std::env::current_dir()` 给的 `PathBuf` 在 Unix 上可经 `OsStrExt` **原样取字节**
/// （安全视图；`T-CX-4` 的禁面是 `std::fs`／`std::net`／`libc`／`cfg(target_os)`，不含
/// `std::os::unix::ffi`——既有 `std::env` 先例同口径）。非 Unix 目标回退 lossy（如实偏差）。
fn current_dir_bytes() -> Result<Vec<u8>, std::io::Error> {
    let path = std::env::current_dir()?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(path.as_os_str().as_bytes().to_vec())
    }
    #[cfg(not(unix))]
    {
        Ok(path.to_string_lossy().as_bytes().to_vec())
    }
}

/// **`posix.getcwdb()`** ✓（task-10；P3-38 S3b-3 修字节口径）：返回 **`bytes`**（原始字节 ✓）。
fn getcwdb_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("posix.getcwdb() takes no arguments ({} given)", args.len()),
        ));
    }
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "posix.getcwdb() takes no keyword arguments",
        ));
    }
    match current_dir_bytes() {
        Ok(bytes) => Ok(instance.new_bytes(&bytes)),
        Err(error) => Err(fs_error(
            instance,
            pyawa_core::CapabilityCallError::Machine(error.raw_os_error().unwrap_or(0)),
            "",
        )),
    }
}

/// **`posix.readlink(path, *, dir_fd=None)`** ✓（task-10）：真值走 **`fs` 能力域**的 `readlink` 槽 ✓
/// （vtable 槽与 provider 本轮补齐 ✓）。
///
/// 参照口径（本机 `python3` 3.14.4 实测 ✓）：
/// * `path` 是 `str` ⇒ 结果 `str` ✓；`path` 是 `bytes` ⇒ 结果 `bytes` ✓；
/// * 报错按 errno 折成 `OSError` 子类 ＋ `[Errno N] …` 文案 ✓（缺路径 ⇒ `FileNotFoundError`／errno 2 ✓；
///   非符号链接 ⇒ `OSError`／errno 22 ✓）—— 走 [`fs_error`]（**一处真相**）；
/// * 无 `strict` 关键字（参照 3.14 实测签名就是 `(path, *, dir_fd=None)` ⇒ 传 `strict` 报
///   `unexpected keyword argument` ✓）；`dir_fd` 只接 `None`（非 `None` ⇒ 本层如实报未接线 ✓）。
fn readlink_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(value) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "readlink() missing required argument 'path' (pos 1)",
        ));
    };
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("readlink() takes exactly 1 positional argument ({} given)", args.len()),
        ));
    }
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            continue;
        };
        match name {
            // `dir_fd=None` 是参照签名的**关键字** ✓（绝对路径时参照也照收 ✓）。
            "dir_fd" => {
                if *value != instance.singletons().none() {
                    return Err(instance.raise_builtin_error(
                        "NotImplementedError",
                        "readlink：dir_fd 尚未接线（`CP-21` 的名字解析）",
                    ));
                }
            }
            other => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("readlink() got an unexpected keyword argument '{other}'"),
                ));
            }
        }
    }
    // **路径形状**决定结果形状 ✓（参照实测）：`str` ⇒ `str`、`bytes` ⇒ `bytes`。
    // P3-38 S3b-2：路径与**目标**都按 UTF-8＋surrogateescape 往返 ⇒ 非 UTF-8 字节不丢。
    let path = path_argument(instance, value, "readlink")?;
    match instance.fs_readlink(&path.bytes) {
        Ok(target) => {
            if path.want_bytes {
                Ok(instance.new_bytes(&target))
            } else {
                decode_path_text(instance, &target)
            }
        }
        Err(error) => Err(fs_error_with_display(instance, error, &path.display)),
    }
}

// ---- P3-38 S3b-2：目录列表／重命名／符号链接／access ---------------------------------------

/// **`posix.listdir(path='.')`**（P3-38 S3b-2）：真值走 `fs` 域的 `listdir` 槽 ✓。
///
/// **PEP 383**：str 路径 ⇒ 结果 str（不可解码字节按 `surrogateescape` 成 `'\\udcff'` 一类）；
/// bytes 路径 ⇒ 结果 bytes（名字原样）。合法 UTF-8 正常名字零变化 ✓。
fn listdir_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    for (key, _value) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("listdir() got an unexpected keyword argument '{name}'"),
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("listdir() takes at most 1 argument ({} given)", args.len()),
        ));
    }
    let path = match args.first().copied() {
        Some(value) => path_argument(instance, value, "listdir")?,
        None => PathArgument { bytes: b".".to_vec(), display: "'.'".to_owned(), want_bytes: false },
    };
    match instance.fs_listdir(&path.bytes) {
        Ok(names) => {
            let mut items: Vec<NonNull<Header>> = Vec::with_capacity(names.len());
            for raw in names {
                if path.want_bytes {
                    items.push(instance.new_bytes(&raw));
                } else {
                    items.push(decode_path_text(instance, &raw)?);
                }
            }
            Ok(instance.new_list(items))
        }
        Err(error) => Err(fs_error_with_display(instance, error, &path.display)),
    }
}

/// **`posix.rename(src, dst)`**（P3-38 S3b-2；`src_dir_fd`／`dst_dir_fd` 如实未接）。
fn rename_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        match name {
            "src_dir_fd" | "dst_dir_fd" if *value == instance.singletons().none() => continue,
            "src_dir_fd" | "dst_dir_fd" => {
                return Err(instance.raise_builtin_error(
                    "NotImplementedError",
                    &format!("rename：{name} 尚未接线（`CP-21` 的名字解析）"),
                ))
            }
            other => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("rename() got an unexpected keyword argument '{other}'"),
                ))
            }
        }
    }
    let (Some(source_value), Some(target_value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "rename() missing required argument 'src'/'dst'",
        ));
    };
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("rename() takes exactly 2 arguments ({} given)", args.len()),
        ));
    }
    let source = path_argument(instance, source_value, "rename")?;
    let target = path_argument(instance, target_value, "rename")?;
    match instance.fs_rename(&source.bytes, &target.bytes) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(fs_error_with_display(instance, error, &source.display)),
    }
}

/// **`posix.symlink(src, dst)`**（P3-38 S3b-2）：`src` 是**链接内容**（原样字节，不解析），
/// `dst` 是新建链接名（走 VM 解析器）。
fn symlink_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        let name = kwargs
            .first()
            .and_then(|(key, _)| instance.text_of(*key))
            .unwrap_or("?")
            .to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("symlink() got an unexpected keyword argument '{name}'"),
        ));
    }
    let (Some(target_value), Some(name_value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "symlink() missing required argument 'src'/'dst'",
        ));
    };
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("symlink() takes exactly 2 arguments ({} given)", args.len()),
        ));
    }
    // 目标按"路径形状"编码（str 走 surrogateescape、bytes 原样），但**不**做 VM 路径解析。
    let target = path_argument(instance, target_value, "symlink")?;
    let name = path_argument(instance, name_value, "symlink")?;
    match instance.fs_symlink(&target.bytes, &name.bytes) {
        Ok(()) => Ok(instance.retain(instance.singletons().none())),
        Err(error) => Err(fs_error_with_display(instance, error, &name.display)),
    }
}

/// **`posix.access(path, mode)`**（P3-38 S3b-2 的最小诚实面）：走 `fs` 域的 `stat` ✓。
///
/// **如实偏差** ✗：`CpFsVtable` 没有 `access(2)` 槽 ⇒ 按 `st_mode` 的权限位判断
/// （`R_OK`⇒`0o444`、`W_OK`⇒`0o222`、`X_OK`⇒`0o111` 任一位置位即真）——**不考虑**有效 uid／gid
/// 与 root 特例（参照的 `access()` 会）。`F_OK` 走 `stat` 是否成功（准确 ✓）。
fn access_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        let name = kwargs
            .first()
            .and_then(|(key, _)| instance.text_of(*key))
            .unwrap_or("?")
            .to_owned();
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("access() got an unexpected keyword argument '{name}'"),
        ));
    }
    let (Some(path_value), Some(mode_value)) = (args.first().copied(), args.get(1).copied()) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "access() missing required argument 'path'/'mode'",
        ));
    };
    let mode = instance.int_value(mode_value).unwrap_or(0);
    let path = path_argument(instance, path_value, "access")?;
    let allowed = match instance.fs_stat(&path.bytes) {
        Ok(info) => {
            let mut allowed = true;
            if mode & 0o4 != 0 {
                allowed &= info.mode & 0o444 != 0;
            }
            if mode & 0o2 != 0 {
                allowed &= info.mode & 0o222 != 0;
            }
            if mode & 0o1 != 0 {
                allowed &= info.mode & 0o111 != 0;
            }
            allowed
        }
        Err(_) => false,
    };
    Ok(instance.new_bool(allowed))
}


/// 本进程环境的**原始字节对**（P3-38 S3b-3）：`std::env::vars()` 在**非 UTF-8 环境值**上会
/// `unwrap` panic（实测：cwd 目录名含 `0xff` 时 shell 的 `PWD` 就是非 UTF-8 ⇒ 启动即 panic ✗）；
/// `vars_os()` ＋ `OsStrExt` 按字节原样取 ⇒ PEP 383 往返正确、正常 UTF-8 零变化 ✓。
fn environ_pairs() -> Vec<(Vec<u8>, Vec<u8>)> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::env::vars_os()
            .map(|(key, value)| (key.as_bytes().to_vec(), value.as_bytes().to_vec()))
            .collect()
    }
    #[cfg(not(unix))]
    {
        std::env::vars()
            .map(|(key, value)| (key.into_bytes(), value.into_bytes()))
            .collect()
    }
}

/// **`posix._create_environ()`** ✓（第 195 轮）：`Lib/os.py:68` 要它 ✓ —— 返回**环境变量字典** ✓。///
/// **一处真相** ✓：真值就是**本进程的环境** ✓（`std::env::vars` ✓）⇒ 本层不另造一份 ✓。
fn create_environ_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // **参照口径** ✓（第 412 轮，task-24 顺带 ✓）：`posix._create_environ()`／`posix.environ`
    // 都是 **bytes → bytes**（`os.py` 的 `_create_environ_mapping` 用 `decode = bytes.decode`
    // 把它们的值解成 str ✓；先前这里造的是 **str → str** ✗ ⇒ `os.environ` 一读就
    // `AttributeError: 'str' object has no attribute 'decode'` ✗（`os.environ` 整条面一直是坏的 ✓，
    // `wsgiref.handlers` 的类体 `os_environ = read_environ()` 正是撞在这 ✓））。
    let environment = instance.new_dict();
    for (key, value) in environ_pairs() {
        let key_object = instance.new_bytes(&key);
        let value_object = instance.new_bytes(&value);
        instance.dict_insert_raw(environment, key_object, value_object);
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
    let path = path_argument(instance, value, kind)?;
    let outcome = if directory {
        instance.fs_rmdir(&path.bytes)
    } else {
        instance.fs_unlink(&path.bytes)
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
        Err(error) => Err(fs_error_with_display(instance, error, &path.display)),
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

/// **`posix.sysconf(name)`** ✓（第 413 轮，task-26 顺带 ✓）：`Lib/asyncio/selector_events.py`
/// 的模块级 `SC_IOV_MAX = os.sysconf('SC_IOV_MAX')` 要它 ✗ —— 缺了它那边只 catch `OSError` ✗
/// ⇒ `AttributeError` 把 `import asyncio` 压住 ✓。
///
/// 名字**只认** `SC_IOV_MAX`（本层实际被读的唯一一个 ✓）：值照 Linux 参照实测 **1024** ✓；
/// 其余名字如实报未接线（`ValueError`）——**不编造** system 配置值 ✓。
fn sysconf_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let Some(name) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sysconf() takes exactly one argument (0 given)",
        ));
    };
    let name = instance.text_of(*name).map(str::to_owned)
        .or_else(|| instance.bytes_value(*name).and_then(|bytes| core::str::from_utf8(bytes).ok().map(str::to_owned)));
    match name.as_deref() {
        Some("SC_IOV_MAX") => Ok(instance.new_int(1024)),
        _ => Err(instance.raise_builtin_error(
            "ValueError",
            "sysconf: 未接线的配置名（本层只接线了 SC_IOV_MAX；如实报 ✓）",
        )),
    }
}

// ---- task-15：`subprocess.py` 导入期要的 `waitpid` 一族 ---------------------------------
//
// 动因（实测 ✓）：`subprocess.py:109-114` 的 `class _del_safe:` **类体**在导入期求值
// `os.waitpid`／`os.waitstatus_to_exitcode`／`os.WIFSTOPPED`／`os.WSTOPSIG`／`os.WNOHANG`
// ⇒ 缺一个就 `AttributeError`、`import subprocess` 直接断。`os.py:55` 的 `from posix import *`
// 把它们带进 `os` ✓（本文件的名字是**唯一真相点** ✓）。
//
// 真面：`WNOHANG` 是常量、`WIFSTOPPED`／`WSTOPSIG`／`waitstatus_to_exitcode` 是**纯宏逻辑**
// （值全部照参照实测 ✓，见各函数注释）；`waitpid(2)` 是真 syscall ⇒ 如实报未接线 ✓。

/// **`posix.WNOHANG`** ✓（参照本机实测 **1** ✓；`subprocess._del_safe.WNOHANG` 读它）。
const WNOHANG: i64 = 1;

/// 取**唯一一个** `status` 实参（`posix.WIFSTOPPED` 一族的 clinic 形态 ✓）：
/// 参照实测：无参/只给未知关键字 ⇒ `X() missing required argument 'status' (pos 1)`；
/// 位置＋关键字合计多于 1 ⇒ `X() takes at most 1 argument (N given)`；`status=` 关键字收 ✓。
fn status_argument(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<i64, pyawa_core::ExecError> {
    let total = args.len() + kwargs.len();
    if total == 0 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() missing required argument 'status' (pos 1)"),
        ));
    }
    if total > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes at most 1 argument ({total} given)"),
        ));
    }
    let object = match args.first() {
        Some(object) => *object,
        None => {
            let (key, value) = kwargs[0];
            if instance.text_of(key) != Some("status") {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("{name}() missing required argument 'status' (pos 1)"),
                ));
            }
            value
        }
    };
    instance.int_value(object).ok_or_else(|| {
        let type_name = instance.type_name(instance.type_of(object));
        instance.raise_builtin_error(
            "TypeError",
            &format!("'{type_name}' object cannot be interpreted as an integer"),
        )
    })
}

/// `posix.WIFSTOPPED(status)` ⇒ `(status & 0xff) == 0x7f`（参照 Linux 宏的同一口径 ✓）。
fn wifstopped_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let status = status_argument(instance, "WIFSTOPPED", args, kwargs)?;
    Ok(instance.new_bool((status & 0xff) == 0x7f))
}

/// `posix.WSTOPSIG(status)` ⇒ `(status >> 8) & 0xff`（参照 Linux 宏 ✓）。
fn wstopsig_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let status = status_argument(instance, "WSTOPSIG", args, kwargs)?;
    Ok(instance.new_int((status >> 8) & 0xff))
}

/// `posix.waitstatus_to_exitcode(status)` ⇒ 参照（`posixmodule.c`）的同一逻辑 ✓：
/// 正常退出 ⇒ 退出码；被信号杀 ⇒ 负信号号；stopped ⇒ `ValueError: process stopped by delivery
/// of signal N`；其余 ⇒ `ValueError: invalid wait status: N`（全部逐条照参照实测 ✓：
/// `0→0`／`256→1`／`512→2`／`6→-6`／`2→-2`／`127→ValueError(stopped by signal 0)`／
/// `32512→127`／`-1→ValueError(invalid wait status)` ✓）。
fn waitstatus_to_exitcode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let status = status_argument(instance, "waitstatus_to_exitcode", args, kwargs)?;
    let exited = (status & 0x7f) == 0;
    let stopped = (status & 0xff) == 0x7f;
    if exited {
        return Ok(instance.new_int((status >> 8) & 0xff));
    }
    if stopped {
        let signal = (status >> 8) & 0xff;
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("process stopped by delivery of signal {signal}"),
        ));
    }
    let terminated = status & 0x7f;
    if terminated != 0 && terminated != 0x7f {
        return Ok(instance.new_int(-terminated));
    }
    Err(instance.raise_builtin_error(
        "ValueError",
        &format!("invalid wait status: {status}"),
    ))
}

/// **`posix.waitpid(pid, options, /)`**（task-15）：`subprocess.py` 的 `_del_safe` 类体在导入期
/// **绑定**它 ✓；真 `waitpid(2)` 是 syscall 属能力域（`CM-8`）⇒ 元数/关键字校验过后**如实报未接线** ✓
/// （**禁止**返回假 pid／假 status）。
fn waitpid_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "posix.waitpid() takes no keyword arguments",
        ));
    }
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("waitpid expected 2 arguments, got {}", args.len()),
        ));
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`posix.waitpid`：真 waitpid(2) 属能力域（CM-8）问题，本轮未接线",
    ))
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
    // **`terminal_size`／`get_terminal_size`** ✓（task-8）：`Lib/shutil.py:1548` 与
    // `Lib/argparse.py:190` 的终端宽度探测（`os.py:55` 的 `from posix import *` 靠 `__all__` 带过去 ✓）。
    let terminal_size = crate::builtins_module::make_native(
        instance,
        "terminal_size",
        terminal_size_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "terminal_size", terminal_size);
    let get_terminal_size = crate::builtins_module::make_native(
        instance,
        "get_terminal_size",
        get_terminal_size_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "get_terminal_size", get_terminal_size);
    for (name, handler) in [
        ("open", open_native as pyawa_core::NativeFn),
        ("close", close_native as pyawa_core::NativeFn),
        // **`T-CP-3` 的 `fd` 面**（round 11）：`os.dup`／`os.dup2` 走实例能力表（`CP-31` 的表项复制 ✓）。
        ("dup", dup_native as pyawa_core::NativeFn),
        ("dup2", dup2_native as pyawa_core::NativeFn),
        ("read", read_native as pyawa_core::NativeFn),
        ("write", write_native as pyawa_core::NativeFn),
        ("unlink", unlink_native as pyawa_core::NativeFn),
        // **`remove` 是 `unlink` 的别名** ✓（task-1 第 2 格）：参照 `posix` 里两者就是同一条
        // `unlink(2)` 包装 ✓ —— `Lib/importlib/resources/_common.py:128` 的
        // `@contextlib.contextmanager def _tempfile(..., _os_remove=os.remove, ...)` 在**导入期**
        // 求值这个仅关键字默认值 ✓ ⇒ 缺名就 `AttributeError: 'module' object has no attribute 'remove'`
        // （实测：`hasattr(os, "remove")` False、`unlink` True ✓）。
        ("remove", unlink_native as pyawa_core::NativeFn),
        ("rmdir", rmdir_native as pyawa_core::NativeFn),
        ("urandom", urandom_native as pyawa_core::NativeFn),
        ("getpid", getpid_native as pyawa_core::NativeFn),
        // **task-10**：`sysconfig` 族的 import 闸（`getcwd` 经 `posixpath.realpath` 隐式要 ✓）＋
        // `test.support` 链的模块级 `SAVEDCWD = os.getcwd()` ✓；`readlink` 同理（`realpath` ✓）。
        ("getcwd", getcwd_native as pyawa_core::NativeFn),
        ("getcwdb", getcwdb_native as pyawa_core::NativeFn),
        ("readlink", readlink_native as pyawa_core::NativeFn),
        // **P3-38 S3b-2**：路径面（真文件、含非 UTF-8 文件名）✓
        ("listdir", listdir_native as pyawa_core::NativeFn),
        ("rename", rename_native as pyawa_core::NativeFn),
        ("symlink", symlink_native as pyawa_core::NativeFn),
        ("access", access_native as pyawa_core::NativeFn),
        // **task-15**：`subprocess.py` 的 `_del_safe` 类体在导入期**绑定**这一族 ✓
        // （`waitpid` 调用期如实报未接线；另外三个是纯宏逻辑真实现 ✓）。
        ("waitpid", waitpid_native as pyawa_core::NativeFn),
        ("waitstatus_to_exitcode", waitstatus_to_exitcode_native as pyawa_core::NativeFn),
        ("WIFSTOPPED", wifstopped_native as pyawa_core::NativeFn),
        ("WSTOPSIG", wstopsig_native as pyawa_core::NativeFn),
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
    // **等待状态常量**（task-15 ✓）：`subprocess.py` 的 `_del_safe.WNOHANG` 导入期读 ✓。
    instance.dict_set(namespace, "WNOHANG", instance.new_int(WNOHANG));
    // **`__all__` 跟着函数面走** ✓：`os.py:55` 的 `from posix import *` 读的就是它 ✓
    //（之前是**空表** ✗ —— 如实记了"函数面未落地" ✓ ⇒ 现在有一条就列一条 ✓）。
    let exports = instance.new_list(vec![
        instance.new_str("stat"),
        instance.new_str("lstat"),
        instance.new_str("stat_result"),
        // **task-8**：`shutil.get_terminal_size()` 的底座（`os.py:55` 的 `from posix import *`）✓
        instance.new_str("terminal_size"),
        instance.new_str("get_terminal_size"),
        instance.new_str("open"),
        instance.new_str("close"),
        instance.new_str("dup"),
        instance.new_str("dup2"),
        instance.new_str("read"),
        instance.new_str("write"),
        instance.new_str("unlink"),
        instance.new_str("remove"),
        instance.new_str("rmdir"),
        instance.new_str("urandom"),
        instance.new_str("getpid"),
        instance.new_str("getcwd"),
        instance.new_str("getcwdb"),
        instance.new_str("readlink"),
        instance.new_str("listdir"),
        instance.new_str("rename"),
        instance.new_str("symlink"),
        instance.new_str("access"),
        instance.new_str("environ"),
        instance.new_str("cpu_count"),
        // **task-26 顺带**：`asyncio/selector_events.py` 的模块级 `os.sysconf('SC_IOV_MAX')`
        // 走 `os.py:55` 的 `from posix import *` ⇒ 名字必须列进 `__all__` ✓（本层 `posix.__all__`
        // 是"函数面清单" ✓，漏一条就进不了 `os` ✓）。
        instance.new_str("sysconf"),
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
        // **task-15**：`subprocess.py` 导入期要的 `waitpid` 一族（`os.py:55` 的 `from posix import *`）✓
        instance.new_str("waitpid"),
        instance.new_str("waitstatus_to_exitcode"),
        instance.new_str("WIFSTOPPED"),
        instance.new_str("WSTOPSIG"),
        instance.new_str("WNOHANG"),
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
    // **同 `_create_environ`：bytes → bytes** ✓（`os.environb` 的底子 ✓；`os.environ` 由它解码 ✓）。
    let environ = instance.new_dict();
    for (key, value) in environ_pairs() {
        let key_object = instance.new_bytes(&key);
        let value_object = instance.new_bytes(&value);
        instance.dict_insert_raw(environ, key_object, value_object);
    }
    instance.dict_set(namespace, "environ", environ);
    let cpu_count =
        crate::builtins_module::make_native(instance, "cpu_count", cpu_count_native as pyawa_core::NativeFn);
    instance.dict_set(namespace, "cpu_count", cpu_count);
    let sysconf = crate::builtins_module::make_native(
        instance,
        "sysconf",
        sysconf_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "sysconf", sysconf);
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
    // **`__all__` 只有一份** ✓（task-10 修 ✗）：这里原先还有一段"现阶段为空"的**陈旧覆盖** ——
    // 它把上面那份按函数面写的 `__all__` **整个冲掉** ✗（实测 `posix.__all__` 一直是 `[]` ✓）。
    // `from posix import *` 在本层**不看 `__all__`**（按"非下划线名字"导 ✓）所以一直没露出来，
    // 但 `os.__all__.extend(_get_exports_list(posix))` 读的是它 ⇒ **一处真相**：留上面那份 ✓。
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
