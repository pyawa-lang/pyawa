//! **`CP-31` 统一实例能力表**：`fd` ＝ 本表下标（`fd` 只是 `int`，`CP-14`）。
//!
//! 为什么要有这一层（`DESIGN.md` §7.1）：POSIX 的 `fd` 本来就是"进程内文件表的下标"，
//! 所以"provider 给不透明能力对象 ＋ VM 侧按实例的能力表"是对 POSIX 语义的**忠实实现**。
//! 在这一层落地之前，provider 的句柄被**原样**当 `fd` 交给 Python ⇒
//! ① 实例销毁不知道关哪些句柄（`CP-19` 缺口）；② `fs`／`net` 号段各自从 3 起，同一个 `int`
//! 可以指两个不同的能力；③ `os.dup2`（表项复制）无从实现（`CP-31`）。
//!
//! # 语义
//!
//! - 条目记 `(domain, provider_handle)`；`fd` 是 `slots` 的下标，**分配从 [`FIRST_DYNAMIC_FD`]**
//!   （0／1／2 是标准流，沿用 `_io` 的 `STDOUT_HANDLE`／`STDERR_HANDLE` 约定，不占表槽）；
//! - **O(1) 分配／释放**：`free` 是空闲 fd 的栈（LIFO）；释放只清槽＋把 fd 压栈（不缩容）；
//! - **`dup`／`dup2` 的别名语义**（`CP-31`／`CP-17`）：表项复制指向**同一个** provider 句柄 ⇒
//!   按 `(domain, provider_handle)` 记**引用计数**；`close(fd)` 只减计数，**计数归零**才调
//!   provider 的 `close` 槽。否则"关掉一个别名"会让另一个别名变成 `EBADF`，违反 POSIX `dup` 语义；
//! - **`CP-19` 兜底**：实例销毁时 [`Instance::capability_close_all`] 逐条（每个 provider 句柄一次）
//!   调 `close` 槽，不依赖回收器先跑完；
//! - **`CP-18`**：`fd` 不在表里（已关／从未分配／越界）⇒ 机器错误 `EBADF` 形状，
//!   **不是**未定义行为；例外是 `close` 自身：`CP-17` 要求它**幂等**，已关闭再关 ⇒ 成功；
//! - **`dup2` 到标准流 0／1／2 未接**：标准流不占表槽（`_io` 的约定）⇒ `dup2(old, 0|1|2)`
//!   返回 `EBADF` 形状的机器错误。这是**如实偏差**（参照实现能重定向标准流；本层等
//!   "标准流也进表"专项，属后续轮）。`dup`／`dup2` 到动态段（≥3）**已接**。
//!
//! # `select` 面的分流（`T-CP-3`；`CP-31`）
//!
//! [`Instance::capability_select`] 是 `select.select` 的 VM 侧入口：
//! - **`fs` 域 fd**（含标准流 0／1／2）：POSIX 常规文件 ⇒ **恒可读可写**，不进 provider；
//! - **`net` 域 fd**：走 `CpNetVtable.poll` 槽（provider 由 `pyawa-runtime::net_posix` 实现）；
//!   槽为 `None` ⇒ 如实 `NotImplemented`（`CP-3`／`CP-5`）；
//! - **混合列表**：两域按上面的口径分流后合并；只要有 `fs` 就绪项，就**不阻塞**（`timeout` 归 0）；
//! - **域查不到／已关 fd** ⇒ `EBADF` 形状的机器错误（`CP-18`）。
//!
//! **如实缺口**（不在此伪造）：空 fd 列表 ＋ 正时长的 `select`（"纯睡眠"）**不真睡**——
//! `clock` 域的 `sleep_ns` 槽还没接，且真睡眠的取消令牌属 `T-CP-10`（见 `SPEC-capabilities.md`
//! `CP-27`）；现在立即返回空就绪集，行为与参照不同（调用方拿不到"等到了"的语义）。
//!
//! # 公开路径
//!
//! `lib.rs` **没有**导出本模块（不在本轮写域）⇒ 公开面全部挂在 [`Instance`] 的**关联方法／常量**上：
//! `capability_dup`／`capability_dup2`／`capability_domain`／`capability_fd_count`／
//! `capability_select` 与 `Instance::POLLIN` 一族。

