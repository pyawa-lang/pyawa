//! **进程环境变量**（`os.putenv`／`os.unsetenv` 的落点，round 92）。
//!
//! 为什么放在核心：Rust 2024 把 `std::env::set_var`／`remove_var` 标成 `unsafe`
//! （多线程下改动环境不安全），而 `pyawa-stdlib` 是 `forbid(unsafe_code)` 的宿主接口层
//! ⇒ 由核心提供这两个**有明确前置条件**的封装：调用方必须保证没有别的线程在读环境
//! —— 本层是**每实例单线程**执行模型（`DESIGN.md` §5）✓。
//!
//! 语义照 `os.putenv`／`os.unsetenv`：只碰**宿主进程**环境；不存在/重复删除都不报错 ✓。

/// 设置一个环境变量（键与值都已是字符串；调用方负责查非法名字）。
///
/// # Safety 前置条件（记在函数名上而非签名上：调用方是安全层）
///
/// 单线程执行模型下成立：本层没有别的线程在并发读写环境（`pa_interrupt` 只碰原子标志）。
pub fn set_var(key: &str, value: &str) {
    // SAFETY: 见上方"前置条件"：本层单线程执行，没有并发读环境。
    unsafe { std::env::set_var(key, value) };
}

/// 删除一个环境变量（不存在 ⇒ 静默成功）。
pub fn remove_var(key: &str) {
    // SAFETY: 同上。
    unsafe { std::env::remove_var(key) };
}
