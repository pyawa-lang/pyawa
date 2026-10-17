# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/contextlib.py`（`contextmanager` ＋ `with` 协议）。
import contextlib


@contextlib.contextmanager
def tag():
    yield "x"


with tag() as value:
    got = value
