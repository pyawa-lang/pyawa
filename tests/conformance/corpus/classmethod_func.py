# **`classmethod.__func__`**（第 346 轮）：上限榜上 `AttributeError: 'classmethod' object has no
# attribute '__func__'` × 39 个模块（`Lib/` 里到处是 `__func__` 的用法）。


class C:
    @classmethod
    def make(cls):
        return cls.__name__

    @staticmethod
    def helper():
        return "h"


print(str(C.make()))
print(str(C.__dict__["make"].__func__.__name__))
print(str(type(C.__dict__["make"]).__name__))
print(str(C.helper()))
print(str(C.__dict__["helper"].__func__.__name__))
