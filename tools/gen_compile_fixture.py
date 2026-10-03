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
# ---- 列跨度差异案卷（第 225 轮）------------------------------------------------------
#
# **为什么要有这个文件**：位置表（`BC-18`）的"哪几条对不上"必须是**实测**的事实，不能靠手写理由。
# 第 225 轮发现生成器原来的配对有 bug（见 `describe_code` 里的注释）：`zip(get_instructions,
# co_positions())` 把位置整体错位 ⇒ 一大批"位置没对齐"的理由其实是**测量错**。修好配对后重新普查，
# 真正对不上的只有 60 条（且**行号级全部一致**）。这 60 条记在这里，生成器据此打标志。
def normalize_argrepr(text: str) -> str:
    """把**不稳定的渲染**归一化，避免夹具自己抖动。

    两类：① 地址（`0x…`）；② **`frozenset` 的字面量元素顺序**——它随**字符串哈希种子**变
    （实测同一条用例两次生成会得到 `frozenset({'b', 'a', 'c'})` 与 `frozenset({'c', 'a', 'b'})` ✗），
    排序之后才可比、可提交。
    """
    text = re.sub(r"0x[0-9a-f]+", "0x…", text)
    if text.startswith("frozenset({") and text.endswith("})"):
        inner = text[len("frozenset({") : -2]
        items = sorted(part.strip() for part in inner.split(",") if part.strip())
        text = "frozenset({" + ", ".join(items) + "})"
    return text


def load_position_census() -> dict:
    path = pathlib.Path(__file__).with_name("compile-positions-census.tsv")
    census = {}
    if not path.exists():
        return census
    for line in path.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        source, _, reason = line.partition("\t")
        census[json.loads(source)] = reason
    return census


POSITION_CENSUS = load_position_census()

