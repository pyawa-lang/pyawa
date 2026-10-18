//! **`_signal`／`signal` 的面（`task-13` ✓）**：`import signal` 的导入闸抽例。
//!
//! 口径（两侧逐行一致 ✓）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   45 个常量（`SIG*`／`SIG_*`／`NSIG`／`ITIMER_*`，含别名 `SIGIOT`／`SIGCLD`／`SIGPOLL`）、
//!   `_signal.__doc__` 非空、`signal`／`getsignal` 可调用、**参数校验的 `TypeError`／`ValueError` 文案**
//!   （关键字／元数／非整数／越界／非法 handler，共 16 条），以及 `Lib/signal.py` 的导入面
//!   （`Signals`／`Handlers` 的 `_member_map_`、`signal.SIGINT == 2`、`SIG_DFL == 0`、包装函数元数
//!   `TypeError` 文案）。期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓（`FACE_EXPECTED` 逐行）。
//! * **`HONEST_SCRIPT`**：**如实报错面**（pyawa 侧钉住；参照侧无对应物 —— 参照会真注册／读 handler）：
//!   `_signal.signal()`／`_signal.getsignal()` 与 `signal.signal()`／`signal.getsignal()` 在
//!   **参数校验全过后一律 `NotImplementedError`**（消息含"未接线"）——**绝不**假装注册成功、
//!   也**不**给 `getsignal` 编一个返回値。
//!
//! **为什么 `getsignal` 不返回 `None`（实测决定 ✓）**：参照对**有效号码**从不返回 `None` ——
//! `_signal.getsignal(10) ⇒ 0`（`SIG_DFL`）、`_signal.getsignal(2) ⇒ default_int_handler`；
//! 返回 `None` 会是编出来的事实 ⇒ 统一如实报未接线 ✓（`HONEST_SCRIPT` 把这两条都钉住）。
//!
//! **已知偏差（如实记，不钉参照那侧）**：
//! - **导出面刻意收窄**（模块头 ✓）：`__all__`＝45 常量＋`signal`/`getsignal` ⇒
//!   `pthread_sigmask`／`sigpending`／`sigwait`／`valid_signals` 四个包装**不定义**、`Sigmasks` 类不建；
//!   其余 17 个函数（`alarm`／`pause`／`default_int_handler`／`setitimer`…）与 2 个类型
//!   （`ItimerError`／`struct_siginfo`）**不导出** ⇒ `signal` 命名空间里没有这些名字
//!   （参照在无这些名字的平台上走同一条路 ✓，但**本机参照有** ⇒ `HONEST_SCRIPT` 用 False 钉住"我们没有"）。
//! - 原生函数的 `__doc__`：core 统一给**空串**（第 406 轮口径）⇒ `@_wraps` 可读、import 不受影响；
//!   **`P3-17` 之后 `@_wraps` 真的生效**（`wrapper.__doc__ = wrapped.__doc__` 走可写数据描述符 ✓）
//!   ⇒ 包装函数 `.signal.__doc__` 读回**空串**（参照给 C 文档串；先前那个 `None` 是旧 setattr 被
//!   getattr 槽遮蔽的假象 ✗）⇒ `HONEST_SCRIPT` 钉住空串。
//! - 假货 enum 的成员是**普通 int**（与 `socket` 同一条口径）⇒ 不钉成员 repr／`_member_names_`
//!   （假货不收别名 ✗）；钉 `_member_map_` 与值相等 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-signal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 参照侧脚本（`python3` 3.14.4 跑同一段 ⇒ `FACE_EXPECTED` ✓）。
const FACE_SCRIPT: &str = r#"import _signal
import signal

keys = ["ITIMER_PROF", "ITIMER_REAL", "ITIMER_VIRTUAL", "NSIG", "SIGABRT", "SIGALRM", "SIGBUS", "SIGCHLD", "SIGCLD", "SIGCONT", "SIGFPE", "SIGHUP", "SIGILL", "SIGINT", "SIGIO", "SIGIOT", "SIGKILL", "SIGPIPE", "SIGPOLL", "SIGPROF", "SIGPWR", "SIGQUIT", "SIGRTMAX", "SIGRTMIN", "SIGSEGV", "SIGSTKFLT", "SIGSTOP", "SIGSYS", "SIGTERM", "SIGTRAP", "SIGTSTP", "SIGTTIN", "SIGTTOU", "SIGURG", "SIGUSR1", "SIGUSR2", "SIGVTALRM", "SIGWINCH", "SIGXCPU", "SIGXFSZ", "SIG_BLOCK", "SIG_DFL", "SIG_IGN", "SIG_SETMASK", "SIG_UNBLOCK"]
for key in keys:
    print(key, getattr(_signal, key))