use super::*;
use std::collections::HashMap;

/// 动态 `fd` 的起点：`0`／`1`／`2` 是标准流（`_io` 的约定），**不占**能力表槽位。
pub(crate) const FIRST_DYNAMIC_FD: u64 = 3;

/// 平台注入表缺失时（只有裸 [`Instance::new`]，无 `pa_create` 注入）借 POSIX 的 `EBADF` 值。
///
/// **不是第二真相**：正式实例由 `pa_create` 注入平台 errno 表 ⇒ `platform_constant("EBADF")`
/// 优先；这个兜底只服务"没有宿主注入"的单元测试。
const FALLBACK_EBADF: i32 = 9;

// `poll(2)` 的事件位（**POSIX 规范值**，不是平台 errno ⇒ 不随平台变化）。
pub(crate) const POLL_EVENT_IN: i32 = 0x001;
pub(crate) const POLL_EVENT_PRI: i32 = 0x002;
pub(crate) const POLL_EVENT_OUT: i32 = 0x004;
pub(crate) const POLL_EVENT_ERR: i32 = 0x008;
pub(crate) const POLL_EVENT_HUP: i32 = 0x010;
pub(crate) const POLL_EVENT_NVAL: i32 = 0x020;

/// 能力表的一项：`fd → (域, provider 的不透明句柄)`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CapabilityTableEntry {
    /// 能力域编号（`pyawa_capabilities::DOMAIN_*`）。
    pub(crate) domain: usize,
    /// provider 自管的不透明句柄（`AB-14`：VM 只存不解释）。
    pub(crate) provider_handle: u64,
}

/// 释放一个 `fd` 的结局（`CP-17`／`CP-18` 的分叉点）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CapabilityRelease {
    /// 这个 `fd` 不在表里（已关／从未分配）⇒ `close` 幂等返回成功，别的操作报 `EBADF`。
    Absent,
    /// 表项已释放，但同一 provider 句柄还有别的别名 ⇒ **不调** provider `close`。
    Shared,
    /// 最后一个别名 ⇒ 调用方必须调 provider 的 `close` 槽（`CP-17` 的释放）。
    Last(CapabilityTableEntry),
}

/// 每实例一张的 `fd → 能力对象` 表（`CP-13`：禁止进程级表）。
pub(crate) struct CapabilityTable {
    /// `slots[fd]`；`0..FIRST_DYNAMIC_FD` 恒为 `None`（标准流不占槽）。
    slots: Vec<Option<CapabilityTableEntry>>,
    /// 空闲 `fd` 栈（LIFO；可能含已被 `dup2` 直接占据的旧项，分配时跳过）。
    free: Vec<u64>,
    /// 按 `(domain, provider_handle)` 的引用计数（`dup`／`dup2` 的别名语义）。
    references: HashMap<(usize, u64), usize>,
}

impl CapabilityTable {
    /// 空表：前三个槽位留给标准流。
    pub(crate) fn new() -> Self {
        Self {
            slots: vec![None; FIRST_DYNAMIC_FD as usize],
            free: Vec::new(),
            references: HashMap::new(),
        }
    }

    fn allocate_fd(&mut self) -> u64 {
        while let Some(candidate) = self.free.pop() {
            let usable = self
                .slots
                .get(candidate as usize)
                .is_some_and(|slot| slot.is_none());
            if usable {
                return candidate;
            }
        }
        let fd = self.slots.len() as u64;
        self.slots.push(None);
        fd
    }

    fn place(&mut self, fd: u64, entry: CapabilityTableEntry) {
        while self.slots.len() <= fd as usize {
            self.slots.push(None);
        }
        self.slots[fd as usize] = Some(entry);
        *self.references.entry((entry.domain, entry.provider_handle)).or_insert(0) += 1;
    }

    /// 登记一个**新**能力对象 ⇒ 分配一个 `fd`（`fs_open`／`net_socket` 一族的落点）。
    pub(crate) fn insert_new(&mut self, entry: CapabilityTableEntry) -> u64 {
        let fd = self.allocate_fd();
        self.place(fd, entry);
        fd
    }

