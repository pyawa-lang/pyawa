//! Diagnostics switches for the VM core (moved out of `instance.rs`).
//!
//! Each `PYAWA_*` switch is read in exactly one place here; hot paths only call
//! these helpers instead of touching `env` directly.

/// `PYAWA_<name>=1` => on.
pub(crate) fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

/// **毒化隔离区**开关 ✓（第 272 轮）：`PYAWA_QUARANTINE=1` ⇒ 释放时不真还给分配器 ✗，
/// 而是把载荷毒化成 `0xDE` 并记进表 ✓ ⇒ 之后每次 `unlink` 复核一遍 ✓：
/// **毒化字节被改** ⇒ 有人**写进了已释放的对象** ✗（use-after-free ✓）⇒ 报出**类型名**并**非零退出** ✓
/// （harness 会把子进程的 stderr 当"事故"记下 ✓ ⇒ 一次就能把凶手带出来 ✓）。
pub(crate) fn quarantine_mode() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PYAWA_QUARANTINE").is_some())
}

/// **悬垂释放哨兵**开关 ✓（第 273 轮）：`PYAWA_DANGLING=1` ⇒ 每次释放／清理**先查活表** ✓
/// ⇒ 指向"已释放过"的指针会在**第一次被碰**时用 `panic!` 报出**地点＋地址** ✓
/// （panic 文本被 test harness 捕获 ✓ ⇒ 一击定位 ✓；活表地址会复用 ✓ ⇒ 判据是"**此刻**在不在" ✓，不假阳性 ✓）。
pub(crate) fn dangling_mode() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PYAWA_DANGLING").is_some())
}

pub(crate) fn leak_mode() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PYAWA_LEAK_MODE").is_some())
}
