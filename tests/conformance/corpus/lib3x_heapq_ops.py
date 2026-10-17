# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/heapq.py`。
import heapq

data = [3, 1, 5]
heapq.heapify(data)
pushed = str(heapq.heappushpop(data, 0))
replaced = str(heapq.heapreplace(data, 9))
after = str(data)
