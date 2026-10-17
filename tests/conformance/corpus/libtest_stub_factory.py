# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/_hypothesis_stubs/_helpers.py 第 27–43 行
# 探针只比工厂缓存与基类初始化（`Stub.__name__` 赋值属另一条挂账）。
# 其余为脚手架。
def stub_factory(klass, name, *, with_repr=None, _seen={}):
    if (klass, name) not in _seen:

        class Stub(klass):
            def __init__(self, *args, **kwargs):
                super().__init__()
                self.__stub_args = args
                self.__stub_kwargs = kwargs

        Stub.__name__ = name
        Stub.__qualname__ = name
        if with_repr is not None:
            Stub._repr = None

        _seen.setdefault((klass, name, with_repr), Stub)

    return _seen[(klass, name, with_repr)]

class Base:
    def __init__(self):
        self.base = 7


Stub = stub_factory(Base, "StubName")
instance = Stub(1, key=2)
same = stub_factory(Base, "StubName") is Stub
base = instance.base
