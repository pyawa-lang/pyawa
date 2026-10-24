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
//! - **审计机制未实现**：没有 hook 注册通道（`sys.addaudithook` 未实现，`sys._audit` 一类也没有）
//!   ⇒ `audit(event, *args)` 在合法调用下恒为**真语义 no-op**（`None`）；**不是**"审计已实现"
//!   （有 hook 可注册时，参照会回调 hook——本层没有那条路径；`P3-29` 侦察 B2）
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
///
/// **形状**照参照「`<版本> (<构建号>, <构建日期>, <构建时间>) [<编译器>]`」那一形 ✓
/// （第 413 轮，task-26 ① ✓）：`Lib/platform.py` 的 `_sys_version` 用
/// `([\w.+]+)\s*\(#?([^,]+)(?:,\s*([\w ]*)(?:,\s*([\w :]*))?)?\)\s*\[([^\]]+)\]?`
/// 解析它 ✓ —— 先前 `'3.14.4 (pyawa 0.0.0)'` 缺后半段 ✗ ⇒ `import platform` 直接
/// `ValueError: failed to parse CPython sys.version` ✗（`wsgiref.simple_server` 一族卡在这 ✓）。
/// **如实标识** ✓：构建号里带 `pyawa` 与本实现版本 ✓；构建号/日期/时间取**确定性**字面值
/// （不塞当前时间 ⇒ 对拍可重复 ✓）。**如实记偏差** ✗：形如 CPython ⇒ `platform.python_implementation()`
/// 会报 `'CPython'`（参照对 pyawa 也不该期待别的值；实现身份仍以 `sys.implementation.name`
/// ＝ `'pyawa'` 为**一处真相** ✓）。
pub fn version_string() -> String {
    let (major, minor, micro, _, _) = LANGUAGE_VERSION;
    let (impl_major, impl_minor, impl_micro, _, _) = IMPLEMENTATION_VERSION;
    format!(
        "{major}.{minor}.{micro} ({IMPLEMENTATION_NAME} {impl_major}.{impl_minor}.{impl_micro}, \
         {IMPLEMENTATION_NAME}, 00:00:00) [Rust]"
    )
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

/// **`sys.implementation._multiarch`** ✓（task-9）：**构建目标（宿主平台）的 Debian/Ubuntu
/// multiarch 三元组** —— 不是语言版本、也不是本实现自报的版本号。
///
/// 为什么需要它 ✓：`Lib/sysconfig/__init__.py:394-399` 用它拼 sysconfigdata 的名字
/// `_sysconfigdata_{abiflags}_{platform}_{multiarch}`；空值会拼出 `_sysconfigdata__linux_`
/// ⇒ `ModuleNotFoundError` ⇒ `test.support` 的 sysconfig 链断 ✗（本仓 `Lib/` 里同步了
/// `_sysconfigdata__linux_x86_64-linux-gnu.py` 与 `_sysconfigdata__x86_64-linux-gnu.py` ✓）。
///
/// 取值口径 ✓：CPython 在 **configure 期**把 `MULTIARCH`（`dpkg-architecture
/// -qDEB_HOST_MULTIARCH` 的同源值）编进解释器 ⇒ 这是**构建目标事实**。本机参照 3.14.4
/// 实测 `'x86_64-linux-gnu'` ✓；本层对拍／同步的 `Lib/` 来自**同一目标**的上游 deb ⇒ 值必须
/// 一致。判据用 `cfg!`（**不是**平台接口，`CX-4` ✓；同 [`byteorder`] 的既有手法 ✓）——
/// **未知目标如实给空串**（不冒充某个三元组 ⇒ sysconfig 会拼出不存在的名字，由它自己报错 ✓）。
pub fn multiarch() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "x86")) {
        "i386-linux-gnu"
    } else {
        ""
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

// ---- round 16：`sys._jit`（`Lib/test/support/__init__.py:2736` 的模块级查询口）----
//
// 参照 3.14.4 实测：`sys._jit` 是 **module 对象**（`<module 'sys._jit'>`），三个无参查询口
// `is_available()`／`is_enabled()`／`is_active()`；本机参照是 JIT 构建 ⇒ `(True, False, False)`。
// **如实登记偏差** ✗：本层**没有 JIT 后端** ⇒ 三个口一律 `False`（`is_available` 的 True 是
// "构建期带 JIT"这一构建事实，编不出来，也不冒充）。`test.support` 只用它驱动
// `requires_jit_*` 的 skip，False 的语义正确。

/// `sys._jit` 的三个查询口的共用实现：**无参**，恒 `False`（本层没有 JIT 后端）。
fn jit_query(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    name: &str,
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes no keyword arguments"),
        ));
    }
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes no arguments ({} given)", args.len()),
        ));
    }
    Ok(instance.new_bool(false))
}