    /// `fd` 当前的条目（**借用语义**：只是拷贝出来的 `(domain, handle)`）。
    pub(crate) fn get(&self, fd: u64) -> Option<CapabilityTableEntry> {
        self.slots.get(fd as usize).copied().flatten()
    }

    /// 复制表项到**新** `fd`（`dup`；同一 provider 句柄 ⇒ 引用计数 +1）。
    pub(crate) fn duplicate(&mut self, entry: CapabilityTableEntry) -> u64 {
        self.insert_new(entry)
    }

    /// 把表项放到**指定** `fd`（`dup2`；调用方已先释放目标槽）。
    pub(crate) fn insert_at(&mut self, fd: u64, entry: CapabilityTableEntry) {
        debug_assert!(fd >= FIRST_DYNAMIC_FD);
        self.place(fd, entry);
    }

    /// 释放一个 `fd`（`close` 的落点）；引用计数归零 ⇒ 返回 [`CapabilityRelease::Last`]。
    pub(crate) fn release(&mut self, fd: u64) -> CapabilityRelease {
        let Some(entry) = self.slots.get_mut(fd as usize).and_then(Option::take) else {
            return CapabilityRelease::Absent;
        };
        self.free.push(fd);
        let key = (entry.domain, entry.provider_handle);
        let remaining = {
            let count = self.references.entry(key).or_insert(1);
            *count -= 1;
            *count
        };
        if remaining == 0 {
            self.references.remove(&key);
            CapabilityRelease::Last(entry)
        } else {
            CapabilityRelease::Shared
        }
    }

    /// 当前**占用中**的 `fd` 个数（测试／诊断用；不含 0／1／2 标准流）。
    pub(crate) fn live_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    /// 清空整张表，交出**每个 provider 句柄一次**（`CP-19` 的兜底释放用）。
    pub(crate) fn drain(&mut self) -> Vec<CapabilityTableEntry> {
        let mut entries: Vec<CapabilityTableEntry> = Vec::new();
        for slot in &self.slots {
            if let Some(entry) = slot {
                if !entries.contains(entry) {
                    entries.push(*entry);
                }
            }
        }
        self.slots.clear();
        self.slots.extend((0..FIRST_DYNAMIC_FD).map(|_| None));
        self.free.clear();
        self.references.clear();
        entries
    }
}

impl Instance {
    // ---- `poll(2)` 事件位（`T-CP-3` 的 `select` 面；公开路径 `pyawa_core::Instance::POLLIN`）----
    //
    // 为什么挂在 `Instance` 上：`pyawa-capabilities` 是本轮**写域之外**、`lib.rs` 也没有导出
    // `instance` 模块 ⇒ 用关联常量给 `pyawa-runtime`／`pyawa-stdlib` 一个可引用的公开路径。
    // 取值是 POSIX `poll(2)` 的**规范值**（不是平台 errno ⇒ 不需要平台注入表）。

    /// `POLLIN`：有数据可读（`poll(2)` 规范值）。
    pub const POLLIN: i32 = POLL_EVENT_IN;
    /// `POLLPRI`：有紧急数据（`poll(2)` 规范值）。
    pub const POLLPRI: i32 = POLL_EVENT_PRI;
    /// `POLLOUT`：可写（`poll(2)` 规范值）。
    pub const POLLOUT: i32 = POLL_EVENT_OUT;
    /// `POLLERR`：错误条件（`poll(2)` 规范值）。
    pub const POLLERR: i32 = POLL_EVENT_ERR;
    /// `POLLHUP`：对端挂断（`poll(2)` 规范值）。
    pub const POLLHUP: i32 = POLL_EVENT_HUP;
    /// `POLLNVAL`：无效 fd（`poll(2)` 规范值）。
    pub const POLLNVAL: i32 = POLL_EVENT_NVAL;

    /// `EBADF` 形状的机器错误（`CP-18`）：平台注入表优先，兜底见 [`FALLBACK_EBADF`]。
    fn capability_ebadf(&self) -> CapabilityCallError {
        CapabilityCallError::Machine(
            self.platform_constant("EBADF").map_or(FALLBACK_EBADF, |value| value as i32),
        )
    }