SOURCES = [
    ("x = 1", True, ""),
    ("for i in s:\n    x = i\n", True, ""),
    ("x = f(a=1)", True, ""),
    ("x = f(1, a=2)", True, ""),
    ("x = f(a=1, b=2)", True, ""),
    ("x = f(b=2, a=1)", True, ""),
    ("while a:\n    x = 1\nelse:\n    y = 2\n", True, ""),
    ("for i in s:\n    x = i\nelse:\n    y = 1\n", True, ""),
    ("x = f(*s)", True, ""),
    ("x = f(**d)", True, ""),
    ("x = f(*s, **d)", True, ""),
    ("x = f(**d, **e)", True, ""),
    ("x = f(1, *s)", True, ""),
    ("x = f(*s, a=1)", True, ""),
    ("x = f(a, *s, b=1, **d)", True, ""),
    ("while a:\n    x = 1\n", True, ""),
    ("while a < b:\n    x = 1\ny = 2\n", True, ""),
    ("f()", True, ""),
    ("x = f()", True, ""),
    ("x = f(1)", True, ""),
    ("x = f(a, b)", True, ""),
    ("def g(a):\n    return a\nx = g(1)\n", True, ""),
    (
        "x = f(g(1))",
        True,
        "位置表未对齐：嵌套调用的位置传播（实测参照的外层 `CALL`／存入／收尾都取**内层调用**的跨度）"
        "——属 `SPEC-bytecode.md` `BC-4`「传播精度不要求」的实现观测面 ⇒ 指令流与常量池**真比对**，"
        "位置不追（`MS-19`：能力缺口／观测面不得当差异补）",
    ),
    ('def outer():\n    def inner():\n        return 1\n    return inner()\n', True, ""),
    ('def outer():\n    def inner(a, b=2):\n        return a\n    return inner\n', True, ""),
    ('def outer():\n    x = 1\n    def inner():\n        return x\n    return inner()\n', False, "未实现：**闭包**（内层引用外层局部）——要 `cellvars`／`freevars`／`MAKE_CELL`／`STORE_DEREF`／`SET_FUNCTION_ATTRIBUTE closure`；本层**如实报错**（第 279 轮起不再静默按全局发 ✗）"),
    ("x = 1 < 2", True, ""),
    ("x = a < b", True, ""),
    ("x = a == b", True, ""),
    # 这四段：**指令流与常量表已对齐，位置表还没有**。实测 `if` 的指令（含分支里的）都取
    # **条件**的跨度，而模块收尾那两条又取另一套（跟着分支体最后一条的两半走）——那是参照实现
    # 位置传播的内部细节。按"不猜"的规矩：先如实标出来，不自造规则。
    ("if a:\n    x = 1\n", True, ""),
    ("if a:\n    x = 1\ny = 2\n", True, ""),
    ("if a:\n    x = 1\nelse:\n    x = 2\n", True, ""),
    ("def f(a):\n    if a:\n        return 1\n    return 2\n", True, ""),
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
    # `P1-12`：`bytes` 字面量（转义在**词法**层解成字节；常量池里是 `bytes` 对象）
    ("x = b'abc'", True, ""),
    # ---- 第 212 轮：表达式面（二元／一元／优先级／折叠）----
    ("x = a - b", True, ""),
    # ---- 第 232 轮：lambda ----
    # ---- 第 234 轮：推导式 ----
    ("y = [x for x in s]\n", True, ""),
    ("y = [x * 2 for x in s if x]\n", True, ""),
    ("def f(s):\n    return [x + 1 for x in s]\n", True, ""),
    ("def scale(factor, values):\n    return [v * factor for v in values]\n", True, ""),
    ("y = {x for x in s}\n", True, ""),
    # ---- 第 242 轮：集合字面量 ----
    ("x = {1, 2}\n", True, ""),
    # ---- 第 244 轮：import ----
    ("import a\n", True, ""),
    ("import a.b\n", True, ""),
    ("import a.b as c\n", True, ""),
    ("import a, b as c\n", True, ""),
    ("from a import b\n", True, ""),
    ("from a import b as c, d\n", True, ""),
    ("from a import *\n", True, ""),
    ("from . import b\n", True, ""),
    # ---- 第 246 轮：try 的 else／finally ----
    ("try:\n    x = 1\nexcept:\n    y = 2\nelse:\n    z = 3\n", True, ""),
    ("try:\n    x = 1\nfinally:\n    z = 3\n", True, ""),
    ("try:\n    x = 1\nexcept:\n    y = 2\nfinally:\n    z = 3\n", True, ""),
    # ---- 第 247 轮：循环体内的 return ----
    ("def f(s):\n    for i in s:\n        return 1\n", True, ""),
    ("def f(s):\n    for i in s:\n        return i\n", True, ""),
    ("def f(s):\n    while s:\n        return 1\n", True, ""),
    ("def f(s, t):\n    for i in s:\n        for j in t:\n            return 1\n", True, ""),
    ("def f(s, t):\n    for i in s:\n        for j in t:\n            return i\n", True, ""),
    ("def f(s, t):\n    for i in s:\n        for j in t:\n            return 1\n", True, ""),
    ("def f(s):\n    for i in s:\n        if i:\n            return i\n", True, ""),
    ("def f(s):\n    for i in s:\n        return g()\n", True, ""),
    # ---- 第 248 轮：For/If 联合窥孔（循环体末尾"体不落到末尾的 if"）----
    ("def f(s):\n    for i in s:\n        if i:\n            return i\n", True, ""),
    ("def f(s):\n    for i in s:\n        if i:\n            break\n", True, ""),
    ("def f(s):\n    for i in s:\n        if i:\n            continue\n", True, ""),
    ("def f(s):\n    while s:\n        if s:\n            return 1\n", True, ""),
    ("def f(s):\n    for i in s:\n        if i:\n            return 1\n    return 2\n", True, ""),
    ("def f(s):\n    for i in s:\n        if i:\n            return 1\n        x = 2\n", True, ""),
    # ---- 第 249 轮：集合字面量折叠 ----
    ("x = {1, 2, 3}\n", True, ""),
    ("x = {1, 2, 3, 4}\n", True, ""),
    ("x = {1, 1, 2}\n", True, ""),
    ("x = {a, 1, 2}\n", True, ""),
    ("x = {\"b\", \"a\", \"c\"}\n", True, ""),
    # ---- 第 250 轮：字符串转义 ----
    ('x = "a\\nb"\n', True, ""),
    ('x = "\\x41"\n', True, ""),
    ('x = "\\u0041"\n', True, ""),
    ('x = "\\N{BULLET}"\n', False, "未实现：`\\N{…}` 具名转义要整张 Unicode 名字表（与 M3 的 `Lib/`／数据面绑定）"),
    ('x = r"a\\nb"\n', True, ""),
    ('x = "a\\\nb"\n', True, ""),
    ('x = "\\t\\r\\\\"\n', True, ""),
    ('y = f"a\\n{b}"\n', True, ""),
    ('y = rf"a\\n{b}"\n', True, ""),
    ('y = f"\\t{x}\\t"\n', True, ""),
    ('y = f"\\x41{x}\\u0042"\n', True, ""),
    # ---- 第 252 轮：三引号／跨行 ----
    ('x = """a\nb"""\n', True, ""),
    ('y = f"""a\n{b}\nc"""\n', True, ""),
    # ---- 第 253 轮：with 体内的 return ----
    ('def f(cm):\n    with cm as y:\n        return y\n', True, ""),
    # ---- 第 254 轮：with 体内 return 的退出调用 ----
    ('def f(cm):\n    with cm as y:\n        with y:\n            return y\n', False, "未对齐：**嵌套 `with` 且体内 `return`** 时清理块的几何不同（参照用 `JUMP_FORWARD`＋`NOP` 复用退出调用，本层重放一遍）；`return` 路径本身已逐字节一致（规则待推）"),
    ('def f(a, b):\n    with a, b:\n        return a\n', False, "未对齐：**多项 `with` 且体内 `return`** 时清理块的几何不同——参照把退出调用与收尾做成**共享收尾块**（清理块用 `JUMP_FORWARD` 跳过三连、再走退出调用），本层重放一遍；`return` 路径本身已逐字节一致"),
    # ---- 第 261 轮：批量扩面 ----
    ('with a:\n    with b:\n        x = 1\n', False, "未对齐：**共享收尾块**——实测 `with a: with b: x = 1`：参照是「退出 b; 退出 a; 收尾」；本层内层 `with` 走完余部（空）就**抢先发了作用域收尾**（`LOAD_CONST None; RETURN_VALUE`），而外层的退出调用还没跑 ⇒ 位置错。第 272 轮试过让 `emit_rest_and_tail` 先跳块尾、把收尾留给外层，当场弄坏 `try/finally` 后接代码与 `for/break` 位点两条 ✗ ⇒ 根子是 `epilogue_needed` 是**作用域级单标志**，表达不了收尾归哪一层；正解要按块记录收尾归属，属结构性改动。**第 273 轮复核**：每次先重新生成夹具再直跑，连续 3 次均失败 ⇒ 登记有效（中途一次误判为「通过」，是因为漏了重新生成夹具、JSON 里该用例仍是 `covered=False` ⇒ 被跳过 ✗；教训：跑夹具前先跑生成器并检查退出码）"),
    # ---- 第 281 轮：扩面 ----
    ('def outer():\n    x = 1\n    def inner():\n        return 1\n    return inner() + x\n', True, ""),
    ('def outer():\n    def g():\n        return 2\n    def h():\n        return 3\n    return g() + h()\n', True, ""),
    ('def outer():\n    x = 1\n    def inner(a=x):\n        return a\n    return inner\n', True, ""),
    ('for i in s:\n    continue\nelse:\n    y = 1\n', True, ""),
    ('while a:\n    break\nelse:\n    y = 1\n', True, "行号级未对齐：**指令流与常量池已逐字节一致**（第 284 轮收窄了 `while` 里 `break` 那条 `NOP`：**只有「循环之后还有代码」时才发**）；只剩**行表**差异：参照把「重放的余部＋收尾」挂在 `break` 自己的**粘性位点**上（`(2,2,4,9)`），本层用收尾的跨度 ⇒ 属 `BC-4`「传播精度不要求」的实现观测面，按 `MS-19` 不追"),
    ('x = 1 // 0\n', True, ""),
    ('y = "a" "b"\n', True, ""),
    # ---- 第 285 轮：f-string 与相邻串 ----
    ('y = "a" f"b{c}"\n', True, ""),
    ('y = f"a{c}" "b"\n', True, ""),
    ('y = "a" "b" f"c{d}"\n', True, ""),
    ('x = a if b else c\n', False, "未对齐：**三元表达式已接线**（第 282 轮，语义正确）。参照的形态是「把余部**复制**进两个分支」（`y = f(a if b else c)` 里 `CALL`＋存入＋收尾各两次、`return a if b else c` 里 `RETURN_VALUE` 两次）⇒ 要**表达式级续延**模型；本层用 `JUMP_FORWARD` 汇合（多一条跳转），布局不同但语义等价。语料 `ternary_expression.py` 守着两分支语义"),
    ('with a, b, c:\n    x = 1\n', True, "位置表未对齐：**多项 `with`** 清理块相关的位点（「共享收尾块」家族），第 262 轮扩面时确认"),
    ('with a as x, b as y, c as z:\n    pass\n', False, "未对齐：**多项 `with`** 的清理块几何（「共享收尾块」家族），同上"),
    ('y = [x for x in s if x if x]\n', True, ""),
    ('y = [x for x in s for y in t if y]\n', True, ""),
    ('y = {k: v for k, v in s if k}\n', True, ""),
    ('y = {x for x in s for y in t}\n', True, ""),
    ('y = f"{x!r:>{w}}"\n', True, ""),
    ('y = f"{x!s}"\n', True, ""),
    ('y = f"{x!a}"\n', True, ""),
    ('y = f"a{x}b{y}c"\n', True, ""),
    ('if not a and not b:\n    x = 1\n', False, "未对齐：指令流不同——与「条件路径的作用域收尾／共享收尾块」一族同源；第 262 轮扩面时暴露，规则待推"),
    ('if a < b < c:\n    x = 1\n', False, "未对齐：**链式比较当条件**时，段间跳转与 `COMPARE_OP |16` 已逐字节一致，差别在**末尾**——参照在 `JUMP_FORWARD`／`POP_TOP` 之后还要接一对**作用域收尾**（`LOAD_CONST None; RETURN_VALUE`），本层只发了 `POP_TOP` ⇒ 需要把「条件里遗留值」的清理与 `emit_scope_tail` 接起来"),
    # ---- 第 262 轮：链式比较 ----
    ('x = a < b < c\n', False, "未对齐：**链式比较的失败路径未外提**——参照把 `SWAP 2; POP_TOP` 与**语句余部**一起外提到语句之后（`POP_JUMP_IF_FALSE` 指过去），本层就地发出；**语义与前半段指令已一致**（语料 `chained_compare.py` 通过）。规则待推（与「共享收尾块」同族）"),
    ('def f(a):\n    try:\n        x = 1\n    finally:\n        y = 2\n    z = 3\n', True, ""),
    ('1 // 0\n', True, ""),
    ('x = a < b < c < d\n', False, "未对齐：**链式比较的失败路径未外提**——参照把 `SWAP 2; POP_TOP` 与**语句余部**一起外提到语句之后（`POP_JUMP_IF_FALSE` 指过去），本层就地发出；**语义与前半段指令已一致**（语料 `chained_compare.py` 通过）。规则待推（与「共享收尾块」同族）"),
    ('x = a < b > c\n', False, "未对齐：**链式比较的失败路径未外提**——参照把 `SWAP 2; POP_TOP` 与**语句余部**一起外提到语句之后（`POP_JUMP_IF_FALSE` 指过去），本层就地发出；**语义与前半段指令已一致**（语料 `chained_compare.py` 通过）。规则待推（与「共享收尾块」同族）"),
    ('x = a == b != c\n', False, "未对齐：**链式比较的失败路径未外提**——参照把 `SWAP 2; POP_TOP` 与**语句余部**一起外提到语句之后（`POP_JUMP_IF_FALSE` 指过去），本层就地发出；**语义与前半段指令已一致**（语料 `chained_compare.py` 通过）。规则待推（与「共享收尾块」同族）"),
    ('x = a and b and c\n', True, ""),
    ('x = not not a\n', True, ""),
    ('for i in s:\n    x = 1\nelse:\n    y = 2\n', True, ""),
    ('while a:\n    x = 1\nelse:\n    y = 2\n', True, ""),
    ('try:\n    x = 1\nexcept A as e:\n    y = 2\nelse:\n    z = 3\nfinally:\n    w = 4\n', True, ""),
    ('def f(x):\n    try:\n        return 1\n    finally:\n        y = 2\n', True, "行号级未对齐：**指令流、常量池与行号形状已全部对齐**（第 270 轮修掉 finally 出口内联的**语义 bug**，并让体终止时的死代码收尾不再发）；只剩 finally 副本之后那条 `LOAD_SMALL_INT`／`RETURN_VALUE` 的**列区间**参照取**粘性**位置（`(5,5,8,9)`＝finally 最后一条）、本层取字面量自己的（`(3,3,15,16)`）——属 `SPEC-bytecode.md` `BC-4`「传播精度不要求」允许的**实现观测面**，按 `MS-19` 不追"),
    ('f = lambda a, b=1, *c, **d: a\n', True, ""),
    ('def outer():\n    def inner():\n        return 1\n    return inner()\n', True, ""),
    # ---- 第 255 轮：定位（with + return + if） ----
    ('class CM:\n    def __init__(self, tag):\n        self.tag = tag\n    def __enter__(self):\n        return self.tag\n    def __exit__(self, kind, value, tb):\n        return False\ndef take(flag):\n    with CM(5) as tag:\n        if flag:\n            return tag\n    return 0\ntotal = take(1)\n', True, ""),
    ('def f(cm):\n    with cm:\n        return 1\n', True, "行号级未对齐：指令流与常量池**已逐字节一致**（第 269 轮修好「单项 `with` ＋ 体必然终止 ⇒ 省掉正常退出块」这块死代码）；只剩**行表尾部**那三连合成清理（`COPY 3; POP_EXCEPT; RERAISE 1`）的行号归属与参照不同——属 `SPEC-bytecode.md` `BC-4`「传播精度不要求」允许的**实现观测面**，不追（`MS-19`：不得当缺口补）"),
    ('def f(cm):\n    with cm as y:\n        if y:\n            return 1\n', False, "未对齐：`with` 序言里上下文那一条——参照发 **`LOAD_FAST`**、本层发 **`LOAD_FAST_BORROW`**（借用优化规则待推）；其余指令与常量池已逐字节一致（字面量 return 的 NOP 也在位）"),
    ('def f(a, b):\n    with a, b:\n        return 1\n', False, "未对齐：**共享收尾块**（清理块几何）——参照把退出调用与收尾做成共享块、清理块 `JUMP_FORWARD` 跳过三连；本层重放一遍。`return` 路径与常量池已一致，差的是几何"),
    ('def f(cm):\n    with cm as y:\n        with y:\n            return 1\n', False, "未对齐：**共享收尾块**（清理块几何）——参照把退出调用与收尾做成共享块、清理块 `JUMP_FORWARD` 跳过三连；本层重放一遍。`return` 路径与常量池已一致，差的是几何"),
    ('x = "\\x41\\u0042\\103"\n', True, ""),
    ('x = "\\"q\\""\n', True, ""),




    ("def f():\n    import a\n", True, ""),

    ("a = 1\nb = 2\nx = {a, b}\n", True, ""),
    ("x = {a}\n", True, ""),

    ("y = {x * 2 for x in s if x}\n", True, ""),
    ("y = {k: 1 for k in s}\n", True, ""),
    # ---- 第 238 轮：f-string ----
    ("y = f\"{x}\"\n", True, ""),
    ("y = f\"a{x}b\"\n", True, ""),
    ("y = f\"{x!r}\"\n", True, ""),
    ("y = f\"{x:>5}\"\n", True, ""),
    ("y = f\"{x + 1}\"\n", True, ""),
    ("y = f\"\"\n", True, ""),
    ("y = f\"{{}}\"\n", True, ""),
    ("y = f\"{x}{y}\"\n", True, ""),
    ("y = f\"{x:>{w}}\"\n", True, ""),

    ("y = {k: k + 1 for k in s if k}\n", True, ""),
    ("y = {k: v for k, v in s}\n", True, ""),
    ("y = [a + b for a in s for b in t]\n", True, ""),
    ("y = [x for x in s if p if q]\n", True, ""),




    ("f = lambda x: x + 1\n", True, ""),
    ("g = lambda: 1\n", True, ""),
    ("h = lambda x, y=2, *a, **k: x\n", True, ""),
    ("z = sorted(k, key=lambda v: v)\n", True, ""),
    ("def outer():\n    return lambda v: v\n", True, ""),

    # ---- 第 230 轮：with ----
    ("with a as x, b as y:\n    z = 1\n", True, "位置表未对齐：参照给 `with`／`try` 的**合成指令**（`PUSH_EXC_INFO`、清理块等）的位点是 `(None, None, None, None)`，本层的位点表表达不了「缺失」；指令流与常量池仍逐字节比"),
    ("with cm:\n    x = 1\n", True, "位置表未对齐：参照给 `try`／`with` 的**合成指令**（`PUSH_EXC_INFO`、清理块等）的位点是 `(None, None, None, None)`，本层的位点表表达不了「缺失」；指令流与常量池仍逐字节比"),
    ("with cm as y:\n    x = 1\n", True, "位置表未对齐：参照给 `try`／`with` 的**合成指令**（`PUSH_EXC_INFO`、清理块等）的位点是 `(None, None, None, None)`，本层的位点表表达不了「缺失」；指令流与常量池仍逐字节比"),
    ("def f(cm):\n    with cm as y:\n        return y\n", True, "位置表未对齐：参照给 `try`／`with` 的**合成指令**（`PUSH_EXC_INFO`、清理块等）的位点是 `(None, None, None, None)`，本层的位点表表达不了「缺失」；指令流与常量池仍逐字节比"),
    # ---- 第 229 轮：块结构模型（try 的出口重放"余部＋收尾"）----
    ("try:\n    x = 1\nexcept:\n    y = 2\n", True, "位置表未对齐：参照给 `PUSH_EXC_INFO`／清理块（`COPY 3; POP_EXCEPT; RERAISE 1`）这些**合成指令**的位点是 `(None, None, None, None)`，而本层的位点表每项都是四个整数 ⇒ **表达不了「缺失」**；指令流与常量池仍逐字节比"),
    ("try:\n    x = 1\nexcept:\n    y = 2\nz = 3\n", True, "位置表未对齐：参照给 `PUSH_EXC_INFO`／清理块（`COPY 3; POP_EXCEPT; RERAISE 1`）这些**合成指令**的位点是 `(None, None, None, None)`，而本层的位点表每项都是四个整数 ⇒ **表达不了「缺失」**；指令流与常量池仍逐字节比"),
    ("try:\n    x = 1\nexcept ValueError:\n    y = 2\n", True, "位置表未对齐：参照给 `PUSH_EXC_INFO`／清理块（`COPY 3; POP_EXCEPT; RERAISE 1`）这些**合成指令**的位点是 `(None, None, None, None)`，而本层的位点表每项都是四个整数 ⇒ **表达不了「缺失」**；指令流与常量池仍逐字节比"),
    ("try:\n    x = 1\nexcept Exception as e:\n    y = 2\n", True, "位置表未对齐：参照给 `PUSH_EXC_INFO`／清理块（`COPY 3; POP_EXCEPT; RERAISE 1`）这些**合成指令**的位点是 `(None, None, None, None)`，而本层的位点表每项都是四个整数 ⇒ **表达不了「缺失」**；指令流与常量池仍逐字节比"),
    ("def f(a):\n    try:\n        x = 1\n    except:\n        y = 2\n    return 3\n", True, "位置表未对齐：参照给 `PUSH_EXC_INFO`／清理块（`COPY 3; POP_EXCEPT; RERAISE 1`）这些**合成指令**的位点是 `(None, None, None, None)`，而本层的位点表每项都是四个整数 ⇒ **表达不了「缺失」**；指令流与常量池仍逐字节比"),
    # ---- 第 229 轮：块结构模型（`break`／`try` 的退出路径重放"余部＋收尾"）----
    ("for i in s:\n    break\n", True, ""),
    ("for i in s:\n    break\nx = 1\n", True, ""),
    ("for i in s:\n    if i:\n        break\n    x = i\n", True, ""),
    ("for i in s:\n    if i:\n        break\n    x = i\ny = 2\n", True, ""),
    ("while a:\n    break\ny = 1\n", True, ""),
    ("for i in s:\n    break\nelse:\n    y = 1\n", True, ""),
    ("def f(s):\n    for i in s:\n        break\n    x = 1\n    return x\n", True, ""),

    # ---- 第 223 轮：continue（`break` 的块结构差异见 PLAN，只走语料）----
    ("for i in s:\n    continue\n", True, ""),
    ("while a:\n    continue\n", True, ""),

    # ---- 第 222 轮：pass 与链式赋值目标 ----
    ("", True, ""),
    ("pass", True, ""),
    ("class C:\n    pass\n", True, ""),
    ("def f():\n    pass\n", True, ""),
    ("if a:\n    pass\n", True, ""),

    # ---- 第 221 轮：矩阵乘 `@`／`@=` ----
    ("x = a @ b", True, ""),
    ("x @= b", True, ""),
    ("def f(a, b):\n    return a @ b\n", True, ""),

    # ---- 第 217 轮：not／is／in ----
    ("x = not a", True, ""),
    # ---- 第 218 轮：and／or ----
    # ---- 第 219 轮：增强赋值 ----
    ("x += 1", True, ""),
    # ---- 第 220 轮：elif ----
    ("if a:\n    x = 1\nelif b:\n    x = 2\nelse:\n    x = 3\n", True, ""),
    ("if a:\n    x = 1\nelif b:\n    x = 2\n", True, ""),
    ("if a:\n    x = 1\nelif b:\n    x = 2\nelif c:\n    x = 3\n", True, ""),

    ("x -= 1", True, ""),
    ("x *= 2", True, ""),
    ("x //= 2", True, ""),
    ("x %= 2", True, ""),
    ("x **= 2", True, ""),
    ("x <<= 2", True, ""),
    ("x >>= 2", True, ""),
    ("x &= 2", True, ""),
    ("x |= 2", True, ""),
    ("x ^= 2", True, ""),
    ("x /= 2", True, ""),
    ("a.b += 1", True, ""),
    ("a[0] += 1", True, ""),
    ("a[b] -= 1", True, ""),
    (
        "def f():\n    x = 0\n    x += 1\n    return x\n",
        True,
        "位置表未对齐：增强赋值之后的 `return` 指令沿用**上一条语句**（`x += 1`）的跨度"
        "——属参照内部粘性 loc 传播；按 `BC-4`「传播精度不要求」⇒ 指令流与常量池**真比对**，位置不追",
    ),
    ("x = a and b", True, ""),
    ("x = a or b", True, ""),
    ("x = a and b and c", True, ""),
    ("x = a or b or c", True, ""),
    ("x = a and b or c", True, ""),
    ("x = a or b and c", True, ""),
    ("x = (a and b) or (c and d)", True, ""),
    ("x = (a or b) and c", True, ""),
    ("x = a and (b or c)", True, ""),
    ("x = a or b or c and d", True, ""),
    ("x = 1 and 2", True, ""),
    ("x = 0 and 3", True, ""),
    ("x = 1 or 2", True, ""),
    ("x = 0 or 3", True, ""),
    ("if a and b:\n    x = 1\ny = 2\n", True, ""),
    ("if a or b:\n    x = 1\ny = 2\n", True, ""),
    ("if not (a and b):\n    x = 1\ny = 2\n", True, ""),
    ("if (a or b) and c:\n    x = 1\ny = 2\n", True, ""),
    ("while a and b:\n    x = 1\ny = 2\n", True, ""),
    ("def f(a, b):\n    return a and b\n", True, ""),

    ("x = not 0", True, ""),
    ("x = not 1", True, ""),
    ("x = not not a", True, ""),
    ("x = not a is b", True, ""),
    ("x = not a in b", True, ""),
    ("x = not a < b", True, ""),
    ("x = not a == b", True, ""),
    ("x = not not a < b", True, ""),
    ("x = not not a is b", True, ""),
    ("x = a is b", True, ""),
    ("def f(a, b):\n    return a is b\n", True, ""),
    ("def f(a, b):\n    return a in b\n", True, ""),
    ("x = a is not b", True, ""),
    ("x = a in b", True, ""),
    ("x = a not in b", True, ""),
    ("if not a:\n    x = 1\n", True, ""),
    ("if a is b:\n    x = 1\n", True, ""),

    # ---- 第 213 轮：括号／元组字面量／下标 ----
    ("x = (1)", True, ""),
    ("x = (a)", True, ""),
    ("x = (a + b)", True, ""),
    ("x = ()", True, ""),
    ("x = (1, 2)", True, ""),
    ("x = (1,)", True, ""),
    ("x = (a, b)", True, ""),
    ("x = 1, 2", True, ""),
    ("x = a, b", True, ""),
    ("x = a[1]", True, ""),
    # ---- 第 221 轮：统一后缀链（`a[0].b` 一族）----
    ("x = a.b.c", True, ""),
    ("x = a.b[0].c", True, ""),
    ("def f(a):\n    return a[0].b\n", True, ""),
    # ---- 第 222 轮：链式赋值目标 ----
    ("a[0].b = v", True, ""),
    ("a.b[0] = v", True, ""),
    ("a[0].b.c = v", True, ""),
    ("a[0].b += v", True, ""),
    ("a.b[0] += v", True, ""),
    ("x = a[0].b", True, ""),
    ("x = a[0][1].b", True, ""),
    ("x = f()[0].b", True, ""),

    # ---- 第 214 轮：切片 ----
    ("x = a[1:2]", True, ""),
    ("x = a[:2]", True, ""),
    ("x = a[1:]", True, ""),
    ("x = a[:]", True, ""),
    ("x = a[::2]", True, ""),
    ("x = a[1:2:3]", True, ""),
    ("x = a[b:c]", True, ""),
    ("x = a[b:]", True, ""),
    ("x = a[:c]", True, ""),
    ("x = a[b:c:d]", True, ""),
    ("x = a[b::d]", True, ""),
    ("a[1:2] = b", True, ""),
    ("def f(a, b, c):\n    return a[b:c]\n", True, ""),
    ("def f(a, b):\n    return a[b]\n", True, ""),
    ("def f(a, b, c, d):\n    return a[b:c:d]\n", True, ""),

    ("a[1] = 2", True, ""),
    ("a[0][1] = 5", True, ""),
    ("a[b] = c", True, ""),

    ("x = a[b]", True, ""),
    ("x = a[1][2]", True, ""),
    ("x = a[1 + 2]", True, ""),
    ("def f(a):\n    return a[0]\n", True, ""),

    ("x = a * b", True, ""),
    ("x = a / b", True, ""),
    ("x = a // b", True, ""),
    ("x = a % b", True, ""),
    ("x = a ** b", True, ""),
    ("x = a & b", True, ""),
    ("x = a | b", True, ""),
    ("x = a ^ b", True, ""),
    ("x = a << b", True, ""),
    ("x = a >> b", True, ""),
    ("x = -a", True, ""),
    ("x = +a", True, ""),
    ("x = ~a", True, ""),
    ("x = a + b * c", True, ""),
    ("x = a * b + c", True, ""),
    ("x = a ** b ** c", True, ""),
    (
        "x = -a ** b",
        True,
        "位置表未对齐：一元套二元时收尾（存入／收尾两条）取**操作数**跨度——属参照内部传播细节；"
        "按 `BC-4`「传播精度不要求」⇒ 指令流与常量池**真比对**，位置不追",
    ),
    ("x = a << b + c", True, ""),
    ("x = a | b & c", True, ""),
    ("x = 2 * 3", True, ""),
    ("x = 7 // 2", True, ""),
    ("x = 7 % 2", True, ""),
    ("x = 2 ** 3", True, ""),
    ("x = 1 << 3", True, ""),
    ("x = -5", True, ""),
    ("x = ~5", True, ""),
    ("x = +5", True, ""),
    ("x = 6 - 2 - 1", True, ""),
    ("def f(a, b):\n    return a * b\nx = f(2, 3)\n", True, ""),

    ("x = b'\\x00\\xff'", True, ""),
    # `b'ab' + b'cd'`：参照在**编译期**折成 `b'abcd'`（与 `'a' + 'b'` 同一条路）
    ("x = b'ab' + b'cd'", True, ""),
    ("long_name = 255", True, ""),
    ("def f():\n    return 1\n", True, ""),
    ("def f(a):\n    return a\n", True, ""),
    ("def f(a, b):\n    return a + b\n", True, ""),
    ("def f(a, b):\n    return b + a\n", True, ""),
    ("def f(a):\n    return a + 1\n", True, ""),
    # **PEP 649**：带注解的 `def` 会多造一个 `__annotate__` 单元（＋ `SET_FUNCTION_ATTRIBUTE 16`）
    ("def f(a: int) -> int:\n    return a\n", True, ""),
    ("def f() -> int:\n    return 1\n", True, ""),
    ("def f(a: int):\n    return a\n", True, ""),
    ("def f(a: list[int]) -> int:\n    return a\n", True, ""),
    # 默认值：实测 `def f(a, b=x)` ⇒ `LOAD_NAME x; BUILD_TUPLE 1` ＋ `SET_FUNCTION_ATTRIBUTE 1`
    ("def f(a, b=x):\n    return a\n", True, ""),
    ("def f(a, b=x, c=y):\n    return a\n", True, ""),
    # 星号形参：`*args`／`**kw`（flags 的 bit2／bit3，`varnames` 排在最后）
    ("def f(*args):\n    return args\n", True, ""),
    ("def f(**kw):\n    return kw\n", True, ""),
    ("def f(a, *args, **kw):\n    return a\n", True, ""),
    ("def f(a: int, *args) -> int:\n    return a\n", True, ""),
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
    ("def f():\n    return g\n", True, ""),
    ("def f():\n    return g(1)\n", True, ""),
    ("def f(a):\n    return a + g\n", True, ""),
    # 类体（**不含方法**那一支）：体里铺 `__module__`／`__qualname__`／`__firstlineno__`／`__static_attributes__` ＋ 隐式 `None`
    ("class C:\n    x = 1\n", True, ""),
    ("class C:\n    \"cdoc\"\n    x = 1\n", True, ""),
    ("class C(B):\n    x = 1\n", True, ""),
    # 类体里带 `def`：会多铺 `__classdict__` cell（`MAKE_CELL` 在 `RESUME` 之前）
    ("class C:\n    def m(self):\n        return 1\n", True, ""),
    ("class C:\n    x = 1\n    def m(self):\n        return x\n", True, ""),
    # 属性读（`self.x`）：`LOAD_FAST_BORROW 0; LOAD_ATTR <名字下标 << 1>`；用**两个**不同属性名把位移暴露出来（别让下标 0 藏住 shift）
    ("class C:\n    x = 1\n    def m(self):\n        return self.x\n", True, ""),
    ("class C:\n    x = 1\n    y = 2\n    def m(self):\n        return self.y\n", True, ""),
    # 属性写（`self.y = 2`）：**先值后对象**再 `STORE_ATTR <名字下标>`；同时读 `self.z` ⇒ 两个名字把写那条的下标暴露出来
    ("class C:\n    x = 1\n    def m(self):\n        self.y = 2\n        return self.z\n", True, ""),
    # `__init__` 那条链：形参 ＋ 属性写 ＋ 属性读（最像真实代码的形态）
    ("class P:\n    def __init__(self, v):\n        self.v = v\n    def get(self):\n        return self.v\n", True, ""),
    ("class C:\n    def __init__(self, v):\n        self.v = 5\n", True, ""),
    # `__static_attributes__` 的静态收集（实测：字母序去重；只读不算；嵌套函数也算）
    ("class C:\n    def m(self):\n        self.x = 1\n", True, ""),
    # `None` 是**常量**（实测：常量表 `['None']`、`LOAD_CONST 0`）
    ("x = None\n", True, ""),
    ("x = []\n", True, ""),
    ("x = [1, 2]\n", True, ""),
    ("x = {}\n", True, ""),
    ("x = {1: 2}\n", True, ""),
    ("x = True\n", True, ""),
    ("x = False\n", True, ""),
    ("y = None\nz = None\n", True, ""),
    ("raise ValueError(1) from None\n", True, ""),
    # `raise`（`RAISE_VARARGS`）：1 带值／2 带因／0 裸重抛
    ("raise ValueError(1)\n", True, ""),
    ("def f():\n    raise ValueError(1)\n", True, ""),
    # 复合语句体里的收集（实测：`if`／`while`／`for` 的体都算，`else` 体也算）
    ("class C:\n    def m(self, x):\n        if x:\n            self.a = 1\n", True, ""),
    ("class C:\n    def m(self, xs):\n        for i in xs:\n            self.b = i\n", True, ""),
    ("class C:\n    def m(self):\n        self.b = 2\n        self.a = 1\n    def n(self):\n        self.c = 3\n", True, ""),
    # **能落到末尾**的函数（隐式返回那一格）：这类函数以前会漏发 `LOAD_CONST None; RETURN_VALUE`
    ("def f():\n    x = 1\n", True, ""),
    ("def f(a):\n    b = a\n", True, ""),
    ("def f():\n    \"doc\"\n    x = 1\n", True, ""),
    ("def f(x):\n    if x:\n        return 1\n", True, ""),
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
    if isinstance(value, slice):
        # 常量切片（`P1-10` 的表达式面）：与 `tests/compile.rs` 的渲染同一口径
        show = lambda item: "None" if item is None else str(item)
        return f"slice:{show(value.start)},{show(value.stop)},{show(value.step)}"
    if isinstance(value, str):
        return f"str:{value}"
    if isinstance(value, bytes):
        # `P1-12`：`bytes` 字面量记成十六进制（与 `tests/compile.rs` 的渲染同一口径）
        return f"bytes:{value.hex()}"
    if isinstance(value, frozenset):
        # 集合字面量折叠（第 249 轮）：`{1,2,3}` ⇒ 常量表里是 `frozenset`，
        # 元素各自渲染后**排序**（与 `tests/compile.rs` 同一口径，避免依赖哈希序）
        return "frozenset:" + ",".join(sorted(describe_constant(item) for item in value))
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
                "argrepr": normalize_argrepr(instruction.argrepr or ""),
                # `BC-18` 的位置表（与指令一一对应；缺失位置原样保留成 JSON null）
                #
                # **踩过的坑（第 225 轮）**：这里原来是
                # `zip(dis.get_instructions(code), code.co_positions())` —— 但
                # `co_positions()` 是**每个码元**一条（**含 inline cache 槽**），而
                # `get_instructions()` 默认**不显示 cache** ⇒ 只要前面有带 cache 的指令，
                # 后面的位置就**整体错位**，夹具里的位置期望因此是错的（一大批
                # `positions_covered=false` 的理由其实是这个测量 bug，不是参照的行为）。
                # 正确配对是**每条指令自己的** `Instruction.positions`。
                "position": (
                    None if instruction.positions is None else list(instruction.positions)
                ),
            }
            for instruction in dis.get_instructions(code)
        ],
        # `BC-18` 的 `co_lines()`：分段（起始字节, 结束字节, 行号）
        "lines": [list(item) for item in code.co_lines()],
        # 嵌套的：位置对齐随外层（外层说没对齐，内层也不比）
        "positions_covered": True,
        "positions_uncovered_because": "",
        # **`MS-17`**：行号级**必须**一致（`co_lines()`／`f_lineno`／`traceback` 是可观察语义）；
        # 只有"参照粘性 loc 还没推出来"的那几条用理由前缀 `行号级未对齐` 显式标出
        "lines_covered": True,
        "lines_uncovered_because": "",
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
        # **列跨度差异以案卷为准**（实测事实覆盖手写理由；案卷为空则该用例位置全比）
        because = POSITION_CENSUS.get(source, because)
        entry["source"] = source
        entry["covered"] = covered
        entry["uncovered_because"] = because
        # 位置表（`BC-18`）单独一个标志：指令流对得上不代表位置也对得上
        positions_ok = (
            covered
            and "位置表未对齐" not in because
            and "行号级未对齐" not in because
        )
        positions_reason = (
            because
            if ("位置表未对齐" in because or "行号级未对齐" in because)
            else ""
        )
        entry["positions_covered"] = positions_ok
        entry["positions_uncovered_because"] = positions_reason
        # `MS-17`：行号级与列跨度**分开**——列跨度可以"未覆盖（写明理由）"，行号不行
        lines_ok = "行号级未对齐" not in because
        # **行号级的能力缺口已经不存在**：`BC-4` 扩之后本层的行表**也能表达「缺失」**
        # （`co_lines()` 第三项可以是 `None`，见 `CodeObject::co_lines`）⇒ 合成指令那点
        # 不再构成"不可比"。此前那段"参照给 `None`、本层给整数 ⇒ 行号级不可比"的理由**作废并删除**
        # （`MS-19`：能力缺口不得登记为差异；用户 5102c24 的裁定）。
        # 仍然为真的只剩"参照的粘性 loc 还没推出来"那几条，它们由理由前缀 `行号级未对齐` 显式标出。
        entry["lines_covered"] = lines_ok
        entry["lines_uncovered_because"] = "" if lines_ok else because
        # **嵌套单元也要打同一个标志**：位置表对不上往往就出在嵌套单元里
        # （例如类体带 `def` 时，参照给合成指令 `MAKE_CELL` 的位置是 `(None, None, None, None)`）
        def mark_nested(node):
            for inner in node.get("nested", []):
                inner["positions_covered"] = positions_ok
                inner["positions_uncovered_because"] = positions_reason
                inner["lines_covered"] = lines_ok
                inner["lines_uncovered_because"] = "" if lines_ok else because
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
