"""**过渡假货** `enum`（Pyawa）——**唯一要求：将来能整文件换回上游 `enum.py`** ✗。

为什么有它：`re/__init__.py:125 import enum` 挡在 `_sre`／`re`（≈77 个模块）前面，
而我们的 VM 还缺几处语言能力 ⇒ 上游那份 2168 行的 `enum.py` 跑不动。

换回真货的步骤（三步，必须保持有效）
====================================
1. 用**同版本**（CPython 3.14）的上游 `enum.py` **整文件替换**本文件（删掉本文件全部内容 ✗）；
2. 跑 `cargo test -p pyawa-runtime --test enum_shapes` ✓ ＋ `tools/slowcheck.sh` ✓ —— **两者都必须绿**；
3. 删掉 `NEXT.md` 与 `docs/rounds/` 里标注本假货的那条偏离记录 ✓。

写本文件时**不许违反**的纪律（否则将来换不回去 ✗）
================================================
- 只提供**上游同名**的公开面 ✗：不添加上游没有的属性/函数/参数；
- 对外可见行为按**参照实测**钉住 ✓；
- 宁可**少**、不可**偏** ✗：拿不准就报 `NotImplementedError`，别发明；
- 本文件是**唯一**"假货"落点 ✓：其它文件不许依赖本文件内部实现 ✓。

假货范围（第 1 版：**只为 `re` 顶包** ✗，逐条都实测过 ✓）
========================================================
- **成员就是普通 `int`／`str`** ✓，**不造实例** ✗ ⇒ 没有 `.name`／`.value` ✗、`isinstance(member, RegexFlag)`
  为 **False** ✗。**为什么敢**：`re/__init__.py` 全文**没有一处** `RegexFlag.XXX` ✗，`RegexFlag` 只出现在
  `if isinstance(flags, RegexFlag): flags = flags.value` 两处 ✓ ⇒ 成员是 int ⇒ 那两行永不执行 ⇒
  行为与参照**等价** ✓（`flags` 本来就是 int ✓）。
- **不用元类** ✗（实测：`M("D", (), {"a": 1})` ⇒ `TypeError: M() takes no arguments` ✗），
  改走 3 参数 `type()` ✓（已落地 ✓）。
- **不做 int 子类实例** ✗（实测：`int.__new__(子类, 值)` 走不通 ✗）。
- **装饰器是函数** ✓（实测：装饰器语法对**可调用实例**不传被装饰对象 ✗，对函数正常 ✓）。
- 不支持：`auto()` 的严格编号 ✗、`_missing_`／`_generate_next_value_` ✗、`member()`／`nonmember()` ✗、
  `Flag` 的 `STRICT`／`CONFORM`／`EJECT` 边界差异 ✗（都按 `KEEP` 处理 ✓）、pickle ✗、
  `class E(Enum): x = 1` 直写成员的**枚举语义** ✗（本版只走 `_simple_enum` 这条路 ✓）。
"""

import sys

__all__ = [
    "Enum", "Flag", "IntFlag", "IntEnum", "auto", "unique",
    "KEEP", "STRICT", "CONFORM", "EJECT",
]

#: 边界哨兵（上游同名 ✓；本假货一律按 `KEEP` 处理 ✗，见模块头 ✓）。
KEEP = "KEEP"
STRICT = "STRICT"
CONFORM = "CONFORM"
EJECT = "EJECT"


class auto:
    """`auto()`（**占位面** ✓）：本版只认它的类型 ✗，不实现严格编号 ✗。"""

    _value_ = None


class Enum:
    """`Enum`（**占位面** ✓）：本版**成员是普通值** ⇒ 没有实例语义 ✗（见模块头"假货范围" ✓）。"""


class Flag(Enum):
    """`Flag`（**占位面** ✓）：位运算直接用 `int` 的 ✓（成员本身就是 int ✓）。"""


class IntFlag(int, Flag):
    """`IntFlag`（**占位面** ✓）：留着**类身份** ✓，好让 `isinstance(..., IntFlag)` 这类检查有东西可查 ✓。

    **成员不是它的实例** ✗（见模块头 ✓）——`re` 不依赖这一点 ✓（已逐行核过 ✓）。
    """