fn jit_is_available_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    jit_query(instance, args, kwargs, "sys._jit.is_available")
}

fn jit_is_enabled_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    jit_query(instance, args, kwargs, "sys._jit.is_enabled")
}

fn jit_is_active_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    jit_query(instance, args, kwargs, "sys._jit.is_active")
}

// ---- round 19：`sys.monitoring` 最小 import 面（`pdb`／`doctest` 的闸）----
//
// 参照 3.14.4 实测：`sys.monitoring` 是 **module 对象**（`<module 'sys.monitoring'>`），
// `sys.monitoring.events` 是 `types.SimpleNamespace`；`bdb.py:20` 在**导入期**只做
// `E = sys.monitoring.events` 并用 `E.PY_START | …` 填字典键 ⇒ 本层只需把常量做成**真 int** ✓。
// pdb 默认 backend='settrace' ⇒ 工具 API（`use_tool_id`／`register_callback` 一族）运行期不调用；
// 本层**如实**不实现它们（显式调用 ⇒ `NotImplementedError`，**不假返回值** ✓）。
//
// **如实偏差** ✗：① `events` 用"属性命名空间"对象（参照是 `types.SimpleNamespace` ⇒ `type` 名／
// `repr` 不同，属性读法一致）；② 私有的 `_all_events` 未接（bdb 导入期不用）；③ 工具 API 全未接。

/// `sys.monitoring.MISSING`／`DISABLE` 这类"独立哨兵"：参照各是**一个 `object()` 实例**
/// （两者互不相同）。走真正的 `object()` 构造（`attribute_read` 那条 `call_value` 公开面 ✅）；
/// `object` 类型在引导期已带 `new` 槽 ⇒ 不会抛。`build` 无返回值通道，构造失败说明引导期坏了 ⇒
/// 与文件里 `expect("… 已登记")` 同一口径直接 panic（不拿别的对象冒充）。
fn monitoring_sentinel(instance: &Instance) -> NonNull<Header> {
    let object_type = instance
        .type_named("object")
        .expect("object 在引导期已登记（OM-13）");
    pyawa_core::executor::call::call_value(instance, object_type.cast(), &[], &[])
        .expect("`object()` 必须可构造（引导期核心不变量）")
}

/// 生成 `sys.monitoring` 的"未接线工具 API"原生：真调用一律如实 `NotImplementedError`。
macro_rules! monitoring_unimplemented_native {
    ($function:ident => $name:literal $(,)?) => {
        fn $function(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            _args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            Err(instance.raise_builtin_error(
                "NotImplementedError",
                concat!(
                    "`sys.monitoring.",
                    $name,
                    "`：round 19 只接 import 期常量（pdb 默认 backend='settrace' 不调用工具 API），真调用如实未实现（随后补）"
                ),
            ))
        }
    };
}

