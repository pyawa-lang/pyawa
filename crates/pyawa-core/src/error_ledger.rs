//! **B 引用账本**（task-16，阶段 0）：`ExecError::Raised` 的生成／移交 pending／消费账本。
//!
//! **定位**：纯诊断仪器（**不是**语义的一部分）。目的＝把"`Err(Raised)` 包里的那份引用
//! 被静默丢掉（既没移交异常状态、也没显式 `discard`）"从"只能靠 GC 收支猜"变成
//! "能按**生成点＋示例 Rust 栈**逐处点名"。`docs/rounds/04-post-m6-01.md` round 171/192-197
//! 的定位过程（`PYAWA_GC_LOG` + 手工反查持有链）就是没有这本账的代价。
//!
//! **开关**：`PYAWA_GC_LOG=4`（进程内只读一次）。关着时：
//! - 正常（成功）路径**零分支**（本模块的入口只在异常路径与 `Instance::collect` 里调用）；
//! - 异常路径 / 每次 GC 回收各多一次 `OnceLock` 读（≈ 不可测）。
//!
//! **账目口径**（与 `BC-60` ② 的两份所有权对齐）：
//! - **生成**：`executor::runtime::raise()` 每调一次 +1 —— 本层"新异常进 `Err`"的**唯一漏斗**
//!   （`raise_builtin`／`Instance::raise_builtin_error` 都经它；直接 `ExecError::Raised{..}`
//!   的地方是把**既有** bundle 继续传播，不重复计数）；
//! - **移交 pending**：`raise()` 里 `set_pending_exception(Some(..))` 每成功一次 +1
//!   （正常应恒等于"生成"，不等就是某条路径没走标准漏斗）；
//! - **显式丢弃**：`discard_raised()`（含 `discard_error`）每调一次 +1；
//! - **移交 pending 所有权**：`record_pending_transfer()` 每调一次 +1（终结器把 bundle 交给
//!   `set_pending_exception` 的接管语义；对象仍由 pending 保活，不算泄漏）；
//! - **隐式丢弃**：flush 时发现账本里的异常**对象已经释放**（`Instance::is_live_address` 为假）
//!   ⇒ 说明消费走了没标账本的路径（例如属性探针一族的局部 `release`）——**不算泄漏**；
//! - **未消费（泄漏）**：flush 时对象**仍存活**且从未被显式/隐式消费。为压掉"异常正在 Rust
//!   栈上传播途中赶上一次 GC"的假阳性：同一个 ptr 要**连续两次 flush** 都仍存活未消费才确认。
//!
//! **触发点**：`Instance::collect()`（自动/显式 GC 都走）末尾 flush。`PYAWA_GC_LOG=4` 下输出：
//! - `[raise-ledger] 未消费 Raised +N 累计=M 生成点=file:line`（每个生成点一条，含一次示例栈）；
//! - `[raise-ledger] generated=… pending=… discarded=… implicit=… leaked=… suspects=…`。
//!
//! **测试收尾断言**：`generated == discarded + transferred + implicit + leaked + suspects +
//! overwritten`（`suspects` 是尚未确认的在途项；见 `crates/pyawa-runtime/tests/error_ledger_shapes.rs`）。
//!
//! **已知边界（如实）**：
//! - 只记 `runtime::raise` 漏斗；少数直接构造 `ExecError::Raised` 的路径（如 stdlib 从 Python
//!   异常对象包 bundle）不增"生成"，它们的消费会被记进 `untracked_consumed`（见 summary）；
//! - ptr 复用**不做 quarantine**：隐式丢弃依赖"flush 前对象还活着"的判定；若某条未标账的消费
//!   路径在同一个 GC 周期内释放后又让新对象复用同一地址，可能留下一条陈旧账 ⇒ 用"连续两次
//!   flush 才确认"降低概率；仍属**诊断近似**（不是内存安全判据）。

use core::panic::Location;
use core::ptr::NonNull;
use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::header::Header;
use crate::instance::Instance;

/// 本实例是否开启账本（`PYAWA_GC_LOG=4`；进程内只读一次）。
fn enabled_cached() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("PYAWA_GC_LOG").as_deref() == Ok("4"))
}

