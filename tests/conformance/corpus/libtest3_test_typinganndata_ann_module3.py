# `MS-13` ② 的 `Lib/test/` 语料（第三批）：被测代码即逐字同步的
# `Lib/test/typinganndata/ann_module3.py`（import 驱动）。探针只读模块面。
import test.typinganndata.ann_module3

m = test.typinganndata.ann_module3
name = m.__name__
surface = str(sorted(n for n in ['C_OK', 'D_bad_ann', 'f_bad_ann', 'g_bad_ann'] if hasattr(m, n)))
