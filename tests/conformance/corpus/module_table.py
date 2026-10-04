# 模块表（第 200 轮回归）：`sys.modules` 与内建模块的**同一性**
# 守的是 set_modules 的 retain 修复 —— 先前少一份引用 ⇒ GC 会把整张模块表当垃圾回收 ⇒ 崩
import sys
import _imp
import _io
import _warnings

assert sys.modules["_imp"] is _imp
assert sys.modules["_io"] is _io
assert sys.modules["_warnings"] is _warnings
assert _imp.__name__ == "_imp"
assert _io.__name__ == "_io"
assert _warnings.__name__ == "_warnings"

# 再导入一次必须是同一对象（模块表命中）
import _io as second

assert second is _io

# 大量分配，逼 GC 跑起来（模块表若被误回收，这里就会露出破绽）
junk = []
for i in range(4000):
    junk.append({"i": i, "s": str(i)})
assert len(junk) == 4000
assert junk[3999]["i"] == 3999

del junk
print("ok")