monitoring_unimplemented_native!(monitoring_use_tool_id_native => "use_tool_id");
monitoring_unimplemented_native!(monitoring_free_tool_id_native => "free_tool_id");
monitoring_unimplemented_native!(monitoring_get_tool_native => "get_tool");
monitoring_unimplemented_native!(monitoring_set_events_native => "set_events");
monitoring_unimplemented_native!(monitoring_get_events_native => "get_events");
monitoring_unimplemented_native!(monitoring_set_local_events_native => "set_local_events");
monitoring_unimplemented_native!(monitoring_get_local_events_native => "get_local_events");
monitoring_unimplemented_native!(monitoring_register_callback_native => "register_callback");
monitoring_unimplemented_native!(monitoring_restart_events_native => "restart_events");
monitoring_unimplemented_native!(monitoring_clear_tool_id_native => "clear_tool_id");

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

/// **`sys.audit(event, *args)`**（`P3-29` 侦察 B2）：**无 hook 注册通道 ⇒ 真语义 no-op**。
///
/// **如实登记** ✗：本实现**没有审计机制**——`sys.addaudithook` 未实现，也没有任何别的 hook
/// 注册通道 ⇒ 没有 hook 可通知，合法调用恒回 `None` ✓。**这不是"审计已实现"**：参照在有 hook
/// 时会依次回调 hook 并传 `(event, *args)`；本层**没有**那条路径。`P3-29` 只需要这个调用点
/// 不炸（`Lib/http/client.py:1031` 在建连前调它）。
///
/// 参数校验照参照 3.14.4 **实测**：关键字**先判** ⇒ `sys.audit() takes no keyword arguments`；
/// 0 个位置实参 ⇒ `audit expected at least 1 argument, got 0`；`event` 不是 `str`（`str` 子类
/// 参照**接受** ⇒ 这里按子类型判）⇒ `audit() argument 1 must be str, not <类型>`——`None` 在
/// 参照的这条消息里渲染成 `None`（**不是** `NoneType` ✓）。其余位置实参**任意**、一律忽略。
fn audit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sys.audit() takes no keyword arguments",
        ));
    }
    let Some(event) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "audit expected at least 1 argument, got 0",
        ));
    };
    let event_type = instance.type_of(*event);
    if !instance.is_subtype(event_type, instance.singletons().str_type()) {
        let type_name = if *event == instance.singletons().none() {
            // 参照的 "argument must be str, not X" 形态把 `None` 渲染成 `None`（实测 ✓）。
            "None".to_owned()
        } else {
            instance.type_name(event_type)
        };
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("audit() argument 1 must be str, not {type_name}"),
        ));
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`sys._clear_type_descriptors(type, /)`** ✓（task-23 ✓；参照 3.14.4 实测签名／文案）：
/// `Lib/dataclasses.py:1340` 在 `__slots__` 重建时调它（`sys._clear_type_descriptors(cls)` ✓）。
///
/// **如实登记的偏差** ✗：参照会清掉类字典里 `__dict__`／`__weakref__` 两类 getset 描述符；
/// 本层的类上没有这两类可清项（`__dict__` 走另一条面）⇒ 合法调用是**幂等 no-op**（回 `None` ✓）。
/// 签名／命令行文案照参照：0／2 参 ⇒ `takes exactly one argument (N given)`；
/// 关键字 ⇒ `takes no keyword arguments`；非 type ⇒ `argument must be type, not <类型>`。
/// （参照对"不可变内建类型"另报 `TypeError: argument is immutable` —— 本层**无该标志位** ⇒ 已登记偏差 ✗。）
fn clear_type_descriptors_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sys._clear_type_descriptors() takes no keyword arguments",
        ));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "sys._clear_type_descriptors() takes exactly one argument ({} given)",
                args.len()
            ),
        ));
    }
    if !instance.is_type_object(args[0]) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "_clear_type_descriptors() argument must be type, not {}",
                instance.type_name(instance.type_of(args[0]))
            ),
        ));
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`sys.getrecursionlimit()`** ✓（task-20 第 3 轮 ✓）：给当前**有效**上限 ✓（默认 64 ✓）。
fn getrecursionlimit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "getrecursionlimit() takes no arguments",
        ));
    }
    Ok(instance.new_int(instance.recursion_limit() as i64))
}

