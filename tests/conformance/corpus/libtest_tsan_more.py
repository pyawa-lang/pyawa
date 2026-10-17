# `MS-13` ② 的 `Lib/test/` 语料：被测代码即逐字同步的 `Lib/test/libregrtest/tsan.py`
# （import 驱动；第二批已有一条计数断言，此条换一组列表断言）。
import test.libregrtest.tsan

m = test.libregrtest.tsan
middle = str(m.TSAN_TESTS[5:9])
parallel_count = len(m.TSAN_PARALLEL_TESTS)
last = m.TSAN_PARALLEL_TESTS[-1]
