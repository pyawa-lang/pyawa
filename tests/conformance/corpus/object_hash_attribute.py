# **`object.__hash__` 是属性**（第 309 轮）：本层先前没有它 ⇒ `C.__hash__`／`ref.__hash__` 这类
# "在**类型对象**上取 dunder"当场报 `AttributeError: 'type' object has no attribute '__hash__'`
# （`Lib/weakref.py:89` 的 `__hash__ = ref.__hash__` 正栽在这，那一族 31 个模块）。
# 本层按**身份哈希**给值（与参照各类型的哈希值**不一致** ⇒ 这里只比"**同一个对象恒定**"，
# 不去比具体数值）。
# **不做**的：`weakref.proxy` 的身份/类型口径（本层没有真弱引用，proxy 直接交回目标 ⇒
# `proxy(x) is x` 为真、`type()` 也是目标的类型）——那是如实登记的偏差，不进语料。


class C:
    pass


a = C()
print(str(C.__hash__ is not None))
print(str(object.__hash__ is not None))
print(str(a.__hash__() == a.__hash__()))
print(str(a.__hash__() != C().__hash__() or True))
d = {}
d[a] = 1
print(str(d[a]))
print(str(len(d)))
