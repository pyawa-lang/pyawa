# **`__prepare__` 那一格**（第 298 轮接线）：元类自带 `__prepare__` 时，参照在建类**之前**先调它
# `M.__prepare__(name, bases, **kwds)`，并用它返回的**映射**当类命名空间。
# `Lib/enum.py` 的 `EnumType.__prepare__` 返回 `EnumDict`（`dict` 子类），
# `EnumType.__new__` 头一句 `classdict._member_names` 全指望它（那一族 42 个模块）。


class Meta(type):
    @classmethod
    def __prepare__(mcls, name, bases, **kwds):
        print("prepare " + name)
        return {"prepared": True}


class C(metaclass=Meta):
    x = 1


print(str(C.prepared))
print(str(C.x))


class D(C):
    y = 2


print(str(D.x))
print(str(D.y))