/// 账本条目上限（防诊断本身把内存吃掉；到顶后停止登记新条目并打一次提示）。
const MAX_TRACKED_BUNDLES: usize = 200_000;
/// 每个生成点最多抓几条示例栈（flush 过一次之后的前若干次生成时抓，去重；安装期噪声不抓）。
const SAMPLE_CAPTURE_LIMIT: u32 = 64;
/// 每个生成点最多保留几条**不同**的示例栈。
const SAMPLE_DISTINCT_LIMIT: usize = 3;
const SAMPLE_STACK_LINES: usize = 22;

/// 一个"生成点"（`raise()` 的调用点）的统计。
struct SiteInfo {
    file: &'static str,
    line: u32,
    /// 累计确认的未消费数。
    leaked: u64,
    /// 已报告到第几只（增量打印用）。
    reported: u64,
    /// 该生成点抓过几条示例栈（前 `SAMPLE_CAPTURE_LIMIT` 次生成各抓一条、去重）。
    captured: u32,
    /// 不同的示例 Rust 栈（`Vec` 只留 `SAMPLE_DISTINCT_LIMIT` 条；已打印的置 `None`）。
    samples: Vec<Option<String>>,
    /// 打印游标（已打印到 `samples` 的哪一项）。
    printed_samples: usize,
}

/// 账本里的一只在途 bundle（`Err(Raised)` 那份引用）。
struct LiveBundle {
    /// 生成点下标（`sites`）。
    site: usize,
    /// 连续几次 flush 都存活未消费（≥2 ⇒ 确认泄漏）。
    strikes: u8,
    /// 生成时异常对象的类型指针（flush 核对：地址可能已被**别的对象**复用 ⇒ 不是同一颗）。
    type_pointer: usize,
}

