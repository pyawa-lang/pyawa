//! `_sre` 模块（**`re` 的底座** ✓，`P3` 的"前 5 个 C 模块面"思路的直接应用 ✓）。
//!
//! 为什么先只落**常量** ✓：`re/_constants.py:18` 在**导入时**就 `from _sre import MAXREPEAT, MAXGROUPS` ✓，
//! `re/_compiler.py:18` 又 `assert _sre.MAGIC == MAGIC` ✓、`:397` 用 `_sre.CODESIZE` ✓
//! ⇒ 常量不对，`re` 连**编译期**都过不去 ✓；而 `compile`／`template`／四个 `*cased`/`*tolower` 函数随后接 ✓。
//!
//! 取值来源 ✓：**本机参照实现实测**（`python3 -c "import _sre; …"` ✓），逐值照抄 ✓：
//! `MAGIC=20230612`／`CODESIZE=4`／`MAXREPEAT=4294967295`／`MAXGROUPS=1073741823` ✓。
//!
//! 依赖口径 ✓（用户 2026-10-07：**允许依赖、不手写一切** ✓）：`compile` 将接到 `regex` crate ✓（已在
//! `pyawa-core` 的依赖边上记明理由 ✓），**不自研正则引擎** ✗。本文件目前不牵任何依赖 ✓。
//!
//! `CX-4`：stdlib 在静态扫描范围内 ⇒ 本文件不碰平台、不用 `unsafe` ✓。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};

/// 模块名（`_sre`）。
pub const NAME: &str = "_sre";

/// 模块的 `__doc__`（与参照实现同源的一句话）。
pub const DOC: &str = "SRE regex support (Pyawa's built-in; the engine itself is a Rust crate).";

/// `re/_constants.py:16` 的 `MAGIC`（`re/_compiler.py:18` 会断言两者相等 ✓）。
pub const MAGIC: i64 = 20230612;

/// 码元宽度（`re/_compiler.py:397` 的 `_CODEBITS = _sre.CODESIZE * 8` ✓）。
pub const CODESIZE: i64 = 4;

/// 重复次数上限（`re/_constants.py:18` 直接 `from _sre import MAXREPEAT` ✓）。
pub const MAXREPEAT: i64 = 4294967295;

/// 分组数上限（同上 ✓）。
pub const MAXGROUPS: i64 = 1073741823;

/// 建 `_sre` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, value) in [
        ("MAGIC", MAGIC),
        ("CODESIZE", CODESIZE),
        ("MAXREPEAT", MAXREPEAT),
        ("MAXGROUPS", MAXGROUPS),
    ] {
        let constant = instance.new_int(value);
        instance.dict_set(namespace, name, constant);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
