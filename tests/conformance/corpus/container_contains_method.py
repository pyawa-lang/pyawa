# **容器的 `__contains__` 是属性**（第 303 轮修 `P3-25`）：本层的 `in` 是**指令内联**的，
# 而 `x.__contains__(y)` 这种**取属性**的路先前只有 `bytes` 接了一个 ⇒ 上限榜上那一族
# `'frozenset' object has no attribute '__contains__'`（12 个模块）。现在容器通用，且与
# `in` **同一处实现**（`bytes` 也换过来了：`b"abc".__contains__(98)` 参照给 `True`）。

s = set([1, 2])
f = frozenset([1, 2])
d = {"a": 1}
items = [1, 2]
text = "abc"
data = b"abc"

print(str(s.__contains__(1)))
print(str(s.__contains__(9)))
print(str(f.__contains__(1)))
print(str(f.__contains__(3)))
print(str(d.__contains__("a")))
print(str(d.__contains__("b")))
print(str(items.__contains__(2)))
print(str(items.__contains__(9)))
print(str(text.__contains__("bc")))
print(str(text.__contains__("z")))
print(str(data.__contains__(98)))
print(str(data.__contains__(122)))
print(str("bc" in text))
print(str(1 in f))
