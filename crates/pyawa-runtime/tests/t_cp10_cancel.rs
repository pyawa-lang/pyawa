//! **`T-CP-10` 的可执行验收**（round 12）：可异步化调用**挂起时中断实例** ⇒ 调用以"已取消"返回、
//! 无后续副作用、脚本不悬挂（`SPEC-capabilities.md` §11；`CP-19`／`CP-27`／`CP-28`）。
//!
//! # 机制（VM 侧可取消等待；不改 ABI）
//!
//! `AB-44` 禁止改槽位签名、`CP-9` 要求调用同步 ⇒ 取消做成 **VM 侧时间片轮询**
//! （`crates/pyawa-core/src/instance/wait.rs`）：长等待切成 ≤10 ms 的片，每片后查实例中断标志
//! （`pa_interrupt` 置位，`AtomicBool`）；取消 ⇒ `ExecError::Interrupted` ⇒ `pa_exec_*` 返回
//! `PA_ERR_INTERRUPT`（**既有中断口径**；参照的 Ctrl-C 是 Python `KeyboardInterrupt`，此为如实差异）。
//!
//! # 三个用例
//!
//! 1. [`interrupt_while_sleeping_returns_cancelled_without_running_later_statements`]：
//!    `time.sleep(10)` 挂起 → 另一线程 `pa_interrupt` ⇒ ≤2 s 返回 `PA_ERR_INTERRUPT`；
//!    并证明 sleep **之后**的语句一条都没跑（全局名未绑定）。
//! 2. [`select_positive_timeout_sleeps_like_the_reference`]：真 CLI 跑
//!    `select.select([],[],[],0.05)` ⇒ **至少** ~50 ms（与参照同量级；round 11 前是立即返回）。
//! 3. [`interrupt_while_select_waits_returns_cancelled`]：挂起中的 `select` 同样被中断取消，
//!    之后语句不跑。
//!
//! **CLI 不能自我中断**（没有暴露宿主入口）⇒ 中断两类用例经 **ABI 进程内**跑（真实例、真 provider、
//! 真 stdlib）；"真睡"用例经**真 CLI 子进程**跑并与 `python3` 的输出逐字节比。
//!
//! 跑法：`cargo test -p pyawa-runtime --test t_cp10_cancel`。

use std::ffi::CString;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use pyawa_abi::capability::{
    PA_ASYNC_OK, PA_DOMAIN_CLOCK, PA_DOMAIN_FS,
};
use pyawa_abi::status::{PA_ERR_INTERRUPT, PA_OK};
use pyawa_abi::tag::PA_TNIL;
use pyawa_abi::{
    pa_create, pa_destroy, pa_exec_string, pa_getglobal, pa_host, pa_interrupt, pa_pop,
    pa_setcapability, pa_setcapability_async, pa_state, pa_type, PA_ABI_SIZE, PA_ABI_VERSION,
};
use pyawa_capabilities::clock::CpClockVtable;
use pyawa_capabilities::fs::CpFsVtable;
use pyawa_runtime::clock_system::SystemClock;
use pyawa_runtime::fs_posix::PosixFs;

/// 一个装好 `fs`＋`clock`（真 provider）与 stdlib 的 ABI 实例。
///
/// provider 与 vtable 都放 `Box`：vtable 的 `state` 里存的是 provider 地址 ⇒ provider **不能**
/// 在注册后移动；`Box` 让"搬进本结构体"不改变堆上地址。字段顺序＝`state` 最后销毁（`Drop` 里先
/// `pa_destroy`，再由字段自动 drop provider／vtable）。
#[allow(dead_code)] // 四个字段只用来**持有** provider 与 vtable 的生命周期（vtable 的 `state` 裸指针借用它们）。
struct Harness {
    fs: Box<PosixFs>,
    clock: Box<SystemClock>,
    fs_table: Box<CpFsVtable>,
    clock_table: Box<CpClockVtable>,
    state: *mut pa_state,
}

