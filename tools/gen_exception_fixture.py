#!/usr/bin/env python3
"""从参照实现**探测**异常的可观察行为，导出
`crates/pyawa-core/tests/fixture-exceptions-3.14.json`。

`T-BC-22` 要求异常派发的对拍包含"`sys.exc_info()`／`__context__`／`__cause__`／`__traceback__`
的**可观察行为**"（`BC-60`）。编码侧已由 `tools/gen_argval_fixture.py` 的异常族语料覆盖；
本脚本管**行为侧**：把每个情形在参照实现里跑一遍，原样记录**可观察值**（`args`／`str`／`repr`／
链的类名／`__suppress_context__`）——**禁止**手写期望值。

`sys.exc_info()` 本身要等 `sys` 模块（`P3-14`）；本脚本记录的是**异常对象上**能看到的那些，
它们与 `sys.exc_info()` 同源（`BC-60` 要求异常状态按实例存放）。

用法::

    python3 tools/gen_exception_fixture.py
"""

from __future__ import annotations

import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-exceptions-3.14.json"

#: 地址归一（`repr` 里可能出现 `<object at 0x…>` 一类）。
ADDRESS = re.compile(r"0x[0-9a-f]+")


def normalize(text: str) -> str:
    return ADDRESS.sub("0x…", text)


def describe(error: BaseException) -> dict[str, object]:
    """把一个异常实例的**可观察值**写成可机械比对的结构。"""
    return {
        "type": type(error).__name__,
        "args": [normalize(repr(argument)) for argument in error.args],
        "str": normalize(str(error)),
        "repr": normalize(repr(error)),
        "cause": None if error.__cause__ is None else type(error.__cause__).__name__,
        "context": None if error.__context__ is None else type(error.__context__).__name__,
        "suppress_context": bool(error.__suppress_context__),
    }


def raise_plain() -> BaseException:
    raise ValueError("x")


def raise_two_args() -> BaseException:
    raise ValueError("a", "b")


def raise_no_args() -> BaseException:
    raise ValueError


def raise_int_arg() -> BaseException:
    raise ValueError(7)


def raise_explicit_cause() -> BaseException:
    try:
        raise KeyError("k")
    except KeyError as inner:
        raise ValueError("v") from inner


def raise_from_none() -> BaseException:
    try:
        raise KeyError("k")
    except KeyError:
        raise ValueError("v") from None


def raise_implicit() -> BaseException:
    try:
        raise KeyError("k")
    except KeyError:
        raise ValueError("v")


def raise_reraise_same() -> BaseException:
    try:
        raise KeyError("k")
    except KeyError:
        raise


def raise_inside_finally() -> BaseException:
    try:
        raise KeyError("k")
    finally:
        # `finally` 里抛出的新异常：`__context__` 指向正在处理的那个（实测）
        raise ValueError("v")


#: 情形名 → 触发函数。名字要与 `crates/pyawa-core/tests/exceptions.rs` 里手搭的等价程序一一对应。
CASES: dict[str, object] = {
    "plain": raise_plain,
    "two_args": raise_two_args,
    "no_args": raise_no_args,
    "int_arg": raise_int_arg,
    "explicit_cause": raise_explicit_cause,
    "from_none": raise_from_none,
    "implicit": raise_implicit,
    "reraise_same": raise_reraise_same,
    "inside_finally": raise_inside_finally,
}


def main() -> int:
    recorded: dict[str, dict[str, object]] = {}
    for name, trigger in CASES.items():
        try:
            trigger()  # type: ignore[operator]
        except BaseException as error:  # noqa: BLE001 —— 对拍就是要那个异常对象
            recorded[name] = describe(error)
        else:  # pragma: no cover - 语料自己保证会抛
            raise SystemExit(f"{name}：语料没有抛异常")
    OUTPUT.write_text(
        json.dumps(
            {
                "_note": "由 tools/gen_exception_fixture.py 从本机 CPython 导出；禁止手改。",
                "reference": f"{__import__('sys').version_info.major}.{__import__('sys').version_info.minor}.{__import__('sys').version_info.micro}",
                "cases": recorded,
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"已写入 {OUTPUT.relative_to(ROOT)}")
    print(f"基线 CPython {__import__('sys').version.split()[0]}｜情形 {len(recorded)} 个")
    for name, data in sorted(recorded.items()):
        print(
            f"  {name:16} type={data['type']:14} args={data['args']} "
            f"cause={data['cause']} context={data['context']} suppress={data['suppress_context']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
