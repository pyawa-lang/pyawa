#!/usr/bin/env python3
"""从参照实现**探测** `itertools` 的可观测面，生成两件：

- `crates/pyawa-stdlib/src/itertools_doc.txt`：`itertools.__doc__` 原文（模块用 `include_str!`）
- `crates/pyawa-stdlib/tests/fixtures/itertools.rs`：对拍夹具（`count` 的序列与三条实测消息）

契约见 `docs/SPEC-c-modules.md` §5.2.6。**注意**：夹具里的浮点序列只用来**记录参照的行为**，
本层 `count` 只收整数 ⇒ 那条是"未接线"的边界（不许拿它当期望去凑）。

用法::

    python3 tools/gen_itertools_fixture.py
"""

from __future__ import annotations

import itertools
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
DOC = ROOT / "crates/pyawa-stdlib/src/itertools_doc.txt"
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixtures/itertools.rs"


def error_message(call) -> str:
    try:
        call()
    except Exception as exc:  # noqa: BLE001 - 探测用
        return f"{type(exc).__name__}: {exc}"
    raise SystemExit("这条调用本该报错")


def main() -> None:
    DOC.write_text(itertools.__doc__)
    cases = [(0, 1), (1, 2), (5, 3), (0, -1), (-7, 4)]
    lines = [
        "//! 由 `tools/gen_itertools_fixture.py` 探测参照实现导出；**禁止手改**。",
        "",
        "/// `itertools.count(start, step)` 的前 5 个值（参照实现实测）。",
        "pub static COUNT_SEQUENCES: &[(i64, i64, [i64; 5])] = &[",
    ]
    for start, step in cases:
        values = list(itertools.islice(itertools.count(start, step), 5))
        rendered = ", ".join(str(value) for value in values)
        lines.append(f"    ({start}, {step}, [{rendered}]),")
    lines.append("];")
    lines.append("")
    lines.append("/// 参照实现在**浮点**起始值下的行为（本层 `count` 只收整数 ⇒ 这是边界，不是期望）。")
    float_values = list(itertools.islice(itertools.count(0.5, 0.5), 3))
    lines.append(
        "pub static REFERENCE_FLOAT_SEQUENCE: &[f64] = &["
        + ", ".join(repr(value) for value in float_values)
        + "];"
    )
    lines.append("")
    lines.append("/// 实测消息：位置实参给多了。")
    lines.append(
        'pub const REFERENCE_TOO_MANY: &str = "'
        + error_message(lambda: itertools.count(1, 2, 3))
        + '";'
    )
    lines.append("")
    lines.append("/// 实测消息：未知关键字。")
    lines.append(
        'pub const REFERENCE_UNKNOWN_KEYWORD: &str = "'
        + error_message(lambda: itertools.count(x=1))
        + '";'
    )
    lines.append("")
    lines.append("/// 实测消息：不是数值。")
    lines.append(
        'pub const REFERENCE_NOT_A_NUMBER: &str = "'
        + error_message(lambda: itertools.count("a"))
        + '";'
    )
    lines.append("")
    lines.append("/// 参照实现导出的公开名（本层只落地 `count`，逐条见 §5.2.6）。")
    names = sorted(name for name in dir(itertools) if not name.startswith("_"))
    lines.append("pub static REFERENCE_NAMES: &[&str] = &[")
    for name in names:
        lines.append(f'    "{name}",')
    lines.append("];")
    lines.append("")
    FIXTURE.write_text("\n".join(lines))
    print(
        f"已写入 {DOC.relative_to(ROOT)}（{len(itertools.__doc__)} 字节）与 "
        f"{FIXTURE.relative_to(ROOT)}（{len(cases)} 组序列、{len(names)} 个公开名）"
    )


if __name__ == "__main__":
    main()
