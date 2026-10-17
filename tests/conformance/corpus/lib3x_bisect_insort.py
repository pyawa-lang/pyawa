# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/bisect.py`。
import bisect

data = [1, 3, 5]
bisect.insort_left(data, 3)
bisect.insort_right(data, 3)
text = str(data)
left = bisect.bisect_left([1, 2, 2, 3], 2)
right = bisect.bisect_right([1, 2, 2, 3], 2)
