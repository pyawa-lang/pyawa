# **`from M import 缺名` 要报 `ImportError`**（第 288 轮，照参照实测：
# `cannot import name 'x' from 'm'`）。先前直接抛 `AttributeError` ⇒ 上游那种
# `try: from _io import _WindowsConsoleIO / except ImportError: pass` **接不住**
# （`Lib/io.py:93` 就是这样，整个 `io` 都导入不了）。

try:
    from sys import missing_name
except ImportError:
    print("ImportError ok")

try:
    import _io
    from _io import missing_name
except ImportError:
    print("ImportError ok again")

import _io
print(str(_io.DEFAULT_BUFFER_SIZE))
print(str(hasattr(_io, "TextIOWrapper")))
print(str(hasattr(_io._IOBase, "__doc__")))
