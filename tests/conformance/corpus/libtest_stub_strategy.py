# `MS-13` ② 的 `Lib/test/` 语料：`StubClass` 正文**逐字**取自
# `Lib/test/support/_hypothesis_stubs/_helpers.py`（第 2–25 行）；
# `StubStrategy` 正文**逐字**取自 `Lib/test/support/_hypothesis_stubs/strategies.py`
# （第 6–22 行）。下面几行调用是脚手架。
# 注：`a | a`（自定义 `__or__`）本层尚未接线 ⇒ 不调用它（未实现不是差异，`MS-19`）。
class StubClass:
    def __init__(self, *args, **kwargs):
        self.__stub_args = args
        self.__stub_kwargs = kwargs
        self.__repr = None

    def _with_repr(self, new_repr):
        new_obj = self.__class__(*self.__stub_args, **self.__stub_kwargs)
        new_obj.__repr = new_repr
        return new_obj

    def __repr__(self):
        if self.__repr is not None:
            return self.__repr

        argstr = ", ".join(self.__stub_args)
        kwargstr = ", ".join(f"{kw}={val}" for kw, val in self.__stub_kwargs.items())

        in_parens = argstr
        if kwargstr:
            in_parens += ", " + kwargstr

        return f"{self.__class__.__qualname__}({in_parens})"


class StubStrategy(StubClass):
    def __make_trailing_repr(self, transformation_name, func):
        func_name = func.__name__ or repr(func)
        return f"{self!r}.{transformation_name}({func_name})"

    def map(self, pack):
        return self._with_repr(self.__make_trailing_repr("map", pack))

    def flatmap(self, expand):
        return self._with_repr(self.__make_trailing_repr("flatmap", expand))

    def filter(self, condition):
        return self._with_repr(self.__make_trailing_repr("filter", condition))

    def __or__(self, other):
        new_repr = f"one_of({self!r}, {other!r})"
        return self._with_repr(new_repr)


def pack(x):
    return x


a = StubStrategy()
b = a.map(pack)
first = str(a)
second = str(b)
