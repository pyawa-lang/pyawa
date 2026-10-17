# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/typinganndata/ann_module8.py`（逐字同步件）。
# 守嵌套类 ＋ 函数注解在 3.14 下的 `__annotations__` 形态。
import test.typinganndata.ann_module8

m = test.typinganndata.ann_module8
inner = str(m.NoTypeCheck_Outer.Inner.__annotations__)
fn = str(m.NoTypeCheck_function.__annotations__)
