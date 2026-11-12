//! **调用边所有权窄类型**（B3 试点，round 252）——`OM-16`／`OM-19`／`AB-15`／`AB-16`。
//!
//! 背景（如实 ✗）：形参所有权这块在台账里已撤回三次（round 83 一族）；根因不是"某一处多放／少放"，
//! 而是**调用边没有类型** —— 进函数的是"借用视图"、出函数的是"新引用"，两边都只是注释里的契约 ✗。
//!
//! 本模块**只**给一条路径做最小类型化试点：`call_value` 的**内建可调用快路**
//! （`builtin_function_or_method` ＋ 无关键字实参）。其余路径（`call_callable` 的转移语义、
//! Python 函数分支）**保持原样** ✓。
//!
//! 类型：
//! - [`NewRef`]：**新引用**（`OM-16`）——`Drop` 时归还一份计数；`into_raw` 才交出所有权。
//! - [`Borrowed`]：**借用视图**（`OM-19`）——不 `Drop`、不增计数；生命周期绑定调用方窗口。
//!
//! canary（`PYAWA_REFCOUNT_CANARY=1`，复用释放路径的诊断位）在调用边核对：
//! ① 调用前每个实参**在活表里**（类型对象按 `TYPE_OBJECT` 位认 ✓）—— 防"调用前就悬垂"；
//! ② 调用前后实参**计数不得下降** —— 下降＝被调方把借用当所有权释放 ⇒ UAF 源头（`OM-19`）；
//! ③ 返回值交给调用方前**活着且计数 ≥ 1** —— `OM-16` 的"新引用"必须真的是一份引用 ✓。
//!
//! 开销：canary 关时每次快路只多一次 `Cell<u32>` 位测（`Instance::release_canary_enabled`）。

use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::header::Header;
use crate::instance::Instance;

/// **OM-16**：新引用（自有）——`Drop` 时 `release` 一份；`into_raw` 不释放（所有权交出）。
pub(crate) struct NewRef<'a> {
    ptr: NonNull<Header>,
    instance: &'a Instance,
}

impl<'a> NewRef<'a> {
    /// 从**原生可调用对象的返回值**接管一份新引用。
    ///
    /// # Safety
    ///
    /// 调用方保证：`ptr` 是原生函数按 `OM-16` 交出的**新引用**（尚未被任何人释放）。
    pub(crate) unsafe fn from_new(instance: &'a Instance, ptr: NonNull<Header>) -> Self {
        let this = Self { ptr, instance };
        this.assert_new_reference();
        this
    }

    /// 裸指针（内部表示；`OM-6`：**禁止**出现在 C ABI 签名里）。
    pub(crate) fn as_ptr(&self) -> NonNull<Header> {
        self.ptr
    }

    /// 把这份新引用交回给调用方（**不**释放）。
    pub(crate) fn into_raw(self) -> NonNull<Header> {
        let ptr = self.as_ptr();
        core::mem::forget(self);
        ptr
    }

    /// canary：返回值在交回前必须**活着**且**计数 ≥ 1**（`OM-16`）。
    fn assert_new_reference(&self) {
        if !self.instance.release_canary_enabled() {
            return;
        }
        assert!(
            object_is_alive(self.instance, self.ptr),
            "[B3-canary] 原生函数返回的\"新引用\"不在活表里（OM-16：新引用必须指向存活对象）\n  指针={:p}",
            self.ptr.as_ptr()
        );
        // SAFETY: 刚确认对象（或其类型对象）活着。
        let count = unsafe { self.ptr.as_ref() }.refcount();
        assert!(
            count >= 1,
            "[B3-canary] 原生函数返回的\"新引用\"计数为 {count}（OM-16：返回值必须自带一份计数）\n  指针={:p}",
            self.ptr.as_ptr()
        );
    }
}

impl Drop for NewRef<'_> {
    fn drop(&mut self) {
        // SAFETY: 本类型只接管一份新引用（`from_new` 的契约），归还的正是这一份。
        unsafe { self.instance.release_object(self.ptr.as_ptr()) };
    }
}

