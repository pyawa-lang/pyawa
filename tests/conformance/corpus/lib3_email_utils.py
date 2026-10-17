# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/utils.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.utils

name = email.utils.__name__
surface = str(sorted(n for n in ['COMMASPACE', 'CRLF', 'EMPTYSTRING', 'TICK', 'UEMPTYSTRING', 'collapse_rfc2231_value', 'datetime', 'decode_params', 'decode_rfc2231', 'encode_rfc2231', 'escapesre', 'format_datetime'] if hasattr(email.utils, n)))
declared = str(sorted(email.utils.__all__))
