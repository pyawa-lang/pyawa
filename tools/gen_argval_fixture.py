#!/usr/bin/env python3
"""导出**逐指令 oparg 解码**的期望值夹具：`crates/pyawa-core/tests/fixture-argval-3.14.json`。

`BC-59` 要求"对语料里出现的**每一条**指令，断言 `argval`／`argrepr` 与 `dis` 逐条一致"，
`T-BC-19`／`T-BC-20` 是它在 `BC-57`／`BC-58` 两张表上的具体化。因此本脚本把语料编译成真
code object，把 `dis` 的判决原样导出——**禁止**在脚本里手写任何 oparg 解释。

**语料的口径**：只覆盖 `§10` 已接线的族（常量／名／局部／运算符／比较／跳转／容器／解包／
调用／属性／迭代），且**不含**嵌套 code object（`def`／推导式／lambda）——那些常量的 `repr`
带地址与文件名，要等 `BC-4` 的 `co_filename`／`co_firstlineno`／`co_qualname` 落地后才能逐字比。
语料必须随 `§10` 的族增长而扩充（`BC-59`）。

用法::

    python3 tools/gen_argval_fixture.py
"""

from __future__ import annotations

import dis
import json
import pathlib
import re
import sys
import types

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-argval-3.14.json"

#: 语料：按族分组。**不含**嵌套 code object。
SNIPPETS: dict[str, str] = {
    "constants": "x = 1\ny = 2.5\nz = 'abc'\nw = None\n",
    "names_and_locals": "def f(a, b):\n    c = a\n    return c\n",
    "operators": "def f(a, b):\n    return a + b * -a\n",
    "compare": "def f(a, b):\n    if a < b:\n        return True\n    return not (a >= b)\n",
    "is_and_in": "def f(a, b):\n    return (a is b) or (a not in b)\n",
    "jumps": "def f(a):\n    while a:\n        a = a - 1\n    return a\n",
    "containers": "x = [1, 2]\ny = (3, 4)\nz = {5: 6}\nw = {7, 8}\n",
    "unpack": "def f(seq):\n    a, b = seq\n    c, *d, e = seq\n    return a\n",
    "calls": "def f(g, a):\n    return g(a, x=1)\n",
    "attributes": "def f(obj, v):\n    obj.alpha = v\n    del obj.beta\n    return obj.gamma\n",
    "iteration": "def f(it):\n    for x in it:\n        x\n    return 0\n",
    "string_ops": "x = 'ab' + 'cd'\n",
    "small_ints": "def f():\n    return 5 + 6\n",
    # §10 的**异常族**（BC-60）：处理块派发与链语义。语料只编译、不执行，
    # 所以未定义的 `guard`／`ctx` 无所谓——这里要的是**参照实现发射的真字节**。
    #
    # 写成**一段**（而不是每例一段）：每个含 `def` 的片段都会产出一个**模块级** code
    # object，而模块级样本因常量里带 code object 而不参与对拍（要等 `BC-4`），
    # 对拍测试对"跳过数"有上限。合并成一段既扩了覆盖，又不推高跳过数。
    "exceptions": (
        "def raise_plain(exc):\n"
        "    raise exc\n"
        "\n"
        "def raise_from(exc, cause):\n"
        "    raise exc from cause\n"
        "\n"
        "def bare_raise():\n"
        "    raise\n"
        "\n"
        "def suppress(exc):\n"
        "    raise exc from None\n"
        "\n"
        "def try_full(guard):\n"
        "    try:\n"
        "        guard()\n"
        "    except ValueError:\n"
        "        return 1\n"
        "    except (KeyError, IndexError) as error:\n"
        "        return error\n"
        "    else:\n"
        "        return 2\n"
        "    finally:\n"
        "        guard()\n"
        "\n"
        "def reraise(guard):\n"
        "    try:\n"
        "        guard()\n"
        "    except Exception:\n"
        "        raise\n"
        "\n"
        "def with_block(ctx, guard):\n"
        "    with ctx as value:\n"
        "        guard(value)\n"
        "    return value\n"
        "\n"
        "def nested(guard):\n"
        "    try:\n"
        "        try:\n"
        "            guard()\n"
        "        except KeyError:\n"
        "            raise ValueError\n"
        "    finally:\n"
        "        guard()\n"
    ),
}

