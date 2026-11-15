//! **`P3-42` 断言失败签名批次（5 组）**：`int.from_bytes` 缺省 byteorder／`time.struct_time`
//! 真类型／`compile()` 的 `PyCF_*` flags／`Exception.__setstate__`／`_opcode.is_valid`。
//!
//! 期望值全部由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致；
//! 已固化的口径：
//! * `int.from_bytes(data)` 缺省 `'big'`（3.11+）；
//! * `time.struct_time(seq)`：取前 9 项、`tm_*` 属性可用；`gmtime(0)` 的 `repr` **逐字**照参照；
//!   `localtime()` 的返回类型是 `struct_time`；
//! * `compile(..., flags=0x200|0x4000)`（`Lib/codeop.py` 的 `PyCF_DONT_IMPLY_DEDENT`／
//!   `PyCF_ALLOW_INCOMPLETE_INPUT`）收下且不影响产物；
//! * `Exception.__setstate__({"a": 1})` 设实例属性；
//! * `_opcode.is_valid`：`0..=266` 去掉 `121–127`／`212–233`。
//!
//! **如实未清** ✗：`KeyError` 族（特殊方法派发缺口＋unittest 异常链污染，见 round 35 台账）、
//! `time.struct_time` 不是 `tuple` 子类、`CO_FUTURE_BARRY_AS_BDFL` 未接线（诚实 NotImplementedError）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-p342-{}-{name}", std::process::id()));
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

/// 五组签名语义（参照 `python3` 3.14.4 实跑）。
const P342_SCRIPT: &str = r#"import time, _opcode

print(int.from_bytes(b"\x01\x00a"))
print(int.from_bytes(b"\x01\x00", "little"))
t = time.struct_time("123456789")
print(tuple(t), t.tm_year)
print(repr(time.gmtime(0)))
print(type(time.localtime()).__name__)
print(_opcode.is_valid(0), _opcode.is_valid(512), _opcode.is_valid(257))
e = Exception()
e.__setstate__({"a": 1})
print(e.a)
c = compile("a = 1\n", "abc", "single", 0x200)
print(c.co_filename)
print(compile("a = 1\n", "abc", "single", 0x4000).co_filename)"#;

/// 参照输出（逐行）。
const P342_EXPECTED: &str = "65633\n1\n('1', '2', '3', '4', '5', '6', '7', '8', '9') 1\ntime.struct_time(tm_year=1970, tm_mon=1, tm_mday=1, tm_hour=0, tm_min=0, tm_sec=0, tm_wday=3, tm_yday=1, tm_isdst=0)\nstruct_time\nTrue False True\n1\nabc\nabc\n";

#[test]
fn assertion_signature_batch_matches_the_reference() {
    assert_eq!(run("signatures", P342_SCRIPT), P342_EXPECTED);
}
