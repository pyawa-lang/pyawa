#!/usr/bin/env python3
"""从参照实现**探测**切片语义，导出 `crates/pyawa-core/tests/fixture-slice-3.14.json`。

`P1-12` 点名的"索引／切片"要 `slice` 类型（`TS-42` 把它排 M3+）；切片语义**跨类型共用**
（`bytes`／`list`／`tuple`／`str` 同一套 `slice.indices()` 规则）⇒ 夹具放一处，
别在 `gen_bytes_fixture.py` 里再抄一份（`AGENTS.md` 的"一处真相"）。

探针全部在**本机参照**上真跑；跑不动就硬失败。

用法::

    python3 tools/gen_slice_fixture.py            # 只打印摘要
    python3 tools/gen_slice_fixture.py --emit     # 写出夹具
"""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/pyawa-core/tests/fixture-slice-3.14.json"

#: 四个序列的**同一个**内容：`bytes` 用十六进制，`str` 用文本，`list`／`tuple` 用整数表。
BYTES_VALUE = b"abcde"
STR_VALUE = "abcde"
ITEMS = [10, 20, 30, 40, 50]

#: 切片矩阵：`(start, stop, step)`，`None` 就是省略那一格。
SLICES: list[tuple[int | None, int | None, int | None]] = [
    (None, None, None),
    (1, None, None),
    (None, 2, None),
    (1, 3, None),
    (None, None, 2),
    (None, None, -1),
    (-2, None, None),
    (None, -1, None),
    (5, None, None),
    (-5, None, None),
    (1, 1, None),
    (2, 0, None),
    (None, None, 3),
    (None, None, -2),
    (1, None, -1),
    (4, 1, -2),
]

#: `slice(...)` 自己的 `repr`（`slice` 是个对象，形状也要对）。
SLICE_REPRS: list[tuple[int | None, int | None, int | None]] = [
    (None, None, None),
    (2, None, None),
    (1, 2, 3),
    (None, 5, None),
    (-1, None, None),
]

PROBE = r'''
import json, sys

from fixture_guard import unchanged

receiver = sys.argv[1]
bytes_value = bytes.fromhex(receiver)
str_value = bytes_value.decode("ascii")
items = json.loads(sys.argv[2])
slices = json.loads(sys.argv[3])
slice_reprs = json.loads(sys.argv[4])

def index_tuple(triple):
    return tuple(triple)

def result_of(kind, piece):
    if kind == "bytes":
        return {"hex": piece.hex()}
    if kind == "str":
        return {"text": piece}
    if kind == "list":
        return {"items": list(piece)}
    return {"items": list(piece)}

rows = []
# **可执行守卫**（第 216 轮）：整段探测前后，三个接收者的快照必须相同。
# 这条断言取代了原先只写在注释里的约定——写入类探测一律用**新**接收者，否则这里直接红。
with unchanged("slice 的接收者", bytes_value, str_value, items):
    for start, stop, step in slices:
        key = slice(start, stop, step)
        row = {"start": start, "stop": stop, "step": step, "results": {}}
        row["results"]["bytes"] = result_of("bytes", bytes_value[key])
        row["results"]["str"] = result_of("str", str_value[key])
        row["results"]["list"] = result_of("list", items[key])
        row["results"]["tuple"] = result_of("tuple", tuple(items)[key])
        rows.append(row)

reprints = [
    {"start": start, "stop": stop, "step": step, "repr": repr(slice(start, stop, step))}
    for start, stop, step in slice_reprs
]

# 越界／类型错误各记一条（`indices` 那条消息也要照实测）
def error_of(operation):
    try:
        operation()
        return None
    except Exception as error:
        return f"{type(error).__name__}: {error}"

# 第 213 轮修过一次真事：先跑 `__setitem__` 再取 `value`，记录下来的期望值其实是被改过的状态。
# 现在由上面的 `unchanged(...)` 守卫**强制**这件事（写在注释里的约定撑不住，断言才撑得住）。
errors = {
    "non_int_index": error_of(lambda: bytes_value["a"]),
    "slice_non_int": error_of(lambda: bytes_value[slice("a")]),
    "step_zero": error_of(lambda: bytes_value[::0]),
}

print(json.dumps({
    "value": {"bytes_hex": receiver, "str": str_value, "items": items},
    "rows": rows,
    "slice_reprs": reprints,
    "errors": errors,
}, ensure_ascii=False))
'''


def probe_environment() -> dict[str, str]:
    """子进程探针的环境：把 `tools/` 放进 `PYTHONPATH`（探针要 `import fixture_guard`）。"""
    tools_dir = str(pathlib.Path(__file__).resolve().parent)
    existing = os.environ.get("PYTHONPATH", "")
    joined = tools_dir + (os.pathsep + existing if existing else "")
    return dict(os.environ, PYTHONPATH=joined)


def main() -> int:
    emit = "--emit" in sys.argv
    argv = [
        BYTES_VALUE.hex(),
        json.dumps(ITEMS),
        json.dumps([list(triple) for triple in SLICES]),
        json.dumps([list(triple) for triple in SLICE_REPRS]),
    ]
    completed = subprocess.run(
        [sys.executable, "-c", PROBE, *argv],
        capture_output=True,
        check=False,
        env=probe_environment(),
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr.decode())
        return completed.returncode
    data = json.loads(completed.stdout.decode())
    if emit:
        FIXTURE.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n")
    print(
        f"切片 {len(data['rows'])} 条（bytes／str／list／tuple × 每种切法）· "
        f"slice repr {len(data['slice_reprs'])} 条 · 错误 {len(data['errors'])} 条"
    )
    if emit:
        print(f"导出 → {FIXTURE.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
