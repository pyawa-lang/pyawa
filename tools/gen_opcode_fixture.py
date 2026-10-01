#!/usr/bin/env python3
"""把本机 CPython 3.14 的 `_opcode`／`_opcode_metadata` 导成**期望值夹具**。

`BC-30` 的基线是"**实测的** 3.14"——**禁止**凭记忆或从 CPython 源码抄数值。本脚本只向
运行时提问，把答案落成 `crates/pyawa-core/tests/fixture-opcode-3.14.json`，供 Rust 侧对拍。

重生成::

    python3 tools/gen_opcode_fixture.py

与任务单原始草案的三处差异（都以本机 3.14.4 的实测为准，不是猜的）:

1. `opcode._inline_cache_entries` 是 **dict（键为指令名）**，不是按编号索引的 list；
2. `_specializations`／`_specialized_opmap` 在**参照实现里非空**（17／84 项）——那是 CPython 的
   特化，`BC-32` 要求 **Pyawa 侧为空**，故夹具里存的是"Pyawa 期望＝空"，oracle 的项数另存备查；
3. `get_nb_ops()` 实测 **27 项**（`NB_SUBSCR` 在最后），不是 26。
"""

from __future__ import annotations

import json
import pathlib
import sys

import _opcode
import opcode
from _opcode_metadata import (
    HAVE_ARGUMENT,
    MIN_INSTRUMENTED_OPCODE,
    _specializations,
    _specialized_opmap,
    opmap,
)

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/tests/fixture-opcode-3.14.json"

#: 采样面：**含二进制边界**的 oparg × 3 种 jump（`T-BC-11` 要求扩采样）。
SAMPLE = (0, 1, 2, 3, 7, 8, 15, 16, 255, 256, 65535, 65536)
JUMPS = (None, True, False)


def main() -> int:
    names = {value: name for name, value in opmap.items()}

    has: dict[str, list[str]] = {}
    for family in ("arg", "const", "name", "jump", "free", "local", "exc"):
        predicate = getattr(_opcode, f"has_{family}")
        has[family] = sorted(names[op] for op in opmap.values() if predicate(op))

    stack_effect: dict[str, dict[str, int | None]] = {}
    for name, op in sorted(opmap.items()):
        samples: dict[str, int | None] = {}
        for oparg in SAMPLE:
            for jump in JUMPS:
                key = f"{oparg}|{'null' if jump is None else str(jump).lower()}"
                try:
                    samples[key] = _opcode.stack_effect(op, oparg, jump=jump)
                except Exception:
                    # 该组合对这条指令无意义（或 oparg 越界）
                    samples[key] = None
        stack_effect[name] = samples

    fixture = {
        "_note": (
            "由 tools/gen_opcode_fixture.py 从本机 CPython 运行时导出；这些是**期望值**，不是实现。"
            " specializations_pyawa／specialized_opmap_pyawa 按 BC-32 必须为空；"
            "oracle_* 两项只记录参照实现的实际项数，供升版时对比。"
        ),
        "reference": {
            "implementation": "CPython",
            "version": sys.version.split()[0],
            "platform": sys.platform,
        },
        "opmap": opmap,
        "have_argument": HAVE_ARGUMENT,
        "min_instrumented_opcode": MIN_INSTRUMENTED_OPCODE,
        "inline_cache_entries": {
            name: entries for name, entries in opcode._inline_cache_entries.items()
        },
        "has": has,
        "nb_ops": [[name, symbol] for name, symbol in _opcode.get_nb_ops()],
        "intrinsic1": list(_opcode.get_intrinsic1_descs()),
        "intrinsic2": list(_opcode.get_intrinsic2_descs()),
        "special_methods": list(_opcode.get_special_method_names()),
        "specializations_pyawa": {},
        "specialized_opmap_pyawa": {},
        "oracle_specializations_count": len(_specializations),
        "oracle_specialized_opmap_count": len(_specialized_opmap),
        "stack_effect": stack_effect,
    }

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        json.dumps(fixture, indent=1, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"已写入 {OUTPUT.relative_to(ROOT)}")
    print(
        f"基线 CPython {fixture['reference']['version']}｜opmap {len(opmap)}｜"
        f"cache 非零 {sum(1 for v in opcode._inline_cache_entries.values() if v)}｜"
        f"nb_ops {len(fixture['nb_ops'])}｜stack_effect 采样 {len(stack_effect)} 条指令"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
