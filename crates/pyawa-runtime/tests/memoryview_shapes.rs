//! **`memoryview` 最小面 ＋ `atexit` ＋ `bytearray(<可迭代>)`**（task-22 第 1 轮 ✓）。
//!
//! 动因：`Lib/re/_compiler.py::_bytes_to_codes` 用 `memoryview(b).cast("I")` ＋ `itemsize`／`len`／
//! `tolist` ✓ ⇒ 先前报 `TypeError: cannot create 'memoryview' instances` ✗（`import logging` 卡这 ✓）。
//! 接着 `Lib/logging/__init__.py` 要 `atexit.register(shutdown)` ✓ —— 参照里 `atexit` 是 **C 模块**
//! ✗（上游没有 `Lib/atexit.py` ✓）⇒ 本层落一个最小 Rust 侧模块 ✓。
//! 另外 `re/_compiler.py` 上游还有 `bytearray(range(256))` ✓ —— `bytearray` 构造先前只认
//! 整数／bytes／str ✗ ⇒ 补"任意可迭代（元素按 `__index__`）" ✓。
//!
//! **如实范围** ✗：`memoryview` 只收 `bytes`／`bytearray` ✓、格式只认整数格式（本机小端 ✓）、
//! 切片读给**列表**（参照给新 `memoryview` ✓）、不做导出锁／多维 ✓；`atexit._run_exitfuncs`
//! **没接解释器退出** ✓（只有显式调用才跑 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-memoryview-{}-{name}", std::process::id()));
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

/// `memoryview(<bytearray>)` ＋ `cast("H")`（共享底层 ✓）＋ `itemsize`／`len`／`tolist`／`tobytes`／索引读写镜像 ✓
const MEMORYVIEW_SCRIPT: &str = r#"data = bytearray(range(8))
view = memoryview(data)
print(type(view).__name__, len(view), view.itemsize, view.readonly, view.format)
print(view.tobytes())
view[0] = 42
print(data[0], view[0])
wide = view.cast("H")
print(len(wide), wide.itemsize, wide.tolist())
wide[1] = 513
print(bytes(data))
print(view[1], view[2])
print(bytes(wide.tobytes()))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const MEMORYVIEW_EXPECTED: &str = "memoryview 8 1 False B\nb'\\x00\\x01\\x02\\x03\\x04\\x05\\x06\\x07'\n42 42\n4 2 [298, 770, 1284, 1798]\nb'*\\x01\\x01\\x02\\x04\\x05\\x06\\x07'\n1 1\nb'*\\x01\\x01\\x02\\x04\\x05\\x06\\x07'\n";

#[test]
fn memoryview_minimal_surface_agrees_with_the_reference() {
    let stdout = run("memoryview_minimal_surface_agrees", MEMORYVIEW_SCRIPT);
    assert_eq!(stdout, MEMORYVIEW_EXPECTED, "{stdout}");
}

/// `atexit.register(func, *args)` ＋ `_run_exitfuncs()`（后进先出 ✓）✓
const ATEXIT_SCRIPT: &str = r#"import atexit
atexit.register(print, "bye")
print("registered")
atexit._run_exitfuncs()
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const ATEXIT_EXPECTED: &str = "registered\nbye\n";

#[test]
fn atexit_register_and_run_agree_with_the_reference() {
    let stdout = run("atexit_register_and_run_agree", ATEXIT_SCRIPT);
    assert_eq!(stdout, ATEXIT_EXPECTED, "{stdout}");
}

/// `bytearray(range(256))`（`re/_compiler.py` 上游形状 ✓）✓
const BYTEARRAY_ITER_SCRIPT: &str = r#"d = bytearray(range(256))
print(len(d), d[0], d[255])
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const BYTEARRAY_ITER_EXPECTED: &str = "256 0 255\n";

#[test]
fn bytearray_from_iterable_agrees_with_the_reference() {
    let stdout = run("bytearray_from_iterable_agrees", BYTEARRAY_ITER_SCRIPT);
    assert_eq!(stdout, BYTEARRAY_ITER_EXPECTED, "{stdout}");
}

