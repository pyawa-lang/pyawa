#!/usr/bin/env python3
"""从参照实现导出**编译产物**，写成 `crates/pyawa-core/tests/fixture-compile-3.14.json`。

`BC-14`…`BC-18` 给了编译器的契约，`§11` 给了「构造 → 指令族」的族级映射；本脚本把
一组**极小源码**交给参照实现编译，把每条指令（偏移／名字／oparg／argrepr）与 code object
的元数据原样导出。Rust 侧的发射器要产出**逐条一致**的指令流（`BC-16` 的纯函数性另有用例）。

语料只放**本轮覆盖到的构造**：模块级赋值 ＋ 整数字面量／字符串字面量／名字／`+`。
**不放**负数常量：实测 `x = -3` 的常量表顺序是 `[3, None, -3]`（折叠发生在 epilogue 之后），
那是参照实现的内部顺序细节，留给以后专门处理。

用法::

    python3 tools/gen_compile_fixture.py
"""

from __future__ import annotations

import dis
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-compile-3.14.json"

#: 语料：`(源码, 本层是否已覆盖, 未覆盖的原因)`。
#:
#: 未覆盖的也导出来（当**证据**留着），但测试会按原因跳过——照 `BC-59` 的规矩：
#: 跳过的样本要少、且**必须写明理由**。
SOURCES = [
    ("x = 1", True, ""),
    ("x = 1; y = 2", True, ""),
    # 这两段专门盯"小整数只在常量表为空时登记"那条实测规则
    ("def f():\n    return 1\nx = 1\n", True, ""),
    ("x = 1\ndef f():\n    return 2\n", True, ""),
    ("x = 0", True, ""),
    ("x = 255", True, ""),
    ("x = 300", True, ""),
    ("x = y", True, ""),
    ("x = 1; y = x", True, ""),
    ("z = w + 2", True, ""),
    ("x = 'a'", True, ""),
    ("long_name = 255", True, ""),
    (
        "x = 1 + 2",
        False,
        "常量折叠未接线：实测折叠后常量表里留下的是操作数 1（参照实现的内部顺序细节）",
    ),
    ("def f():\n    return 1\n", True, ""),
    ("def f(a):\n    return a\n", True, ""),
    ("def f(a, b):\n    return a + b\n", True, ""),
    ("def f(a, b):\n    return b + a\n", True, ""),
    ("def f(a):\n    return a + 1\n", True, ""),
    ("def f(a):\n    x = a\n    return x\n", True, ""),
    ("def f(a):\n    return a\nx = 1\n", True, ""),
    (
        "x = 200 + 100",
        False,
        "常量折叠未接线：实测常量表是 [200, None, 300]——折叠发生在 epilogue 之后",
    ),
]


def describe_constant(value: object) -> str:
    if value is None:
        return "none"
    if hasattr(value, "co_code"):
        # 嵌套 code object：只记名字——`repr` 里带**地址**，跨运行都不一致
        return f"code:{value.co_name}"
    if isinstance(value, bool):
        return f"bool:{value}"
    if isinstance(value, int):
        return f"int:{value}"
    if isinstance(value, str):
        return f"str:{value}"
    return f"{type(value).__name__}"


def describe_code(code) -> dict:
    """把一个 code object 描述成夹具的一节（**递归**带上嵌套的）。"""
    return {
        "mode": "pure",
        "argcount": code.co_argcount,
        "posonlyargcount": code.co_posonlyargcount,
        "kwonlyargcount": code.co_kwonlyargcount,
        "nlocals": code.co_nlocals,
        "flags": code.co_flags,
        "names": list(code.co_names),
        "varnames": list(code.co_varnames),
        "consts": [describe_constant(value) for value in code.co_consts],
        "instructions": [
            {
                "offset": instruction.offset,
                "opname": instruction.opname,
                "arg": instruction.arg,
                "argrepr": instruction.argrepr,
                # `BC-18` 的位置表（与指令一一对应；元组里的 `None` 原样保留成 JSON null）
                "position": list(position),
            }
            for instruction, position in zip(
                dis.get_instructions(code), code.co_positions()
            )
        ],
        # `BC-18` 的 `co_lines()`：分段（起始字节, 结束字节, 行号）
        "lines": [list(item) for item in code.co_lines()],
        "nested": [
            describe_code(value) for value in code.co_consts if hasattr(value, "co_code")
        ],
    }


def main() -> int:
    recorded = {}
    for source, covered, because in SOURCES:
        # **`dont_inherit=True` 是必须的**：`compile()` 会继承**调用方模块**的 `__future__` 标志，
        # 而本脚本头上有 `from __future__ import annotations` ⇒ 不加这个参数，产物的 `co_flags`
        # 会带上 `CO_FUTURE_ANNOTATIONS`（0x1000000），与"干净源码文件"编出来的对不上。
        # 这是实测踩出来的（夹具里 `flags` 全是 16777216 才发现）。
        code = compile(source, "<t>", "exec", dont_inherit=True)
        entry = describe_code(code)
        entry["source"] = source
        entry["covered"] = covered
        entry["uncovered_because"] = because
        recorded[source] = entry
    OUTPUT.write_text(
        json.dumps(
            {
                "_note": "由 tools/gen_compile_fixture.py 从本机 CPython 导出；禁止手改。",
                "reference": sys.version.split()[0],
                "cases": recorded,
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"已写入 {OUTPUT.relative_to(ROOT)}（{len(recorded)} 段源码）")
    for source, entry in recorded.items():
        ops = " ".join(
            f"{item['opname']}({item['arg']})" if item["arg"] is not None else item["opname"]
            for item in entry["instructions"]
        )
        print(f"  {source!r:24} names={entry['names']} consts={entry['consts']}")
        print(f"      {ops}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
