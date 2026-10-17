# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/bisect.py`（纯 Python 回退实现；不带 import）。
import bisect

data = [1, 3, 3, 5, 7]
left = bisect.bisect_left(data, 3)
right = bisect.bisect_right(data, 3)
bisect.insort(data, 4)
after = str(data)
