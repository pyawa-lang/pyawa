#!/usr/bin/env python3
"""从参照实现导出 `min`／`max`／`sorted` 的结果，写成
`crates/pyawa-stdlib/tests/fixtures/builtins.rs`（**Rust 源码**，不是 JSON）。

为什么不是 JSON：消费方是 `pyawa-stdlib` 的测试，那个 crate 里没有 JSON 解析器
（核心测试的 `common` 跨不过来），生成源码可以省掉解析、也就省掉一份重复的解析器。
与其它夹具一样：**由脚本产出、禁止手改**。

用例的形状：`(调用名, 实参描述, 关键字描述)`，描述只覆盖本层能造的值
（`int`／`str`／`list`／`tuple`）；`key=` 只允许写成**内建函数名**。

用法::

    python3 tools/gen_builtins_fixture.py
"""

from __future__ import annotations

import builtins
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-stdlib/tests/fixtures/builtins.rs"

ADDRESS = re.compile(r"at 0x[0-9a-f]+")


def build(description):
    kind = description["kind"]
    if kind == "int":
        return int(description["value"])
    if kind == "str":
        return str(description["text"])
    if kind == "list":
        return [build(item) for item in description["items"]]
    if kind == "tuple":
        return tuple(build(item) for item in description["items"])
    raise AssertionError("夹具里出现了没见过的值种类：" + str(kind))


def i(value):
    return {"kind": "int", "value": value}


def s(text):
    return {"kind": "str", "text": text}


def lst(*items):
    return {"kind": "list", "items": list(items)}


def tup(*items):
    return {"kind": "tuple", "items": list(items)}


CASES = [
    ("min", [i(3), i(1), i(2)], {}),
    ("max", [i(3), i(1), i(2)], {}),
    ("min", [lst(i(3), i(1), i(2))], {}),
    ("max", [lst(i(3), i(1), i(2))], {}),
    ("min", [lst()], {}),
    ("min", [lst()], {"default": i(5)}),
    ("min", [lst(i(1), i(2))], {"default": i(9)}),
    ("min", [s("b"), s("a")], {}),
    ("min", [i(1), s("a")], {}),
    ("min", [lst(i(3), i(-1))], {"key": "abs"}),
    ("min", [], {}),
    ("min", [i(1)], {}),
    ("min", [tup(i(3), i(1), i(2))], {}),
    ("sorted", [tup(i(3), i(1), i(2))], {}),
    ("sorted", [lst(i(3), i(1), i(2))], {}),
    ("sorted", [lst(i(3), i(1), i(2))], {"reverse": True}),
    ("sorted", [s("ba")], {}),
    ("sorted", [i(3)], {}),
    ("sorted", [lst(i(1), s("a"))], {}),
    ("sorted", [lst(i(3), i(-1))], {"key": "abs"}),
    ("sorted", [], {}),
    # `sum`／`all`／`any`（同一批夹具；`all`／`any` 的真值走核心的 `truth_of`）
    ("sum", [lst(i(1), i(2), i(3))], {}),
    ("sum", [lst(i(1), i(2)), i(10)], {}),
    ("sum", [lst(i(1), {"kind": "str", "text": "a"})], {}),
    ("sum", [lst({"kind": "str", "text": "a"}, {"kind": "str", "text": "b"})], {}),
    ("sum", [lst(i(1))], {}),
    ("sum", [i(5)], {}),
    ("sum", [], {}),
    ("sum", [lst(), i(7)], {}),
    ("all", [lst(i(1), i(2))], {}),
    ("all", [lst(i(1), i(0))], {}),
    ("all", [lst()], {}),
    ("any", [lst()], {}),
    ("any", [lst(i(0), {"kind": "str", "text": ""})], {}),
    ("any", [lst(i(0), i(3))], {}),
    ("any", [lst({"kind": "str", "text": "x"})], {}),
    ("all", [i(5)], {}),
    ("all", [], {}),
    ("all", [{"kind": "str", "text": "ab"}], {}),
    ("any", [{"kind": "str", "text": ""}], {}),
]


