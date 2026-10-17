# `MS-13` ② 的 `Lib/test/` 语料（第三批）：被测代码即逐字同步的
# `Lib/test/typinganndata/ann_module6.py`（import 驱动）。探针只读模块面。
import test.typinganndata.ann_module6

m = test.typinganndata.ann_module6
name = m.__name__
surface = str(sorted(n for n in ['ClassVar', 'annotations', 'wrong'] if hasattr(m, n)))
wrong_value = m.wrong
