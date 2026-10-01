#!/usr/bin/env python3
"""导出 code object 层面的**期望值**夹具：`crates/pyawa-core/tests/fixture-code-3.14.json`。

`BC-55` 钉死了跳转目标的算法，`T-BC-17` 要求"与 `dis` 给出的 `argval` 逐条一致"；
`BC-54` 要求异常表能被 `dis.py` 的 `_parse_exception_table` 原样解析。
本脚本把若干小片段**编译**成真 code object，导出：

- `co_code` 的十六进制（解码器的输入）
- 每条跳转的 `dis` 偏移（字节）、`opname`、`oparg`，以及 `dis` 算出的 `argval`（字节）
- `co_exceptiontable` 的十六进制，以及 `dis._parse_exception_table` 解出来的四条记录

这样 Rust 侧的测试就是拿**参照实现产出的字节**验证自己的算术，而不是自己跟自己对。
**禁止**在脚本里写死任何偏移或数值——全部由运行时给出。

用法::

    python3 tools/gen_jump_fixture.py
"""

from __future__ import annotations

import dis
import json
import pathlib
import sys
import types

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-code-3.14.json"

#: 片段面：覆盖前向、后向、带 cache 的跳转、`and`／`or` 短路、`for` 与 `try`。
SNIPPETS: dict[str, str] = {
    "if_else": "def f(x):\n    if x:\n        return 1\n    return 2\n",
    "while_loop": "def f():\n    x = 0\n    while x < 3:\n        x = x + 1\n    return x\n",
    "even_odd": "def f(x):\n    if x % 2 == 0:\n        return 'even'\n    return 'odd'\n",
    "short_circuit": "def f(a, b):\n    return a and b or None\n",
    "for_loop": "def f(items):\n    total = 0\n    for item in items:\n        total = total + item\n    return total\n",
    "try_except": "def f():\n    try:\n        return 1\n    except ValueError:\n        return 2\n",
    "continue_break": "def f(x):\n    while x:\n        x = x - 1\n        if x == 2:\n            continue\n        if x == 0:\n            break\n    return x\n",
}


def code_objects(source: str) -> list[types.CodeType]:
    namespace: dict[str, object] = {}
    exec(compile(source, "<fixture>", "exec"), namespace)  # noqa: S102 - 夹具脚本，输入自控
    found = [
        value.__code__
        for value in namespace.values()
        if isinstance(value, types.FunctionType)
    ]
    # 嵌套（推导式／生成器）也一并收进来
    for outer in list(found):
        for value in outer.co_consts:
            if isinstance(value, types.CodeType):
                found.append(value)
    return found


def describe(code: types.CodeType) -> dict[str, object]:
    jumps = [
        {
            "offset": instruction.offset,
            "opname": instruction.opname,
            "oparg": instruction.arg,
            "target": instruction.argval,
        }
        for instruction in dis.get_instructions(code)
        # `hasjabs` 为空（BC-55 实测），所以跳转都是相对的
        if instruction.opcode in dis.hasjrel
    ]
    exceptions = [
        {
            "start": entry.start,
            "end": entry.end,
            "target": entry.target,
            "depth": entry.depth,
            "lasti": bool(entry.lasti),
        }
        for entry in dis._parse_exception_table(code)  # noqa: SLF001 - `BC-54` 点名的参照解析
    ]
    return {
        "name": code.co_name,
        "co_code": code.co_code.hex(),
        "jumps": jumps,
        "co_exceptiontable": code.co_exceptiontable.hex(),
        "exceptions": exceptions,
    }


def main() -> int:
    samples: list[dict[str, object]] = []
    for label, source in SNIPPETS.items():
        for index, code in enumerate(code_objects(source)):
            described = describe(code)
            described["snippet"] = label if index == 0 else f"{label}#{index}"
            samples.append(described)

    fixture = {
        "_note": "由 tools/gen_jump_fixture.py 从本机 CPython 导出；禁止手改。",
        "reference": {"version": sys.version.split()[0]},
        "samples": samples,
    }
    OUTPUT.write_text(
        json.dumps(fixture, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    total = sum(len(sample["jumps"]) for sample in samples)
    print(f"已写入 {OUTPUT.relative_to(ROOT)}")
    handlers = sum(len(sample["exceptions"]) for sample in samples)
    print(
        f"基线 CPython {fixture['reference']['version']}｜"
        f"code object {len(samples)} 个｜跳转 {total} 条｜异常表记录 {handlers} 条"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
