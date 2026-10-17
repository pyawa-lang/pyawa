# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/collections/__init__.py`（`defaultdict.__missing__`）。
# 注：`defaultdict.items()`／`len()` 本层尚未接线 ⇒ 不收它们（未实现不是差异，`MS-19`）。
import collections

d = collections.defaultdict(list)
d["a"].append(1)
d["a"].append(2)
d["b"].append(3)
first = str(d["a"])