#: `BC-59` 的对拍可以与 `MS-9` 同口径地归一化**地址**（code object 的 `repr` 里那串）。
ADDRESS = re.compile(r"0x[0-9a-f]+")


def normalize(text: str) -> str:
    return ADDRESS.sub("0x…", text)


def describe_constant(value: object) -> dict[str, object]:
    """把常量写成可重建的结构（本层只做语料里出现的那几种）。"""
    if value is None:
        return {"kind": "none"}
    if isinstance(value, bool):
        return {"kind": "bool", "value": value}
    if isinstance(value, int):
        return {"kind": "int", "value": value}
    if isinstance(value, float):
        return {"kind": "float", "value": repr(value)}
    if isinstance(value, str):
        return {"kind": "str", "value": value}
    if isinstance(value, tuple):
        return {"kind": "tuple", "items": [describe_constant(item) for item in value]}
    if isinstance(value, types.CodeType):
        return {"kind": "code", "name": value.co_name}
    return {"kind": "unsupported", "repr": normalize(repr(value))}


def describe(code: types.CodeType) -> dict[str, object]:
    instructions = [
        {
            "offset": instruction.offset,
            "opname": instruction.opname,
            "oparg": instruction.arg,
            "argval": normalize(repr(instruction.argval)),
            "argrepr": normalize(instruction.argrepr),
        }
        for instruction in dis.get_instructions(code)
    ]
    has_code_constant = any(isinstance(value, types.CodeType) for value in code.co_consts)
    return {
        "name": code.co_name,
        # 常量里含 code object 的样本**不参与逐条对拍**：它的 `repr` 带地址与文件名，
        # 要等 `BC-4` 的 `co_filename`／`co_firstlineno`／`co_qualname` 落地后才能逐字比。
        # 生成器把这件事**显式标出来**，测试据此断言，而不是悄悄跳过（`BC-59`）。
        "comparable": not has_code_constant,
        "skipped_because": "code object 常量的 repr 需要 co_filename／co_firstlineno"
        if has_code_constant
        else "",
        "co_code": code.co_code.hex(),
        "co_consts": [describe_constant(value) for value in code.co_consts],
        "co_names": list(code.co_names),
        "co_varnames": list(code.co_varnames),
        "co_cellvars": list(code.co_cellvars),
        "co_freevars": list(code.co_freevars),
        "instructions": instructions,
    }


def main() -> int:
    samples: list[dict[str, object]] = []
    for label, source in SNIPPETS.items():
        module = compile(source, "<fixture>", "exec")
        sample = describe(module)
        sample["snippet"] = label
        samples.append(sample)
        for value in module.co_consts:
            if isinstance(value, types.CodeType):
                nested = describe(value)
                nested["snippet"] = f"{label}#{value.co_name}"
                samples.append(nested)

    total = sum(len(sample["instructions"]) for sample in samples)
    opnames = sorted(
        {instruction["opname"] for sample in samples for instruction in sample["instructions"]}
    )
    fixture = {
        "_note": "由 tools/gen_argval_fixture.py 从本机 CPython 导出；禁止手改。",
        "reference": {"version": sys.version.split()[0]},
        "samples": samples,
        "opnames": opnames,
    }
    OUTPUT.write_text(
        json.dumps(fixture, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"已写入 {OUTPUT.relative_to(ROOT)}")
    print(
        f"基线 CPython {fixture['reference']['version']}｜code object {len(samples)} 个｜"
        f"指令 {total} 条｜不同指令 {len(opnames)} 种"
    )
    print("指令集合：" + " ".join(opnames))
    return 0


if __name__ == "__main__":
    sys.exit(main())
