# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/_hypothesis_stubs/__init__.py 第 77–79 行
def register_random(*args, **kwargs):
    pass  # pragma: no cover


register_random()
register_random(1, 2, key=3)
returned = 'None' if register_random() is None else 'not None'
