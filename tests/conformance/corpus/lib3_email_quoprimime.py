# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/quoprimime.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.quoprimime

name = email.quoprimime.__name__
surface = str(sorted(n for n in ['CRLF', 'EMPTYSTRING', 'NL', 'ascii_letters', 'body_check', 'body_decode', 'body_encode', 'body_length', 'decode', 'decodestring', 'digits', 'header_check'] if hasattr(email.quoprimime, n)))
declared = str(sorted(email.quoprimime.__all__))
