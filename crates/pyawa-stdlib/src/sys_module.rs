//! `sys` 模块（**不依赖能力域**的那部分；契约 `docs/SPEC-c-modules.md` §5.2.3）。
//!
//! 要点：
//!
//! - **`CX-13`**：`implementation.name` **必须**报 `pyawa`——谎报 `cpython` 会让库去加载
//!   **不存在**的 C 扩展，而库自带的纯 Python 回退路径才是"生态可用"能成立的原因
//! - **语言版本 vs 实现版本**（`DESIGN.md` §9 的"实现观测面"）：`version_info`／`hexversion`
//!   报**语言级别**（对拍参照 3.14.4 ⇒ `(3, 14, 4, 'final', 0)`），供库做特性检测；
//!   身份由 `implementation` 承载（`name`／`cache_tag`／`version`）
//! - 本段只做**不依赖能力域、输出通道与 import 机制**的属性；`stdout` 一族（`_io` ⇒ `fs` 域）、
//!   路径一族（`prefix`／`executable`）、importlib 一族（`meta_path`）与绑定未定实现参数的
//!   `*_info`（哈希／整数表示）都不在其中
//!
//! 本模块**不碰**平台（`CX-4`：stdlib 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）：
//! 字节序用 `cfg!(target_endian)`，不调任何平台接口。

use core::ptr::NonNull;

use pyawa_core::{AttributeObject, ExecError, Header, Instance};

/// 按**真实入口**改写 `sys.argv`（`["<程序名>", <参数>…]` ✓；组合根调用 ✓）。
pub fn set_argv(
    instance: &Instance,
    namespace: NonNull<Header>,
    program: &str,
    arguments: &[String],
) {
    let mut items: Vec<NonNull<Header>> = Vec::with_capacity(arguments.len() + 1);
    items.push(instance.new_str(program));
    for argument in arguments {
        items.push(instance.new_str(argument));
    }
    let argv = instance.new_list(items);
    instance.dict_set(namespace, "argv", argv);
}

/// 按**真实入口**改写 `sys.path`（`site.py`（`IM-24`）未接之前，组合根把脚本所在目录放进去 ✓ ——
/// 与参照实现的 `sys.path[0]` 同义 ✓）。
pub fn set_path(instance: &Instance, namespace: NonNull<Header>, directories: &[String]) {
    let items: Vec<NonNull<Header>> = directories
        .iter()
        .map(|directory| instance.new_str(directory))
        .collect();
    let path = instance.new_list(items);
    instance.dict_set(namespace, "path", path);
}

/// 模块名（`sys`）。
pub const NAME: &str = "sys";

/// 模块的 `__doc__`（与参照实现同源的一句话）。
pub const DOC: &str = "This module provides access to some objects used or maintained by the\ninterpreter and to functions that interact strongly with the interpreter.";

/// **本实现所实现的语言级别**：`(主, 次, 微, 发布级, 序号)`。
///
/// 对拍参照是 3.14.4（`REQUIREMENTS.md`）⇒ `version_info`／`hexversion` 报它，
/// **不是**报本实现自己的版本号——库用 `sys.version_info >= (3, 11)` 一类做特性检测。
pub const LANGUAGE_VERSION: (u32, u32, u32, &str, u32) = (3, 14, 4, "final", 0);

/// **本实现自己的**版本：工作区版本（`Cargo.toml`），现为 `0.0.0`（未发布）。
pub const IMPLEMENTATION_VERSION: (u32, u32, u32, &str, u32) = (0, 0, 0, "alpha", 0);

/// **`CX-13`**：`implementation.name`——**必须**报这个名字。
pub const IMPLEMENTATION_NAME: &str = "pyawa";

/// `implementation.cache_tag`：**自己的值**（`CX-13`）＝ `pyawa-<指令集版本>`（`BC-29`）。
pub fn cache_tag() -> String {
    format!(
        "{IMPLEMENTATION_NAME}-{}",
        pyawa_core::opcode_metadata::INSTRUCTION_SET_VERSION
    )
}

