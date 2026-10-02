#!/usr/bin/env python3
"""从参照实现**探测**格式化结果，导出 `crates/pyawa-core/tests/fixture-format-3.14.json`。

`§10` 格式化族（`FORMAT_SIMPLE`／`CONVERT_VALUE`／`FORMAT_WITH_SPEC`）此前只有手写期望的用例，
没有"对拍参照真产物"的夹具。本脚本按 §6 的规矩把它补上：语料里的每个 `(值, 规格)` 都由参照
算一遍，把**文本或错误原话**原样导出；Rust 侧跑同一条真字节码路径再逐条比对。

值用**描述**表示（Rust 侧照描述造对象）：`int`／`float`／`str`／`bool`／`none`。

用法::

    python3 tools/gen_format_fixture.py
"""

from __future__ import annotations

import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-format-3.14.json"

# 只归一化**地址**（`… at 0x…`），别把 `0x2a` 这种十六进制**结果**也吃掉——第一版就吃掉了
ADDRESS = re.compile(r"at 0x[0-9a-f]+")


def normalize(text: str) -> str:
    return ADDRESS.sub("at 0x…", text)


#: 语料：`(值的描述, 规格)`。值的描述是**规格的一部分**（Rust 侧照它造对象）。
CASES: list[tuple[dict[str, object], str]] = [
    # 整数：对齐、填充、符号、千分位、进制、字符、百分号
    ({"kind": "int", "value": 42}, ""),
    ({"kind": "int", "value": 42}, "5"),
    ({"kind": "int", "value": 42}, "<5"),
    ({"kind": "int", "value": 42}, ">5"),
    ({"kind": "int", "value": 42}, "^5"),
    ({"kind": "int", "value": 42}, "05"),
    ({"kind": "int", "value": 42}, "+"),
    ({"kind": "int", "value": 42}, " "),
    ({"kind": "int", "value": 42}, "=+6"),
    ({"kind": "int", "value": 42}, "*>6"),
    ({"kind": "int", "value": 1234567}, ","),
    ({"kind": "int", "value": 1234567}, "_"),
    ({"kind": "int", "value": 42}, "d"),
    ({"kind": "int", "value": 42}, "x"),
    ({"kind": "int", "value": 42}, "#x"),
    ({"kind": "int", "value": 42}, "X"),
    ({"kind": "int", "value": 42}, "#o"),
    ({"kind": "int", "value": 42}, "b"),
    ({"kind": "int", "value": 42}, "c"),
    ({"kind": "int", "value": 42}, "%"),
    ({"kind": "int", "value": -42}, "05"),
    # 整数：参照会报错的规格
    ({"kind": "int", "value": 42}, "z"),
    ({"kind": "int", "value": 42}, ".2f"),
    # 浮点
    ({"kind": "float", "text": "3.14159"}, ""),
    ({"kind": "float", "text": "3.14159"}, ".2f"),
    ({"kind": "float", "text": "3.14159"}, "10.3f"),
    ({"kind": "float", "text": "3.14159"}, "e"),
    ({"kind": "float", "text": "3.14159"}, ".0e"),
    ({"kind": "float", "text": "3.0"}, "g"),
    ({"kind": "float", "text": "3.14159"}, "%"),
    ({"kind": "float", "text": "1234.5"}, ",.2f"),
    ({"kind": "float", "text": "3.14159"}, "<10.1f"),
    ({"kind": "float", "text": "3.14159"}, "+.2f"),
    # 字符串
    ({"kind": "str", "text": "ab"}, ""),
    ({"kind": "str", "text": "ab"}, "10"),
    ({"kind": "str", "text": "ab"}, "<10"),
    ({"kind": "str", "text": "ab"}, ">10"),
    ({"kind": "str", "text": "ab"}, "^10"),
    ({"kind": "str", "text": "abcdef"}, ".3"),
    ({"kind": "str", "text": "abcdef"}, "10.3"),
    ({"kind": "str", "text": "ab"}, "s"),
    ({"kind": "str", "text": "ab"}, "d"),
    # 布尔与 None：`bool` 走 `int` 的路径，`None` 只认空规格
    ({"kind": "bool", "value": True}, ""),
    ({"kind": "bool", "value": True}, "d"),
    ({"kind": "bool", "value": True}, "5"),
    ({"kind": "none"}, ""),
    ({"kind": "none"}, "d"),
]


def describe(case: dict[str, object]) -> str:
    """给每个用例一个稳定的名字（测试里按它对号）。"""
    kind = case["kind"]
    if kind == "none":
        return "none"
    if kind == "str":
        return f"str:{case['text']}"
    if kind == "float":
        return f"float:{case['text']}"
    return f"{kind}:{case['value']}"


def build(case: dict[str, object]) -> object:
    kind = case["kind"]
    if kind == "int":
        return int(case["value"])  # type: ignore[arg-type]
    if kind == "float":
        return float(case["text"])  # type: ignore[arg-type]
    if kind == "str":
        return str(case["text"])
    if kind == "bool":
        return bool(case["value"])
    return None


def main() -> int:
    recorded: dict[str, dict[str, object]] = {}
    for case, spec in CASES:
        value = build(case)
        name = describe(case)
        entry: dict[str, object] = {"value": case, "spec": spec}
        try:
            entry["text"] = normalize(format(value, spec))
        except BaseException as error:  # noqa: BLE001 —— 参照给什么就记什么
            entry["error"] = type(error).__name__
            entry["message"] = normalize(str(error))
        recorded[f"{name}|{spec}"] = entry

    OUTPUT.write_text(
        json.dumps(
            {
                "_note": "由 tools/gen_format_fixture.py 从本机 CPython 导出；禁止手改。",
                "reference": __import__("sys").version.split()[0],
                "cases": recorded,
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"已写入 {OUTPUT.relative_to(ROOT)}（{len(recorded)} 个用例）")
    ok = sum(1 for entry in recorded.values() if "text" in entry)
    print(f"  有文本 {ok} 个，报错 {len(recorded) - ok} 个")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
