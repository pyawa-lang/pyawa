#!/usr/bin/env python3
"""从参照实现**探测**参数绑定的错误消息，导出
`crates/pyawa-core/tests/fixture-argbind-3.14.json`。

`BC-56`：绑定失败（多余位置实参／缺必填／同一参数重复给／未知关键字）**必须**报 `TypeError`，
且**消息必须与参照实现一致**——"消息文本必须由探测参照实现为准，**禁止手写近似文本**"。
`T-BC-18` 是它的验收。本脚本把 `BC-56` 的四类 ＋ 仅关键字／仅位置／`*args`／`**kwargs`
几种形态的消息原样导出。

语料里函数的**名字固定为 `demo`**（消息里带名字），参数名也只从下表取——这样 Rust 侧能
用手搭的等价签名一一对应（`crates/pyawa-core/tests/argbind.rs`）。

用法::

    python3 tools/gen_argbind_fixture.py
"""

from __future__ import annotations

import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-argbind-3.14.json"

ADDRESS = re.compile(r"0x[0-9a-f]+")


def normalize(text: str) -> str:
    return ADDRESS.sub("0x…", text)


#: 语料：用例名 → 一个**签名**（用 `exec` 定义，名字固定 `demo`）＋ 一次调用。
#:
#: `signature` 里的形参名与 `call` 的写法都是**规格的一部分**：Rust 侧按同一份描述手搭。
CASES: dict[str, dict[str, str]] = {
    # `BC-56` 四类
    "missing_two": {
        "signature": "def demo(a, b): pass",
        "call": "demo()",
    },
    "missing_one": {
        "signature": "def demo(a, b): pass",
        "call": "demo(1)",
    },
    "too_many": {
        "signature": "def demo(a, b): pass",
        "call": "demo(1, 2, 3)",
    },
    "too_many_with_default": {
        "signature": "def demo(a, b=1, *, c=2): pass",
        "call": "demo(1, 2, 3)",
    },
    "duplicate": {
        "signature": "def demo(a, b): pass",
        "call": "demo(1, a=2)",
    },
    "unexpected_keyword": {
        "signature": "def demo(a, b): pass",
        "call": "demo(1, 2, x=3)",
    },
    # 仅关键字／仅位置
    "missing_keyword_only": {
        "signature": "def demo(a, b, *args, c, **kwargs): pass",
        "call": "demo(1, 2)",
    },
    "posonly_as_keyword": {
        "signature": "def demo(a, /, b, *, c): pass",
        "call": "demo(a=1, b=2, c=3)",
    },
    # 收得住的情形（对照：不该报错）
    "varargs_ok": {
        "signature": "def demo(*args, **kwargs): pass",
        "call": "demo(1, x=2)",
    },
    "full_ok": {
        "signature": "def demo(a, b, *args, c, **kwargs): pass",
        "call": "demo(1, 2, 3, 4, c=5, d=6)",
    },
    "none_positional": {
        "signature": "def demo(**kwargs): pass",
        "call": "demo(1)",
    },
}


def main() -> int:
    recorded: dict[str, dict[str, object]] = {}
    for name, case in CASES.items():
        namespace: dict[str, object] = {}
        exec(case["signature"], namespace)  # noqa: S102 —— 语料是我们自己写的
        callable_ = namespace["demo"]
        try:
            eval(case["call"], {"demo": callable_})  # noqa: S307 —— 同上
            message = None
        except TypeError as error:
            message = normalize(str(error))
        recorded[name] = {
            "signature": case["signature"],
            "call": case["call"],
            "message": message,
        }
    OUTPUT.write_text(
        json.dumps(
            {
                "_note": "由 tools/gen_argbind_fixture.py 从本机 CPython 导出；禁止手改。",
                "reference": f"{__import__('sys').version.split()[0]}",
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
    for name, data in sorted(recorded.items()):
        print(f"  {name:22} {data['message']!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