def rust_value(description):
    kind = description["kind"]
    if kind == "int":
        return "Value::Int(" + str(description["value"]) + ")"
    if kind == "str":
        return 'Value::Str("' + str(description["text"]) + '")'
    variant = "List" if kind == "list" else "Tuple"
    inner = ", ".join(rust_value(item) for item in description["items"])
    return "Value::" + variant + "(&[" + inner + "])"


def rust_string(text):
    return '"' + str(text).replace("\\", "\\\\").replace('"', '\\"') + '"'


def rust_case(name, entry):
    arguments = ", ".join(rust_value(argument) for argument in entry["args"])
    keywords = entry["kwargs"]
    key = "Some(" + rust_string(keywords["key"]) + ")" if "key" in keywords else "None"
    reverse = "Some(true)" if keywords.get("reverse") else "None"
    default = (
        "Some(" + rust_value(keywords["default"]) + ")" if "default" in keywords else "None"
    )
    if "repr" in entry:
        outcome, error, message = "Some(" + rust_string(entry["repr"]) + ")", "None", "None"
    else:
        outcome = "None"
        error = "Some(" + rust_string(entry["error"]) + ")"
        message = "Some(" + rust_string(entry["message"]) + ")"
    return "\n".join(
        [
            "    Case {",
            "        name: " + rust_string(name) + ",",
            "        call: " + rust_string(entry["call"]) + ",",
            "        args: &[" + arguments + "],",
            "        key: " + key + ",",
            "        reverse: " + reverse + ",",
            "        default: " + default + ",",
            "        repr: " + outcome + ",",
            "        error: " + error + ",",
            "        message: " + message + ",",
            "    },",
        ]
    )


def main():
    recorded = {}
    for function, arguments, keywords in CASES:
        built = [build(argument) for argument in arguments]
        call_keywords = {}
        for name, description in keywords.items():
            if name == "key":
                call_keywords[name] = getattr(builtins, str(description))
            elif name == "reverse":
                call_keywords[name] = bool(description)
            else:
                call_keywords[name] = build(description)

        printed = ", ".join(repr(argument) for argument in built)
        if keywords:
            printed += ", " + ", ".join(
                str(key) + "=" + repr(value) for key, value in keywords.items()
            )
        name = function + "(" + printed + ")"

        entry = {"call": function, "args": arguments, "kwargs": keywords}
        try:
            result = getattr(builtins, function)(*built, **call_keywords)
            entry["repr"] = ADDRESS.sub("at 0x…", repr(result))
        except BaseException as error:  # noqa: BLE001 —— 参照给什么就记什么
            entry["error"] = type(error).__name__
            entry["message"] = ADDRESS.sub("at 0x…", str(error))
        recorded[name] = entry

    lines = [
        "//! 由 `tools/gen_builtins_fixture.py` 从本机 CPython 导出；**禁止手改**。",
        "//! 参照版本：" + sys.version.split()[0],
        "",
        "/// 夹具里能表达的值。",
        "#[derive(Debug, Clone, Copy)]",
        "pub enum Value {",
        "    Int(i64),",
        "    Str(&'static str),",
        "    List(&'static [Value]),",
        "    Tuple(&'static [Value]),",
        "}",
        "",
        "/// 一个用例。",
        "#[derive(Debug)]",
        "pub struct Case {",
        "    pub name: &'static str,",
        "    pub call: &'static str,",
        "    pub args: &'static [Value],",
        "    pub key: Option<&'static str>,",
        "    pub reverse: Option<bool>,",
        "    pub default: Option<Value>,",
        "    pub repr: Option<&'static str>,",
        "    pub error: Option<&'static str>,",
        "    pub message: Option<&'static str>,",
        "}",
        "",
        "pub const CASES: &[Case] = &[",
    ]
    for name, entry in sorted(recorded.items()):
        lines.append(rust_case(name, entry))
    lines.append("];")

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("已写入 " + str(OUTPUT.relative_to(ROOT)) + "（" + str(len(recorded)) + " 个用例）")
    for case, entry in sorted(recorded.items()):
        value = entry.get("repr") or (entry["error"] + ": " + entry["message"])
        print("  " + case.ljust(46) + " => " + value)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
