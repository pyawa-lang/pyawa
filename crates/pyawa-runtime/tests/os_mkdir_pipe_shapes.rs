//! **`os.mkdir`／`os.pipe`／`codecs.unregister`**（`P3-40` 第 3 轮 ✓）。
//!
//! 本轮落地：
//! * `posix.mkdir(path, mode=0o777)`：经 `fs` 域的 `mkdir` 槽（VM 路径解析 `CP-21`；叶子不跟随符号
//!   链接），provider 走 **`mkdir(2)`** ⇒ `mode` 真生效、平台 umask 由内核施加；错误按 errno 分
//!   `OSError` 子类（`EEXIST` ⇒ `FileExistsError` ✓）。
//! * `posix.pipe()`：`CpFsVtable` **表尾追加**的 `pipe` 槽（`AB-41` ⇒ `PA_ABI_MINOR` 3→4）；
//!   provider 造真管道交两个不透明句柄，VM 各给一个**实例能力表下标**（`CP-31`：动态段从 3 起、
//!   与 fs 其余 fd 同一张表）⇒ `os.read`／`os.write`／`os.close`／`select` 直接可用 ✓。
//! * `codecs.unregister(search_function)`：把**同一个对象**从注册表删掉（参照按身份比；找不到静默
//!   返回 `None` ✓）。**更正分诊标签**：`'module' object has no attribute 'unregister'` 是
//!   **`codecs.unregister`**（`test_charmapcodec`／`test_str` 的 `addCleanup`），CPython 的 `os`
//!   **没有** `unregister` 这个名字。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`）；输出与 python 3.14.4 逐字节一致。
//! **如实未接** ✗：`os.close` 对已关 fd 仍静默返回（参照报 `EBADF`）；`os.get_inheritable`／
//! `os.set_inheritable` 未接（`test_pipe` 的那两条断言仍会失败）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-os-p340-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
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

/// `os.mkdir`（含 mode 生效＋`FileExistsError` 分类）／`os.pipe`（真 fd、可 `os.write`／`os.read`）／
/// `codecs.unregister`（身份比＋静默返回）。
const OS_MKDIR_PIPE_SCRIPT: &str = r#"import os, stat, codecs

base = "p340_test_dir"
try:
    os.rmdir(base)
except OSError:
    pass
os.mkdir(base, 0o700)
info = os.stat(base)
print("mkdir isdir", stat.S_ISDIR(info.st_mode), (stat.S_IMODE(info.st_mode) & 0o700) == 0o700)
try:
    os.mkdir(base)
except FileExistsError:
    print("mkdir dup FileExistsError")
try:
    os.mkdir()
except TypeError as e:
    print("mkdir noarg", "|" + str(e) + "|")
os.rmdir(base)

r, w = os.pipe()
print("pipe dynamic", r >= 3, w >= 3, r != w)
print("pipe write", os.write(w, b"hello"))
print("pipe read", os.read(r, 5))
os.close(w)
print("pipe eof", os.read(r, 5))
os.close(r)

def search(name):
    if name == "probe_enc":
        return codecs.CodecInfo(name="probe_enc", encode=None, decode=None)
    return None

codecs.register(search)
print("codec registered", codecs.lookup("probe-enc").name)
codecs.unregister(search)
try:
    codecs.lookup("probe-enc")
except LookupError as e:
    print("codec unregistered", "|" + str(e) + "|")"#;

/// 参照（本机 `python3` 3.14.4 实跑，逐行）。
const OS_MKDIR_PIPE_EXPECTED: &str = "mkdir isdir True True\nmkdir dup FileExistsError\nmkdir noarg |mkdir() missing required argument 'path' (pos 1)|\npipe dynamic True True True\npipe write 5\npipe read b'hello'\npipe eof b''\ncodec registered probe_enc\ncodec unregistered |unknown encoding: probe-enc|\n";

#[test]
fn mkdir_pipe_and_codecs_unregister_match_the_reference() {
    assert_eq!(run("semantics", OS_MKDIR_PIPE_SCRIPT), OS_MKDIR_PIPE_EXPECTED);
}
