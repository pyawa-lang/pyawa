//! **冲线轮四小格的回归抽例**（第 413 轮，task-26 ✓）——两侧逐行一致、不许中止。
//!
//! 覆盖：
//! * **① `sys.version` 格式**：修成参照的 `<版本> (<构建号>, <日期>, <时间>) [<编译器>]` 一形后，
//!   `Lib/platform.py` 的 `_sys_version` 才能解析 ⇒ `import wsgiref.simple_server` 通 ✓
//!   （抽例钉 `platform.python_version()`／`python_build()` 的形状，不钉构建串内容——
//!   本层构建串**如实**含 `pyawa` 标识 ✓）；
//! * **② `tuple.count`／`tuple.index`**（实例＋类型级＋错参文案 ✓；`import asyncio` 链的前置 ✓）；
//! * **④ 链式赋值的非名字目标**：`self.size, self.name = self.state = …`（属性／元组目标 ✓）；
//! * 顺带钉 **`from M import *` 尊重 `__all__`**（修前把模块里 `import os` 的 `os` 也搬进调用方 ✓）
//!   与 **`import asyncio` 通**（`wsgiref.simple_server`／`asyncio` 都是本轮新解锁 ✓）。
//!
//! 期望由本机 `python3`（3.14.4，`-S`＋`PYTHONPATH=本仓 Lib` 同源）跑**同一棵树**取得 ✓。

fn run(name: &str, script: &str, helper: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-threshold-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    std::fs::write(root.join("moda.py"), helper).expect("写 helper");
    std::fs::write(root.join("probe.py"), script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(root.join("probe.py"))
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

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const EXPECTED: &str = "star_names ['x']\nstar_value 1\ntuple_count 2 0\ntuple_index 1 2 2\ntuple_type_level 3 2\ntuple_miss -> ValueError tuple.index(x): x not in tuple\ntuple_bad_start -> TypeError slice indices must be integers or have an __index__ method\nchain_attrs 1 x (1, 'x')\nchain_mixed 1 2 (1, 2)\nplatform 3.14.4 ('3', '14', '4')\nplatform_build 2 True\nwsgiref True True\nasyncio asyncio ('BaseEventLoop', 'Server') True\n";

const SCRIPT: &str = "import asyncio\nimport platform\nimport wsgiref.simple_server\n\nfrom moda import *\n\nprint(\"star_names\", sorted(name for name in (\"x\", \"y\", \"os\") if name in globals()))\nprint(\"star_value\", x)\n\nt = (1, 2, 1, 3)\nprint(\"tuple_count\", t.count(1), t.count(5))\nprint(\"tuple_index\", t.index(2), t.index(1, 2), t.index(1, -2))\nprint(\"tuple_type_level\", tuple.index(t, 3), tuple.count(t, 1))\n\n\ndef show(label, fn):\n    try:\n        print(label, \"->\", repr(fn()))\n    except Exception as error:\n        print(label, \"->\", type(error).__name__, error)\n\n\nshow(\"tuple_miss\", lambda: t.index(5))\nshow(\"tuple_bad_start\", lambda: t.index(1, \"x\"))\n\n\nclass Holder:\n    def __init__(self):\n        self.size, self.name = self.state = (1, \"x\")\n\n\nholder = Holder()\nprint(\"chain_attrs\", holder.size, holder.name, holder.state)\nholder.a, holder.b = holder.c = (1, 2)\nprint(\"chain_mixed\", holder.a, holder.b, holder.c)\nprint(\"platform\", platform.python_version(), platform.python_version_tuple())\nbuild = platform.python_build()\nprint(\"platform_build\", len(build), all(isinstance(part, str) for part in build))\nprint(\"wsgiref\", hasattr(wsgiref.simple_server, \"WSGIServer\"),\n      hasattr(wsgiref.simple_server, \"WSGIRequestHandler\"))\nprint(\"asyncio\", asyncio.__name__, asyncio.__all__[:2], hasattr(asyncio, \"Task\"))\n";

const MODA: &str = "__all__ = [\"x\"]\nx = 1\ny = 2\nimport os  # 不应被 `from moda import *` 带进调用方（__all__ 限定 ✓）\n";

#[test]
fn threshold_gaps_agree_with_the_reference() {
    let stdout = run("gaps", SCRIPT, MODA);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
