//! `time` 的日历／格式化面（round 16）：`clock` 域表尾三槽的端到端形状。
//!
//! 为什么在 runtime tests：验收要的是端到端可观察行为（编译 → 执行 → `Lib/` import → `print`），
//! 而 provider 的 `localtime`／`strftime`／`mktime` 只有 CLI 才注册（`SystemClock`）。
//!
//! 期望值全部由本机 `python3` 3.14.4 在 **`TZ=UTC`** 下跑**同一段探针**取得（`MS-17`：
//! 不改变 `.py` 的既有可观察语义）。子进程显式 `TZ=UTC` ⇒ 结果与宿主时区无关，可重复。
//!
//! 覆盖：`timezone` 一族据实（CST／UTC 都走 provider）＋ `localtime`／`gmtime` 9 元组
//! ＋ `strftime`（含 glibc 的 `%4Y` 扩展、空格式、`%a %b %e`）＋ `asctime`／`ctime`
//! ＋ `mktime`（float）＋ 三类参数错误的类型与文案。

fn run_with_utc(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-time-shapes-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .env("TZ", "UTC")
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

/// 探针（`TZ=UTC`；`python3` 3.14.4 实测逐行一致）。
const TIME_PROBE: &str = r#"import time

print("tz:", time.timezone, time.altzone, time.daylight, time.tzname)
print("localtime:", tuple(time.localtime(0)))
print("gmtime:", tuple(time.gmtime(0)))
print("strftime:", time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(0)))
print("strftime-ext:", time.strftime("%4Y", (2020, 1, 2, 3, 4, 5, 3, 1, 0)))
print("strftime-empty:", repr(time.strftime("", time.gmtime(0))))
print("strftime-abe:", time.strftime("%a %b %e", time.gmtime(0)))
print("asctime:", time.asctime((2020, 1, 2, 3, 4, 5, 3, 1, -1)))
print("ctime:", time.ctime(0))
m = time.mktime((2020, 1, 2, 3, 4, 5, 6, 7, 0))
print("mktime:", m, type(m).__name__)
print("mktime-localtime0:", time.mktime(time.localtime(0)))
try:
    time.strftime(1)
except TypeError as e:
    print("TypeError:", e)
try:
    time.asctime((2020, 1, 2))
except TypeError as e:
    print("TypeError:", e)
try:
    time.localtime(2 ** 63)
except OverflowError as e:
    print("OverflowError:", e)
"#;

/// 参照（`TZ=UTC` 下 `python3` 3.14.4 实测 ✓，逐行）。
const TIME_PROBE_EXPECTED: &str = "tz: 0 0 0 ('UTC', 'UTC')\n\
localtime: (1970, 1, 1, 0, 0, 0, 3, 1, 0)\n\
gmtime: (1970, 1, 1, 0, 0, 0, 3, 1, 0)\n\
strftime: 1970-01-01 00:00:00\n\
strftime-ext: 2020\n\
strftime-empty: ''\n\
strftime-abe: Thu Jan  1\n\
asctime: Thu Jan  2 03:04:05 2020\n\
ctime: Thu Jan  1 00:00:00 1970\n\
mktime: 1577934245.0 float\n\
mktime-localtime0: 0.0\n\
TypeError: strftime() argument 1 must be str, not int\n\
TypeError: asctime(): illegal time tuple argument\n\
OverflowError: timestamp out of range for platform time_t\n";

#[test]
fn calendar_and_formatting_faces_match_the_reference_in_utc() {
    let stdout = run_with_utc("utc", TIME_PROBE);
    assert_eq!(stdout, TIME_PROBE_EXPECTED, "{stdout}");
}

/// 时区敏感性：`TZ` 不同 ⇒ `localtime`／`timezone`／`tzname` 跟着变（provider 真的在用宿主时区），
/// 而 `gmtime` 必须**不**变。期望值来自 `TZ=Asia/Shanghai` 的 `python3` 3.14.4 实测。
#[test]
fn localtime_follows_the_host_timezone_while_gmtime_stays_utc() {
    let script = r#"import time
print(tuple(time.localtime(0)))
print(tuple(time.gmtime(0)))
print(time.timezone, time.tzname)
print(time.ctime(0))
"#;
    let root =
        std::env::temp_dir().join(format!("pyawa-time-shapes-{}-tz", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .env("TZ", "Asia/Shanghai")
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "(1970, 1, 1, 8, 0, 0, 3, 1, 0)\n\
(1970, 1, 1, 0, 0, 0, 3, 1, 0)\n\
-28800 ('CST', 'CST')\n\
Thu Jan  1 08:00:00 1970\n"
    );
}