/// `sys.version`：**构建串**，含 `pyawa` 与本实现自己的版本（**禁止**伪装成 CPython 的构建串）。
pub fn version_string() -> String {
    let (major, minor, micro, _, _) = LANGUAGE_VERSION;
    let (impl_major, impl_minor, impl_micro, _, _) = IMPLEMENTATION_VERSION;
    format!("{major}.{minor}.{micro} ({IMPLEMENTATION_NAME} {impl_major}.{impl_minor}.{impl_micro})")
}

/// 发布级 → `hexversion` 里的那一段（照参照实现的编码：`alpha` `0xA`、`beta` `0xB`、
/// `candidate` `0xC`、`final` `0xF`）。
fn release_level_code(release_level: &str) -> Option<i64> {
    match release_level {
        "alpha" => Some(0xA),
        "beta" => Some(0xB),
        "candidate" => Some(0xC),
        "final" => Some(0xF),
        _ => None,
    }
}

/// `hexversion`：`主 << 24 ｜ 次 << 16 ｜ 微 << 8 ｜ 发布级 << 4 ｜ 序号`（与参照同式）。
pub fn hexversion(version: (u32, u32, u32, &str, u32)) -> Option<i64> {
    let (major, minor, micro, release_level, serial) = version;
    let level = release_level_code(release_level)?;
    Some(
        ((major as i64) << 24)
            | ((minor as i64) << 16)
            | ((micro as i64) << 8)
            | (level << 4)
            | (serial as i64),
    )
}

/// 本机字节序（`byteorder` 的值）：与参照同源——都取宿主端序，不硬编码 `'little'`。
pub const fn byteorder() -> &'static str {
    if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    }
}

/// 造一个原生可调用对象（**新引用**；与 `builtins_module` 同一做法）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        // `TypeObject::name` 要 `&'static str`：本模块的函数名是常量，泄漏一份即可
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// `sys.intern(str)`（第 335 轮）。
///
/// **如实登记的偏差** ✗：本层**没有驻留池**（intern 表）——直接返回**同一个实参对象** ✓
/// （不去重、不新建 ✓）。对"把 `Lib/` 跑起来"这一步够用 ✓（调用点几乎都是 `intern` 一个刚生成的
/// 字面量 ✓），但参照保证的 `sys.intern(a) is sys.intern(b)` 同一性本层**不保证** ✗。
fn intern_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(text) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "intern() takes exactly one argument (0 given)",
        ));
    };
    if instance.text_of(*text).is_none() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "intern() argument must be str, not something else",
        ));
    }
    Ok(instance.retain(*text))
}

/// `sys.getrefcount(obj)`（`OM-22`：**真实计数加一**——借用参数的那一份）。
///
/// 三种用法的消息**逐条实测**：0／2 个实参 ⇒ `sys.getrefcount() takes exactly one argument
/// (0 given)`；带关键字 ⇒ `sys.getrefcount() takes no keyword arguments`（**关键字先判**，
/// 与参照一致）。返回的数字是本实现的**真实计数**（`§13-5`：不实现 immortal）⇒ 具体数字
/// **不进**严格对照（`MS-18`／差异清单口径），只保证"计数加一"这条语义。
fn getrefcount_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sys.getrefcount() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "sys.getrefcount() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    // `OM-22`：计数加一（借用参数的那一份）；读计数走核心的**安全**访问器
    let count = i64::from(instance.refcount_of(args[0]));
    Ok(instance.new_int(count + 1))
}

/// `sys.get_int_max_str_digits()`（`TS-45` ①）：当前的 `int`↔`str` 位数上限（`0` ＝ 不限）。
///
/// 消息照参照**实测**：这一条带 `sys.` 前缀、且**不收实参**
/// （`sys.get_int_max_str_digits() takes no arguments (1 given)`）。
fn get_int_max_str_digits_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() || !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "sys.get_int_max_str_digits() takes no arguments ({} given)",
                args.len()
            ),
        ));
    }
    Ok(instance.new_int(i64::from(instance.int_max_str_digits())))
}