/// **OM-19**：借用引用（不 `Drop`、不增计数）；有效期是调用方窗口，不是本类型自己。
#[repr(transparent)]
pub(crate) struct Borrowed<'a> {
    ptr: NonNull<Header>,
    _marker: PhantomData<&'a Header>,
}

impl<'a> Borrowed<'a> {
    /// 裸指针（内部表示）。
    pub(crate) fn as_ptr(&self) -> NonNull<Header> {
        self.ptr
    }

    /// 当前引用计数（`OM-22` 的可见口径：`getrefcount` 再 +1）。
    ///
    /// # Safety
    ///
    /// 借用窗口必须成立（见 [`Borrowed::new`]）。
    pub(crate) unsafe fn refcount(&self) -> u32 {
        // SAFETY: 借用窗口成立（调用方保证，见上）。
        unsafe { self.ptr.as_ref() }.refcount()
    }

    /// 把**裸借用视图**零拷贝地转成类型化视图。
    ///
    /// # Safety
    ///
    /// `raw` 的元素在 `'a` 窗口内全部存活；`Borrowed` 是 `repr(transparent)`（`NonNull` ＋
    /// 零大小标记）⇒ 布局与 `NonNull<Header>` 一致，切片重解释不移动数据 ✓。
    pub(crate) unsafe fn view(raw: &'a [NonNull<Header>]) -> &'a [Self] {
        // SAFETY: 调用方保证生命周期；布局一致（见上）。
        unsafe { core::slice::from_raw_parts(raw.as_ptr().cast::<Self>(), raw.len()) }
    }

    /// 把类型化视图交回**裸借用视图**（传给 `NativeFn` 的唯一转换点）。
    pub(crate) fn raw(view: &'a [Self]) -> &'a [NonNull<Header>] {
        // SAFETY: 与 [`Borrowed::view`] 互逆；布局一致。
        unsafe {
            core::slice::from_raw_parts(
                view.as_ptr().cast::<NonNull<Header>>(),
                view.len(),
            )
        }
    }
}

/// 调用边的 canary 记录：抓调用前的实参计数，调用后核对（`OM-19`：借用视图不得被消费）。
pub(crate) struct ArgCanary<'a> {
    /// 被调原生函数的名字（诊断输出用；`None` 时不抓）。
    native: &'static str,
    args: &'a [Borrowed<'a>],
    /// `None` ⇒ canary 没开（零分配、零断言）。
    before: Option<Vec<u32>>,
    /// 调用前的 GC 回收次数：调用中若跑过 GC，实参计数可能因**别的持有者**被回收而合法下降 ✓
    ///（round 253 实测：`test_collections` 的 `repeat` 下降 311→118 是 GC 释放了引用者，不是借用被消费）。
    gc_before: usize,
}

/// **`[B3-edge]` 上升日志**是否开（`PYAWA_CALL_EDGE_TRACE=1`；仅 canary 模式下生效 ✓）。
fn trace_increases() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("PYAWA_CALL_EDGE_TRACE").is_some())
}

