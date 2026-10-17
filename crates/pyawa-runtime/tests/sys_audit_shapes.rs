//! **`sys.audit(event, *args)` 的面（`P3-29` 侦察 B2 ✓）**。
//!
//! 口径（两侧逐行一致 ✓）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   `sys.audit` 存在且可调、合法调用恒回 `None`（任意位置实参；`str` 子类事件名也接受）、
//!   以及错参数 `TypeError` **文案逐字**：0 个位置实参／关键字（**先判**）／非 `str` 事件名
//!   （含 `None` 在参照里渲染成 `None` 而**不是** `NoneType` ✓）。期望值由**本机 `python3`
//!   3.14.4 跑同一段脚本**取得（`FACE_EXPECTED` 逐行）✓。
//! * **`HONEST_SCRIPT`**：**如实边界**（pyawa 侧钉住，参照侧无对应物——参照有 hook 机制）：
//!   **审计机制未实现** —— 没有 hook 注册通道（`sys.addaudithook`／`sys._audit` 都不存在）⇒
//!   `audit` 是"**无 hook 时**"的真语义 no-op，**不是**审计已实现；且它本身**不写任何输出**
//!   （`HONEST_SCRIPT` 的 stdout 逐行钉死）。`P3-29` 只依赖这个调用点不炸
//!   （`Lib/http/client.py:1031` 建连前调它）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-sys-audit-{}-{name}", std::process::id()));
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
const FACE_SCRIPT: &str = r#"import sys

print("callable", callable(sys.audit))
print("type", type(sys.audit).__name__)
print("one", sys.audit("x") is None)
print("many", sys.audit("x", 1, None, [2], {"a": 1}, object()) is None)
print("empty", sys.audit("") is None)


class S(str):
    pass


class Foo:
    pass


print("str-subclass", sys.audit(S("x")) is None)


def show(label, fn):
    try:
        result = fn()
        print(label, "=>", result)
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)


show("no-args", lambda: sys.audit())
show("kw-event", lambda: sys.audit(event="x"))
show("kw-extra", lambda: sys.audit("x", flag=True))
show("event-int", lambda: sys.audit(1))
show("event-none", lambda: sys.audit(None))
show("event-bytes", lambda: sys.audit(b"x"))
show("event-list", lambda: sys.audit([]))
show("event-type", lambda: sys.audit(type))
show("event-instance", lambda: sys.audit(Foo()))
"#;

/// 参照 3.14.4 的逐行输出（**逐字**：消息、空格、`:` 前那一格都照抄 ✓）。
const FACE_EXPECTED: &str = concat!(
    "callable True\n",
    "type builtin_function_or_method\n",
    "one True\n",
    "many True\n",
    "empty True\n",
    "str-subclass True\n",
    "no-args !! TypeError : audit expected at least 1 argument, got 0\n",
    "kw-event !! TypeError : sys.audit() takes no keyword arguments\n",
    "kw-extra !! TypeError : sys.audit() takes no keyword arguments\n",
    "event-int !! TypeError : audit() argument 1 must be str, not int\n",
    "event-none !! TypeError : audit() argument 1 must be str, not None\n",
    "event-bytes !! TypeError : audit() argument 1 must be str, not bytes\n",
    "event-list !! TypeError : audit() argument 1 must be str, not list\n",
    "event-type !! TypeError : audit() argument 1 must be str, not type\n",
    "event-instance !! TypeError : audit() argument 1 must be str, not Foo\n",
);

/// pyawa 侧的如实边界脚本：**没有 hook 注册通道** ⇒ no-op 是真语义；且 audit 自己不输出。
const HONEST_SCRIPT: &str = r#"import sys

print("has-audit", hasattr(sys, "audit"))
print("no-hook-channel", hasattr(sys, "addaudithook"), hasattr(sys, "_audit"))
print("noop-is-none", sys.audit("pyawa.honest") is None)
print("multi-args", sys.audit("pyawa.honest", 1, None) is None)
print("no-output-sentinel")
"#;

/// 见 `HONEST_SCRIPT` 的注释：**钉住"审计机制未实现"**（将来真做 hook 机制时这条必然要改）。
const HONEST_EXPECTED: &str = concat!(
    "has-audit True\n",
    "no-hook-channel False False\n",
    "noop-is-none True\n",
    "multi-args True\n",
    "no-output-sentinel\n",
);

#[test]
fn sys_audit_matches_reference_shapes() {
    let output = run("face", FACE_SCRIPT);
    assert_eq!(output, FACE_EXPECTED, "sys.audit 的面对不上参照 3.14.4");
}

#[test]
fn sys_audit_is_honest_noop_without_hooks() {
    let output = run("honest", HONEST_SCRIPT);
    assert_eq!(output, HONEST_EXPECTED, "audit 的如实边界面变了");
}
