//! **`binascii` 的入库护栏**（第 319 轮 ✓）：期望值**全部取自参照 `python3` 的实际输出** ✓
//! （不拿我们自己的输出反过来当标准 ✗）。契约见 `docs/SPEC-c-modules.md`。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-binascii-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "应当成功：{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn hex_and_crc32_match_the_reference() {
    // 参照：`b'6869'`、`b'hi'`、`0`、`3421780262`（zlib 口径：初值/末异或 ✓）。
    let stdout = run(
        "hex",
        "import binascii\n\
         print(binascii.hexlify(b'hi'))\n\
         print(binascii.unhexlify('6869'))\n\
         print(binascii.crc32(b''), binascii.crc32(b'123456789'))\n",
    );
    assert!(stdout.contains("b'6869'"), "{stdout}");
    assert!(stdout.contains("b'hi'"), "{stdout}");
    assert!(stdout.contains("0 3421780262"), "{stdout}");
}

#[test]
fn base64_wrappers_match_the_reference() {
    // 参照：`b'aGk=\n'`（`b2a_base64` 默认补 `\n` ✓）与 `b'hi'`。
    let stdout = run(
        "b64",
        "import binascii\nprint(binascii.b2a_base64(b'hi'))\nprint(binascii.a2b_base64(b'aGk=\\n'))\n",
    );
    assert!(stdout.contains("b'aGk=\\n'"), "{stdout}");
    assert!(stdout.contains("b'hi'"), "{stdout}");
}

#[test]
fn error_is_a_real_valueerror_subclass_and_carries_the_message() {
    // 参照：`True` ／ `接住了 binascii.Error: 'Non-hexadecimal digit found'`。
    let stdout = run(
        "error",
        "import binascii\n\
         print(issubclass(binascii.Error, ValueError))\n\
         try:\n\
         \x20   binascii.unhexlify('zz')\n\
         except binascii.Error as error:\n\
         \x20   print('caught:', str(error))\n",
    );
    assert!(stdout.contains("True"), "{stdout}");
    assert!(stdout.contains("caught: Non-hexadecimal digit found"), "{stdout}");
}
