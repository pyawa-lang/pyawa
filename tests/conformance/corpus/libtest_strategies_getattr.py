# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/_hypothesis_stubs/_helpers.py 第 2–43 行
#   Lib/test/support/_hypothesis_stubs/strategies.py 第 6–22、25–70、83–91 行
# 其余为脚手架。
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
_STRATEGIES = {
    "binary",
    "booleans",
    "builds",
    "characters",
    "complex_numbers",
    "composite",
    "data",
    "dates",
    "datetimes",
    "decimals",
    "deferred",
    "dictionaries",
    "emails",
    "fixed_dictionaries",
    "floats",
    "fractions",
    "from_regex",
    "from_type",
    "frozensets",
    "functions",
    "integers",
    "iterables",
    "just",
    "lists",
    "none",
    "nothing",
    "one_of",
    "permutations",
    "random_module",
    "randoms",
    "recursive",
    "register_type_strategy",
    "runner",
    "sampled_from",
    "sets",
    "shared",
    "slices",
    "timedeltas",
    "times",
    "text",
    "tuples",
    "uuids",
}

__all__ = sorted(_STRATEGIES)
def __getattr__(name):
    if name not in _STRATEGIES:
        raise AttributeError(f"Unknown attribute {name}")

    return stub_factory(StubStrategy, f"hypothesis.strategies.{name}")


def __dir__():
    return __all__

import functools

strategy = __getattr__("integers")
dir_names = str(__dir__())
all_count = len(__all__)
is_strategy = str(hasattr(strategy, 'map'))
