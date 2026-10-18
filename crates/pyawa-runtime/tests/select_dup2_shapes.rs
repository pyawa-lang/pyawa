//! **`select.select` 真接线 ＋ `os.dup`／`os.dup2`**（round 11 · `T-CP-3` 的 Python 可见面）
//! —— 与参照 `python3` **同脚本、两侧逐行一致**。
//!
//! 口径（参照 3.14.4 实测）：
//! * `select.select`：`socketpair` 的 net fd 就绪面（send 前不可读、send 后可读、可写恒真、
//!   正 timeout 后空集）；常规文件 fd **恒可读**；返回**原对象**（`int` 原样、带 `fileno()` 的对象
//!   原样交回）；错误形状（负 fd／越界 fd／已关 fd／坏元素／坏序列／负 timeout／NaN）；
//! * `os.dup`／`os.dup2`：表项复制（同一能力对象、**共享打开偏移**）、`dup2` 覆盖已开目标、
//!   **返回目标 fd**（参照返回 `fd2`，不是 `None`）、坏 fd 的 `OSError` 形状；
//! * `os.dup` 出来的 socket fd 也能被 `select` 认（`CP-31` 的别名语义）。
//!
//! 两侧跑**同一段脚本**、比较 stdout **逐字节相同**（脚本只打布尔／相对事实，不钉 fd 数字 ✓）。

fn run_python3(script_path: &std::path::Path, directory: &std::path::Path) -> String {
    let output = std::process::Command::new("python3")
        .arg("-S")
        .arg(script_path)
        .current_dir(directory)
        .output()
        .expect("跑 python3");
    assert!(
        output.status.success(),
        "参照侧失败（{:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn run_pyawa(script_path: &std::path::Path, directory: &std::path::Path) -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(script_path)
        .current_dir(directory)
        .output()
        .expect("跑 CLI");
    assert!(
        output.status.success(),
        "pyawa 侧失败（{:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 两侧跑同一段脚本，返回（参照 stdout，pyawa stdout）。
fn both_side_by_side(name: &str, script: &str) -> (String, String) {
    let root = std::env::temp_dir().join(format!(
        "pyawa-select-dup2-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let reference = run_python3(&path, &root);
    let ours = run_pyawa(&path, &root);
    let _ = std::fs::remove_dir_all(&root);
    (reference, ours)
}

const PROBE: &str = r#"import os
import select
import socket

print("== select on socketpair ==")
a, b = socket.socketpair()
fa, fb = a.fileno(), b.fileno()
print("initial-empty:", select.select([fb], [], [], 0) == ([], [], []))
print("writable:", select.select([], [fa], [], 0) == ([], [fa], []))
a.send(b"hi")
print("after-send:", select.select([fb], [], [], 0) == ([fb], [], []))
print("other-not-ready:", select.select([fa], [], [], 0) == ([], [], []))
print("timeout-empty:", select.select([fb], [], [], 0.01) == ([], [], []))
r, w, x = select.select([b], [], [], 0)
print("objects:", len(r) == 1 and r[0] is b, w == [], x == [])
print("recv:", b.recv(2))
a.close()
b.close()

print("== select on regular file ==")
fd = os.open(__file__, os.O_RDONLY)
r, w, x = select.select([fd], [], [], 0)
print("readable:", r == [fd], w == [], x == [])
os.close(fd)

print("== os.dup / os.dup2 ==")
fd = os.open(__file__, os.O_RDONLY)
dup = os.dup(fd)
print("dup-is-int-distinct:", isinstance(dup, int), dup != fd)
print("dup2-onto-dup:", os.dup2(fd, dup) == dup)
first = os.read(fd, 1)
second = os.read(dup, 1)
print("shared-offset:", first, second)
os.close(fd)
os.close(dup)

p = os.open(__file__, os.O_RDONLY)
q = os.open(__file__, os.O_RDONLY)
print("dup2-over-open:", os.dup2(p, q) == q)
print("read-after-over:", len(os.read(q, 1)))
os.close(p)
os.close(q)

print("== errors ==")
def show(label, fn):
    try:
        fn()
        print(label, "no error")
    except OSError as exc:
        print(label, "OSError", str(exc))
    except ValueError as exc:
        print(label, "ValueError", str(exc))
    except TypeError as exc:
        print(label, "TypeError", str(exc))

show("dup-bad", lambda: os.dup(99999))
show("dup2-bad-old", lambda: os.dup2(99999, 40))
show("dup2-bad-new", lambda: os.dup2(0, -1))
show("select-neg-timeout", lambda: select.select([], [], [], -1))
show("select-bad-fd", lambda: select.select([99999], [], [], 0))
c = socket.socket()
fd_c = c.fileno()
c.close()
show("select-closed-fd", lambda: select.select([fd_c], [], [], 0))
show("select-bad-seq", lambda: select.select(5, [], [], 0))
show("select-bad-elem", lambda: select.select([None], [], [], 0))
show("select-bad-timeout", lambda: select.select([], [], [], "x"))

print("== select via dup'd socket fd ==")
a, b = socket.socketpair()
dup_a = os.dup(a.fileno())
b.send(b"z")
r, w, x = select.select([dup_a], [], [], 0)
print("dup-readable:", r == [dup_a])
print("recv-on-dup:", a.recv(1))
os.close(dup_a)
a.close()
b.close()
print("done")
"#;

#[test]
fn select_and_dup2_match_the_reference_line_by_line() {
    let (reference, ours) = both_side_by_side("probe", PROBE);
    assert_eq!(
        ours, reference,
        "两侧 stdout 必须逐字节一致\n--- reference ---\n{reference}\n--- pyawa ---\n{ours}"
    );
}
