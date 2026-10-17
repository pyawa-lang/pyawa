# `MS-13` ② 的 `Lib/test/` 语料（第三批）：被测代码即逐字同步的
# `Lib/test/typinganndata/partialexecution/__init__.py`（import 驱动）。探针只读模块面。
import test.typinganndata.partialexecution

m = test.typinganndata.partialexecution
name = m.__name__
surface = str(sorted(n for n in ['a', 'b'] if hasattr(m, n)))
