# **`collections.deque`**（第 331 轮）：`Lib/collections/__init__.py` 的
# `from _collections import deque` 那一行要的名字 —— 也是上限榜上
# `ImportError: cannot import name 'deque' from 'collections'` × 18 个模块的卡点。
# 类型（载荷、方法面、repr、len）都在**核心**；`_collections` 只把它导出成 `deque` 这个名字。
# **如实登记的未接面**：迭代协议／下标／`__contains__`／`__eq__`／`reverse`（随后补）；
# `maxlen` 目前只认**位置**写法（`deque([1, 2], 2)` ✓），关键字写法 `maxlen=2` 还没接
# （那要给 `new` 槽接上 kwargs）⇒ 语料只用位置写法。

from collections import deque

d = deque([1, 2, 3])
print(repr(d))
print(str(len(d)))
d.append(4)
d.appendleft(0)
print(repr(d))
print(str(d.pop()))
print(str(d.popleft()))
print(repr(d))

e = deque([1, 2, 3], 2)
print(repr(e))
e.append(4)
print(repr(e))
print(str(e.maxlen))

f = deque()
f.extend([1, 2])
f.extendleft([3, 4])
print(repr(f))
print(str(f.count(1)))
f.remove(1)
print(repr(f))
print(str(f.index(2)))
g = f.copy()
print(repr(g))
f.clear()
print(repr(f))
print(repr(g))
h = deque([1, 2, 3, 4])
h.rotate(1)
print(repr(h))
h.rotate(-2)
print(repr(h))
i = deque()
i.insert(0, "a")
i.insert(0, "b")
print(repr(i))
