#!/usr/bin/env python3
"""从参照实现**探测**内建类型的集合与基类关系，生成
`crates/pyawa-core/src/builtin_types.rs` 与对拍夹具
`crates/pyawa-core/tests/fixture-builtin-types.json`。

`TS-41` 要求这张表**必须**由探测导出（**禁止手写枚举**）；`TS-42` 的阶梯只决定**先后**，
不改变"集合与基类关系以参照实现为准"这一条。因此本脚本不写死任何 `__bases__`／`__mro__`——
连探测哪些名字也只从 `builtins` 与 `types` 里现取。

用法::

    python3 tools/gen_builtin_types.py
"""

from __future__ import annotations

import builtins
import json
import pathlib
import sys
import types

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/src/builtin_types.rs"
FIXTURE = ROOT / "crates/pyawa-core/tests/fixture-builtin-types.json"

#: `TS-42` 的阶梯**只标先后**（名字与层次仍由探测给出）。
#: `TS-42` 的行点名了 `NoneType`／`bool`／`int`／`float`／`str`；`object` 与 `type` 是它们的根，
#: 第一阶梯就得在，故一并编入——这是**分组**的人工判断，不是"手写类型表"（名字与层次仍靠探测）。
STEP1 = ("object", "type", "NoneType", "bool", "int", "float", "str")
M2_NAMES = ("tuple", "list", "dict", "set", "function")


def ladder_of(name: str, value: type) -> str:
    if name in STEP1:
        return "step1"
    if name in M2_NAMES or name.endswith("_iterator"):
        return "m2"
    if issubclass(value, BaseException):
        return "m2"  # TS-42：BaseException 层次属 M2
    return "later"


def candidates() -> dict[str, type]:
    """探测面：`builtins` 里的类型 ＋ 几个只能用**取值**拿到的类型。"""
    found: dict[str, type] = {}
    for name in dir(builtins):
        value = getattr(builtins, name)
        if isinstance(value, type):
            found[value.__name__] = value
    for name in dir(types):
        value = getattr(types, name)
        if isinstance(value, type):
            found.setdefault(value.__name__, value)
    # 迭代器对象：没有名字可用，只能取值
    for factory in (tuple, list, str, dict, set, frozenset, bytes, bytearray, range):
        try:
            iterator = iter(factory())
        except TypeError:
            continue
        found.setdefault(type(iterator).__name__, type(iterator))
    found.setdefault(type(None).__name__, type(None))
    found.setdefault(type(lambda: 0).__name__, type(lambda: 0))
    found.setdefault(type((x for x in ())).__name__, type((x for x in ())))
    found.setdefault(type(iter([]).__next__).__name__, type(iter([]).__next__))
    return found


def describe(name: str, value: type) -> dict[str, object]:
    return {
        "name": name,
        "bases": [base.__name__ for base in value.__bases__],
        "mro": [klass.__name__ for klass in value.__mro__],
        "ladder": ladder_of(name, value),
    }


def emit_ladder(ladder: str) -> str:
    return {"step1": "Step1", "m2": "M2", "later": "Later"}[ladder]


def emit(entries: list[dict[str, object]]) -> str:
    lines = [
        "//! **TS-41**：内建类型的集合与基类关系——**由 `tools/gen_builtin_types.py` 从参照实现探测生成**。",
        "//!",
        "//! **禁止手改本文件**；也**禁止**在别处手写内建类型枚举（`PLAN-milestones.md` §9.4 的复核清单）。",
        "//! `TS-42` 的阶梯只标先后：`ladder` 字段说明这个类型被哪一批指令族逼出来。",
        "//!",
        "//! 载荷布局**不**在这张表里：`TS-43` 把它留给实现。",
        "",
        "/// `TS-42` 的阶梯。",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub enum Ladder {",
        "    /// 常量、名、跳转、运算符、`co_names`／`co_varnames` 逼出来的那一批。",
        "    Step1,",
        "    /// 容器与解包、调用与返回、迭代、异常四族逼出来的那一批。",
        "    M2,",
        "    /// 其余（随 `CM-` 分批）。",
        "    Later,",
        "}",
        "",
        "/// 一个内建类型的层次信息（名字、直接基类、MRO）。",
        "#[derive(Debug)]",
        "pub struct BuiltinType {",
        "    /// `__name__`。",
        "    pub name: &'static str,",
        "    /// `__bases__` 的名字（升序保持参照实现的顺序）。",
        "    pub bases: &'static [&'static str],",
        "    /// `__mro__` 的名字（含自身）。",
        "    pub mro: &'static [&'static str],",
        "    /// `TS-42` 的阶梯。",
        "    pub ladder: Ladder,",
        "}",
        "",
        "/// **TS-41** 的表，按名字升序（可用 [`builtin_type`] 二分查找）。",
        "pub static BUILTIN_TYPES: &[BuiltinType] = &[",
    ]
    for entry in entries:
        bases = ", ".join(f'"{name}"' for name in entry["bases"])
        mro = ", ".join(f'"{name}"' for name in entry["mro"])
        lines.append("    BuiltinType {")
        lines.append(f'        name: "{entry["name"]}",')
        lines.append(f"        bases: &[{bases}],")
        lines.append(f"        mro: &[{mro}],")
        lines.append(f'        ladder: Ladder::{emit_ladder(str(entry["ladder"]))},')
        lines.append("    },")
    lines += [
        "];",
        "",
        "/// 按名字取一个内建类型（表按名字升序，故二分）。",
        "pub fn builtin_type(name: &str) -> Option<&'static BuiltinType> {",
        "    BUILTIN_TYPES",
        "        .binary_search_by(|entry| entry.name.cmp(name))",
        "        .ok()",
        "        .map(|index| &BUILTIN_TYPES[index])",
        "}",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    entries = [describe(name, value) for name, value in candidates().items()]
    entries.sort(key=lambda entry: str(entry["name"]))
    names = [str(entry["name"]) for entry in entries]
    assert len(names) == len(set(names)), "探测面里出现了重名"

    OUTPUT.write_text(emit(entries), encoding="utf-8")
    FIXTURE.write_text(
        json.dumps(
            {
                "_note": "由 tools/gen_builtin_types.py 从本机 CPython 探测导出；禁止手改。",
                "reference": {"version": sys.version.split()[0]},
                "types": entries,
            },
            ensure_ascii=False,
            indent=1,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )

    counted: dict[str, int] = {}
    for entry in entries:
        counted[str(entry["ladder"])] = counted.get(str(entry["ladder"]), 0) + 1
    print(f"已写入 {OUTPUT.relative_to(ROOT)} 与 {FIXTURE.relative_to(ROOT)}")
    print(
        f"基线 CPython {sys.version.split()[0]}｜类型 {len(entries)} 个｜"
        + "／".join(f"{key} {value}" for key, value in sorted(counted.items()))
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
