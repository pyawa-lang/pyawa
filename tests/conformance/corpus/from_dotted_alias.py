# `from a.b import x` 要先看 sys.modules（第 203 轮真 bug 修复）
# （`Lib/os.py:103` 的 `sys.modules['os.path'] = path` 靠它 ✓）
import sys
import itertools

sys.modules["demo.sub"] = itertools
from demo.sub import count

print(str(count is itertools.count))
