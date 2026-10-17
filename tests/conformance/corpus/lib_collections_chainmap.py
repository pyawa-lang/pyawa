# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/collections/__init__.py`（`ChainMap` 的查找链）。
# 注：`ChainMap.keys()` 本层尚未接线（返回空）⇒ 不收它（未实现不是差异，`MS-19`）。
import collections

a = {"x": 1}
b = {"y": 2}
cm = collections.ChainMap(a, b)
x = cm["x"]
y = cm["y"]
