# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/typinganndata/ann_module3.py`（import 驱动，属性面）。
import test.typinganndata.ann_module3

m = test.typinganndata.ann_module3
has_ok = str(hasattr(m, 'C_OK'))
has_bad = str(hasattr(m, 'D_bad_ann'))
has_func = str(hasattr(m, 'g_bad_ann'))