    /// 把 `(domain, provider_handle)` 登记进实例能力表 ⇒ 返回**新的 `fd`**（`CP-31`）。
    pub(crate) fn capability_register(&self, domain: usize, provider_handle: u64) -> u64 {
        self.capability_table
            .borrow_mut()
            .insert_new(CapabilityTableEntry { domain, provider_handle })
    }

    /// `fd` 的**域**（`0`／`1`／`2` 标准流按 `fs` 域处理）；不在表里 ⇒ `None`。
    pub fn capability_domain(&self, fd: i64) -> Option<usize> {
        if (0..FIRST_DYNAMIC_FD as i64).contains(&fd) {
            return Some(pyawa_capabilities::DOMAIN_FS);
        }
        let fd = u64::try_from(fd).ok()?;
        self.capability_table.borrow().get(fd).map(|entry| entry.domain)
    }

    /// 当前能力表里**占用中**的 `fd` 个数（测试／诊断用；不含标准流）。
    pub fn capability_fd_count(&self) -> usize {
        self.capability_table.borrow().live_count()
    }

    /// `fd` → provider 句柄；**域必须相符**，否则 `EBADF`（`CP-18`）。
    ///
    /// 供 `fs`／`net` 适配层内部使用（不是 Python 可见面：Python 只见 `fd`）。
    pub(crate) fn capability_provider_handle(
        &self,
        fd: u64,
        expected_domain: usize,
    ) -> Result<u64, CapabilityCallError> {
        let Some(entry) = self.capability_table.borrow().get(fd) else {
            return Err(self.capability_ebadf());
        };
        if entry.domain != expected_domain {
            return Err(self.capability_ebadf());
        }
        Ok(entry.provider_handle)
    }

    /// 调 provider 的 `close` 槽（域分派；`CP-17` 的"外部资源释放的唯一入口"）。
    fn capability_provider_close(&self, entry: CapabilityTableEntry) -> Result<(), CapabilityCallError> {
        match entry.domain {
            pyawa_capabilities::DOMAIN_FS => self.fs_provider_close(entry.provider_handle),
            pyawa_capabilities::DOMAIN_NET => self.net_provider_close(entry.provider_handle),
            // v1 只有 `fs`／`net` 两句柄域有 `close` 槽；别的域不可能出现在表里。
            _ => Ok(()),
        }
    }

    /// 关一个 `fd`（`CP-17`／`CP-18` 的交汇点）：
    /// - 不在表里 ⇒ 成功（`CP-17`：`close` **幂等**）；
    /// - 还有别名 ⇒ 只减引用计数；
    /// - 最后一个别名 ⇒ 调 provider `close` 槽。
    pub(crate) fn capability_release(&self, fd: u64) -> Result<(), CapabilityCallError> {
        match self.capability_table.borrow_mut().release(fd) {
            CapabilityRelease::Absent | CapabilityRelease::Shared => Ok(()),
            CapabilityRelease::Last(entry) => self.capability_provider_close(entry),
        }
    }

    /// **实例销毁兜底**（`CP-19`）：清空能力表，每个 provider 句柄调一次 `close` 槽。
    ///
    /// 由 [`Drop for Instance`] 调用；错误一律忽略（销毁路径不能再抛）。
    pub(crate) fn capability_close_all(&self) {
        let entries = self.capability_table.borrow_mut().drain();
        for entry in entries {
            let _ = self.capability_provider_close(entry);
        }
    }

    /// **`dup(fd)`**（`CP-31`）：复制表项 ⇒ 新 `fd`；两个 `fd` 指向**同一个**能力对象。
    pub fn capability_dup(&self, fd: i64) -> Result<i64, CapabilityCallError> {
        let fd = u64::try_from(fd).map_err(|_| self.capability_ebadf())?;
        if fd < FIRST_DYNAMIC_FD {
            // 标准流不占表槽，无法复制（`_io` 的约定；`dup(stdout)` 不在本层范围）。
            return Err(self.capability_ebadf());
        }
        let entry = self
            .capability_table
            .borrow()
            .get(fd)
            .ok_or_else(|| self.capability_ebadf())?;
        let duplicate = self.capability_table.borrow_mut().duplicate(entry);
        i64::try_from(duplicate).map_err(|_| self.capability_ebadf())
    }

