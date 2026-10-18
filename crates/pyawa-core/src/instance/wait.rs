//! **`CP-27` 可取消等待 ＋ `CP-28` 取消语义**（`T-CP-10` 的 VM 侧机制）。
//!
//! # 为什么取消在 VM 侧、且不改 ABI
//!
//! `AB-44` 禁止改既有槽位签名、`CP-9` 要求能力调用**同步** ⇒ 不能给 vtable 加取消令牌参数，
//! 也不能把接口改成异步回调。本层的做法：**把长等待切成有界时间片**（≤ [`CANCEL_SLICE_NS`]），
//! 每片调 provider 的**有界**同步槽位（`clock.sleep_ns` 一片；`net.poll` 由 `select` 分片），
//! 片与片之间检查实例中断标志（`pa_interrupt`）：
//!
//! - **`CP-27`**"必须用可取消的等待（带超时／轮询）取回结果，禁止无限阻塞"：每一片都有上界，
//!   循环每片后轮询中断 ⇒ 取消延迟 ≤ 10 ms；连"无限 timeout"也是循环分片，不是一次永久阻塞；
//! - **`CP-28`**"取消结果必须是 未开始／已取消／已完成 三者之一"：
//!   [`Instance::cancellable_sleep_ns`] 返回 `Ok(true)`＝**已完成**、`Ok(false)`＝**已取消**；
//!   "未开始"由进入循环前的检查覆盖（已中断 ⇒ 不发起任何 provider 调用）；
//! - **`CP-19`**"实例中断必须使在途可异步化调用进入取消流程"：中断标志由 `pa_interrupt` 置位，
//!   本模块所有循环以它为唯一取消源；取消返回后**没有该调用之后的任何副作用**（调用方负责把
//!   `Ok(false)` 转成 `ExecError::Interrupted`，执行器随后的指令一条都不跑）。
//!
//! # 取消 → Python 层呈现（如实差异）
//!
//! **沿用既有中断口径**：调用方把"已取消"转成 `ExecError::Interrupted` ⇒ `pa_exec_*` 返回
//! `PA_ERR_INTERRUPT`（4）、`pa_errmsg` 给既有文案（"执行被中断"）。**不**投递 Python 层
//! `KeyboardInterrupt`——参照实现的 Ctrl-C 是它，本层 `pa_interrupt` 是**宿主级中断**（`AB-5`①），
//! 这是如实差异：脚本里 `except KeyboardInterrupt` 捕不到本层的中断。
//!
//! # 已可取消的槽位（本轮，`T-CP-10`）
//!
//! - `clock.sleep_ns`：`time.sleep` 的底座（[`Instance::cancellable_sleep_ns`]）；
//! - `select.select` 的等待：空 fd 列表 ⇒ 纯 `sleep` 片；`net` fd ⇒ `poll` 片
//!   （[`Instance::capability_select_cancellable`]）。
//!
//! # 尚未覆盖（如实记，不假装）
//!
//! - `net.recv`／`send` 这类**没有 timeout 参数**的阻塞槽位：provider 一次调用内阻塞，VM 无从
//!   切片（要真取消得等 worker＋取消令牌那一档；本轮不做）；
//! - provider 内部的**长内核操作**（大文件 `fsync` 等）：只在调用**之间**可取消；
//! - `Python` 层的 `KeyboardInterrupt` 投递（见上"如实差异"）。

use super::*;
use std::time::{Duration, Instant};

/// 时间片上界：**10 ms**。
///
/// 取值理由：取消延迟 ≤ 片长（`T-CP-10` 要求"1–2 s 内返回"，这里富余两个数量级）；
/// 片再小则调度开销占比变大；10 ms 是常见可取消等待的粒度。
pub(crate) const CANCEL_SLICE_NS: i64 = 10_000_000;

