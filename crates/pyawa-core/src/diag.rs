//! Diagnostics switches for the VM core (moved out of `instance.rs`).
//!
//! Each `PYAWA_*` switch is read in exactly one place here; hot paths only call
//! these helpers instead of touching `env` directly.

/// `PYAWA_<name>=1` => on.
pub(crate) fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

/// **每个调用点一份 `OnceLock`** 的便捷宏（round 247 性能，A2-2）：
/// `crate::diag::flag_site!("PYAWA_X")` ≡ `flag_cached("PYAWA_X", &本调用点的槽)`。
///
/// 为什么要宏：热路径上的 `flag("…")` 每次走 libc `getenv`（线性扫 environ；A2-1 后实测占基线
/// ~28%，第一热点 ✗）。`flag_cached` 需要调用点各自持有一个 `'static` 槽 ⇒ 宏把"声明槽 ＋ 查缓存"
/// 收成一行，避免几十处样板 ✓。诊断开关本来就不需要运行期改变 ✓（与 `flag_cached` 同口径）。
macro_rules! flag_site {
    ($name:literal) => {{
        static SLOT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        $crate::diag::flag_cached($name, &SLOT)
    }};
}
pub(crate) use flag_site;

/// `flag_off` 的**每调用点缓存**版（语义：`PYAWA_<name>` 未设置 ⇒ true）。
macro_rules! flag_off_site {
    ($name:literal) => {{
        static SLOT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        !$crate::diag::flag_cached($name, &SLOT)
    }};
}
pub(crate) use flag_off_site;

/// **缓存版开关**（round 199 性能修 ✗）：热路径（每次 `Owned` 落域／每次清属性／每次 raise）上
/// `std::env::var_os` 太贵（实测 `test_userstring` 86 s → 108 s ✗）⇒ 每个开关一个 `OnceLock<bool>`，
/// 进程内只查一次环境 ✓（诊断开关本来就不需要运行期改变 ✓）。
pub(crate) fn flag_cached(name: &'static str, slot: &'static std::sync::OnceLock<bool>) -> bool {
    *slot.get_or_init(|| std::env::var_os(name).is_some())
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