    /// **`dup2(old, new)`**（`CP-31`）：让 `new` 指向 `old` 的能力对象（先释放 `new` 原来的表项）。
    ///
    /// 返回 `new`（`os.dup2` 的 Python 面丢弃返回值；`os.dup` 另一入口见 [`Instance::capability_dup`]）。
    /// **限制（如实）**：`new` 必须落在动态段（`>= 3`）；`dup2` 到标准流 0／1／2 **未接** ⇒ `EBADF`。
    pub fn capability_dup2(&self, old: i64, new: i64) -> Result<i64, CapabilityCallError> {
        let old = u64::try_from(old).map_err(|_| self.capability_ebadf())?;
        let new = u64::try_from(new).map_err(|_| self.capability_ebadf())?;
        if old < FIRST_DYNAMIC_FD || new < FIRST_DYNAMIC_FD {
            return Err(self.capability_ebadf());
        }
        let entry = self
            .capability_table
            .borrow()
            .get(old)
            .ok_or_else(|| self.capability_ebadf())?;
        if old == new {
            return Ok(new as i64);
        }
        // 目标表项先按 `close` 语义释放（如果它是某个 provider 句柄的最后一个别名，会真关）。
        if let CapabilityRelease::Last(previous) =
            self.capability_table.borrow_mut().release(new)
        {
            self.capability_provider_close(previous)?;
        }
        self.capability_table.borrow_mut().insert_at(new, entry);
        Ok(new as i64)
    }

