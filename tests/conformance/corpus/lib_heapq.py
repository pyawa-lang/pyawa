# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/heapq.py`（堆操作 ＋ `nlargest`／`nsmallest`）。
import heapq

data = [5, 1, 3]
heapq.heapify(data)
smallest = str(heapq.nsmallest(2, [5, 1, 3, 2]))
largest = str(heapq.nlargest(2, [5, 1, 3, 2]))
heapq.heappush(data, 0)
popped = heapq.heappop(data)
after = str(data)
