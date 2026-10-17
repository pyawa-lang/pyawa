# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/abc.py`（`ABC` ＋ `abstractmethod` 的契约面）。
import abc


class Base(abc.ABC):
    @abc.abstractmethod
    def run(self):
        ...


class Impl(Base):
    def run(self):
        return "ok"


value = Impl().run()
is_abstract = "run" in Base.__abstractmethods__
