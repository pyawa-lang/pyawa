#!/usr/bin/env python3
"""从参照实现**探测**模式匹配的判定结果，导出 `crates/pyawa-core/tests/fixture-match-3.14.json`。

`§10` 的模式匹配族（`MATCH_SEQUENCE`／`MATCH_MAPPING`／`MATCH_KEYS`／`GET_LEN`／
`STORE_FAST_STORE_FAST`）此前只有**手写期望**的用例。按 §6 的规矩补上"对拍参照真产物"：

语料是 **一类形状 ＋ 一组被测值**。参照侧跑真的 `match` 语句，记录**命中哪一支**与**绑定值**：

```python
match subject:
    case [a, b]:      return {"kind": "seq", "bound": [a, b]}
    case {'k': v}:    return {"kind": "map", "bound": v}
    case _:           return None
```

被测值用**描述**表示（`int`／`str`／`list`／`tuple`／`dict`），Rust 侧照描述造对象。
注意 `str`／`dict` **不算**序列（实测），缺键 ⇒ 该 case 不匹配（实测）。

用法::

    python3 tools/gen_match_fixture.py
"""

from __future__ import annotations

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-match-3.14.json"


#: 被测值：**描述**（Rust 侧照它造对象）。
SUBJECTS: list[dict[str, object]] = [
    {"kind": "list", "items": [{"kind": "int", "value": 1}, {"kind": "int", "value": 2}]},
    {"kind": "tuple", "items": [{"kind": "int", "value": 1}, {"kind": "int", "value": 2}]},
    {
        "kind": "list",
        "items": [
            {"kind": "int", "value": 1},
            {"kind": "int", "value": 2},
            {"kind": "int", "value": 3},
        ],
    },
    {"kind": "list", "items": []},
    {"kind": "list", "items": [{"kind": "int", "value": 1}]},
    {"kind": "str", "text": "ab"},
    {"kind": "dict", "entries": [("k", {"kind": "int", "value": 7})]},
    {
        "kind": "dict",
        "entries": [("k", {"kind": "int", "value": 7}), ("x", {"kind": "int", "value": 1})],
    },
    {"kind": "dict", "entries": [("other", {"kind": "int", "value": 1})]},
    {"kind": "int", "value": 42},
]


def build(description: dict[str, object]) -> object:
    kind = description["kind"]
    if kind == "int":
        return int(description["value"])  # type: ignore[arg-type]
    if kind == "str":
        return str(description["text"])
    if kind == "list":
        return [build(item) for item in description["items"]]  # type: ignore[union-attr]
    if kind == "tuple":
        return tuple(build(item) for item in description["items"])  # type: ignore[union-attr]
    if kind == "dict":
        return {
            key: build(value) for key, value in description["entries"]  # type: ignore[union-attr]
        }
    raise AssertionError(f"夹具里出现了没见过的值种类：{kind}")


def describe(description: dict[str, object]) -> str:
    kind = description["kind"]
    if kind == "int":
        return f"int:{description['value']}"
    if kind == "str":
        return f"str:{description['text']}"
    if kind in ("list", "tuple"):
        inner = ", ".join(describe(item) for item in description["items"])  # type: ignore[union-attr]
        return f"{kind}[{inner}]"
    if kind == "dict":
        inner = ", ".join(
            f"{key}={describe(value)}" for key, value in description["entries"]  # type: ignore[union-attr]
        )
        return f"dict{{{inner}}}"
    raise AssertionError(kind)


def run(subject: object) -> object:
    match subject:
        case [a, b]:
            return {"kind": "seq", "bound": [a, b]}
        case {"k": v}:
            return {"kind": "map", "bound": v}
        case _:
            return None


def main() -> int:
    recorded: dict[str, dict[str, object]] = {}
    for description in SUBJECTS:
        subject = build(description)
        recorded[describe(description)] = {
            "subject": description,
            "outcome": run(subject),
        }
    OUTPUT.write_text(
        json.dumps(
            {
                "_note": "由 tools/gen_match_fixture.py 从本机 CPython 导出；禁止手改。",
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
    for name, entry in sorted(recorded.items()):
        print(f"  {name:34} => {entry['outcome']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