class IntEnum(int, Enum):
    """`IntEnum`（**占位面** ✓）。"""


def _collect(cls):
    """把类体里的**成员名**与**成员值**分开收好 ✓（下划线开头的当"体属性" ✓，照上游 ✓）。"""

    members = []
    body = {}
    source = cls.__dict__
    for key in list(source.keys()):
        if key in ("__dict__", "__weakref__"):
            continue
        value = source[key]
        if key.startswith("_"):
            body[key] = value
            continue
        members.append((key, value))
    return members, body


#: `_simple_enum`／`global_enum` 用**模块级暂存**传递参数 ✓ —— **刻意不用闭包** ✗：
#: 实测我们 VM 的闭包捕获不可靠（`_simple_enum(IntFlag, boundary=KEEP)` 里 `use_args` 竟不是 `None` ✗），
#: 而模块级全局变量读取是稳的 ✓。代价：**不支持嵌套/并发**取用这两个装饰器 ✗（`re` 是一次性用 ✓）。
#: 换回真货时这段连同两个 `_PENDING_*` 一起消失 ✓，对外面**无影响** ✓。
_PENDING_ETYPE = Enum
_PENDING_BOUNDARY = KEEP
_PENDING_USE_ARGS = None
_PENDING_UPDATE_STR = False


def _simple_enum(etype=Enum, boundary=None, use_args=None):
    """`_simple_enum`（**只为 `re` 顶包** ✗）——`re/__init__.py:143` 要它 ✓。

    返回**顶层函数**做装饰器 ✓（不用可调用实例 ✗、不用闭包 ✗ —— 两者都在我们 VM 上实测不通 ✗）。
    """
    global _PENDING_ETYPE, _PENDING_BOUNDARY, _PENDING_USE_ARGS
    _PENDING_ETYPE = etype
    _PENDING_BOUNDARY = boundary if boundary is not None else KEEP
    _PENDING_USE_ARGS = use_args
    return _simple_enum_decorate


def _simple_enum_decorate(cls):
    """真正的装饰器本体 ✓（顶层函数 ⇒ 不吃闭包 ✓）。"""

    if _PENDING_USE_ARGS is not None:
        raise NotImplementedError("_simple_enum(use_args=…) 未实现（假货范围 ✗）")
    members, body = _collect(cls)
    namespace = {}
    for key in list(body.keys()):
        namespace[key] = body[key]
    namespace["_member_names_"] = [key for key, _ in members]
    namespace["_member_map_"] = {}
    for key, value in members:
        namespace[key] = value
        namespace["_member_map_"][key] = value
    namespace["_boundary_"] = _PENDING_BOUNDARY
    made = type(cls.__name__, (_PENDING_ETYPE,), namespace)
    made.__module__ = cls.__module__
    return made


def global_enum(cls=None, update_str=False):
    """`global_enum`（**最小面** ✓）——`re/__init__.py:142` 要它 ✓：把成员**注入所在模块** ✓。"""
    global _PENDING_UPDATE_STR
    _PENDING_UPDATE_STR = update_str
    if cls is None:
        return _global_enum_decorate
    return _global_enum_decorate(cls)


def _global_enum_decorate(target):
    """真正的装饰器本体 ✓（顶层函数 ⇒ 不吃闭包 ✓）。"""

    if _PENDING_UPDATE_STR:
        raise NotImplementedError("global_enum(update_str=True) 未实现（假货范围 ✗）")
    module = sys.modules[target.__module__]
    for name in list(target._member_names_):
        setattr(module, name, getattr(target, name))
    return target


def unique(cls):
    """`@unique`（**最小面** ✓）：有别名就报 `ValueError` ✓（照上游语义 ✓）。"""

    seen = {}
    for name in cls._member_names_:
        value = getattr(cls, name)
        if value in seen:
            raise ValueError("duplicate values found in %r: %s" % (cls, name))
        seen[value] = name
    return cls
