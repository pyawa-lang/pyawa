# 单行体（第 174 轮）：def/class 的同行语句
# 注：class C: ... 那种要 Ellipsis 词法（本层还没接 ✗），此处只守已通形状 ✓


def _f(): pass


def _g(): return 7


class C:
    def _m(self): pass

    def _n(self): return 3


assert _g() == 7
assert C()._n() == 3
assert _f() is None
print("ok")
