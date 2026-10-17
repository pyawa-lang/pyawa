# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/_hypothesis_stubs/__init__.py 第 81–83 行
def settings(*args, **kwargs):
    return lambda f: f  # pragma: nocover


def plain(x):
    return x

wrapped = settings(max_examples=3)(plain)
same = wrapped is plain