impl<'a> ArgCanary<'a> {
    /// 调用**前**抓快照；canary 关时什么都不做。
    pub(crate) fn capture(
        instance: &Instance,
        native: &'static str,
        args: &'a [Borrowed<'a>],
    ) -> Self {
        if !instance.release_canary_enabled() {
            return Self {
                native,
                args,
                before: None,
                gc_before: 0,
            };
        }
        for (index, argument) in args.iter().enumerate() {
            assert!(
                object_is_alive(instance, argument.as_ptr()),
                "[B3-canary] 第 {index} 个实参**在调用前就不在活表里**（OM-19：借用窗口已失效）\n  指针={:p}",
                argument.as_ptr()
            );
        }
        // SAFETY: 上面逐个确认过存活。
        let before = args.iter().map(|argument| unsafe { argument.refcount() }).collect();
        Self {
            native,
            args,
            before: Some(before),
            gc_before: instance.gc_collections(),
        }
    }

    /// 调用**后**核对：实参计数**不得下降**（下降 ⇒ 借用被当所有权释放 ✗）。
    ///
    /// 上升是允许的：被调方合法保留（`OM-16` 的新引用）✓；开 `PYAWA_CALL_EDGE_TRACE=1`
    /// 时把上升也记成 `[B3-edge]` 行（供 `tools/call_edge_survey.py` 收站点）。
    pub(crate) fn check(&self, instance: &Instance) {
        let Some(before) = &self.before else {
            return;
        };
        let gc_ran = instance.gc_collections() != self.gc_before;
        for (index, argument) in self.args.iter().enumerate() {
            // SAFETY: capture 已确认存活；这里核对的是"窗口内"的计数。
            let now = unsafe { argument.refcount() };
            // **只在有偏离时才求现场**（round 253：`current_site_with_line` 会拼 String，
            // 每个实参每轮都算会把 canary 模式拖慢一个数量级 ✗）。
            if now < before[index] {
                let site = instance.current_site_with_line();
                if gc_ran {
                    // **合法下降**（round 253）：调用中跑过 GC ⇒ 别的引用者被回收 ⇒ 计数会掉 ✓。
                    // 仍记一行（供 survey 统计），但**不**当 UAF。
                    eprintln!(
                        "[B3-edge]\t下降(GC)\t{}\t{index}\t{}\t{now}\t{site}",
                        self.native, before[index]
                    );
                    continue;
                }
                // 机器可读行（先打、再断言）：`下降 <函数> <实参下标> <前> <后> <现场>`。
                eprintln!(
                    "[B3-edge]\t下降\t{}\t{index}\t{}\t{now}\t{site}",
                    self.native, before[index]
                );
                panic!(
                    "[B3-canary] 调用原生函数 `{}` 时第 {index} 个实参**计数下降**（{} → {}）：借用视图被当成所有权释放 ✗（OM-19／AB-16）\n  现场={site}",
                    self.native, before[index], now
                );
            }
            if now > before[index] && trace_increases() {
                let site = instance.current_site_with_line();
                eprintln!(
                    "[B3-edge]\t上升\t{}\t{index}\t{}\t{now}\t{site}",
                    self.native, before[index]
                );
            }
        }
    }
}

/// 对象是否还活着：在活表里，或是注册表持有的类型对象（类型对象不进活表，`OM-15`）。
fn object_is_alive(instance: &Instance, ptr: NonNull<Header>) -> bool {
    if instance.is_live_address(ptr) {
        return true;
    }
    // SAFETY: 这里只读对象头；调用方（canary）在"可能已悬垂"的窗口里读头部本身有风险，
    // 但 canary 模式下释放会先写毒值（`PYAWA_REFCOUNT_CANARY`），且本检查紧跟调用边 ⇒
    // 与既有 `refcount_canary` 的读法同口径。类型对象永不释放（注册表持有）✓。
    unsafe { ptr.as_ref() }.has_flag(crate::flags::TYPE_OBJECT)
}

