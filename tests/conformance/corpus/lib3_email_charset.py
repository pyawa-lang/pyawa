# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/charset.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.charset

name = email.charset.__name__
surface = str(sorted(n for n in ['ALIASES', 'BASE64', 'CHARSETS', 'CODEC_MAP', 'Charset', 'DEFAULT_CHARSET', 'EMPTYSTRING', 'QP', 'RFC2047_CHROME_LEN', 'SHORTEST', 'UNKNOWN8BIT', 'add_alias'] if hasattr(email.charset, n)))
declared = str(sorted(email.charset.__all__))