    /// **`select.select` 的 VM 侧入口**（`T-CP-3`；`CP-31` 的表是它的前置）。
    ///
    /// 入参／返回都是 `fd` 列表（`i64`，Python 的 `int` 子集；负数 ⇒ `EBADF`）。
    /// 分流口径见本模块文档：`fs` 域（含标准流）恒就绪；`net` 域走 `poll` 槽；
    /// `timeout_ns`：`None` ＝ 无限等待、`0` ＝ 轮询、`>0` ＝ 纳秒上限。
    ///
    /// 返回 `(可读, 可写, 异常)` 三个就绪 `fd` 列表（**去重**；顺序不承诺，与 `select(2)` 一致）。
    pub fn capability_select(
        &self,
        read: &[i64],
        write: &[i64],
        error: &[i64],
        timeout_ns: Option<i64>,
    ) -> Result<(Vec<i64>, Vec<i64>, Vec<i64>), CapabilityCallError> {
        // 列表编号：0＝可读、1＝可写、2＝异常。
        let mut inputs: Vec<(i64, i32, u8)> = Vec::new();
        for fd in read {
            inputs.push((*fd, POLL_EVENT_IN, 0));
        }
        for fd in write {
            inputs.push((*fd, POLL_EVENT_OUT, 1));
        }
        for fd in error {
            inputs.push((*fd, POLL_EVENT_PRI, 2));
        }

        // 1) 分流：`fs`（含标准流）恒就绪；`net` 收进 `poll` 的输入集。
        let mut fs_ready: Vec<(i64, u8)> = Vec::new();
        let mut net_order: Vec<(i64, u64)> = Vec::new();
        let mut net_events: HashMap<u64, i32> = HashMap::new();
        for (fd, events, list) in &inputs {
            match self.capability_classify_fd(*fd)? {
                FdClass::Fs => {
                    if !fs_ready.contains(&(*fd, *list)) {
                        fs_ready.push((*fd, *list));
                    }
                }
                FdClass::Net(handle) => {
                    *net_events.entry(handle).or_insert(0) |= *events;
                    if !net_order.contains(&(*fd, handle)) {
                        net_order.push((*fd, handle));
                    }
                }
            }
        }

        // 2) `net` 部分：一次 `poll`（`CP-3`：槽为 `None` ⇒ 如实未实现）。
        let mut net_ready: Vec<(i64, u8)> = Vec::new();
        if !net_order.is_empty() {
            let net_table = self.net_vtable().ok_or(CapabilityCallError::NotRegistered)?;
            let poll = net_table.poll.ok_or(CapabilityCallError::NotImplemented)?;
            let poll_fds: Vec<pyawa_capabilities::net::PollFd> = net_order
                .iter()
                .map(|(_, handle)| pyawa_capabilities::net::PollFd {
                    handle: pyawa_capabilities::net::SocketHandle(*handle),
                    events: net_events.get(handle).copied().unwrap_or(0),
                    revents: 0,
                })
                .collect();
            // 已有 `fs` 就绪项 ⇒ 不阻塞（POSIX：有任一就绪立即返回）。
            let timeout = if fs_ready.is_empty() { timeout_ns.unwrap_or(-1) } else { 0 };
            let mut capacity = poll_fds.len();
            let ready = loop {
                let mut output = vec![
                    pyawa_capabilities::net::PollFd {
                        handle: pyawa_capabilities::net::SocketHandle(0),
                        events: 0,
                        revents: 0,
                    };
                    capacity
                ];
                let mut total = 0usize;
                let mut errno = 0i32;
                match poll(
                    net_table.state,
                    poll_fds.as_ptr(),
                    poll_fds.len(),
                    timeout,
                    output.as_mut_ptr(),
                    capacity,
                    &mut total,
                    &mut errno,
                ) {
                    pyawa_capabilities::net::CapStatus::Ok => {
                        if total <= capacity {
                            output.truncate(total);
                            break output;
                        }
                        // 两段式出参：总项数超容量 ⇒ 扩缓冲重来（同 `fs.readlink` 约定）。
                        capacity = total;
                    }
                    pyawa_capabilities::net::CapStatus::Unimplemented => {
                        return Err(CapabilityCallError::NotImplemented)
                    }
                    pyawa_capabilities::net::CapStatus::Machine => {
                        return Err(CapabilityCallError::Machine(errno))
                    }
                }
            };
            let revents: HashMap<u64, i32> = ready
                .iter()
                .map(|item| (item.handle.0, item.revents))
                .collect();
            for (fd, events, list) in &inputs {
                let FdClass::Net(handle) = self.capability_classify_fd(*fd)? else {
                    continue;
                };
                let bits = revents.get(&handle).copied().unwrap_or(0);
                let hit = match *list {
                    0 => bits & POLL_EVENT_IN != 0,
                    1 => bits & POLL_EVENT_OUT != 0,
                    _ => bits
                        & (POLL_EVENT_PRI | POLL_EVENT_ERR | POLL_EVENT_HUP | POLL_EVENT_NVAL)
                        != 0,
                };
                if hit && !net_ready.contains(&(*fd, *list)) {
                    net_ready.push((*fd, *list));
                }
                let _ = events;
            }
        }

        // 3) 合并（去重；顺序不承诺）。
        let mut ready_read: Vec<i64> = Vec::new();
        let mut ready_write: Vec<i64> = Vec::new();
        let mut ready_error: Vec<i64> = Vec::new();
        for (fd, list) in fs_ready.into_iter().chain(net_ready) {
            let target = match list {
                0 => &mut ready_read,
                1 => &mut ready_write,
                _ => &mut ready_error,
            };
            if !target.contains(&fd) {
                target.push(fd);
            }
        }
        Ok((ready_read, ready_write, ready_error))
    }

    /// `select` 的分流：`fd` 属哪个域（`CP-18`：无效 ⇒ `EBADF`）。
    fn capability_classify_fd(&self, fd: i64) -> Result<FdClass, CapabilityCallError> {
        if fd < 0 {
            return Err(self.capability_ebadf());
        }
        let raw = fd as u64;
        if raw < FIRST_DYNAMIC_FD {
            // 标准流：沿用 `_io` 的约定，按 `fs` 域处理（表外，恒就绪）。
            return Ok(FdClass::Fs);
        }
        let entry = self
            .capability_table
            .borrow()
            .get(raw)
            .ok_or_else(|| self.capability_ebadf())?;
        match entry.domain {
            pyawa_capabilities::DOMAIN_FS => Ok(FdClass::Fs),
            pyawa_capabilities::DOMAIN_NET => Ok(FdClass::Net(entry.provider_handle)),
            _ => Err(self.capability_ebadf()),
        }
    }
}

/// `select` 分流的结果（`fs` 域恒就绪；`net` 域要问 `poll` 槽）。
enum FdClass {
    Fs,
    Net(u64),
}
