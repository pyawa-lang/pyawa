# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/libregrtest/parallel_case.py`（import 驱动）。
import test.libregrtest.parallel_case

m = test.libregrtest.parallel_case

has_cls = str(hasattr(m, 'ParallelTestCase'))
name = m.ParallelTestCase.__name__
