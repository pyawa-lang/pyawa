# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/libregrtest/filter.py`（逐字同步件）的 `match_test`
# —— 默认 `_test_matchers` 为空 ⇒ 恒真。补第一批 `_is_full_match_test` 没覆盖的那条函数。
import test.libregrtest.filter


class Fake:
    def __init__(self, name):
        self._name = name

    def id(self):
        return self._name


matched = test.libregrtest.filter.match_test(Fake("test_os"))