/// **类型化的调用边界**：调用一个内建可调用对象。
///
/// - 实参：借用视图（`AB-16`）——本函数**不**取得、也**不**归还它们的引用；
/// - 返回值：`NewRef`（`OM-16`）——成功时由调用方决定是 `Drop` 还是 `into_raw`；
/// - canary：见模块文档。
///
/// # Safety
///
/// - `args`／`kwargs` 的元素在本次调用期间必须存活（调用方持有）；
/// - `native` 必须遵守 `NativeFn` 契约（借用视图 ＋ 新引用返回值）。
pub(crate) unsafe fn call_builtin_borrowed<'a>(
    instance: &'a Instance,
    native_name: &'static str,
    native: crate::builtin_objects::NativeFn,
    args: &'a [NonNull<Header>],
    kwargs: &'a [(NonNull<Header>, NonNull<Header>)],
) -> Result<NewRef<'a>, crate::ExecError> {
    // SAFETY: 调用方保证 args 生命周期（见函数 Safety）。
    let borrowed = unsafe { Borrowed::view(args) };
    let canary = ArgCanary::capture(instance, native_name, borrowed);
    let raw_args = Borrowed::raw(borrowed);
    // SAFETY: NativeFn 契约（借用视图 ＋ 新引用返回值），见函数 Safety。
    let result = unsafe { native(instance, None, raw_args, kwargs) };
    canary.check(instance);
    // SAFETY: 按 NativeFn 契约，`Ok` 的新引用由本函数接管。
    Ok(unsafe { result.map(|ptr| NewRef::from_new(instance, ptr)) }?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NewRef::from_new` 不增减计数；`drop` 恰好归还一份（用单例核计数，避免悬垂读 ✗）。
    #[test]
    fn new_ref_drop_releases_exactly_one() {
        let instance = crate::instance::Instance::new();
        // SAFETY: 单例由实例持有、实例存活期间有效。
        let none = unsafe { instance.singletons().none().as_ref() };
        let before = none.refcount();
        // SAFETY: 人工造一份"新引用"（模拟原生函数的返回契约）。
        unsafe { instance.incref_object(none as *const _ as *mut Header) };
        // SAFETY: 上面那份就是这个 NewRef 接管的新引用。
        let owned = unsafe { NewRef::from_new(&instance, NonNull::from(none).cast()) };
        // SAFETY: 同上。
        assert_eq!(unsafe { &*NonNull::from(none).cast::<Header>().as_ptr() }.refcount(), before + 1,
            "构造不额外增减");
        drop(owned);
        // SAFETY: 单例仍由实例持有。
        assert_eq!(unsafe { &*NonNull::from(none).cast::<Header>().as_ptr() }.refcount(), before,
            "drop 归还恰好一份");
    }

    /// `into_raw` 不释放：计数保持，直到调用方自己归还。
    #[test]
    fn new_ref_into_raw_does_not_release() {
        let instance = crate::instance::Instance::new();
        let value = instance.new_str("b3-ref");
        // SAFETY: value 是实例分配的新引用。
        let before = unsafe { &*value.as_ptr() }.refcount();
        // SAFETY: 接管 new_str 交出的新引用。
        let owned = unsafe { NewRef::from_new(&instance, value) };
        let raw = owned.into_raw();
        assert_eq!(raw, value);
        // SAFETY: raw 仍持有那份引用。
        assert_eq!(unsafe { &*raw.as_ptr() }.refcount(), before, "into_raw 不释放");
        // SAFETY: 归还本测试持有的那份。
        unsafe { instance.release_object(raw.as_ptr()) };
    }

    /// `Borrowed` 的类型化视图与裸切片**零拷贝互逆**（repr(transparent) 布局一致）。
    #[test]
    fn borrowed_view_round_trips_raw_slice() {
        let instance = crate::instance::Instance::new();
        let first = instance.new_int(1);
        let second = instance.new_int(2);
        let raw = [first, second];
        // SAFETY: 两个对象在 raw 的窗口内都存活。
        let view = unsafe { Borrowed::view(&raw) };
        assert_eq!(view.len(), 2);
        assert_eq!(view[0].as_ptr(), first);
        assert_eq!(view[1].as_ptr(), second);
        assert_eq!(Borrowed::raw(view), &raw[..]);
        for object in raw {
            // SAFETY: 归还 new_int 交出的两份新引用。
            unsafe { instance.release_object(object.as_ptr()) };
        }
    }

    /// canary 关时 `ArgCanary` 零分配、`check` 空转。
    #[test]
    fn arg_canary_is_inert_when_disabled() {
        let instance = crate::instance::Instance::new();
        let value = instance.new_int(3);
        let raw = [value];
        // SAFETY: value 存活。
        let view = unsafe { Borrowed::view(&raw) };
        let canary = ArgCanary::capture(&instance, "测试函数", view);
        assert!(canary.before.is_none(), "未开 canary 时不抓快照");
        canary.check(&instance);
        // SAFETY: 归还 new_int 交出的新引用。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}