/// **`sys.setrecursionlimit(n)`** ✓（task-20 第 3 轮 ✓）：**只降不升** ✓（夹到 `MAX_CALL_DEPTH` ✓，
/// 理由见那一处：本层 Python 调用吃原生栈 ✗）。回 `None` ✓。
fn setrecursionlimit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(limit) = args.first().and_then(|value| instance.int_value(*value)) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "setrecursionlimit() argument must be an integer",
        ));
    };
    if limit < 1 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "cannot set the recursion limit to a value less than 1",
        ));
    }
    instance.set_recursion_limit(limit as usize);
    Ok(instance.singletons().none())
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

/// `sys.excepthook(type, value, traceback)` ✓（第 724 轮）：**简化实现** ✗ —— 只打
/// `类型: 消息` 到 `stderr` ✓（完整回溯需要 `traceback`，它被 `textwrap` 的 look-around 挡着 ✓）。
fn excepthook_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let type_name = args
        .first()
        .map(|value| instance.type_name(instance.type_of(*value)))
        .unwrap_or_else(|| "Exception".to_owned());
    let message = args.get(1).and_then(|value| instance.text_of(*value)).unwrap_or_default();
    if message.is_empty() {
        eprintln!("{type_name}");
    } else {
        eprintln!("{type_name}: {message}");
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// **`sys.exc_info()`**（task-26 真实现）：当前**处理中**的异常 ⇒ `(type, value, traceback)` ✓；
/// 没有 ⇒ `(None, None, None)` ✓（参照 3.14.4 实测口径 ✓）。
///
/// 底座是 `BC-60` ② 的**实例级异常状态**（`PUSH_EXC_INFO` 压、`POP_EXCEPT` 弹 ✓，见
/// `executor/step_exceptions.rs` ✓）—— 本层现在**真的**记"正在处理的异常" ✓。第 724 轮那个
/// **如实桩**（恒 `(None, None, None)`）会让 `unittest` 的错误恢复在 `sys.exc_info()[0]` 上拿到
/// `None` ✗ ⇒ `issubclass(None, failureException)` ⇒ 真实失败被掩盖、官方套件一遇失败就 abort ✗。
///
/// `traceback` 取异常实例的 `__traceback__`（`raise` 时挂上的**最小起步对象** ✓；
/// 未抛过的异常给 `None` ✓）—— 与 `exception_getattr` 的 `"__traceback__"` 臂读**同一格** ✓。
fn exc_info_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let none = instance.singletons().none();
    // `new_tuple` **吃掉**每项引用 ⇒ 每项都自己 retain 一份 ✓。
    let Some(exception) = instance.current_exception() else {
        let first = instance.retain(none);
        let second = instance.retain(none);
        let third = instance.retain(none);
        return Ok(instance.new_tuple(vec![first, second, third]));
    };
    // `type(value)` 是**类型对象**（参照给的也是类对象本身 ✓）；`retain` 那份给元组 ✓。
    let exception_type = instance.type_of(exception).cast::<Header>();
    instance.retain(exception_type);
    instance.retain(exception);
    // `__traceback__` 在异常实例的属性字典里（与 `exception_getattr` 同一条读取路径 ✓）。
    let traceback = pyawa_core::mounted_instance_dict(instance, exception)
        .and_then(|mapping| instance.dict_get(mapping, "__traceback__"))
        .map(|value| instance.retain(value))
        .unwrap_or_else(|| instance.retain(none));
    Ok(instance.new_tuple(vec![exception_type, exception, traceback]))
}

/// **`sys.exception()`**（3.11+；task-26）：当前**处理中**的异常**值**；没有 ⇒ `None` ✓
/// （参照实测：`except` 块里给那个异常实例 ✓、块外给 `None` ✓）。
fn exception_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(match instance.current_exception() {
        Some(value) => instance.retain(value),
        None => instance.retain(instance.singletons().none()),
    })
}

