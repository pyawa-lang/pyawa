# `MS-13` ② 的 `Lib/test/` 语料（第三批）：被测代码即逐字同步的
# `Lib/test/typinganndata/fwdref_module.py`（import 驱动）。探针只读模块面。
import test.typinganndata.fwdref_module

m = test.typinganndata.fwdref_module
name = m.__name__
surface = str(sorted(n for n in ['ForwardRef', 'MyDict', 'MyList', 'fw'] if hasattr(m, n)))
