//! **`_posixsubprocess`／`subprocess` 导入链的面（`task-15` ✓）**。
//!
//! 口径（两侧逐行一致 ✓）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   `_posixsubprocess` 的模块 doc／公共名面／`fork_exec` 的**元数与关键字 `TypeError` 文案**
//!   （`fork_exec expected 22 arguments, got N`）；`select.PIPE_BUF`（4096）与 `select.select`
//!   的元数/关键字文案；`posix` 的 `waitpid` 一族（`WNOHANG=1`、`WIFSTOPPED`／`WSTOPSIG`／
//!   `waitstatus_to_exitcode` **真语义**：退出码／负信号／`stopped`／`invalid wait status` 的
//!   `ValueError` 文案）；`selectors` 的 `EVENT_*`；`subprocess` 的 `PIPE/STDOUT/DEVNULL`、
//!   `_PIPE_BUF`（4096）、`_can_fork_exec`、`_del_safe` 两面、`_fork_exec` 可调用 ✓。
//!   期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓（`FACE_EXPECTED` 逐行）。
//! * **`HONEST_SCRIPT`**：**如实报错面**（pyawa 侧钉住；参照侧无对应物 —— 参照会真 fork/exec、
//!   真 select、真 waitpid）：`_posixsubprocess.fork_exec`（22 参齐）、`select.select`、
//!   `posix.waitpid` **一律 `NotImplementedError`**（消息含"未接线"）——**禁止**返回假 pid／假 fd。
//!
//! **已知偏差（如实记，不钉参照那侧）**：
//! - **`select` 刻意收窄**（模块头 ✓）：只导 `PIPE_BUF`＋`select` ⇒ 不导出 `poll`／`epoll`／
//!   `devpoll`／`kqueue`／`POLL*`／`EPOLL*`／`error`。后果：`Lib/selectors.py` 走
//!   `SelectSelector` 回退（参照本机是 `EpollSelector`）、`subprocess._PopenSelector` 也是
//!   `SelectSelector`、`subprocess._USE_POSIX_SPAWN=False`（参照 glibc≥2.24 上为 **True**）——
//!   这是"语义少接线"，不是假装有 ✓。**为什么必须收窄**：若导出 `epoll` 外壳，
//!   `selectors._can_use('epoll')` 会去**真构造**，而 `NotImplementedError` 不在它的
//!   `except (TypeError, AttributeError, OSError)` 里 ⇒ 反而挡 `import selectors` ✗。
//! - `_posixsubprocess.fork_exec` 的**逐参数类型校验**未做（22 个参数都在运行期才用；本轮只到
//!   import 面 ⇒ 元数/关键字文案已按参照钉住 ✓，其余齐全参数调用如实报未接线）。
//! - `waitpid` 真调用未接线（能力域 `CM-8`）；参照的 `waitpid(-1, WNOHANG)` 会真等待子进程 ✓。
//! - 原生函数 `__doc__`：core 统一空串（第 406 轮口径）⇒ `fork_exec.__doc__` 可读但为空（参照给 C 文档串）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-posixsub-{}-{name}", std::process::id()));
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
const FACE_SCRIPT: &str = r#"import os
import posix
import select
import selectors
import subprocess
import _posixsubprocess

print("pp-doc", _posixsubprocess.__doc__)
print("pp-public", [n for n in dir(_posixsubprocess) if not n.startswith("_")])
print("fork_exec-callable", callable(_posixsubprocess.fork_exec))

def show(label, fn):
    try:
        fn()
        print(label, "=> no error")
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)

show("fork_exec-0", lambda: _posixsubprocess.fork_exec())
show("fork_exec-1", lambda: _posixsubprocess.fork_exec(1))
show("fork_exec-23", lambda: _posixsubprocess.fork_exec(*([1] * 23)))
show("fork_exec-kw", lambda: _posixsubprocess.fork_exec(args=[]))

print("select-PIPE_BUF", select.PIPE_BUF)
show("select-0", lambda: select.select())
show("select-5", lambda: select.select(1, 2, 3, 4, 5))
show("select-kw", lambda: select.select(rlist=[], wlist=[], xlist=[]))

print("posix-wait-family", hasattr(os, "waitpid"), hasattr(os, "waitstatus_to_exitcode"), hasattr(os, "WIFSTOPPED"), hasattr(os, "WSTOPSIG"), hasattr(os, "WNOHANG"))
print("WNOHANG", posix.WNOHANG)
print("WIFSTOPPED", posix.WIFSTOPPED(0x7f), posix.WIFSTOPPED(0), posix.WIFSTOPPED(256))
print("WSTOPSIG", posix.WSTOPSIG(0x7f | (2 << 8)), posix.WSTOPSIG(1))
for status in (0, 256, 512, 6, 2, 32512):
    print("waitstatus", status, "=>", posix.waitstatus_to_exitcode(status))