/// `sys.set_int_max_str_digits(maxdigits)`（`TS-45` ①）：`0`（不限）或 `>= 640`。
///
/// 四种非法形态与参数个数都照参照**实测**的消息（`tools/gen_int_fixture.py` 的 `sys_limits`）：
/// 少给／多给实参、给的不是整数、取值落在 `(0, 640)`、以及超出 `u32` 的取值。
fn set_int_max_str_digits_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "set_int_max_str_digits() takes no keyword arguments",
        ));
    }
    let only = match args {
        [] => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "set_int_max_str_digits() missing required argument 'maxdigits' (pos 1)",
            ))
        }
        [only] => only,
        args => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "set_int_max_str_digits() takes at most 1 argument ({} given)",
                    args.len()
                ),
            ))
        }
    };
    let Some(payload) = instance.int_of(*only) else {
        let name = instance.type_name(instance.type_of(*only));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object cannot be interpreted as an integer"),
        ));
    };
    let Some(value) = payload.to_i64().filter(|value| *value <= i64::from(u32::MAX)) else {
        return Err(instance.raise_builtin_error(
            "OverflowError",
            "Python int too large to convert to C int",
        ));
    };
    if value != 0 && value < i64::from(pyawa_core::INT_MAX_STR_DIGITS_THRESHOLD) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "maxdigits must be >= 640 or 0 for unlimited",
        ));
    }
    instance.set_int_max_str_digits(value as u32);
    // 返回 `None`（新引用）；stdlib **禁止 unsafe** ⇒ 走核心的安全入口 `retain`
    Ok(instance.retain(instance.singletons().none()))
}

/// 建 `sys` 模块的命名空间（**新引用** 的 `dict`）。
/// **编码名**（第 265 轮）：`sys.getfilesystemencoding()`／`sys.getdefaultencoding()` ✓。
fn encoding_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    // `getfilesystemencodeerrors` 与两个"编码名"共用一个原生 ✓ —— 用**函数名**区分值 ✓
    //（值取参照实测：`utf-8`／`surrogateescape` ✓）。
    let encode_errors = bound
        .and_then(|object| instance.text_of(object))
        .is_some_and(|name| name == "getfilesystemencodeerrors");
    Ok(instance.new_str(if encode_errors {
        "surrogateescape"
    } else {
        "utf-8"
    }))
}

pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();

    // `argv`：启动参数列表。REPL／嵌入式默认 `[""]`（`-c` 入口由 CLI 改写，§5.2.3）
    let empty = instance.new_str("");
    let argv = instance.new_list(vec![empty]);
    instance.dict_set(namespace, "argv", argv);
    // `path`：由 `site.py` 构建（`IM-24`）——本层只暴露这个列表，不另立路径逻辑
    let path = instance.new_list(Vec::new());
    instance.dict_set(namespace, "path", path);
    // `modules`：import 系统的模块表；import 未接之前只保证这个键存在
    let modules = instance.new_dict();
    instance.dict_set(namespace, "modules", modules);
    // **`builtin_module_names`**（第 138 轮）：`Lib/os.py` 靠 `'posix' in sys.builtin_module_names`
    // 选平台分支 ✓ ⇒ 报我们**真正内建**的那些名字 ✓（如实 ✓；CPython 这里是元组 ✓）。
    // **第 280 轮据实修正** ✗：先前写的是 `imp`（3.14 已移除 ✗）且漏了 `_io`／`_warnings`／`_weakref`／
    // `_thread` ✓ —— 而 `_bootstrap._setup` 正是按这张表给模块建 spec 的 ✓ ⇒ 表错一行，import 链就断 ✓。
    // **由模块表派生** ✓（第 403 轮，一处真相 ✓）：先前这张表是手写的 ✗ ⇒ 与 `lib.rs` 的模块表
    // 各自漂移 ✗（新加 `_ast` 时漏同步 ✓ ⇒ 上游按旧表建 spec ⇒ 上限反而 -1 ✗）。
    let builtin_names: Vec<NonNull<Header>> = crate::builtin_module_names()
        .iter()
        .map(|name| instance.new_str(name))
        .collect();
    let builtin_names = instance.new_tuple(builtin_names);
    instance.dict_set(namespace, "builtin_module_names", builtin_names);
    // **`warnoptions`**（第 403 轮）：参照里是"命令行 `-W` 选项"的**列表** ✓（无 `-W` ⇒ 空表 ✓）。
    // 缺它时 `import warnings` 一族在**模块级**就 `AttributeError: module 没有 warnoptions` ✗
    // （实测那一族 **7** 个模块：`warnings`／`codeop`／`sre_compile`／`sre_constants`／`sre_parse`／
    // `_pyrepl.readline`／`nturl2path` ✓）。
    let warnoptions = instance.new_list(Vec::new());
    instance.dict_set(namespace, "warnoptions", warnoptions);
    // **`stdout`／`stderr`**：`_io` 的文本流对象（`CM-26`：`print` 的目的地就是**这两个对象** ✓，
    // 字节经 `_io` 的文本层走 `fs` 域的 `write` ✓；本层不碰平台 ✓ `CX-4`）
    let stdout = crate::_io_module::make_stream(instance, crate::_io_module::STDOUT_HANDLE);
    instance.dict_set(namespace, "stdout", stdout);
    let stderr = crate::_io_module::make_stream(instance, crate::_io_module::STDERR_HANDLE);
    instance.dict_set(namespace, "stderr", stderr);

    // **`_getframe`** ✓（第 230 轮）：`_collections_abc.py:89` 的 `sys._getframe().f_locals` 要它 ✓。
    let getframe = crate::builtins_module::make_native(
        instance,
        "_getframe",
        pyawa_core::getframe_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "_getframe", getframe);
    // **`sys.getfilesystemencoding()`／`getdefaultencoding()`** ✓（第 265 轮）：`Lib/os.py` 的
    // `_create_environ_mapping()` 调前者 ✓ ⇒ 缺了它 `import os` 就停在那儿 ✗（实测 ✓）。
    // 值取**参照在本机的实测值** ✓（`utf-8` ✓）——**如实记** ✗：本层还没有"按平台查编码"的能力 ✓。
    for name in ["getfilesystemencoding", "getdefaultencoding", "getfilesystemencodeerrors", "getfilesystemencoding"] {
        let native =
            crate::builtins_module::make_native(instance, name, encoding_native as pyawa_core::NativeFn);
        instance.dict_set(namespace, name, native);
    }

    // 语言版本（供特性检测）
    let (major, minor, micro, release_level, serial) = LANGUAGE_VERSION;
    let version_info = instance.new_tuple(vec![
        instance.new_int(i64::from(major)),
        instance.new_int(i64::from(minor)),
        instance.new_int(i64::from(micro)),
        instance.new_str(release_level),
        instance.new_int(i64::from(serial)),
    ]);
    instance.dict_set(namespace, "version_info", version_info);
    if let Some(hexversion) = hexversion(LANGUAGE_VERSION) {
        let encoded = instance.new_int(hexversion);
        instance.dict_set(namespace, "hexversion", encoded);
    }

    // 构建串：必须含 `pyawa`
    let version = instance.new_str(&version_string());
    instance.dict_set(namespace, "version", version);
    // **前缀一族** ✓（第 196 轮）：`Lib/site.py` 一导入就用 `sys.prefix` ✓。
    // **真值来源** ✓：与参照**同一路数** —— 由**可执行文件的位置**推 ✓（`current_exe` 的父目录 ✓）；
    // 推不出来就退到 `"."` ✓（**如实**：本层还没有"安装前缀"这个概念 ✓）。
    let prefix_text = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.to_string_lossy().into_owned()))
        .unwrap_or_else(|| ".".to_owned());
    for name in ["prefix", "exec_prefix", "base_prefix", "base_exec_prefix"] {
        let value = instance.new_str(&prefix_text);
        instance.dict_set(namespace, name, value);
    }
    // **`platlibdir`** ✓（参照给 `lib` ✓）、**`platform`** ✓（本层只在 linux 上跑 ✓）、
    // **`executable`** ✓（就是本进程的可执行文件 ✓ —— 与 `prefix` **同一处真相** ✓）。
    for (name, value) in [
        ("platlibdir", "lib"),
        ("platform", "linux"),
        ("executable", &prefix_text),
    ] {
        let text = instance.new_str(value);
        instance.dict_set(namespace, name, text);
    }

    // **`sys.flags`** ✓（第 196 轮）：`Lib/site.py` 读 `verbose`／`no_user_site`／`ignore_environment` ✓。
    // **如实自报** ✓：本层**没有**命令行开关解析（CLI 只收脚本路径 ✓）⇒ 这些位**一律 0** ✓ ——
    // 这与参照"不带任何开关"时的取值**完全一致** ✓。
    let flags_type = instance.new_attribute_type("sys.flags");
    // **全集** ✓（第 196 轮）：参照的 `sys.flags` 字段名逐个照抄 ✓ —— 值与"不带开关"一致 ✓；
    // **两处例外**是**诚实的真值** ✓：`hash_randomization`（3.4 起默认开 ✓）与 `int_max_str_digits`（默认 4300 ✓）。
    let flag_fields: [(&str, i64); 21] = [
        ("debug", 0),
        ("inspect", 0),
        ("interactive", 0),
        ("optimize", 0),
        ("dont_write_bytecode", 0),
        ("no_user_site", 0),
        ("no_site", 0),
        ("ignore_environment", 0),
        ("verbose", 0),
        ("bytes_warning", 0),
        ("quiet", 0),
        ("hash_randomization", 1),
        ("isolated", 0),
        ("dev_mode", 0),
        ("utf8_mode", 0),
        ("warn_default_encoding", 0),
        ("int_max_str_digits", 4300),
        ("safe_path", 0),
        ("is_venv", 0),
        ("thread_inherit_context", 0),
        ("context_aware_warnings", 0),
    ];
    for (field, value) in flag_fields {
        let number = instance.new_int(value);
        instance.set_type_attribute(flags_type, field, number);
    }
    let flags = instance.alloc(AttributeObject::new(flags_type, core::cell::RefCell::new(None)));
    instance.dict_set(namespace, "flags", flags.into_raw().cast::<Header>());

    // 与实现无关的常量
    let maxunicode = instance.new_int(0x10FFFF);
    instance.dict_set(namespace, "maxunicode", maxunicode);
    let maxsize = instance.new_int(isize::MAX as i64);
    instance.dict_set(namespace, "maxsize", maxsize);
    // **`sys.platform`** ✓（第 194 轮：`Lib/importlib/_bootstrap_external.py` 要它 ✓）。
    // **分层说明** ✓：本 crate 不碰平台（`CX-4` ✓）⇒ 这里是**常量** ✓；真值应由**平台集中点**
    // （`pyawa-runtime` ✓ 见 `DESIGN.md` §7）注入 ✓ —— 已登记 ✓。
    let platform = instance.new_str(crate::PLATFORM);
    instance.dict_set(namespace, "platform", platform);
    let byteorder = instance.new_str(byteorder());
    instance.dict_set(namespace, "byteorder", byteorder);

    // `implementation`：点号可访问的命名空间（`CX-13` 的载体）。
    // 用核心**安全**的公开面搭：`new_attribute_type` ＋ `set_type_attribute`（不碰 unsafe）。
    let namespace_type = instance.new_attribute_type("sys.implementation");
    let name = instance.new_str(IMPLEMENTATION_NAME);
    instance.set_type_attribute(namespace_type, "name", name);
    let tag = instance.new_str(&cache_tag());
    instance.set_type_attribute(namespace_type, "cache_tag", tag);
    let (impl_major, impl_minor, impl_micro, impl_level, impl_serial) = IMPLEMENTATION_VERSION;
    let impl_version = instance.new_tuple(vec![
        instance.new_int(i64::from(impl_major)),
        instance.new_int(i64::from(impl_minor)),
        instance.new_int(i64::from(impl_micro)),
        instance.new_str(impl_level),
        instance.new_int(i64::from(impl_serial)),
    ]);
    instance.set_type_attribute(namespace_type, "version", impl_version);
    let implementation =
        instance.alloc(AttributeObject::new(namespace_type, core::cell::RefCell::new(None)));
    instance.dict_set(
        namespace,
        "implementation",
        implementation.into_raw().cast::<Header>(),
    );

    // `float_info`／`int_info`（§5.2.3 的第 216 轮口径；载体与 `implementation` 同一种：
    // **属性命名空间**——点号可访问，structseq 的元组行为（下标／len／迭代／repr）未接线）。
    //
    // `float_info`：我们与参照**同一个物**（IEEE-754 `f64`）⇒ 逐字段值必须相同（用 std 常量）。
    let float_info_type = instance.new_attribute_type("sys.float_info");
    let float_fields: [(&str, f64); 3] = [
        ("epsilon", f64::EPSILON),
        ("max", f64::MAX),
        ("min", f64::MIN_POSITIVE),
    ];
    for (field, value) in float_fields {
        let number = instance.new_float(value);
        instance.set_type_attribute(float_info_type, field, number);
    }
    // 其余 11 个字段是**整数**（`radix`／`rounds` 与 structseq 的元数据）
    let float_int_fields: [(&str, i64); 11] = [
        ("dig", f64::DIGITS as i64),
        ("mant_dig", f64::MANTISSA_DIGITS as i64),
        ("max_10_exp", f64::MAX_10_EXP as i64),
        ("max_exp", f64::MAX_EXP as i64),
        ("min_10_exp", f64::MIN_10_EXP as i64),
        ("min_exp", f64::MIN_EXP as i64),
        ("radix", f64::RADIX as i64),
        // 参照实测：`rounds == 1`（就近偶数舍入；Rust 侧没有对应的 std 常量）
        ("rounds", 1),
        ("n_fields", 11),
        ("n_sequence_fields", 11),
        ("n_unnamed_fields", 0),
    ];
    for (field, value) in float_int_fields {
        let number = instance.new_int(value);
        instance.set_type_attribute(float_info_type, field, number);
    }
    let float_info =
        instance.alloc(AttributeObject::new(float_info_type, core::cell::RefCell::new(None)));
    instance.dict_set(namespace, "float_info", float_info.into_raw().cast::<Header>());

    // `int_info`：`bits_per_digit`／`sizeof_digit` 是**实现观测面** ⇒ **如实自报**我们的表示
    // （`bigint.rs`：`limbs` 是 **2^32 进制的小端** `Vec<u32>` ⇒ 每"位"32 bit、4 字节），
    // **禁止**照抄参照的 30／4；两个位数上限则**必须**与参照一致（复用核心那两个常量）。
    let int_info_type = instance.new_attribute_type("sys.int_info");
    let int_info_fields: [(&str, i64); 4] = [
        ("bits_per_digit", u32::BITS as i64),
        ("sizeof_digit", core::mem::size_of::<u32>() as i64),
        (
            "default_max_str_digits",
            i64::from(pyawa_core::INT_MAX_STR_DIGITS_DEFAULT),
        ),
        (
            "str_digits_check_threshold",
            i64::from(pyawa_core::INT_MAX_STR_DIGITS_THRESHOLD),
        ),
    ];
    for (field, value) in int_info_fields {
        let number = instance.new_int(value);
        instance.set_type_attribute(int_info_type, field, number);
    }
    let int_info =
        instance.alloc(AttributeObject::new(int_info_type, core::cell::RefCell::new(None)));
    instance.dict_set(namespace, "int_info", int_info.into_raw().cast::<Header>());

    // `getrefcount`（`OM-22`）
    // **`sys.intern(str)`**（第 335 轮）：上限榜上 `AttributeError: 'module' object has no attribute
    // 'intern'` × 72 个模块就卡这一条 ✓（`Lib/` 里大量 `sys.intern(...)` 用在名字表上 ✓）。
    let intern = make_native(instance, "intern", intern_native);
    instance.dict_set(namespace, "intern", intern);
    let getrefcount = make_native(instance, "getrefcount", getrefcount_native);
    instance.dict_set(namespace, "getrefcount", getrefcount);

    // `int`↔`str` 的位数上限（`TS-45` ①）：读／写各一个入口
    let get_limit = make_native(instance, "get_int_max_str_digits", get_int_max_str_digits_native);
    instance.dict_set(namespace, "get_int_max_str_digits", get_limit);
    let set_limit = make_native(instance, "set_int_max_str_digits", set_int_max_str_digits_native);
    instance.dict_set(namespace, "set_int_max_str_digits", set_limit);

    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