impl Harness {
    fn new() -> Self {
        let fs = Box::new(PosixFs::new());
        let clock = Box::new(SystemClock::new());
        let fs_table = Box::new(fs.vtable());
        let clock_table = Box::new(clock.vtable());
        let host = pa_host {
            abi_size: PA_ABI_SIZE,
            abi_version: PA_ABI_VERSION,
            capabilities: core::ptr::null(),
        };
        let mut state: *mut pa_state = core::ptr::null_mut();
        // SAFETY: 按 `AB-8`／`AB-43` 的契约传宿主结构；`state` 由本结构体持有并负责销毁。
        let created = unsafe { pa_create(&host, &mut state) };
        assert_eq!(created, PA_OK, "建实例失败");
        // SAFETY: `state` 刚建成功；两张 vtable 都比 state 活得久（本结构体字段）。
        unsafe {
            assert_eq!(pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK), PA_OK);
            assert_eq!(
                pa_setcapability(state, PA_DOMAIN_FS, (&*fs_table as *const CpFsVtable).cast()),
                PA_OK,
                "注册 `fs` 域失败"
            );
            assert_eq!(
                pa_setcapability_async(state, PA_DOMAIN_CLOCK, PA_ASYNC_OK),
                PA_OK
            );
            assert_eq!(
                pa_setcapability(
                    state,
                    PA_DOMAIN_CLOCK,
                    (&*clock_table as *const CpClockVtable).cast(),
                ),
                PA_OK,
                "注册 `clock` 域失败"
            );
            pyawa_stdlib::install((&*state).instance(), "probe", &[]);
        }
        Self { fs, clock, fs_table, clock_table, state }
    }

    /// 在**另一线程**跑一段脚本；返回 `(状态码, 墙钟耗时)`，用通道等 ⇒ 悬挂会变成 `recv_timeout` 失败。
    fn run_while_interrupting(
        &self,
        source: &'static str,
        before_interrupt: Duration,
        wait: Duration,
    ) -> (i32, Duration) {
        let state_address = self.state as usize;
        let (sender, receiver) = mpsc::channel();
        let started = Instant::now();
        std::thread::spawn(move || {
            let mode = b"python\0";
            // SAFETY: `state` 由主线程的 `Harness` 持有到本线程结束之后（主线程先 `recv_timeout`）；
            // `AB-5`① 的契约就是"宿主在执行中从另一线程请求中断"。
            let status = unsafe {
                pa_exec_string(
                    state_address as *mut pa_state,
                    source.as_ptr().cast(),
                    source.len() as isize,
                    core::ptr::null(),
                    mode.as_ptr().cast(),
                    core::ptr::null(),
                )
            };
            let _ = sender.send(status);
        });
        std::thread::sleep(before_interrupt);
        // SAFETY: `state` 由本结构体持有、尚未销毁。
        assert_eq!(unsafe { pa_interrupt(self.state) }, PA_OK, "`pa_interrupt` 应当成功");
        let status = receiver
            .recv_timeout(wait)
            .expect("`CP-27`：中断后必须在限定时间内返回——脚本悬挂");
        (status, started.elapsed())
    }

    /// 脚本里的全局名是否**已绑定**（用来证明"取消之后没有副作用"）。
    ///
    /// 注意：`pa_getglobal` 对不存在的名字**不是错误**——它在栈顶压 `None`（`T-AB-1` 口径）
    /// ⇒ 要判类型标签；判完把这一个栈元素弹掉。
    fn global_is_bound(&self, name: &str) -> bool {
        let name = CString::new(name).expect("名字里没有 NUL");
        // SAFETY: `state` 活着；名字 NUL 结尾；`pa_getglobal` 成功时栈顶恰好一个值。
        unsafe {
            if pa_getglobal(self.state, name.as_ptr()) != PA_OK {
                return false;
            }
            let bound = pa_type(self.state, -1) != PA_TNIL;
            let _ = pa_pop(self.state, 1);
            bound
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        // SAFETY: `state` 由 `Harness::new` 交回且尚未销毁。
        unsafe { pa_destroy(self.state) };
    }
}

// --------------------------------------------------------------------------- #
// 1) 挂起中的 time.sleep：中断 ⇒ 已取消返回、不悬挂、后续语句不跑
// --------------------------------------------------------------------------- #

#[test]
fn interrupt_while_sleeping_returns_cancelled_without_running_later_statements() {
    let harness = Harness::new();
    let (status, elapsed) = harness.run_while_interrupting(
        "import time\n\ntime.sleep(10)\nafter_sleep = 1\n",
        Duration::from_millis(300),
        Duration::from_secs(2),
    );
    assert_eq!(
        status, PA_ERR_INTERRUPT,
        "`CP-28`：挂起中的可异步化调用被中断 ⇒ `PA_ERR_INTERRUPT`（已取消口径）"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "`CP-27`：中断后必须很快返回（实测 {elapsed:?}）"
    );
    assert!(
        !harness.global_is_bound("after_sleep"),
        "`CP-28`：取消之后**不得**有后续副作用——`time.sleep` 之后的赋值不该执行"
    );
    eprintln!("[T-CP-10] `time.sleep(10)` 挂起 300 ms 后 `pa_interrupt` ⇒ status={status}、耗时 {elapsed:?}、`after_sleep` 未绑定");
}

