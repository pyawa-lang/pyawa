# `MS-13` ② 的 `Lib/test/` 语料（第三批）：被测代码即逐字同步的
# `Lib/test/typinganndata/ann_module5.py`（import 驱动）。探针只读模块面。
import test.typinganndata.ann_module5

m = test.typinganndata.ann_module5
name = m.__name__
surface = str(sorted(n for n in ['Final', 'MyClass', 'annotations', 'name'] if hasattr(m, n)))
final_name = m.name
class_value = m.MyClass.value