/// B 引用账本（每实例一份；默认关闭时几乎是空结构）。
pub(crate) struct RaisedLedger {
    enabled: bool,
    generated: u64,
    pending_handovers: u64,
    discarded: u64,
    implicit_consumed: u64,
    /// 显式"移交 pending 槽"的 bundle（Err 那份所有权转成异常状态的持有，不算泄漏）。
    transferred: u64,
    untracked_consumed: u64,
    /// 同一地址的在途账被新一次生成覆盖（地址复用 / 同一异常被重复 raise）。
    /// 这些旧 bundle 无法再观测 ⇒ 单独列账，不混进"泄漏"。
    overwritten: u64,
    leaked: u64,
    /// 在途 bundle：对象地址 → 条目。
    live_bundles: HashMap<usize, LiveBundle>,
    sites: Vec<SiteInfo>,
    /// 生成点去重：`(file, line)` → `sites` 下标。
    site_index: HashMap<(&'static str, u32), usize>,
    /// 到过上限的提示只打一次。
    capped: bool,
    /// 已 flush 过的次数（第一次 flush 之后才开始抓示例栈 ⇒ 跳过安装期噪声）。
    flushes: u64,
}

impl RaisedLedger {
    /// 建一个账本（读取一次环境开关）。
    pub(crate) fn new() -> Self {
        Self {
            enabled: enabled_cached(),
            generated: 0,
            pending_handovers: 0,
            discarded: 0,
            implicit_consumed: 0,
            transferred: 0,
            untracked_consumed: 0,
            overwritten: 0,
            leaked: 0,
            live_bundles: HashMap::new(),
            sites: Vec::new(),
            site_index: HashMap::new(),
            capped: false,
            flushes: 0,
        }
    }
}

/// **生成记账**（在 `raise()` 里、pending 交接之后调用；`#[track_caller]` 把"生成点"取到
/// `raise()` 的**调用者**）。
#[track_caller]
pub(crate) fn record_raised(instance: &Instance, exception: NonNull<Header>) {
    if !enabled_cached() {
        return;
    }
    let location = Location::caller();
    let Ok(mut ledger) = instance.raised_ledger.try_borrow_mut() else {
        return;
    };
    if !ledger.enabled {
        return;
    }
    ledger.generated += 1;
    ledger.pending_handovers += 1;
    let address = exception.as_ptr() as usize;
    if ledger.live_bundles.len() >= MAX_TRACKED_BUNDLES {
        if !ledger.capped {
            ledger.capped = true;
            eprintln!(
                "[raise-ledger] 在途条目到上限 {MAX_TRACKED_BUNDLES}，停止登记新条目（已有条目照常报告）"
            );
        }
        return;
    }
    let site = site_index_for(&mut ledger, location);
    capture_sample_if_needed(&mut ledger, site);
    // SAFETY: exception 由 `raise()` 保证存活（刚 incref 过）。
    let type_pointer = unsafe { exception.as_ref() }.ty().as_ptr() as usize;
    if ledger
        .live_bundles
        .insert(
            address,
            LiveBundle {
                site,
                strikes: 0,
                type_pointer,
            },
        )
        .is_some()
    {
        // 地址已有在途账 ⇒ 旧 bundle 的消费没走标账路径（或同一异常重复 raise）
        // ⇒ 记"覆盖"，让 summary 的恒等式仍能对平。
        ledger.overwritten += 1;
    }
}

/// **显式消费记账**（`discard_raised` 里调用）。
pub(crate) fn record_consumed(instance: &Instance, exception: NonNull<Header>) {
    if !enabled_cached() {
        return;
    }
    let Ok(mut ledger) = instance.raised_ledger.try_borrow_mut() else {
        return;
    };
    if !ledger.enabled {
        return;
    }
    let address = exception.as_ptr() as usize;
    if ledger.live_bundles.remove(&address).is_some() {
        ledger.discarded += 1;
    } else {
        ledger.untracked_consumed += 1;
    }
}

/// **移交 pending 记账**：把 `Err(Raised)` 那份 bundle 的所有权转交给实例的异常状态槽
/// （`replace_pending(Some(..))` / `RaiseRef::into_pending` 的"接管"语义）。语义上这是一次**消费**（不再归 `Err`），
/// 但对象由 pending 槽继续保活 ⇒ 不能记成 `leaked`。终结器路径用它（task-18）。
pub(crate) fn record_pending_transfer(instance: &Instance, exception: NonNull<Header>) {
    if !enabled_cached() {
        return;
    }
    let Ok(mut ledger) = instance.raised_ledger.try_borrow_mut() else {
        return;
    };
    if !ledger.enabled {
        return;
    }
    let address = exception.as_ptr() as usize;
    if ledger.live_bundles.remove(&address).is_some() {
        ledger.transferred += 1;
    } else {
        ledger.untracked_consumed += 1;
    }
}

/// 取/建生成点条目；第一次见时抓一份示例 Rust 栈（只抓一次）。
fn site_index_for(ledger: &mut RaisedLedger, location: &'static Location<'static>) -> usize {
    let key = (location.file(), location.line());
    if let Some(index) = ledger.site_index.get(&key) {
        return *index;
    }
    let index = ledger.sites.len();
    ledger.sites.push(SiteInfo {
        file: location.file(),
        line: location.line(),
        leaked: 0,
        reported: 0,
        captured: 0,
        samples: Vec::new(),
        printed_samples: 0,
    });
    ledger.site_index.insert(key, index);
    index
}

/// 该生成点前 `SAMPLE_CAPTURE_LIMIT` 次生成各抓一条短栈、去重后最多留
/// `SAMPLE_DISTINCT_LIMIT` 条（安装期一次性生成 + 脚本主体的泄漏路径都抓得到 ✓）。
fn capture_sample_if_needed(ledger: &mut RaisedLedger, site: usize) {
    let entry = &mut ledger.sites[site];
    // **只给"已确认泄漏过"的生成点抓栈**：安装期大量类建路径共用同一个"缺属性"生成点
    // （采样只会抓到噪声）；等该生成点被确认为泄漏后，后续生成（同一泄漏路径反复发生）
    // 才是有信息量的样本。
    if entry.leaked == 0 {
        return;
    }
    if entry.captured >= SAMPLE_CAPTURE_LIMIT || entry.samples.len() >= SAMPLE_DISTINCT_LIMIT {
        return;
    }
    entry.captured += 1;
    let text = format!("{}", Backtrace::force_capture());
    let mut lines: Vec<&str> = text.lines().collect();
    let truncated = lines.len() > SAMPLE_STACK_LINES;
    lines.truncate(SAMPLE_STACK_LINES);
    // 跳过 `record_raised`／`raise` 两帧（每个栈都一样，没信息量）。
    let body: Vec<&str> = lines.into_iter().skip(2).collect();
    let mut out = String::new();
    for (index, line) in body.iter().enumerate() {
        out.push_str(if index == 0 { "" } else { "\n" });
        out.push_str("      ");
        out.push_str(line);
    }
    if truncated {
        out.push_str("\n      …（截断）");
    }
    if !entry.samples.iter().any(|existing| existing.as_deref() == Some(out.as_str())) {
        entry.samples.push(Some(out));
    }
}

/// **flush**：在 `Instance::collect()` 末尾调用（`PYAWA_GC_LOG=4` 才干活）。
///
/// 对每条在途 bundle：
/// 1. 对象已释放 ⇒ 记"隐式丢弃"，摘账；
/// 2. 对象仍存活但本轮第一次见 ⇒ `strikes += 1`（suspect，暂不报）；
/// 3. 对象仍存活且本轮是第二次及以后 ⇒ **确认未消费**，累计到生成点并摘账（防重复）。
///
/// 然后按生成点**增量**打印，并打一行 summary。summary 的恒等式：
/// `generated == discarded + transferred + implicit + leaked + suspects + overwritten`
/// （`overwritten` ＝同一地址的在途账被新一次生成覆盖、无法再观测的旧 bundle；
/// `untracked_consumed` 是"消费了但没生成账"的计数，单独列，不参与恒等式）。
pub(crate) fn flush(instance: &Instance) {
    if !enabled_cached() {
        return;
    }
    let Ok(mut ledger) = instance.raised_ledger.try_borrow_mut() else {
        return;
    };
    if !ledger.enabled {
        return;
    }
    ledger.flushes += 1;
    // 第一遍：判定每条在途 bundle；先收集地址（避免借用冲突）。
    let addresses: Vec<usize> = ledger.live_bundles.keys().copied().collect();
    let mut confirmed: Vec<usize> = Vec::new();
    for address in addresses {
        // SAFETY 无关：只构造 NonNull 交给 `is_live_address`（它只做集合查，不解引用）。
        let Some(pointer) = NonNull::new(address as *mut Header) else {
            ledger.live_bundles.remove(&address);
            continue;
        };
        if !instance.is_live_address(pointer) {
            // 对象已释放 ⇒ 消费走了未标账的路径（不算泄漏）。
            ledger.live_bundles.remove(&address);
            ledger.implicit_consumed += 1;
            continue;
        }
        // **地址复用核对**：活表里有这个地址、但类型不是生成时那颗 ⇒ 旧对象早已释放、
        // 地址被别的对象复用（诊断近似，见模块头"已知边界"）⇒ 旧账按隐式消费处理。
        // SAFETY: `is_live_address` 刚确认它是本实例的存活普通对象。
        let current_type = unsafe { pointer.as_ref() }.ty().as_ptr() as usize;
        let expected_type = ledger
            .live_bundles
            .get(&address)
            .map(|entry| entry.type_pointer);
        if expected_type != Some(current_type) {
            ledger.live_bundles.remove(&address);
            ledger.implicit_consumed += 1;
            continue;
        }
        let Some(entry) = ledger.live_bundles.get_mut(&address) else {
            continue;
        };
        if entry.strikes >= 1 {
            confirmed.push(address);
        } else {
            entry.strikes = 1;
        }
    }
    // 第二遍：确认的摘账、累计到生成点。
    for address in confirmed {
        if let Some(entry) = ledger.live_bundles.remove(&address) {
            ledger.leaked += 1;
            ledger.sites[entry.site].leaked += 1;
        }
    }
    // 增量打印生成点。
    for site in ledger.sites.iter_mut() {
        if site.leaked > site.reported {
            let delta = site.leaked - site.reported;
            site.reported = site.leaked;
            eprintln!(
                "[raise-ledger] 未消费 Raised +{delta} 累计={} 生成点={}:{}",
                site.leaked, site.file, site.line
            );
            while site.printed_samples < site.samples.len() {
                if let Some(sample) = site.samples[site.printed_samples].take() {
                    eprintln!("[raise-ledger]   示例栈：");
                    eprintln!("{sample}");
                }
                site.printed_samples += 1;
            }
        }
    }
    // summary：只在"有动静"时打（避免大循环里几百次空转刷屏；显式 gc.collect 收尾那次
    // 即使无变化也打一行，便于测试拿基线）。
    let suspects = ledger.live_bundles.len();
    if ledger.generated > 0 && (ledger.leaked > 0 || ledger.discarded > 0 || suspects > 0) {
        eprintln!(
            "[raise-ledger] generated={} pending={} discarded={} transferred={} implicit={} leaked={} suspects={} overwritten={} untracked_consumed={}",
            ledger.generated,
            ledger.pending_handovers,
            ledger.discarded,
            ledger.transferred,
            ledger.implicit_consumed,
            ledger.leaked,
            suspects,
            ledger.overwritten,
            ledger.untracked_consumed
        );
    }
}
