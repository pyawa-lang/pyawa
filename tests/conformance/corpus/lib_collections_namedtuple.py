# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/collections/__init__.py`（`namedtuple` 生成器）。
import collections

Point = collections.namedtuple("Point", "x y")
p = Point(1, 2)
text = str(p)
x = p.x
