# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/functools.py`（`reduce` ＋ `partial`）。
import functools

total = functools.reduce(lambda a, b: a + b, [1, 2, 3, 4])
add = functools.partial(lambda a, b: a + b, 10)
partial_5 = add(5)
