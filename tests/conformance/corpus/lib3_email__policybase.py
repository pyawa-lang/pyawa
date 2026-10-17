# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/_policybase.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email._policybase

name = email._policybase.__name__
surface = str(sorted(n for n in ['Compat32', 'Policy', 'abc', 'compat32', 'header', 're', 'valid_header_name_re', 'validate_header_name'] if hasattr(email._policybase, n)))
declared = str(sorted(email._policybase.__all__))
