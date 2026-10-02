#!/usr/bin/env python3
"""生成"构造器失败消息"夹具（`OM-11` 扩里的 `NewFn` 槽失败语义，`T-` 侧的素材）。

**为什么有它**：`OM-11` 扩（`22f08a5` 裁决）要求每个槽位**必须**能表达失败；`NewFn` 各实现
里那些 `return None;`（形状照原文读出来的，不是模式匹配猜的）将来要换成 `Err(...)`，而 `Err`
里该带什么**必须来自参照实测**（`MS-19`：数值与行为一律实测导出，禁手写）。

**用法**：

    python3 tools/gen_constructors_fixture.py              # 只打印（默认）
    python3 tools/gen_constructors_fixture.py --emit       # 写出 tests/fixtures/constructors.rs

**为什么默认只打印**：`docs-rule` 要求夹具与**实现**同笔入库 —— 实现还没改完就先落夹具只会
多出一份没人用的死文件。实现落地那一笔再 `--emit`，并把它 `mod` 进去。

导出内容：
- `CONSTRUCTOR_FAILURES`：`(调用写法, 期望的异常类, 期望的消息)`
- 每条都在**本机参照**上真跑出来（跑不动就硬失败，不静默跳过）
"""

from __future__ import annotations

import sys
from pathlib import Path

#: 11 个 `NewFn` 槽对应的**明显错实参**调用（写法就是测试里要复现的那一句）。
CASES: list[tuple[str, str]] = [
    ("object()", "object(1)"),
    ("int()", "int('a')"),
    ("int()", "int([])"),
    ("bool()", "bool(1, 2)"),
    ("float()", "float('x')"),
    ("float()", "float([])"),
    ("str()", "str(1, 2, 3)"),
    ("list()", "list(5)"),
    ("tuple()", "tuple(5)"),
    ("dict()", "dict(5)"),
    ("set()", "set(5)"),
    ("exception()", "ValueError(a=1)"),
]


def measure(source: str) -> tuple[str, str]:
    """在参照实现上跑一句，回 `(异常类, 消息)`；跑不出异常就硬失败。"""
    try:
        eval(source, {})  # noqa: S307 —— 只求"参照到底报什么"
    except Exception as error:  # noqa: BLE001 —— 要的就是各类异常的名字与消息
        return type(error).__name__, str(error)
    raise SystemExit(f"参照没有报错，夹具前提不成立：{source}")


def rust_string(text: str) -> str:
    """Rust 字符串字面量（本文件只处理这里出现过的字符，够用且不猜）。"""
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> None:
    rows = []
    for slot, source in CASES:
        kind, message = measure(source)
        rows.append((slot, source, kind, message))

    body = [
        "//! 构造器失败消息夹具 —— 由 `tools/gen_constructors_fixture.py` 从**参照实测**导出。",
        "//!",
        "//! 供 `OM-11` 扩（`NewFn` 槽必须能表达失败）的实现用来逐条对拍：",
        "//! 槽位报的 `Err` 转成 Python 异常后，类名与消息都要与这里一致。",
        "",
        "/// `(槽（哪个 `*_new`）, 调用写法, 期望异常类, 期望消息)`。",
        "pub static CONSTRUCTOR_FAILURES: &[(&str, &str, &str, &str)] = &[",
    ]
    for slot, source, kind, message in rows:
        body.append(
            f"    ({rust_string(slot)}, {rust_string(source)}, "
            f"{rust_string(kind)}, {rust_string(message)}),"
        )
    body += ["];", ""]
    text = "\n".join(body)

    if "--emit" in sys.argv:
        target = Path("crates/pyawa-core/tests/fixtures/constructors.rs")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        print(f"导出 {len(rows)} 条 → {target}")
    else:
        print(text)


if __name__ == "__main__":
    main()
