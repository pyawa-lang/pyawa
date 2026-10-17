# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/__init__.py`（包驱动）。
import test as m

name = m.__name__
is_pkg = str(hasattr(m, '__path__'))
