//! **`os.getcwd`／`os.getcwdb`／`os.readlink`**（task-10 ✓；立据＝`sysconfig` 一族的 import 闸
//! —— `Lib/posixpath.py` 的 `realpath` 对**绝对路径**也调 `os.getcwd()`／`os.readlink()`，
//! `sysconfig/__init__.py:219` 的 `_safe_realpath(sys.executable)` 经它 ✓；另 `test.support` 链的
//! 模块级 `SAVEDCWD = os.getcwd()`（`test/support/os_helper.py:115`）✓）。
//!
//! 口径（参照 `python3` 3.14.4 实测 ✓，两侧逐行一致）：
//! * `getcwd()` ⇒ `str`、`getcwdb()` ⇒ `bytes`（同一份真值 ✓）；无参数 ⇒ 传参报参照那两条 `TypeError` ✓；
//! * `readlink(path)`：`str` 路径 ⇒ `str`、`bytes` 路径 ⇒ `bytes` ✓；`dir_fd=None` 照收 ✓；
//! * 报错按 errno 折成 `OSError` 子类 ＋ `[Errno N] …: '…'` 文案 ✓（缺路径 ⇒ `FileNotFoundError`／2 ✓、
//!   非符号链接 ⇒ `OSError`／22 ✓）；无 `strict` 参数（参照实测签名 `(path, *, dir_fd=None)` ✓）。
//!
//! **为什么路径留在临时目录里**：`getcwd` 的真值两侧不同（跑的是两个进程）⇒ 只比**形状**
//! （`endswith("/work")`／`isabs`／类型／`getcwdb().decode() == getcwd()` ✓）；错误文案里的路径
//! 一律用**相对路径** ✓ —— 这样期望值是确定的，两侧逐行一致 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-posixfs-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let work = root.join("work");
    std::fs::create_dir_all(&work).expect("建临时目录");
    // 一条真符号链接 ＋ 一个普通文件：`readlink` 的成功／`EINVAL` 两种形态各要一个 ✓
    std::fs::write(work.join("target.txt"), "data").expect("写普通文件");
    std::os::unix::fs::symlink("target.txt", work.join("link")).expect("建符号链接");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    // cwd ＝ `work/` ⇒ 脚本里 `os.getcwd().endswith("/work")` 两侧同真 ✓
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&work)
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

const GETCWD_READLINK_SCRIPT: &str = r#"import os

print(os.getcwd().endswith("/work"))
print(os.path.isabs(os.getcwd()))
print(type(os.getcwd()).__name__, type(os.getcwdb()).__name__)
print(os.getcwdb().decode() == os.getcwd())
print(os.readlink("link"))
print(os.readlink(b"link"))
print(os.readlink("link", dir_fd=None))
print(os.path.isabs(os.readlink("/proc/self/exe")))
print(os.readlink("/proc/self").isdigit())


def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except OSError as exc:
        print(label, "!!", type(exc).__name__, str(exc))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__ + ":", exc)


show("missing", lambda: os.readlink("missing-xyz"))
show("bytes-missing", lambda: os.readlink(b"missing-xyz"))
show("regular", lambda: os.readlink("target.txt"))
show("no-arg", lambda: os.readlink())
show("int-path", lambda: os.readlink(1))
show("two-args", lambda: os.readlink(1, 2))
show("getcwd-arg", lambda: os.getcwd(1))
show("getcwdb-arg", lambda: os.getcwdb(1))
show("getcwd-kw", lambda: os.getcwd(x=1))
show("strict", lambda: os.readlink("link", strict=True))
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const GETCWD_READLINK_EXPECTED: &str = "True\nTrue\nstr bytes\nTrue\ntarget.txt\nb'target.txt'\ntarget.txt\nTrue\nTrue\nmissing !! FileNotFoundError [Errno 2] No such file or directory: 'missing-xyz'\nbytes-missing !! FileNotFoundError [Errno 2] No such file or directory: b'missing-xyz'\nregular !! OSError [Errno 22] Invalid argument: 'target.txt'\nno-arg !! TypeError: readlink() missing required argument 'path' (pos 1)\nint-path !! TypeError: readlink: path should be string, bytes or os.PathLike, not int\ntwo-args !! TypeError: readlink() takes exactly 1 positional argument (2 given)\ngetcwd-arg !! TypeError: posix.getcwd() takes no arguments (1 given)\ngetcwdb-arg !! TypeError: posix.getcwdb() takes no arguments (1 given)\ngetcwd-kw !! TypeError: posix.getcwd() takes no keyword arguments\nstrict !! TypeError: readlink() got an unexpected keyword argument 'strict'\n";

#[test]
fn getcwd_and_readlink_agree_with_the_reference() {
    let stdout = run("getcwd_and_readlink_agree", GETCWD_READLINK_SCRIPT);
    assert_eq!(stdout, GETCWD_READLINK_EXPECTED, "{stdout}");
}
