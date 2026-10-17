# `MS-13` ② 的 `Lib/test/` 语料（第三批）：被测代码即逐字同步的
# `Lib/test/libregrtest/parallel_case.py`（import 驱动）。探针只读模块面。
import test.libregrtest.parallel_case

m = test.libregrtest.parallel_case
name = m.__name__
surface = str(sorted(n for n in ['ParallelTestCase', 'TestCase', 'copy', 'threading', 'unittest'] if hasattr(m, n)))