print("module-doc-nonempty", bool(_signal.__doc__))
print("signal/getsignal-callable", callable(_signal.signal), callable(_signal.getsignal))
print("signal-doc-attr", hasattr(_signal.signal, "__doc__"), hasattr(_signal.getsignal, "__doc__"))

def show(label, fn):
    try:
        fn()
        print(label, "=> no error")
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)

show("getsignal-0", lambda: _signal.getsignal())
show("getsignal-2", lambda: _signal.getsignal(2, 3))
show("getsignal-kw", lambda: _signal.getsignal(signalnum=2))
show("getsignal-str", lambda: _signal.getsignal("x"))
show("getsignal-0-num", lambda: _signal.getsignal(0))
show("getsignal-nsig", lambda: _signal.getsignal(65))
show("getsignal-neg", lambda: _signal.getsignal(-1))
show("signal-0", lambda: _signal.signal())
show("signal-1", lambda: _signal.signal(2))
show("signal-3", lambda: _signal.signal(2, 0, 3))
show("signal-kw", lambda: _signal.signal(signalnum=2, handler=0))
show("signal-9999", lambda: _signal.signal(9999, 0))
show("signal-str-num", lambda: _signal.signal("x", 0))
show("signal-bad-handler", lambda: _signal.signal(2, 5))
show("signal-bool-handler", lambda: _signal.signal(2, True))
show("signal-float-handler", lambda: _signal.signal(2, 0.0))

print("signal.SIGINT", signal.SIGINT, signal.SIGINT == 2)
print("signal.SIGTERM", signal.SIGTERM, signal.SIGTERM == 15)
print("signal.SIG_DFL", signal.SIG_DFL, signal.SIG_DFL == 0)
print("signal.SIG_IGN", signal.SIG_IGN, signal.SIG_IGN == 1)
print("Signals-name", signal.Signals.__name__)
print("Signals-values", signal.Signals.SIGINT == 2, signal.Signals.SIGTERM == 15, signal.Signals.SIGKILL == 9)
print("Handlers-values", signal.Handlers.__name__, signal.Handlers.SIG_DFL == 0, signal.Handlers.SIG_IGN == 1)
print("Signals-map", sorted(signal.Signals._member_map_))
print("Handlers-map", sorted(signal.Handlers._member_map_))
print("has-signal", callable(signal.signal), callable(signal.getsignal))
show("wrapper-signal-0", lambda: signal.signal())
show("wrapper-signal-1", lambda: signal.signal(2))
show("wrapper-signal-3", lambda: signal.signal(2, 0, 3))
show("wrapper-getsignal-0", lambda: signal.getsignal())
show("wrapper-getsignal-2", lambda: signal.getsignal(2, 3))
"#;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r#"ITIMER_PROF 2
ITIMER_REAL 0
ITIMER_VIRTUAL 1
NSIG 65
SIGABRT 6
SIGALRM 14
SIGBUS 7
SIGCHLD 17
SIGCLD 17
SIGCONT 18
SIGFPE 8
SIGHUP 1
SIGILL 4
SIGINT 2
SIGIO 29
SIGIOT 6
SIGKILL 9
SIGPIPE 13
SIGPOLL 29
SIGPROF 27
SIGPWR 30
SIGQUIT 3
SIGRTMAX 64
SIGRTMIN 34
SIGSEGV 11
SIGSTKFLT 16
SIGSTOP 19
SIGSYS 31
SIGTERM 15
SIGTRAP 5
SIGTSTP 20
SIGTTIN 21
SIGTTOU 22
SIGURG 23
SIGUSR1 10
SIGUSR2 12
SIGVTALRM 26
SIGWINCH 28
SIGXCPU 24
SIGXFSZ 25
SIG_BLOCK 0
SIG_DFL 0
SIG_IGN 1
SIG_SETMASK 2
SIG_UNBLOCK 1
module-doc-nonempty True
signal/getsignal-callable True True
signal-doc-attr True True
getsignal-0 !! TypeError : _signal.getsignal() takes exactly one argument (0 given)
getsignal-2 !! TypeError : _signal.getsignal() takes exactly one argument (2 given)
getsignal-kw !! TypeError : _signal.getsignal() takes no keyword arguments
getsignal-str !! TypeError : 'str' object cannot be interpreted as an integer
getsignal-0-num !! ValueError : signal number out of range
getsignal-nsig !! ValueError : signal number out of range
getsignal-neg !! ValueError : signal number out of range
signal-0 !! TypeError : signal expected 2 arguments, got 0
signal-1 !! TypeError : signal expected 2 arguments, got 1
signal-3 !! TypeError : signal expected 2 arguments, got 3
signal-kw !! TypeError : _signal.signal() takes no keyword arguments
signal-9999 !! ValueError : signal number out of range
signal-str-num !! TypeError : 'str' object cannot be interpreted as an integer
signal-bad-handler !! TypeError : signal handler must be signal.SIG_IGN, signal.SIG_DFL, or a callable object
signal-bool-handler !! TypeError : signal handler must be signal.SIG_IGN, signal.SIG_DFL, or a callable object
signal-float-handler !! TypeError : signal handler must be signal.SIG_IGN, signal.SIG_DFL, or a callable object
signal.SIGINT 2 True
signal.SIGTERM 15 True
signal.SIG_DFL 0 True
signal.SIG_IGN 1 True
Signals-name Signals
Signals-values True True True
Handlers-values Handlers True True
Signals-map ['SIGABRT', 'SIGALRM', 'SIGBUS', 'SIGCHLD', 'SIGCLD', 'SIGCONT', 'SIGFPE', 'SIGHUP', 'SIGILL', 'SIGINT', 'SIGIO', 'SIGIOT', 'SIGKILL', 'SIGPIPE', 'SIGPOLL', 'SIGPROF', 'SIGPWR', 'SIGQUIT', 'SIGRTMAX', 'SIGRTMIN', 'SIGSEGV', 'SIGSTKFLT', 'SIGSTOP', 'SIGSYS', 'SIGTERM', 'SIGTRAP', 'SIGTSTP', 'SIGTTIN', 'SIGTTOU', 'SIGURG', 'SIGUSR1', 'SIGUSR2', 'SIGVTALRM', 'SIGWINCH', 'SIGXCPU', 'SIGXFSZ']
Handlers-map ['SIG_DFL', 'SIG_IGN']
has-signal True True
wrapper-signal-0 !! TypeError : signal() missing 2 required positional arguments: 'signalnum' and 'handler'
wrapper-signal-1 !! TypeError : signal() missing 1 required positional argument: 'handler'
wrapper-signal-3 !! TypeError : signal() takes 2 positional arguments but 3 were given
wrapper-getsignal-0 !! TypeError : getsignal() missing 1 required positional argument: 'signalnum'
wrapper-getsignal-2 !! TypeError : getsignal() takes 1 positional argument but 2 were given
"#;

