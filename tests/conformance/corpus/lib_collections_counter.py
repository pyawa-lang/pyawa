# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/collections/__init__.py`（`Counter` 的计数面）。
# 注：`Counter.most_common` 本层尚未接线（函数内 `import heapq` 后报 UnboundLocalError）⇒ 不收它
# （未实现不是差异，`MS-19`）。
import collections

counter = collections.Counter("abracadabra")
n = len(counter)
b = counter["b"]
total = sum(counter.values())
has = "z" in counter