// --------------------------------------------------------------------------- #
// 2) `select.select([],[],[],0.05)` 真睡（与参照同量级）
// --------------------------------------------------------------------------- #

#[test]
fn select_positive_timeout_sleeps_like_the_reference() {
    let base = std::env::temp_dir().join(format!("pyawa-t-cp10-sleep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("建临时目录");
    let script = "import select\nimport time\n\nstart = time.monotonic()\nselect.select([], [], [], 0.05)\nelapsed = time.monotonic() - start\nprint(elapsed >= 0.04, elapsed < 2.0)\n";
    let path = base.join("sleep.py");
    std::fs::write(&path, script).expect("写脚本");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&base)
        .output()
        .expect("跑 CLI");
    assert!(
        output.status.success(),
        "CLI 应当正常退出：status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pyawa_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert_eq!(
        pyawa_stdout, "True True\n",
        "`T-CP-10`：空 fd 列表 ＋ 正 timeout 必须**真睡**（round 11 前是立即返回）"
    );

    // 参照同脚本（逐字节比）：参照也会睡满 ⇒ 两侧同输出。
    let reference = std::process::Command::new("python3")
        .arg(&path)
        .current_dir(&base)
        .output()
        .expect("跑 python3 参照");
    assert_eq!(
        String::from_utf8_lossy(&reference.stdout),
        pyawa_stdout,
        "与参照输出一致（同量级睡眠）"
    );
    let _ = std::fs::remove_dir_all(&base);
}


// --------------------------------------------------------------------------- #
// 2b) 真 CLI：`time.sleep(0.05)` 也真睡（同一可取消等待；e2e）
// --------------------------------------------------------------------------- #

#[test]
fn cli_time_sleep_sleeps_like_the_reference() {
    let base = std::env::temp_dir().join(format!("pyawa-t-cp10-time-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("建临时目录");
    let script = "import time\n\nstart = time.monotonic()\ntime.sleep(0.05)\nelapsed = time.monotonic() - start\nprint(elapsed >= 0.04, elapsed < 2.0)\n";
    let path = base.join("sleep_time.py");
    std::fs::write(&path, script).expect("写脚本");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&base)
        .output()
        .expect("跑 CLI");
    assert!(
        output.status.success(),
        "CLI 应当正常退出：status={:?}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let pyawa_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert_eq!(pyawa_stdout, "True True\n", "`T-CP-10`：`time.sleep` 必须真睡");
    let reference = std::process::Command::new("python3")
        .arg(&path)
        .current_dir(&base)
        .output()
        .expect("跑 python3 参照");
    assert_eq!(
        String::from_utf8_lossy(&reference.stdout),
        pyawa_stdout,
        "与参照输出一致（同量级睡眠）"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// --------------------------------------------------------------------------- #
// 3) 挂起中的 select：中断 ⇒ 已取消、不悬挂、后续语句不跑
// --------------------------------------------------------------------------- #

#[test]
fn interrupt_while_select_waits_returns_cancelled() {
    let harness = Harness::new();
    let (status, elapsed) = harness.run_while_interrupting(
        "import select\n\nselect.select([], [], [], 10)\nafter_select = 1\n",
        Duration::from_millis(300),
        Duration::from_secs(2),
    );
    assert_eq!(status, PA_ERR_INTERRUPT, "挂起中的 `select` 被中断 ⇒ `PA_ERR_INTERRUPT`");
    assert!(elapsed < Duration::from_secs(2), "中断后必须很快返回（实测 {elapsed:?}）");
    assert!(
        !harness.global_is_bound("after_select"),
        "`CP-28`：取消之后不得有后续副作用"
    );
    eprintln!("[T-CP-10] `select([],[],[],10)` 挂起 300 ms 后 `pa_interrupt` ⇒ status={status}、耗时 {elapsed:?}、`after_select` 未绑定");
    // `CP-19`：中断返回后实例状态保持一致——清掉中断标志，再跑一段普通脚本应当正常完成。
    harness.instance_clear_interrupt();
    // SAFETY: `state` 活着；短脚本会立刻跑完，本线程不并发访问 state。
    let mode = b"python\0";
    let source = "x = 1\n";
    let status = unsafe {
        pa_exec_string(
            harness.state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "`CP-19`：清掉中断后同一实例仍能正常执行");
    assert!(harness.global_is_bound("x"));
}

impl Harness {
    /// 清中断请求（模拟宿主重新开始执行；`AB-5`① 的配套）。
    fn instance_clear_interrupt(&self) {
        // SAFETY: `state` 活着；只清本实例的中断标志。
        unsafe { (&*self.state).instance().clear_interrupt() };
    }
}
