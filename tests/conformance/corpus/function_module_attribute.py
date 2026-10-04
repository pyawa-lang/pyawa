# **函数的 `__module__`／`__class__`**（第 312 轮）：参照在**定义时**把 `__module__` 写死成
# 当时那个模块的 `__name__`；本层从函数的 `__globals__` 里取同名的那一个（结果一致）。
# 动因：上限榜上 `object has no attribute '__module__'` × 67 个模块 ——
# `Lib/_collections_abc.py` 一族用 `getattr(x, "__module__")` 探 typing 别名。


def f():
    pass


class C:
    def m(self):
        pass


print(str(f.__module__))
print(str(getattr(f, "__module__")))
print(str(C.m.__module__))
print(str(C.__module__))
print(str(f.__class__ is type(f)))
print(str(getattr(f, "__module__") == "__main__"))
