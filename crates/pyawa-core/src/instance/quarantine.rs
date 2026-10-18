use super::*;

impl Instance {
    /// **诊断**：`ptr` 是不是隔离区里的**已释放对象**？是就报**原类型**与**当前帧的 qualname** ✓。
    ///
    /// 只在 `PYAWA_QUARANTINE=1` 时工作 ✓（其余时候第一句就返回 ✓）。
    pub(crate) fn quarantine_report(&self, ptr: *mut Header, what: &str) {
        if !quarantine_mode() {
            return;
        }
        let address = ptr as usize;
        let found = self
            .quarantine
            .borrow()
            .iter()
            .find(|(address_of, _, _, _)| *address_of == address)
            .cloned();
        let Some((_, size, name, site)) = found else {
            return;
        };
        let frame = self
            .current_frame()
            .and_then(|frame| {
                // SAFETY: 当前帧由执行器的守卫挂着，存活 ⇒ 指针指向一个 `Frame` 载荷。
                let code = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() }.code()?;
                // SAFETY: 代码对象由函数对象持有，存活 ⇒ 指针指向一个 `CodeObject` 载荷。
                Some(
                    unsafe { &*code.as_ptr().cast::<crate::code::CodeObject>() }
                        .qualname()
                        .to_owned(),
                )
            })
            .unwrap_or_else(|| "<无当前帧>".to_owned());
        eprintln!(
            "[隔离区] {what} 撞上**已释放对象** {address:#x}（原类型 {name}，{size} 字节；释放于 {site}）⇒ 提前释放／多放一份 ✗；当前帧：{frame}"
        );
    }

    /// 从"存活集合"与回收链表上同时摘除。
    /// 隔离区：**毒化载荷** ＋ 记账 ✓（第 272 轮）。
    pub(crate) fn quarantine_put(&self, header: NonNull<Header>) {
        let ty = unsafe { header.as_ref() }.ty();
        let size = unsafe { ty.as_ref() }.instance_size;
        let name = self.type_name(ty);
        let site = self.current_site();
        let payload = header.as_ptr().cast::<u8>();
        let head = core::mem::size_of::<Header>();
        // SAFETY: 载荷大小来自类型元数据 ✓；对象已不在活表里、且不再交还分配器 ✓ ⇒ 本层独占 ✓。
        unsafe {
            core::ptr::write_bytes(payload.add(head), 0xDE, size.saturating_sub(head));
        }
        self.quarantine
            .borrow_mut()
            .push((header.as_ptr() as usize, size, name, site));
    }

    /// **当前执行现场**（第 297 轮诊断）：`<帧 qualname>@<指令指针>`；没有当前帧时给个占位。
    pub(crate) fn current_site(&self) -> String {
        let Some(frame) = self.current_frame() else {
            return "<无当前帧>".to_owned();
        };
        // SAFETY: 当前帧由执行器的守卫挂着，存活 ⇒ 指针指向 `Frame` 载荷。
        let frame = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() };
        let pointer = frame.instruction_pointer();
        let name = frame
            .code()
            .map(|code| {
                // SAFETY: 代码对象由函数对象持有，存活 ⇒ 指针指向 `CodeObject` 载荷。
                unsafe { &*code.as_ptr().cast::<crate::code::CodeObject>() }
                    .qualname()
                    .to_owned()
            })
            .unwrap_or_else(|| "<无代码对象>".to_owned());
        format!("{name}@{pointer}")
    }

    /// 复核隔离区 ✓：毒化字节被改 ⇒ **释放后仍被写** ✓（use-after-free ✗）。
    pub(crate) fn quarantine_check(&self) {
        if !quarantine_mode() {
            return;
        }
        let head = core::mem::size_of::<Header>();
        let suspects: Vec<(usize, usize, String, String)> = self
            .quarantine
            .borrow()
            .iter()
            .filter(|(address, size, _, _)| {
                let payload = (*address as *mut u8).wrapping_add(head);
                let length = size.saturating_sub(head);
                // SAFETY: 隔离区的对象**没有**还给分配器 ⇒ 这段内存仍属本层 ✓。
                unsafe { (0..length).any(|index| *payload.add(index) != 0xDE) }
            })
            .cloned()
            .collect();
        if let Some((address, size, name, site)) = suspects.first() {
            eprintln!(
                "[隔离区] {address:#x}（{name}，{size} 字节；释放于 {site}）的载荷在**释放之后**被写过 ✗ ⇒ use-after-free ✓"
            );
            std::process::exit(3);
        }
    }

    pub(crate) fn unlink(&self, header: NonNull<Header>) {
        // **释放探针**（第 112 轮，`PYAWA_FREE_DEBUG=1`）：每次真正摘除一个对象都报
        // **地址 ＋ 类型 ＋ Python 现场 ＋ Rust 回溯** ✓ —— 用来分辨"同一条指令放了两次" ✗
        // 还是"重绑放一次、调用收尾又放一次" ✗（上限榜那一族的内存缺陷 ✓，见第 107～111 轮台账 ✓）。
        if flag("PYAWA_FREE_DEBUG") {
            // SAFETY: header 由调用方保证存活（正要摘除）。
            let name = unsafe { header.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            let name = unsafe { name.as_ref() }.name().to_owned();
            eprintln!(
                "[free 探针] {:#x} 类型={name} 现场={}\n{}",
                header.as_ptr() as usize,
                self.current_site(),
                std::backtrace::Backtrace::force_capture()
            );
        }
        if self.zombie_trace.get() {
            // SAFETY: header 由调用方保证存活（正要摘除）。
            let name = unsafe { header.as_ref() }.ty();
            // SAFETY: ty 由类型注册表持有。
            let name = unsafe { name.as_ref() }.name().to_owned();
            // **只记"第一次"释放** ✓（第 89 轮）：地址会被复用 ✓ ⇒ 后一次释放会把现场覆盖掉 ✗
            // ⇒ 那样只能看到"最后那个占着它的对象是谁" ✓（实测就是 `str` ✓，不是我们要抓的 ✓）。
            // 用 `or_insert` 留住**最早**那次释放的现场 ✓ —— 那才是"谁把这个 dict 放多了" ✓。
            self.freed_sites
                .borrow_mut()
                .insert(header.as_ptr() as usize, (name, self.current_site()));
        }
        self.quarantine_check();
        // **野释放检测** ✓（第 238 轮，**与布局无关** ✓、**先查后删** ✓）：要摘除的地址**必须在活表里** ✓。
        // 不在 ⇒ 三种可能：**从没分配过** ✗／**已经释放过** ✗（glibc 要到**进程退出**才报
        // `tcache_thread_shutdown(): unaligned tcache chunk detected` ✓）／**内部指针** ✗。
        // 注意：地址会被复用 ✓ ⇒ 所以判据是"**摘除时**在不在表里" ✓（不在 ⇒ 一定放多了 ✓），不会假阳性 ✓。
        if !self.live.borrow_mut().remove(&(header.as_ptr() as usize)) {
            eprintln!(
                "[野释放] {:#x} 不在活表里 ✗ —— 重复释放／内部指针／从未分配（见 PLAN 第 183 轮 ✓）",
                header.as_ptr() as usize
            );
            std::process::abort();
        }
        if ruler_on() {
            let refs = self.who_points_at(header.as_ptr() as usize);
            if !refs.is_empty() {
                eprintln!(
                    "[尺子] {:#x}（{}）仍被 {} 处指着 ✗：{:?}",
                    header.as_ptr() as usize,
                    self.type_name(unsafe { header.as_ref() }.ty()),
                    refs.len(),
                    refs
                );
            }
        }
        // SAFETY: header 尚未释放。
        if unsafe { header.as_ref() }.has_flag(flags::GC_TRACKED) {
            self.unlink_gc(header);
        }
    }

}
