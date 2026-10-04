//! Pyawa 能力域接口的形状：宿主实现的 vtable／trait。
//!
//! 接口形状归属 `docs/SPEC-capabilities.md`（`CP-`），见本 crate 的 `README.md`。

#![forbid(unsafe_code)]

pub mod clock;
pub mod fs;

/// **能力域个数**（`CP-1`：与 `DESIGN.md` §7.3 的九域一一对应；顺序照 `SPEC-capabilities.md` §4 的表）。
///
/// 一处真相：域编号表在**本 crate**（形状层）；`pyawa-abi` 侧那份 `capability::*` 是同一张表的
/// 注册接口投影（`AB-33`），两边**顺序必须一致** ✓。
pub const DOMAIN_COUNT: usize = 9;

/// `fs` 域的编号（`SPEC-capabilities.md` §4 表里排第一 ✓）。
pub const DOMAIN_FS: usize = 0;

/// `clock` 域的编号（`SPEC-capabilities.md` §4 表里排第四 ⇒ 下标 3 ✓；对应 C 层模块 `time` ✓）。
pub const DOMAIN_CLOCK: usize = 3;
