//! **`_signal`／`signal` 面**（task-13 导入闸 → `PLAN-milestones.md` §12.2 记账式最小面 ✓）。
//!
//! 口径（两侧逐行一致 ✓）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   45 个常量（`SIG*`／`SIG_*`／`NSIG`／`ITIMER_*`，含别名 `SIGIOT`／`SIGCLD`／`SIGPOLL`）、
//!   模块 `__doc__` 非空、四个导出可调用（`signal`／`getsignal`／`default_int_handler`／
//!   `set_wakeup_fd`）、**记账语义**（`getsignal` 初值：`SIGINT` ⇒ `default_int_handler`（同一对象）、
//!   `SIGUSR1` ⇒ `SIG_DFL`、32／33 保留号 ⇒ `None`；`signal()` 记账后读回、**返回前值**）、
//!   **不可捕获号**（9／19／32／33 ⇒ 参照的 `OSError: [Errno 22] Invalid argument`）、
//!   `default_int_handler` 的元数文案与**调用抛 `KeyboardInterrupt`**、`set_wakeup_fd` 的
//!   `-1` 记账与参数校验文案、`signal` 模块面（`Signals`／`Handlers`、包装函数元数 `TypeError`）。
//!   期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓（`FACE_EXPECTED` 逐行）；
//!   对拍里**刻意避开**依赖宿主继承处置的号码（参照进程里 `SIGPIPE`(13)／`SIGXFSZ`(25) 初值是
//!   `SIG_IGN`；记账层一律 `SIG_DFL`——差异见 `DIV-10`）与**真实投递**（方案 A 不投递，`DIV-10`）。
//! * **`ACCOUNTING_SCRIPT`**：**pyawa 侧独有面**（参照侧无对应物，故只在本侧钉住并写明理由）：
//!   ① 记账表落在模块命名空间的**每实例隐藏键**（`vars(_signal)` 多两个键、不在 `__all__`）；
//!   ② `set_wakeup_fd(fd != -1)` 需要宿主 fd 校验 ⇒ **如实报 `NotImplementedError`**（`CM-6`，
//!      禁止假成功；理由见模块头）；③ 收窄导出面（四个可选包装／其余 17 函数／2 类型不导出，
//!      无投递 API）；④ 原生函数 `__doc__` 空串（core 口径）。
//!
//! **handler 永不触发**（方案 A 的代价）无法在进程内机械观测：本文件把可观测的"记账"面钉住，
//! 投递差异登记在 `tests/conformance/divergences.md` 的 `DIV-10`。

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
const FACE_SCRIPT: &str = r##"import _signal
import signal

keys = ["ITIMER_PROF", "ITIMER_REAL", "ITIMER_VIRTUAL", "NSIG", "SIGABRT", "SIGALRM", "SIGBUS", "SIGCHLD", "SIGCLD", "SIGCONT", "SIGFPE", "SIGHUP", "SIGILL", "SIGINT", "SIGIO", "SIGIOT", "SIGKILL", "SIGPIPE", "SIGPOLL", "SIGPROF", "SIGPWR", "SIGQUIT", "SIGRTMAX", "SIGRTMIN", "SIGSEGV", "SIGSTKFLT", "SIGSTOP", "SIGSYS", "SIGTERM", "SIGTRAP", "SIGTSTP", "SIGTTIN", "SIGTTOU", "SIGURG", "SIGUSR1", "SIGUSR2", "SIGVTALRM", "SIGWINCH", "SIGXCPU", "SIGXFSZ", "SIG_BLOCK", "SIG_DFL", "SIG_IGN", "SIG_SETMASK", "SIG_UNBLOCK"]
for key in keys:
    print(key, getattr(_signal, key))
print("module-doc-nonempty", bool(_signal.__doc__))
print("exports-callable", callable(_signal.signal), callable(_signal.getsignal), callable(_signal.default_int_handler), callable(_signal.set_wakeup_fd))
print("native-doc-attr", hasattr(_signal.signal, "__doc__"), hasattr(_signal.getsignal, "__doc__"))
print("dih-type", type(_signal.default_int_handler).__name__)
print("dih-is-getsignal2", _signal.getsignal(2) is _signal.default_int_handler)
print("signal-getsignal-dih", signal.getsignal(signal.SIGINT) is signal.default_int_handler)
print("signal-dih-is", signal.default_int_handler is _signal.default_int_handler)
print("signal-swf-is", signal.set_wakeup_fd is _signal.set_wakeup_fd)

