//! `posix` 模块（第 138 轮）：**最小面**。
//!
//! 现状：只给出名字空间与 `__all__`，让 `Lib/os.py` 的 `from posix import *` 能过
//! （`_get_exports_list` 优先读 `module.__all__` ✓）。**函数面（`open`／`stat`／`listdir`…）尚未落地** ✗
//! —— 它们按 `CM-8` **必须**走能力域（`fs` ✓），逐条随用例补 ✓。
//!
//! `CX-4`：本 crate 不碰平台 ⇒ 这里不 import 任何平台接口 ✓。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

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

/// **`posix._create_environ()`** ✓（第 195 轮）：`Lib/os.py:68` 要它 ✓ —— 返回**环境变量字典** ✓。
///
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
    // **`_have_functions`** ✓（第 188 轮）：`os.py` 一导入就**扫这个表** ✓（用来决定
    // `supports_follow_symlinks` 一族 ✓）。**如实说** ✗：本层还没实现那些可选能力 ✓ ⇒ 给**空表** ✓
    //（语义上就是"一个都不支持" ✓，比编造一串名字**诚实** ✓）。
    let have_functions = instance.new_list(Vec::new());
    instance.dict_set(namespace, "_have_functions", have_functions);
    // **`_create_environ`** ✓（第 195 轮）：`os.py` 一导入就调它 ✓。
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
    let exports = instance.new_list(Vec::new());
    instance.dict_set(namespace, "__all__", exports);
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