impl Instance {
    /// 调一次 `clock.sleep_ns`（**有界**：调用方保证 `ns <= CANCEL_SLICE_NS`；`ns <= 0` 直接成功）。
    ///
    /// 三态照 `CP-5`：成功／机器错误（`errno` 带回）／未实现（域或槽位没提供）。
    pub(crate) fn clock_sleep_slice_ns(&self, ns: i64) -> Result<(), CapabilityCallError> {
        if ns <= 0 {
            return Ok(());
        }
        let table = self.clock_vtable().ok_or(CapabilityCallError::NotRegistered)?;
        let sleep = table.sleep_ns.ok_or(CapabilityCallError::NotImplemented)?;
        let mut errno = 0i32;
        match sleep(table.state, ns, &mut errno) {
            pyawa_capabilities::clock::CapStatus::Ok => Ok(()),
            pyawa_capabilities::clock::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::clock::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(errno))
            }
        }
    }

    /// **`CP-27` 可取消等待：睡够 `total_ns` 纳秒，期间可被 `pa_interrupt` 取消**（`T-CP-10`）。
    ///
    /// - `Ok(true)` ＝ **已完成**（睡满，`CP-28` 的"已完成"）；
    /// - `Ok(false)` ＝ **已取消**（`CP-28` 的"已取消"／"未开始"；调用方**必须**把它转成
    ///   `ExecError::Interrupted`，且**不得**执行该调用之后的任何副作用）。
    ///
    /// 截止时刻用 `Instant` 单调钟算（不累计每片的名义长度 ⇒ 不随片数漂移）；每片 ≤ 10 ms。
    /// `total_ns <= 0` ⇒ 不睡直接完成；进入时已处于中断态 ⇒ 直接已取消（不发起 provider 调用）。
    pub fn cancellable_sleep_ns(&self, total_ns: i64) -> Result<bool, CapabilityCallError> {
        if self.interrupted() {
            return Ok(false);
        }
        if total_ns <= 0 {
            return Ok(true);
        }
        let deadline = Instant::now() + Duration::from_nanos(total_ns as u64);
        loop {
            if self.interrupted() {
                // `CP-28`：片间取消 ⇒ "已取消"；此前的睡眠只是"等待"，没有外部副作用。
                return Ok(false);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(true);
            }
            let slice = (remaining.as_nanos().min(CANCEL_SLICE_NS as u128)) as i64;
            self.clock_sleep_slice_ns(slice)?;
        }
    }

    /// **`select.select` 的可取消版**（`CP-27`）：分流语义同 [`Instance::capability_select`]，
    /// 但把等待切成 ≤10 ms 的片，期间 `pa_interrupt` ⇒ `Ok(None)`（**已取消**；调用方转
    /// `ExecError::Interrupted`）。`timeout_ns`：`None` 无限、`0` 轮询、`>0` 纳秒。
    ///
    /// 无限等待同样是**循环分片**（不会一次永久阻塞）；`fs` 恒就绪项直接返回（`CP-28` 已完成）。
    pub fn capability_select_cancellable(
        &self,
        read: &[i64],
        write: &[i64],
        error: &[i64],
        timeout_ns: Option<i64>,
    ) -> Result<Option<(Vec<i64>, Vec<i64>, Vec<i64>)>, CapabilityCallError> {
        // 先做一次非阻塞探测：`fs` 恒就绪／`net` 已有就绪/错误 ⇒ 立即"已完成"。
        let probe = self.capability_select(read, write, error, Some(0))?;
        if !probe.0.is_empty() || !probe.1.is_empty() || !probe.2.is_empty() {
            return Ok(Some(probe));
        }
        let has_inputs = !read.is_empty() || !write.is_empty() || !error.is_empty();
        let deadline = match timeout_ns {
            None => None,
            Some(ns) if ns <= 0 => return Ok(Some(probe)),
            Some(ns) => Some(Instant::now() + Duration::from_nanos(ns as u64)),
        };
        loop {
            if self.interrupted() {
                return Ok(None); // `CP-28`：已取消
            }
            let slice = match deadline {
                None => CANCEL_SLICE_NS,
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Ok(Some((Vec::new(), Vec::new(), Vec::new()))); // 已完成（超时）
                    }
                    (remaining.as_nanos().min(CANCEL_SLICE_NS as u128)) as i64
                }
            };
            if has_inputs {
                // `net` fd 的 `poll` 用片长做上界 ⇒ 阻塞也 ≤ 10 ms 就回来查中断。
                let ready = self.capability_select(read, write, error, Some(slice))?;
                if !ready.0.is_empty() || !ready.1.is_empty() || !ready.2.is_empty() {
                    return Ok(Some(ready));
                }
            } else {
                // 空 fd 列表：POSIX 的 `select` 就是"睡满 timeout"（`T-CP-10` 前是立即返回）。
                self.clock_sleep_slice_ns(slice)?;
            }
        }
    }
}
