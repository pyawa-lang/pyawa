//! **`except` 处理块的出口清理 ＋ `bytes(<bytes-like>)`**（task-22 第 1 轮 ✓）。
//!
//! 真 bug ✗：`except` 处理块里的 `continue`／`break` **直接跳** ✗ —— 参照在回跳**之前**发
//! `POP_EXCEPT` ✓ ⇒ 少了它，异常状态（`prev_exc` ＋ 异常对象 ✓）**留在栈上** ✓，外层 `FOR_ITER`
//! 把异常对象当迭代器 ✗（实测 `re.compile(r'[a-z]', re.I)` 报"既不是内建迭代器" ✓，
//! `import logging`／`concurrent.futures` 整条链卡这 ✓）。修法：按循环帧记的 `handler_depth` 逐层发
//! `POP_EXCEPT` ✓（`break` 的"复制循环之后语句"那段按**已在处理块之外**发射 ✓，否则会多发
//! `SWAP 2; POP_EXCEPT` ⇒ `StackUnderflow` ✗）。
//! 另一格 ✗：`bytes(<bytearray>)`／`bytes(<bytes>)` 先前落到 `collect_iterable` ✗ ⇒ "只接线了
//! list／tuple／set／frozenset" ✓（`re._compiler` 的 `charmap = bytes(charmap)` ✓）⇒ 走原始字节 ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-handler-{}-{name}", std::process::id()));
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

/// `except` 处理块里的 `continue`／`break` 先逐层 `POP_EXCEPT` ✓（先前直接跳 ⇒ 异常状态留栈 ⇒ 外层 `FOR_ITER` 把它当迭代器 ✗）
const HANDLER_EXIT_SCRIPT: &str = r#"def cont(items):
    out = []
    for x in items:
        try:
            if x == 2:
                raise ValueError("boom")
            out.append(("ok", x))
        except ValueError:
            out.append(("handled", x))
            continue
    return out


def brk(items):
    out = []
    for x in items:
        try:
            if x == 2:
                raise ValueError("boom")
            out.append(("ok", x))
        except ValueError:
            out.append(("handled", x))
            break
    return out


print(cont([1, 2, 3]))
print(brk([1, 2, 3]))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const HANDLER_EXIT_EXPECTED: &str = "[('ok', 1), ('handled', 2), ('ok', 3)]\n[('ok', 1), ('handled', 2)]\n";

#[test]
fn continue_and_break_leaving_a_handler_agree_with_the_reference() {
    let stdout = run("continue_and_break_leaving_a_handler_agree", HANDLER_EXIT_SCRIPT);
    assert_eq!(stdout, HANDLER_EXIT_EXPECTED, "{stdout}");
}

/// `bytes(<bytes／bytearray>)` 直接拷原始字节 ✓（`re._compiler` 的 `bytes(charmap)` 要它 ✓）
const BYTES_FAST_SCRIPT: &str = r#"charmap = bytearray(256)
for i in range(256):
    charmap[i] = i
data = bytes(charmap)
print(len(data), data[0], data[255])
print(bytes(b"abc"), len(bytes(b"abc")))
print(bytes(bytearray(b"xyz")))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const BYTES_FAST_EXPECTED: &str = "256 0 255\nb'abc' 3\nb'xyz'\n";

#[test]
fn bytes_from_bytes_like_agree_with_the_reference() {
    let stdout = run("bytes_from_bytes_like_agree", BYTES_FAST_SCRIPT);
    assert_eq!(stdout, BYTES_FAST_EXPECTED, "{stdout}");
}

