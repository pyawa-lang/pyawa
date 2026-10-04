# **类关键字转交元类**（第 292 轮，照参照）：`class C(Base, tag=...)` 的关键字除了 `metaclass=`
# 都**原样**转交给元类的 `__new__`／`__init__`（`type.__call__` 的次序）；没写 `metaclass=` 时
# 元类按基类里**最派生**的那个取（`Lib/enum.py:1400` 的 `class Flag(Enum, boundary=STRICT)` 靠它）。


class Meta(type):
    def __new__(mcls, name, bases, namespace, *, tag=None, **kwds):
        cls = super().__new__(mcls, name, bases, namespace)
        cls.tag = tag
        return cls

    def __init__(cls, name, bases, namespace, *, tag=None, **kwds):
        type.__init__(cls, name, bases, namespace)
        cls.inited = True


class A(metaclass=Meta, tag="hello"):
    pass


class B(A, tag="world"):
    pass


print(str(A.tag))
print(str(A.inited))
print(str(B.tag))
print(str(B.inited))
print(str(type(A) is Meta))
print(str(type(B) is Meta))


class Plain:
    pass


print(str(type(Plain) is type))
print(str(Plain.__name__))
