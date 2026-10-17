# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/reprlib.py`（`Repr` 的截断规则）。
import reprlib

short = reprlib.repr([1, 2, 3])
long = reprlib.repr(list(range(1000)))
maxlist = reprlib.aRepr.maxlist
