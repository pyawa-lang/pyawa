# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/operator.py`（纯 Python 回退实现）。
# 注：`operator.attrgetter("real")(3)` 本层尚未接线（`'real' 取不到属性`）⇒ 不进语料
#（未实现不是差异，`MS-19`）。
import operator

summed = operator.add(2, 3)
first = operator.itemgetter(1)(["a", "b", "c"])
truth = operator.truth([]) or operator.truth([1])
