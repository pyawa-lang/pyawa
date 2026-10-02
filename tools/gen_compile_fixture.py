#!/usr/bin/env python3
"""从参照实现导出**编译产物**，写成 `crates/pyawa-core/tests/fixture-compile-3.14.json`。

`BC-14`…`BC-18` 给了编译器的契约，`§11` 给了「构造 → 指令族」的族级映射；本脚本把
一组**极小源码**交给参照实现编译，把每条指令（偏移／名字／oparg／argrepr——**地址归一**为 `0x…`）与 code object
的元数据原样导出。Rust 侧的发射器要产出**逐条一致**的指令流（`BC-16` 的纯函数性另有用例）。

语料只放**本轮覆盖到的构造**：模块级赋值 ＋ 整数字面量／字符串字面量／名字／`+`。
**不放**负数常量：实测 `x = -3` 的常量表顺序是 `[3, None, -3]`（折叠发生在 epilogue 之后），
那是参照实现的内部顺序细节，留给以后专门处理。

用法::

    python3 tools/gen_compile_fixture.py
"""

from __future__ import annotations

import dis
import re
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
    ("for i in s:\n    x = i\n", True, "位置表未对齐：同 `while`／`if`"),
    ("x = f(a=1)", True, ""),
    ("x = f(1, a=2)", True, ""),
    ("x = f(a=1, b=2)", True, ""),
    ("x = f(b=2, a=1)", True, ""),
    ("while a:\n    x = 1\nelse:\n    y = 2\n", True, "位置表未对齐：同 `while`"),
    ("for i in s:\n    x = i\nelse:\n    y = 1\n", True, "位置表未对齐：同 `for`"),
    ("x = f(*s)", True, "位置表未对齐：`CALL_FUNCTION_EX` 形态下存入／收尾另取一套（取目标）"),
    ("x = f(**d)", True, "位置表未对齐：同上"),
    ("x = f(*s, **d)", True, "位置表未对齐：同上"),
    ("x = f(**d, **e)", True, "位置表未对齐：同上"),
    ("x = f(1, *s)", True, "位置表未对齐：同上"),
    ("x = f(*s, a=1)", True, "位置表未对齐：同上"),
    ("x = f(a, *s, b=1, **d)", True, "位置表未对齐：同上"),
    ("while a:\n    x = 1\n", True, "位置表未对齐：`while` 体与收尾另取一套（同 `if`）"),
    ("while a < b:\n    x = 1\ny = 2\n", True, "位置表未对齐：同上"),
    ("f()", True, ""),
    ("x = f()", True, ""),
    ("x = f(1)", True, ""),
    ("x = f(a, b)", True, ""),
    ("def g(a):\n    return a\nx = g(1)\n", True, ""),
    (
        "x = f(g(1))",
        False,
        "嵌套调用的位置传播未对齐（实测外层 `CALL`／存入／收尾都取**内层调用**的跨度）",
    ),
    ("x = 1 < 2", True, ""),
    ("x = a < b", True, ""),
    ("x = a == b", True, ""),
    # 这四段：**指令流与常量表已对齐，位置表还没有**。实测 `if` 的指令（含分支里的）都取
    # **条件**的跨度，而模块收尾那两条又取另一套（跟着分支体最后一条的两半走）——那是参照实现
    # 位置传播的内部细节。按"不猜"的规矩：先如实标出来，不自造规则。
    ("if a:\n    x = 1\n", True, "位置表未对齐：`if` 指令取条件跨度，而模块收尾两条另取一套"),
    ("if a:\n    x = 1\ny = 2\n", True, "位置表未对齐：同上"),
    ("if a:\n    x = 1\nelse:\n    x = 2\n", True, "位置表未对齐：同上"),
    ("def f(a):\n    if a:\n        return 1\n    return 2\n", True, "位置表未对齐：同上"),
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
    ("def f():\n    return 1\n", True, ""),
    ("def f(a):\n    return a\n", True, ""),
    ("def f(a, b):\n    return a + b\n", True, ""),
    ("def f(a, b):\n    return b + a\n", True, ""),
    ("def f(a):\n    return a + 1\n", True, ""),
    # **PEP 649**：带注解的 `def` 会多造一个 `__annotate__` 单元（＋ `SET_FUNCTION_ATTRIBUTE 16`）
    ("def f(a: int) -> int:\n    return a\n", False, "注解单元的位置表未对齐：参照的 `__annotate__` 合成函数里，`RESUME` 取合成位点、末条 `RETURN_VALUE` 取**注解自身**的位点（要给注解记 span）"),
    ("def f() -> int:\n    return 1\n", False, "注解单元的位置表未对齐：同上"),
    ("def f(a: int):\n    return a\n", False, "注解单元的位置表未对齐：同上"),
    ("def f(a: list[int]) -> int:\n    return a\n", False, "注解单元的位置表未对齐：同上"),
    # 默认值：实测 `def f(a, b=x)` ⇒ `LOAD_NAME x; BUILD_TUPLE 1` ＋ `SET_FUNCTION_ATTRIBUTE 1`
    ("def f(a, b=x):\n    return a\n", True, ""),
    ("def f(a, b=x, c=y):\n    return a\n", True, ""),
    # 星号形参：`*args`／`**kw`（flags 的 bit2／bit3，`varnames` 排在最后）
    ("def f(*args):\n    return args\n", True, ""),
    ("def f(**kw):\n    return kw\n", True, ""),
    ("def f(a, *args, **kw):\n    return a\n", True, ""),
    ("def f(a: int, *args) -> int:\n    return a\n", False, "注解单元的位置表未对齐：同上"),
    # 仅关键字形参：默认值走 `BUILD_MAP` ＋ `SET_FUNCTION_ATTRIBUTE 2`（挂载次序 16→2→1）
    ("def f(a, *, c=3):\n    return a\n", True, ""),
    ("def f(a, *, c):\n    return a\n", True, ""),
    ("def f(a, b=2, *, c=3):\n    return a\n", True, ""),
    ("def f(a, *args, c=3, **kw):\n    return a\n", True, ""),
    # 仅位置形参：`/` 只改元数据（不产生指令），`co_posonlyargcount` 是前缀个数
    ("def f(a, /, b):\n    return a\n", True, ""),
    ("def f(a, /, b, *args):\n    return a\n", True, ""),
    ("def f(a, /, b=2):\n    return a\n", True, ""),
    # 文档字符串：进**常量 0**、不产生指令；函数置 `co_flags` 的 0x4000000；模块发 `STORE_NAME __doc__`
    ("def f():\n    \"doc\"\n    return 1\n", True, ""),
    ("def f():\n    return \"x\"\n", True, ""),
    ("\"mod\"\nx = 1\n", True, ""),
    ("def f(a):\n    \"doc\"\n    return a\n", True, ""),
    # 函数里读**全局名**：`LOAD_GLOBAL`（oparg 低位是压 NULL 标志 ⇒ 纯取值是 `下标 << 1`）
    ("def f():\n    return g\n", False, "位置表未对齐：`return <全局名>` 时参照把 `RETURN_VALUE` 也记在**表达式**的跨度上（而 `return <局部名>` 用的是语句跨度——两种口径并存）"),
    ("def f():\n    return g(1)\n", False, "位置表未对齐：`return <全局名>` 时参照把 `RETURN_VALUE` 也记在**表达式**的跨度上（而 `return <局部名>` 用的是语句跨度——两种口径并存）"),
    ("def f(a):\n    return a + g\n", False, "位置表未对齐：`return <全局名>` 时参照把 `RETURN_VALUE` 也记在**表达式**的跨度上（而 `return <局部名>` 用的是语句跨度——两种口径并存）"),
    # 类体（**不含方法**那一支）：体里铺 `__module__`／`__qualname__`／`__firstlineno__`／`__static_attributes__` ＋ 隐式 `None`
    ("class C:\n    x = 1\n", True, ""),
    ("class C:\n    \"cdoc\"\n    x = 1\n", True, ""),
    ("class C(B):\n    x = 1\n", True, ""),
    # 类体里带 `def`：会多铺 `__classdict__` cell（`MAKE_CELL` 在 `RESUME` 之前）
    ("class C:\n    def m(self):\n        return 1\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    ("class C:\n    x = 1\n    def m(self):\n        return x\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    # 属性读（`self.x`）：`LOAD_FAST_BORROW 0; LOAD_ATTR <名字下标 << 1>`；用**两个**不同属性名把位移暴露出来（别让下标 0 藏住 shift）
    ("class C:\n    x = 1\n    def m(self):\n        return self.x\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    ("class C:\n    x = 1\n    y = 2\n    def m(self):\n        return self.y\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    # 属性写（`self.y = 2`）：**先值后对象**再 `STORE_ATTR <名字下标>`；同时读 `self.z` ⇒ 两个名字把写那条的下标暴露出来
    ("class C:\n    x = 1\n    def m(self):\n        self.y = 2\n        return self.z\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    # `__init__` 那条链：形参 ＋ 属性写 ＋ 属性读（最像真实代码的形态）
    ("class P:\n    def __init__(self, v):\n        self.v = v\n    def get(self):\n        return self.v\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    ("class C:\n    def __init__(self, v):\n        self.v = 5\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    # `__static_attributes__` 的静态收集（实测：字母序去重；只读不算；嵌套函数也算）
    ("class C:\n    def m(self):\n        self.x = 1\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    ("class C:\n    def m(self):\n        self.b = 2\n        self.a = 1\n    def n(self):\n        self.c = 3\n", True, "位置表未对齐：参照给 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`（合成指令没有位置），而本层的位点表每项都是四个整数 ⇒ 表达不了「缺失」"),
    # **能落到末尾**的函数（隐式返回那一格）：这类函数以前会漏发 `LOAD_CONST None; RETURN_VALUE`
    ("def f():\n    x = 1\n", True, ""),
    ("def f(a):\n    b = a\n", True, ""),
    ("def f():\n    \"doc\"\n    x = 1\n", True, ""),
    ("def f(x):\n    if x:\n        return 1\n", True, "位置表未对齐：`if` 分支末尾的隐式 `LOAD_CONST None; RETURN_VALUE`，参照把它们记在**那条 `if` 语句**的跨度上（本层用的是最后一条真指令的位点——类体那条规则）"),
    ("def f(a):\n    x = a\n    return x\n", True, ""),
    ("def f(a):\n    return a\nx = 1\n", True, ""),
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
    if isinstance(value, tuple) and all(isinstance(item, str) for item in value):
        # `CALL_KW` 之前那条 `LOAD_CONST` 的**名元组**（本层只接线这种元组）
        return "names:" + ",".join(value)
    return f"{type(value).__name__}"


def describe_code(code) -> dict:
    """把一个 code object 描述成夹具的一节（**递归**带上嵌套的）。"""
    return {
        "mode": "pure",
        # `BC-4` 的 `co_name` 与 `co_qualname`（模块 `<module>`、模块级 def `f`、类体 `C`、方法 `C.m`）
        "name": code.co_name,
        "qualname": code.co_qualname,
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
                # **地址归一**：`<code object f at 0x…>` 里的地址每次运行都不同 ⇒ 换成 `0x…`，
                # 否则夹具每重生成一次就有 39 行噪声 diff（也不符合"夹具可复现"）
                "argrepr": re.sub(r"0x[0-9a-f]+", "0x…", instruction.argrepr or ""),
                # `BC-18` 的位置表（与指令一一对应；元组里的 `None` 原样保留成 JSON null）
                "position": list(position),
            }
            for instruction, position in zip(
                dis.get_instructions(code), code.co_positions()
            )
        ],
        # `BC-18` 的 `co_lines()`：分段（起始字节, 结束字节, 行号）
        "lines": [list(item) for item in code.co_lines()],
        # 嵌套的：位置对齐随外层（外层说没对齐，内层也不比）
        "positions_covered": True,
        "positions_uncovered_because": "",
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
        # 位置表（`BC-18`）单独一个标志：指令流对得上不代表位置也对得上
        positions_ok = covered and "位置表未对齐" not in because
        positions_reason = because if "位置表未对齐" in because else ""
        entry["positions_covered"] = positions_ok
        entry["positions_uncovered_because"] = positions_reason
        # **嵌套单元也要打同一个标志**：位置表对不上往往就出在嵌套单元里
        # （例如类体带 `def` 时，参照给合成指令 `MAKE_CELL` 的位置是 `(None, None, None, None)`）
        def mark_nested(node):
            for inner in node.get("nested", []):
                inner["positions_covered"] = positions_ok
                inner["positions_uncovered_because"] = positions_reason
                mark_nested(inner)

        mark_nested(entry)
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
