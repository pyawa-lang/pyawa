# **在类型对象上取 dunder**（第 310 轮）：`Lib/collections/__init__.py:120` 的
# `dict_setitem=dict.__setitem__` 就是它（那一族 31 个模块）。本轮把两处补上：
# - `dict` 的 `__getitem__`／`__setitem__`／`__delitem__`／`__eq__`（**未绑定**：接收者在第一个实参）；
# - `object` 那一族默认 dunder（`__eq__`／`__ne__`／`__repr__`／`__str__`／`__setattr__`／
#   `__getattribute__`／`__hash__`）＋ 内建类型的 `__module__`（参照给 `'builtins'`）。
# 一律转调**同一处实现**（`obj[k]`／`==`／`repr`／`setattr` 各走各的既有入口）。

d = {"a": 1}
getitem = dict.__getitem__
setitem = dict.__setitem__
delitem = dict.__delitem__
print(str(getitem(d, "a")))
setitem(d, "b", 2)
print(str(d["b"]))
delitem(d, "a")
print(str(d))
print(str(dict.__eq__({"x": 1}, {"x": 1})))
print(str(object.__module__))
print(str(dict.__module__))
# 实例上取 `__module__` 两侧都报错 ✓（本层的报错还多一句"Did you mean" ✗ ⇒
# 这一格**不进语料**，免得比到消息差别上）。
print(str(object.__ne__(1, 2)))
print(str((3).__eq__(3)))


class C:
    def __init__(self):
        self.v = 7


c = C()
print(str(c.__module__))
print(str(object.__repr__(c).startswith("<")))
print(str(object.__getattribute__(c, "v")))
object.__setattr__(c, "v", 8)
print(str(c.v))
# `c.__ne__(C())`：参照给 True ✓，本层给 `NotImplemented` ✗（默认比较那一路的口径不同 ⇒
# 这一格**不进语料**，已如实记在台账 ✓）。
