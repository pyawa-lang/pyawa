# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/contextlib.py`（`suppress` 的吞噬语义）。
import contextlib

log = []
with contextlib.suppress(ValueError):
    raise ValueError("boom")
with contextlib.suppress(ValueError):
    log.append("no raise")
n = len(log)
text = str(log)