def show(label, fn):
    try:
        value = fn()
        print(label, "=>", type(value).__name__)
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)

print("getsignal(10)", _signal.getsignal(10))
print("getsignal(14)", _signal.getsignal(14))
print("getsignal(32)-none", _signal.getsignal(32) is None)
print("getsignal(33)-none", _signal.getsignal(33) is None)

def handler(signum, frame):
    pass

print("signal(10,0)-prev", _signal.signal(10, 0))
print("signal(10,handler)-prev", _signal.signal(10, handler))
print("getsignal(10)-is-handler", _signal.getsignal(10) is handler)
print("signal(10,1)-prev-is-handler", _signal.signal(10, 1) is handler)
print("getsignal(10)", _signal.getsignal(10))
print("signal(10,0)-prev", _signal.signal(10, 0))
print("getsignal(10)", _signal.getsignal(10))
print("signal(2,handler)-prev-dih", _signal.signal(2, handler) is _signal.default_int_handler)
print("getsignal(2)-is-handler", _signal.getsignal(2) is handler)
print("signal(2,dih)-prev-is-handler", _signal.signal(2, _signal.default_int_handler) is handler)
print("getsignal(2)-is-dih", _signal.getsignal(2) is _signal.default_int_handler)

show("signal(9,h)", lambda: _signal.signal(9, handler))
show("signal(19,h)", lambda: _signal.signal(19, handler))
show("signal(32,h)", lambda: _signal.signal(32, handler))
show("signal(33,0)", lambda: _signal.signal(33, 0))
show("dih-0", lambda: _signal.default_int_handler())
show("dih-1", lambda: _signal.default_int_handler(2))
show("dih-3", lambda: _signal.default_int_handler(1, 2, 3))
show("dih-kw", lambda: _signal.default_int_handler(signum=2))
show("dih-2", lambda: _signal.default_int_handler(2, None))

show("swf-1", lambda: _signal.set_wakeup_fd(-1))
show("swf-1-warn", lambda: _signal.set_wakeup_fd(-1, warn_on_full_buffer=False))
show("swf-0pos", lambda: _signal.set_wakeup_fd())
show("swf-2pos", lambda: _signal.set_wakeup_fd(-1, 0))
show("swf-3pos", lambda: _signal.set_wakeup_fd(-1, 0, 1))
show("swf-kw", lambda: _signal.set_wakeup_fd(fd=-1))
show("swf-bogus-kw", lambda: _signal.set_wakeup_fd(-1, bogus=1))
show("swf-str", lambda: _signal.set_wakeup_fd("x"))
show("swf-float", lambda: _signal.set_wakeup_fd(1.0))
show("swf-none", lambda: _signal.set_wakeup_fd(None))

show("getsignal-0", lambda: _signal.getsignal())
show("getsignal-2args", lambda: _signal.getsignal(2, 3))
show("getsignal-kw", lambda: _signal.getsignal(signalnum=2))
show("getsignal-str", lambda: _signal.getsignal("x"))
show("getsignal-0-num", lambda: _signal.getsignal(0))
show("getsignal-nsig", lambda: _signal.getsignal(65))
show("getsignal-neg", lambda: _signal.getsignal(-1))
show("getsignal-big", lambda: _signal.getsignal(2**100))
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
"##;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r##"ITIMER_PROF 2
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
exports-callable True True True True
native-doc-attr True True
dih-type builtin_function_or_method
dih-is-getsignal2 True
signal-getsignal-dih True
signal-dih-is True
signal-swf-is True
getsignal(10) 0
getsignal(14) 0
getsignal(32)-none True
getsignal(33)-none True
signal(10,0)-prev 0
signal(10,handler)-prev 0
getsignal(10)-is-handler True
signal(10,1)-prev-is-handler True
getsignal(10) 1
signal(10,0)-prev 1
getsignal(10) 0
signal(2,handler)-prev-dih True
getsignal(2)-is-handler True
signal(2,dih)-prev-is-handler True
getsignal(2)-is-dih True
signal(9,h) !! OSError : [Errno 22] Invalid argument
signal(19,h) !! OSError : [Errno 22] Invalid argument
signal(32,h) !! OSError : [Errno 22] Invalid argument
signal(33,0) !! OSError : [Errno 22] Invalid argument
dih-0 !! TypeError : default_int_handler expected 2 arguments, got 0
dih-1 !! TypeError : default_int_handler expected 2 arguments, got 1
dih-3 !! TypeError : default_int_handler expected 2 arguments, got 3
dih-kw !! TypeError : _signal.default_int_handler() takes no keyword arguments
dih-2 !! KeyboardInterrupt : 
swf-1 => int
swf-1-warn => int
swf-0pos !! TypeError : set_wakeup_fd() takes exactly 1 positional argument (0 given)
swf-2pos !! TypeError : set_wakeup_fd() takes exactly 1 positional argument (2 given)
swf-3pos !! TypeError : set_wakeup_fd() takes at most 2 arguments (3 given)
swf-kw !! TypeError : set_wakeup_fd() takes exactly 1 positional argument (0 given)
swf-bogus-kw !! TypeError : set_wakeup_fd() got an unexpected keyword argument 'bogus'
swf-str !! TypeError : 'str' object cannot be interpreted as an integer
swf-float !! TypeError : 'float' object cannot be interpreted as an integer
swf-none !! TypeError : 'NoneType' object cannot be interpreted as an integer
getsignal-0 !! TypeError : _signal.getsignal() takes exactly one argument (0 given)
getsignal-2args !! TypeError : _signal.getsignal() takes exactly one argument (2 given)
getsignal-kw !! TypeError : _signal.getsignal() takes no keyword arguments
getsignal-str !! TypeError : 'str' object cannot be interpreted as an integer
getsignal-0-num !! ValueError : signal number out of range
getsignal-nsig !! ValueError : signal number out of range
getsignal-neg !! ValueError : signal number out of range
getsignal-big !! OverflowError : Python int too large to convert to C int
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
"##;

