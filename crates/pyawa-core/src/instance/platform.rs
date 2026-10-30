//! `Instance` 的平台/时钟/能力域方法（从 `instance.rs` 整体搬来，纯移动、零逻辑改动）。
//!
//! 子模块看得见父模块的私有字段 ⇒ `use super::*;` 即可 ✓。

use super::*;

impl Instance {
    /// **经 `clock` 域取挂钟纳秒**（`CP-2`／`CP-3`／`CP-5` 的三态在这里落成结果 ✓）。
    /// 调用方（stdlib 的 `time` 模块）只管把纳秒折成秒；**平台**在提供者那边 ✓（`CX-4`）。
    pub fn clock_now_ns(&self) -> Result<i64, CapabilityCallError> {
        let Some(table) = self.clock_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(now) = table.now_ns else {
            return Err(CapabilityCallError::NotImplemented);
        };
        let mut value = 0i64;
        match now(table.state, &mut value) {
            pyawa_capabilities::clock::CapStatus::Ok => Ok(value),
            pyawa_capabilities::clock::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::clock::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(0))
            }
        }
    }

    /// **经 `clock` 域取单调钟纳秒**（同上 ✓）。
    pub fn clock_monotonic_ns(&self) -> Result<i64, CapabilityCallError> {
        let Some(table) = self.clock_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(monotonic) = table.monotonic_ns else {
            return Err(CapabilityCallError::NotImplemented);
        };
        let mut value = 0i64;
        match monotonic(table.state, &mut value) {
            pyawa_capabilities::clock::CapStatus::Ok => Ok(value),
            pyawa_capabilities::clock::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::clock::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(0))
            }
        }
    }

    /// **经 `clock` 域取本地时间**（`AB-41` 表尾追加槽 ①）：
    /// `secs` ⇒ [`pyawa_capabilities::clock::CpTm`]（含 `tm_gmtoff`／`tm_zone`）。
    ///
    /// `time.localtime` 用它拿**真实时区**（stdlib 侧没有时区库 ✓）；槽缺失／未实现／机器错误
    /// 三态照 [`Instance::clock_now_ns`] 的映射（`CP-5`）。
    pub fn clock_localtime(
        &self,
        secs: i64,
    ) -> Result<pyawa_capabilities::clock::CpTm, CapabilityCallError> {
        let Some(table) = self.clock_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(localtime) = table.localtime else {
            return Err(CapabilityCallError::NotImplemented);
        };
        let mut tm = pyawa_capabilities::clock::CpTm::default();
        let mut errno = 0i32;
        match localtime(table.state, secs, &mut tm, &mut errno) {
            pyawa_capabilities::clock::CapStatus::Ok => Ok(tm),
            pyawa_capabilities::clock::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::clock::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(errno))
            }
        }
    }

    /// **经 `clock` 域做 C locale 的 `strftime`**（表尾追加槽 ②）：`fmt` 是 UTF-8 字节、
    /// `tm` 是 [`pyawa_capabilities::clock::CpTm`] ⇒ 格式化结果。
    ///
    /// 缓冲按 `fs.readlink` 的**两段式**约定重试：先 512 字节；provider 回的总长超容量就按总长
    /// 再要一次（最多 4 次，够不到就按机器错误处理——`CP-5` 不编造成功）。
    pub fn clock_strftime(
        &self,
        fmt: &[u8],
        tm: &pyawa_capabilities::clock::CpTm,
    ) -> Result<String, CapabilityCallError> {
        let Some(table) = self.clock_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(strftime) = table.strftime else {
            return Err(CapabilityCallError::NotImplemented);
        };
        let mut capacity = 512usize;
        for _ in 0..4 {
            let mut buffer: Vec<u8> = vec![0u8; capacity];
            let mut needed = 0usize;
            let mut errno = 0i32;
            match strftime(
                table.state,
                fmt.as_ptr(),
                fmt.len(),
                tm,
                buffer.as_mut_ptr(),
                capacity,
                &mut needed,
                &mut errno,
            ) {
                pyawa_capabilities::clock::CapStatus::Ok => {
                    if needed <= capacity {
                        buffer.truncate(needed);
                        return Ok(String::from_utf8_lossy(&buffer).into_owned());
                    }
                    capacity = needed;
                }
                pyawa_capabilities::clock::CapStatus::Unimplemented => {
                    return Err(CapabilityCallError::NotImplemented)
                }
                pyawa_capabilities::clock::CapStatus::Machine => {
                    return Err(CapabilityCallError::Machine(errno))
                }
            }
        }
        // 4 次扩容仍装不下：如实按机器错误报（不截断、不伪装成功）。`errno` 用 0（本层没有
        // 专门的"缓冲不足"errno；调用方按 `Machine` 转成 `OSError`）。
        Err(CapabilityCallError::Machine(0))
    }

    /// **经 `clock` 域做 `mktime`**（表尾追加槽 ③）：本地时间 [`pyawa_capabilities::clock::CpTm`]
    /// ⇒ Unix 秒（C 语义：越界字段由 provider 的 libc 归一化）。
    pub fn clock_mktime(
        &self,
        tm: &pyawa_capabilities::clock::CpTm,
    ) -> Result<i64, CapabilityCallError> {
        let Some(table) = self.clock_vtable() else {
            return Err(CapabilityCallError::NotRegistered);
        };
        let Some(mktime) = table.mktime else {
            return Err(CapabilityCallError::NotImplemented);
        };
        let mut seconds = 0i64;
        let mut errno = 0i32;
        match mktime(table.state, tm, &mut seconds, &mut errno) {
            pyawa_capabilities::clock::CapStatus::Ok => Ok(seconds),
            pyawa_capabilities::clock::CapStatus::Unimplemented => {
                Err(CapabilityCallError::NotImplemented)
            }
            pyawa_capabilities::clock::CapStatus::Machine => {
                Err(CapabilityCallError::Machine(errno))
            }
        }
    }

    /// **`fs` 域的形状视图**（`SPEC-capabilities.md` §9.1）：把宿主注册的不透明指针按
    /// [`pyawa_capabilities::fs::CpFsVtable`] 解释 ✓。`None` ＝ 该域未提供（`CP-2` ✓）。
    ///
    /// # Safety
    ///
    /// 注册方（宿主）必须保证：该指针指向一个**在实例存活期间有效**的 `CpFsVtable` ✓
    /// （`AB-16`／`AB-17` 的借用纪律由调用方遵守 ✓）。
    /// **`clock` 域的形状视图**（`SPEC-capabilities.md` §4⑷）：与 [`Instance::fs_vtable`] 同一手法 ✓。
    /// `None` ＝ 该域未提供（`CP-2` ✓）。
    ///
    /// # Safety
    ///
    /// 注册方（宿主）必须保证：该指针指向一个**在实例存活期间有效**的
    /// `CpClockVtable` ✓（`AB-16`／`AB-17` 的借用纪律由调用方遵守 ✓）。
    pub fn clock_vtable(&self) -> Option<pyawa_capabilities::clock::CpClockVtable> {
        let pointer = self.capability(pyawa_capabilities::DOMAIN_CLOCK)?;
        // SAFETY: 见函数文档——注册方保证指针有效且布局正确。
        Some(unsafe { *pointer.cast::<pyawa_capabilities::clock::CpClockVtable>() })
    }

    /// **当前帧对象**（第 230 轮，**借用**）：`sys._getframe()` 的取值口 ✓（与全局映射同款 RAII ✓）。
    pub fn current_frame(&self) -> Option<NonNull<Header>> {
        self.current_frame.get()
    }

    /// **当前帧的全局映射**（第 156 轮，**借用**）：`globals()` 的取值口 ✓。
    pub fn current_globals(&self) -> Option<NonNull<Header>> {
        self.current_globals.get()
    }

    /// 按**名字**取平台常量（`CM-20`：映射按名字匹配，**禁止**硬编码数字）。
    pub fn platform_constant(&self, name: &str) -> Option<i64> {
        let table = self.platform_constants.borrow();
        table
            .binary_search_by_key(&name, |(candidate, _)| *candidate)
            .ok()
            .map(|position| table[position].1)
    }

    /// **整张平台常量表**（第 134 轮）：`errno` 模块要按**整表**建名字空间 ✓
    /// （`platform_constant` 只按名查 ✗ ⇒ `errno_module::build` 收的是一张切片 ✓）。
    pub fn platform_constants(&self) -> Vec<(&'static str, i64)> {
        self.platform_constants.borrow().clone()
    }

    /// 平台常量条数（测试与诊断用）。
    pub fn platform_constants_len(&self) -> usize {
        self.platform_constants.borrow().len()
    }

    /// **BC-60** ②：**本实例**当前正在处理的异常（**借用**）。
    pub fn current_exception(&self) -> Option<NonNull<Header>> {
        self.exception_state.borrow().last().copied()
    }
}