/// pyawa 侧脚本：如实报错面（见文件头 ✓）。
const HONEST_SCRIPT: &str = r#"import _signal
import signal

def verdict(label, fn):
    try:
        value = fn()
        print(label, "=> returned", type(value).__name__, repr(value))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, "未接线" in str(exc))

verdict("raw-signal-dfl", lambda: _signal.signal(10, 0))
verdict("raw-signal-ign", lambda: _signal.signal(10, 1))
verdict("raw-signal-callable", lambda: _signal.signal(10, print))
verdict("raw-getsignal", lambda: _signal.getsignal(10))
verdict("raw-getsignal-int", lambda: _signal.getsignal(2))
verdict("wrapper-signal", lambda: signal.signal(10, signal.SIG_DFL))
verdict("wrapper-signal-callable", lambda: signal.signal(10, print))
verdict("wrapper-getsignal", lambda: signal.getsignal(signal.SIGINT))
verdict("wrapper-getsignal-usr1", lambda: signal.getsignal(signal.SIGUSR1))
print("narrowed-in-signal", hasattr(signal, "pthread_sigmask"), hasattr(signal, "sigpending"), hasattr(signal, "sigwait"), hasattr(signal, "valid_signals"), hasattr(signal, "Sigmasks"))
print("narrowed-other", hasattr(signal, "alarm"), hasattr(signal, "pause"), hasattr(signal, "default_int_handler"), hasattr(signal, "ItimerError"), hasattr(signal, "struct_siginfo"))
print("wrapper-doc", signal.signal.__doc__, signal.getsignal.__doc__)
"#;

/// pyawa 侧期望（有效调用全部 `NotImplementedError` ＋ 消息含"未接线" ✓；收窄面 absent、包装 doc 为 None）。
const HONEST_EXPECTED: &str = r#"raw-signal-dfl !! NotImplementedError True
raw-signal-ign !! NotImplementedError True
raw-signal-callable !! NotImplementedError True
raw-getsignal !! NotImplementedError True
raw-getsignal-int !! NotImplementedError True
wrapper-signal !! NotImplementedError True
wrapper-signal-callable !! NotImplementedError True
wrapper-getsignal !! NotImplementedError True
wrapper-getsignal-usr1 !! NotImplementedError True
narrowed-in-signal False False False False False
narrowed-other False False False False False
wrapper-doc  
"#;

#[test]
fn signal_face_matches_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}

#[test]
fn signal_registration_is_honestly_not_wired() {
    assert_eq!(run("honest", HONEST_SCRIPT), HONEST_EXPECTED);
}
