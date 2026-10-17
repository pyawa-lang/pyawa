# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/libregrtest/tsan.py`（逐字同步件）。
import test.libregrtest.tsan

m = test.libregrtest.tsan
n_tsan = len(m.TSAN_TESTS)
n_parallel = len(m.TSAN_PARALLEL_TESTS)
first = m.TSAN_TESTS[0]
has_thread = "test_thread" in m.TSAN_TESTS
