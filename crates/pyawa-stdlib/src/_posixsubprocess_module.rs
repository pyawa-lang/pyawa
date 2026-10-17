//! `_posixsubprocess` 模块（**task-15 ✓**）：`import subprocess` 的第一道 C 模块闸。
//!
//! 侦察（本轮实测 ✓）：上游 `subprocess.py` 对它的导入期引用**只有绑定、没有调用** ——
//! `subprocess.py:106` 的 `from _posixsubprocess import fork_exec as _fork_exec`（`_can_fork_exec`
//! 在 Linux 为真 ⇒ 走这条）⇒ 本轮只要**模块存在＋`fork_exec` 名字可绑定**，`import subprocess`
//! 就不会被它挡（真正还会挡的是 `os.waitpid` 一族与 `select`／`selectors`，见 task-15 报告）。
//!
//! ## 真面
//!
//! - 模块 `__doc__` 照参照逐字抄（`A POSIX helper for the subprocess module.`）；
//! - `fork_exec` 的**名字＋元数/关键字校验**：参照是 22 个位置参数（实测
//!   `fork_exec expected 22 arguments, got N`、`_posixsubprocess.fork_exec() takes no keyword arguments`）。
//!
//! ## 如实报错面（硬边界 ✓：**禁止返回假 pid／假 fd**）
//!
//! 真 `fork()`／`exec()` 是能力域问题（`CM-8`）⇒ 元数与关键字校验过后**调用即**
//! `NotImplementedError`（消息含"未接线"）——**不返回假 pid**、**不返回假 fd 对**。
//!
//! ## 未落地（如实记）
//!
//! - 22 个参数的**逐参数类型校验**（`args` 要序列、`pass_fds` 要整数序列、fd 要整数…）不在本轮：
//!   那些都在**运行期**才用，而本轮只到 import 面；元数错与关键字错的文案已按参照钉住 ✓。
//! - 参照 `fork_exec` 的 C 文档串：core 给原生函数统一空串（第 406 轮口径）⇒ `__doc__` 可读但为空 ✓。
//!
//! 本模块**不碰平台**（`CX-4`：`pyawa-stdlib/src` 在静态扫描范围内 ⇒ `#![forbid(unsafe_code)]`）。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, NativeFn};

/// 模块名（`_posixsubprocess`）。
pub const NAME: &str = "_posixsubprocess";

/// 模块 `__doc__`（**照参照逐字抄** ✓）。
pub const DOC: &str = "A POSIX helper for the subprocess module.";

/// 参照实测：`fork_exec` 是 **22 个位置参数**（`fork_exec expected 22 arguments, got N` ✓）。
const FORK_EXEC_ARITY: usize = 22;

/// 建 `_posixsubprocess` 的命名空间（**新引用**的 `dict`；`CM-6`：模块**存在**即可 import）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let fork_exec = make_native(instance, "fork_exec", fork_exec_native);
    instance.dict_set(namespace, "fork_exec", fork_exec);
    let exports = instance.new_list(vec![instance.new_str("fork_exec")]);
    instance.dict_set(namespace, "__all__", exports);
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// `_posixsubprocess.fork_exec(args, executable_list, …, preexec_fn, /)`：
/// 元数/关键字校验照参照（22 个位置参数、拒关键字）⇒ 校验过后**如实报未接线**。
fn fork_exec_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_posixsubprocess.fork_exec() takes no keyword arguments",
        ));
    }
    if args.len() != FORK_EXEC_ARITY {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("fork_exec expected {FORK_EXEC_ARITY} arguments, got {}", args.len()),
        ));
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_posixsubprocess.fork_exec`：真 fork/exec 属能力域（CM-8）问题，本轮未接线",
    ))
}