/// **`sys.exit([code])`** ✓（`P3-32`）：**实现＝抛 `SystemExit`** ✓（参照同义：`sys.exit` 本身
/// 不直接退出进程，`except SystemExit` 必须能接住）——进程级退出码由组合根在**未捕获**时解释
/// （`pyawa_core::executor::ExitDisposition`）。
///
/// 参数校验照参照 3.14.4 **实测**（关键字**先判**）：
/// - 带关键字 ⇒ `sys.exit() takes no keyword arguments`；
/// - 位置实参 ≥2 ⇒ `exit expected at most 1 argument, got N`（N 是实际个数）；
/// - 0 参 ⇒ `SystemExit()`；1 参 ⇒ `SystemExit(该参)`（`int`／`None`／别的都原样交给异常载荷，
///   退出码／`stderr` 文本的解释在组合根那一处，**不在**这里）。
///
/// `CX-22`：载荷布局只有核心能碰 ⇒ 造异常走 `raise_builtin_with_args_public`。
fn exit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sys.exit() takes no keyword arguments",
        ));
    }
    if args.len() >= 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("exit expected at most 1 argument, got {}", args.len()),
        ));
    }
    // 实参进异常对象 ⇒ 每项交一份**新引用**（`new_exception_with_args` 接手）。
    let payload: Vec<NonNull<Header>> = args.iter().map(|argument| instance.retain(*argument)).collect();
    Err(pyawa_core::executor::raise_builtin_with_args_public(
        instance, "SystemExit", payload,
    ))
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

    // **`excepthook`／`__excepthook__`** ✓（第 724 轮）：`Lib/threading.py` 的
    // `_make_invoke_excepthook()` 读 `_sys.excepthook` ✓ —— 缺它 `import threading` 走到
    // **71 个名字**处报 `AttributeError: 'module' object has no attribute 'excepthook'` ✗（实测 ✓）。
    // **如实口径** ✗：完整回溯要 `traceback`／`textwrap`，后者被 look-around 正则挡着 ✓
    // ⇒ 这里只打**类型与消息**一行到 `stderr` ✓（不伪造完整回溯 ✗）。
    let excepthook = crate::builtins_module::make_native(
        instance,
        "excepthook",
        excepthook_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "excepthook", excepthook);
    let excepthook_builtin = crate::builtins_module::make_native(
        instance,
        "__excepthook__",
        excepthook_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "__excepthook__", excepthook_builtin);
    // **`exc_info()`／`exception()`**（task-26 真实现）：`Lib/threading.py` 的
    // `_make_invoke_excepthook()` 在**模块级**就取 `sys_exc_info = _sys.exc_info` ✓；
    // `unittest` 的错误恢复靠 `sys.exc_info()` 拿真实异常 ✓（先前是如实桩 ⇒ 官方套件一遇
    // 失败就 `issubclass() arg 1 must be a class` abort ✗）。两者都读 `BC-60` ② 的
    // 实例级异常状态 ✓（`PUSH_EXC_INFO`／`POP_EXCEPT` 维护 ✓，见 `executor/step_exceptions.rs` ✓）。
    let exc_info = crate::builtins_module::make_native(
        instance,
        "exc_info",
        exc_info_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "exc_info", exc_info);
    // `sys.exception()`（3.11+）：同一个状态的**值**那一项 ✓。
    let exception = crate::builtins_module::make_native(
        instance,
        "exception",
        exception_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "exception", exception);

    // **`sys.exit([code])`** ✓（`P3-32`）：`Lib/` 里遍地 `sys.exit(...)` ✓ —— 实现＝抛 `SystemExit`
    //（参照同义 ✓）；进程级退出码由组合根在**未捕获**时解释 ✓（`ExitDisposition` 一处真相 ✓）。
    let exit = make_native(instance, "exit", exit_native);
    instance.dict_set(namespace, "exit", exit);

    // **`_getframe`** ✓（第 230 轮）：`_collections_abc.py:89` 的 `sys._getframe().f_locals` 要它 ✓。
    let getframe = crate::builtins_module::make_native(
        instance,
        "_getframe",
        pyawa_core::getframe_native as pyawa_core::NativeFn,
    );
    instance.dict_set(namespace, "_getframe", getframe);
    // **`_getframemodulename([depth])`** ✓（第 617 轮）：`collections.namedtuple` 先试它 ✓
    //（`Lib/collections/__init__.py:519-527` ✓）—— 免得走我们没接的 `f_globals` ✗。
    let getframemodulename = make_native(
        instance,
        "_getframemodulename",
        pyawa_core::getframemodulename_native,
    );
    instance.dict_set(namespace, "_getframemodulename", getframemodulename);
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
    // **`copyright`**（M5 体量实测的窄修 ✓）：`Lib/site.py:467-469` 的 `setcopyright()` 直接取
    // `sys.copyright` ⇒ 缺这个属性时 `import site` 在**模块级**就
    // `AttributeError: 'module' object has no attribute 'copyright'` ✗（实测 ✓，P3-28 ② 的前置）。
    // 值＝**参照 3.14.4 的原文** ✓（`Lib/` 逐字同步自 CPython ⇒ 该声明随库走；`sys.copyright` 属
    // `DESIGN.md` §9 的"实现观测面"，这里只保证**存在且为 str**，不声称 Pyawa 自身的版权归属）。
    let copyright = instance.new_str(
        "Copyright (c) 2001 Python Software Foundation.\nAll Rights Reserved.\n\n\
         Copyright (c) 2000 BeOpen.com.\nAll Rights Reserved.\n\n\
         Copyright (c) 1995-2001 Corporation for National Research Initiatives.\nAll Rights Reserved.\n\n\
         Copyright (c) 1991-1995 Stichting Mathematisch Centrum, Amsterdam.\nAll Rights Reserved.",
    );
    instance.dict_set(namespace, "copyright", copyright);
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
    // **`_multiarch`** ✓（task-9）：`sysconfig._get_sysconfigdata_name()` 拼 sysconfigdata 名字要用它
    //（`Lib/sysconfig/__init__.py:394-399`）；值是**构建目标事实**，见 [`multiarch`]。
    let multiarch = instance.new_str(multiarch());
    instance.set_type_attribute(namespace_type, "_multiarch", multiarch);
    let implementation =
        instance.alloc(AttributeObject::new(namespace_type, core::cell::RefCell::new(None)));
    instance.dict_set(
        namespace,
        "implementation",
        implementation.into_raw().cast::<Header>(),
    );

    // **`sys._jit`**（round 16）：模块对象式的三个查询口（见 `jit_query` 的如实说明）。
    // 建法与 `install` 里那些 Rust 侧模块同一手法：`module` 类型 ＋ 挂一个命名空间 dict。
    let jit_namespace = instance.new_dict();
    for (name, handler) in [
        ("is_available", jit_is_available_native as pyawa_core::NativeFn),
        ("is_enabled", jit_is_enabled_native as pyawa_core::NativeFn),
        ("is_active", jit_is_active_native as pyawa_core::NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(jit_namespace, name, function);
    }
    let jit_name = instance.new_str("sys._jit");
    instance.dict_set(jit_namespace, "__name__", jit_name);
    // `module` 在引导期不保证登记（与 `lib.rs::install` 的兜底同口径），缺就现建一个。
    let module_type = instance
        .type_named("module")
        .unwrap_or_else(|| instance.new_attribute_type("module"));
    let jit_module = instance
        .alloc(AttributeObject::new(
            module_type,
            core::cell::RefCell::new(Some(jit_namespace)),
        ))
        .into_raw()
        .cast::<Header>();
    instance.dict_set(namespace, "_jit", jit_module);

    // **`sys.monitoring`**（round 19）：最小的 **import 期常量** 面（见 `monitoring_sentinel` 上方
    // 的如实说明）。数值全部照本机 `python3` 3.14.4 逐条实测。
    let monitoring_namespace = instance.new_dict();
    // 工具 id 常量（`bdb.MonitoringBackend.__init__` 用 DEBUGGER_ID）。
    let monitoring_ids: [(&str, i64); 4] = [
        ("DEBUGGER_ID", 0),
        ("COVERAGE_ID", 1),
        ("PROFILER_ID", 2),
        ("OPTIMIZER_ID", 5),
    ];
    for (name, value) in monitoring_ids {
        let number = instance.new_int(value);
        instance.dict_set(monitoring_namespace, name, number);
    }
    // 两个**互不相同**的 `object()` 哨兵（参照 `MISSING`／`DISABLE` 各是独立实例）。
    for name in ["MISSING", "DISABLE"] {
        let sentinel = monitoring_sentinel(instance);
        instance.dict_set(monitoring_namespace, name, sentinel);
    }
    // `sys.monitoring.events`：20 个**整数位**（参照 `types.SimpleNamespace`；见文件头如实偏差）。
    let events_type = instance.new_attribute_type("sys.monitoring.events");
    let monitoring_events: [(&str, i64); 20] = [
        ("NO_EVENTS", 0),
        ("PY_START", 1),
        ("PY_RESUME", 2),
        ("PY_RETURN", 4),
        ("PY_YIELD", 8),
        ("CALL", 16),
        ("LINE", 32),
        ("INSTRUCTION", 64),
        ("JUMP", 128),
        ("BRANCH_LEFT", 256),
        ("BRANCH_RIGHT", 512),
        ("STOP_ITERATION", 1024),
        ("RAISE", 2048),
        ("EXCEPTION_HANDLED", 4096),
        ("PY_UNWIND", 8192),
        ("PY_THROW", 16384),
        ("RERAISE", 32768),
        ("C_RETURN", 65536),
        ("C_RAISE", 131072),
        ("BRANCH", 262144),
    ];
    for (field, value) in monitoring_events {
        let number = instance.new_int(value);
        instance.set_type_attribute(events_type, field, number);
    }
    let events = instance
        .alloc(AttributeObject::new(
            events_type,
            core::cell::RefCell::new(None),
        ))
        .into_raw()
        .cast::<Header>();
    instance.dict_set(monitoring_namespace, "events", events);
    // 工具 API 的名字要在（运行期真调用 ⇒ 如实 NotImplementedError）。
    for (name, handler) in [
        (
            "use_tool_id",
            monitoring_use_tool_id_native as pyawa_core::NativeFn,
        ),
        (
            "free_tool_id",
            monitoring_free_tool_id_native as pyawa_core::NativeFn,
        ),
        ("get_tool", monitoring_get_tool_native as pyawa_core::NativeFn),
        (
            "set_events",
            monitoring_set_events_native as pyawa_core::NativeFn,
        ),
        (
            "get_events",
            monitoring_get_events_native as pyawa_core::NativeFn,
        ),
        (
            "set_local_events",
            monitoring_set_local_events_native as pyawa_core::NativeFn,
        ),
        (
            "get_local_events",
            monitoring_get_local_events_native as pyawa_core::NativeFn,
        ),
        (
            "register_callback",
            monitoring_register_callback_native as pyawa_core::NativeFn,
        ),
        (
            "restart_events",
            monitoring_restart_events_native as pyawa_core::NativeFn,
        ),
        (
            "clear_tool_id",
            monitoring_clear_tool_id_native as pyawa_core::NativeFn,
        ),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(monitoring_namespace, name, function);
    }
    let monitoring_name = instance.new_str("sys.monitoring");
    instance.dict_set(monitoring_namespace, "__name__", monitoring_name);
    let monitoring_module_type = instance
        .type_named("module")
        .unwrap_or_else(|| instance.new_attribute_type("module"));
    let monitoring_module = instance
        .alloc(AttributeObject::new(
            monitoring_module_type,
            core::cell::RefCell::new(Some(monitoring_namespace)),
        ))
        .into_raw()
        .cast::<Header>();
    instance.dict_set(namespace, "monitoring", monitoring_module);

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

    // `hash_info`（第 406 轮 ✓）：`_pydecimal.py:6394` 与 `fractions.py:18` 在**导入期**读
    // `sys.hash_info.modulus`／`.inf`／`.nan` ✓。字段值与参照 3.14.4 **逐字实测**一致
    //（`width=64`、`modulus=2**61-1`、`inf=314159`、`nan=0`、`imag=1000003`、`algorithm='siphash13'`）；
    // 元数据 `n_fields=9`／`n_sequence_fields=9`／`n_unnamed_fields=0` 同参照 ✓。
    // **如实范围** ✗：structseq 的元组行为（下标／`len`／迭代／`repr`）仍未接线（与 `float_info` 同）。
    let hash_info_type = instance.new_attribute_type("sys.hash_info");
    let hash_int_fields: [(&str, i64); 9] = [
        ("width", 64),
        ("modulus", 2_305_843_009_213_693_951),
        ("inf", 314_159),
        ("nan", 0),
        ("imag", 1_000_003),
        ("hash_bits", 64),
        ("seed_bits", 128),
        ("cutoff", 0),
        ("n_fields", 9),
    ];
    for (field, value) in hash_int_fields {
        let number = instance.new_int(value);
        instance.set_type_attribute(hash_info_type, field, number);
    }
    for (field, value) in [
        ("algorithm", "siphash13"),
        ("n_sequence_fields", "9"),
        ("n_unnamed_fields", "0"),
    ] {
        if field == "algorithm" {
            let text = instance.new_str(value);
            instance.set_type_attribute(hash_info_type, field, text);
        } else {
            let number: i64 = value.parse().expect("整数元数据");
            let number = instance.new_int(number);
            instance.set_type_attribute(hash_info_type, field, number);
        }
    }
    let hash_info =
        instance.alloc(AttributeObject::new(hash_info_type, core::cell::RefCell::new(None)));
    instance.dict_set(namespace, "hash_info", hash_info.into_raw().cast::<Header>());

    // `getrefcount`（`OM-22`）
    // **`sys.intern(str)`**（第 335 轮）：上限榜上 `AttributeError: 'module' object has no attribute
    // 'intern'` × 72 个模块就卡这一条 ✓（`Lib/` 里大量 `sys.intern(...)` 用在名字表上 ✓）。
    let intern = make_native(instance, "intern", intern_native);
    instance.dict_set(namespace, "intern", intern);
    // **`sys.abiflags`** ✓（task-23）：Linux 标准构建是**空串** ✓（参照实测 `''` ✓）。
    let abiflags = instance.new_str("");
    instance.dict_set(namespace, "abiflags", abiflags);
    // **`sys._clear_type_descriptors(type, /)`** ✓（task-23）：`Lib/dataclasses.py:1340` 要它 ✓。
    let clear_type_descriptors = make_native(
        instance,
        "_clear_type_descriptors",
        clear_type_descriptors_native,
    );
    instance.dict_set(namespace, "_clear_type_descriptors", clear_type_descriptors);
    // **递归上限两口** ✓（task-20 第 3 轮 ✓）：`Lib/dataclasses.py` 一族要 `getrecursionlimit` ✓。
    // **如实** ✗：`setrecursionlimit` **只降不升** ✓（有效上限受 `MAX_CALL_DEPTH` 夹住 ✓）。
    let get_recursionlimit = make_native(instance, "getrecursionlimit", getrecursionlimit_native);
    instance.dict_set(namespace, "getrecursionlimit", get_recursionlimit);
    let set_recursionlimit = make_native(instance, "setrecursionlimit", setrecursionlimit_native);
    instance.dict_set(namespace, "setrecursionlimit", set_recursionlimit);
    let getrefcount = make_native(instance, "getrefcount", getrefcount_native);
    instance.dict_set(namespace, "getrefcount", getrefcount);

    // **`sys.audit(event, *args)`** ✓（`P3-29` 侦察 B2）：无 hook 注册通道 ⇒ 真语义 no-op（如实：
    // 审计机制未实现，见函数注释）；`http.client` 建连前会调它（`Lib/http/client.py:1031`）。
    let audit = make_native(instance, "audit", audit_native);
    instance.dict_set(namespace, "audit", audit);

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