/// pyawa 侧脚本：记账表／收窄面／如实未实现面（见文件头 ✓）。
const ACCOUNTING_SCRIPT: &str = r##"import _signal
import signal

def honest(label, fn):
    try:
        value = fn()
        print(label, "=> returned", type(value).__name__, repr(value))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, "记账" in str(exc))

print("all-len", len(_signal.__all__))
print("hidden-keys", sorted(key for key in vars(_signal) if key.startswith("__pyawa_signal")))
print("hidden-not-exported", "__pyawa_signal_handlers__" in _signal.__all__, "__pyawa_signal_wakeup_fd__" in _signal.__all__)
print("narrowed-in-signal", hasattr(signal, "pthread_sigmask"), hasattr(signal, "sigpending"), hasattr(signal, "sigwait"), hasattr(signal, "valid_signals"), hasattr(signal, "Sigmasks"))
print("narrowed-other", hasattr(signal, "alarm"), hasattr(signal, "pause"), hasattr(signal, "setitimer"), hasattr(signal, "ItimerError"), hasattr(signal, "struct_siginfo"))
print("no-delivery-api", hasattr(_signal, "raise_signal"), hasattr(_signal, "alarm"), hasattr(_signal, "pause"))
print("present", hasattr(signal, "default_int_handler"), hasattr(signal, "set_wakeup_fd"))
print("getsignal-no-error", type(_signal.getsignal(10)).__name__)
print("register-no-error", type(_signal.signal(10, signal.SIG_DFL)).__name__)
print("registered-readback", _signal.getsignal(10) == signal.SIG_DFL)

honest("swf-fd-0", lambda: _signal.set_wakeup_fd(0))
honest("swf-fd-2", lambda: _signal.set_wakeup_fd(2))
honest("swf-fd-neg2", lambda: _signal.set_wakeup_fd(-2))
honest("swf-fd-max", lambda: _signal.set_wakeup_fd(999999))
print("swf-prev-unchanged", _signal.set_wakeup_fd(-1))
print("wrapper-doc-empty", signal.signal.__doc__ == "", signal.getsignal.__doc__ == "")
"##;

/// pyawa 侧期望（逐行；来源是本侧运行，理由见文件头 ✓）。
const ACCOUNTING_EXPECTED: &str = r##"all-len 49
hidden-keys ['__pyawa_signal_handlers__', '__pyawa_signal_wakeup_fd__']
hidden-not-exported False False
narrowed-in-signal False False False False False
narrowed-other False False False False False
no-delivery-api False False False
present True True
getsignal-no-error int
register-no-error int
registered-readback True
swf-fd-0 !! NotImplementedError True
swf-fd-2 !! NotImplementedError True
swf-fd-neg2 !! NotImplementedError True
swf-fd-max !! NotImplementedError True
swf-prev-unchanged -1
wrapper-doc-empty True True
"##;

#[test]
fn signal_face_matches_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}

#[test]
fn signal_accounting_surface_is_honest() {
    assert_eq!(run("accounting", ACCOUNTING_SCRIPT), ACCOUNTING_EXPECTED);
}