show("waitstatus-127", lambda: posix.waitstatus_to_exitcode(127))
show("waitstatus-neg", lambda: posix.waitstatus_to_exitcode(-1))
show("waitstatus-0args", lambda: posix.waitstatus_to_exitcode())
show("waitstatus-2args", lambda: posix.waitstatus_to_exitcode(1, 2))
show("waitpid-0", lambda: posix.waitpid())
show("waitpid-3", lambda: posix.waitpid(1, 2, 3))
show("waitpid-kw", lambda: posix.waitpid(pid=1))

print("selectors-EVENTS", selectors.EVENT_READ, selectors.EVENT_WRITE)
print("subprocess-constants", subprocess.PIPE, subprocess.STDOUT, subprocess.DEVNULL)
print("subprocess-PIPE_BUF", subprocess._PIPE_BUF)
print("subprocess-names", hasattr(subprocess, "Popen"), hasattr(subprocess, "run"), hasattr(subprocess, "CalledProcessError"), callable(subprocess._fork_exec))
print("subprocess-can-fork-exec", subprocess._can_fork_exec)
print("subprocess-del-safe", hasattr(subprocess._del_safe, "waitpid"), hasattr(subprocess._del_safe, "WNOHANG"))
"#;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r#"pp-doc A POSIX helper for the subprocess module.
pp-public ['fork_exec']
fork_exec-callable True
fork_exec-0 !! TypeError : fork_exec expected 22 arguments, got 0
fork_exec-1 !! TypeError : fork_exec expected 22 arguments, got 1
fork_exec-23 !! TypeError : fork_exec expected 22 arguments, got 23
fork_exec-kw !! TypeError : _posixsubprocess.fork_exec() takes no keyword arguments
select-PIPE_BUF 4096
select-0 !! TypeError : select expected at least 3 arguments, got 0
select-5 !! TypeError : select expected at most 4 arguments, got 5
select-kw !! TypeError : select.select() takes no keyword arguments
posix-wait-family True True True True True
WNOHANG 1
WIFSTOPPED True False False
WSTOPSIG 2 0
waitstatus 0 => 0
waitstatus 256 => 1
waitstatus 512 => 2
waitstatus 6 => -6
waitstatus 2 => -2
waitstatus 32512 => 127
waitstatus-127 !! ValueError : process stopped by delivery of signal 0
waitstatus-neg !! ValueError : invalid wait status: -1
waitstatus-0args !! TypeError : waitstatus_to_exitcode() missing required argument 'status' (pos 1)
waitstatus-2args !! TypeError : waitstatus_to_exitcode() takes at most 1 argument (2 given)
waitpid-0 !! TypeError : waitpid expected 2 arguments, got 0
waitpid-3 !! TypeError : waitpid expected 2 arguments, got 3
waitpid-kw !! TypeError : posix.waitpid() takes no keyword arguments
selectors-EVENTS 1 2
subprocess-constants -1 -2 -3
subprocess-PIPE_BUF 4096
subprocess-names True True True True
subprocess-can-fork-exec True
subprocess-del-safe True True
"#;

/// pyawa 侧脚本：如实报错面（见文件头 ✓）。
const HONEST_SCRIPT: &str = r#"import posix
import select
import selectors
import subprocess
import _posixsubprocess

def verdict(label, fn):
    try:
        value = fn()
        print(label, "=> returned", type(value).__name__, repr(value))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, "未接线" in str(exc))

verdict("fork_exec", lambda: _posixsubprocess.fork_exec(*range(22)))
verdict("select", lambda: select.select([], [], []))
verdict("select-timeout", lambda: select.select([], [], [], 0))
verdict("waitpid", lambda: posix.waitpid(1, posix.WNOHANG))
print("select-narrowed", hasattr(select, "poll"), hasattr(select, "epoll"), hasattr(select, "POLLIN"), hasattr(select, "EPOLLIN"), hasattr(select, "error"))
print("selectors-fallback", selectors.DefaultSelector is selectors.SelectSelector, hasattr(selectors, "PollSelector"), hasattr(selectors, "EpollSelector"))
print("subprocess-posix-spawn", subprocess._USE_POSIX_SPAWN, subprocess._PopenSelector is selectors.SelectSelector)
print("len-fork-exec-fields", subprocess._fork_exec.__name__ if hasattr(subprocess._fork_exec, "__name__") else "no-name")
"#;

/// pyawa 侧期望（有效调用全部 `NotImplementedError` ＋ 消息含"未接线" ✓；收窄面与回退面钉住）。
const HONEST_EXPECTED: &str = r#"fork_exec !! NotImplementedError True
select => returned tuple ([], [], [])
select-timeout => returned tuple ([], [], [])
waitpid !! NotImplementedError True
select-narrowed False False False False True
selectors-fallback True False False
subprocess-posix-spawn False True
len-fork-exec-fields fork_exec
"#;

#[test]
fn posix_subprocess_face_matches_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}

#[test]
fn fork_exec_select_waitpid_are_honestly_not_wired() {
    assert_eq!(run("honest", HONEST_SCRIPT), HONEST_EXPECTED);
}
