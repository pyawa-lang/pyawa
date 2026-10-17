# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/base64mime.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.base64mime

name = email.base64mime.__name__
surface = str(sorted(n for n in ['CRLF', 'EMPTYSTRING', 'MISC_LEN', 'NL', 'a2b_base64', 'b2a_base64', 'b64encode', 'body_decode', 'body_encode', 'decode', 'decodestring', 'header_encode'] if hasattr(email.base64mime, n)))
declared = str(sorted(email.base64mime.__all__))
