# `MS-13` ② 的 `Lib/test/` 语料：`StubClass` 正文**逐字**取自
# `Lib/test/support/_hypothesis_stubs/_helpers.py`（第 2–25 行）。
# 下面两行调用是语料自己的脚手架（不改正文）。
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


a = StubClass("x", "y")

b = a._with_repr("STUB!")
