# 描述符类型（第 161 轮）：classmethod / staticmethod / property 作为类型对象
# （abc.py 的用法就是拿它们做基类：class abstractclassmethod(classmethod)）


def f():
    return 1


assert classmethod(f) is not None
assert staticmethod(f) is not None
assert property(f) is not None


class MyClassMethod(classmethod):
    pass


class MyStaticMethod(staticmethod):
    pass


class MyProperty(property):
    pass


assert MyClassMethod is not None
assert MyStaticMethod is not None
assert MyProperty is not None
assert issubclass(MyClassMethod, classmethod)
print("ok")
