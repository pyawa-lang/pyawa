# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/header.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.header

name = email.header.__name__
surface = str(sorted(n for n in ['BSPACE', 'Charset', 'EMPTYSTRING', 'FWS', 'Header', 'HeaderParseError', 'MAXLINELEN', 'NL', 'SPACE', 'SPACE8', 'USASCII', 'UTF8'] if hasattr(email.header, n)))
declared = str(sorted(email.header.__all__))
