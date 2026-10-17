# `MS-13` ② 的 `Lib/test/` 语料：被测代码即逐字同步的 `Lib/test/libregrtest/pgo.py`
# （经加载器 `import test.libregrtest.pgo` 执行本仓 `Lib/` 里那一份；参照侧同一份上游源码）。
# 探针只取数据面，不改正文。
import test.libregrtest.pgo

count = len(test.libregrtest.pgo.PGO_TESTS)
first = test.libregrtest.pgo.PGO_TESTS[0]
last = test.libregrtest.pgo.PGO_TESTS[-1]
has_test_int = "test_int" in test.libregrtest.pgo.PGO_TESTS
