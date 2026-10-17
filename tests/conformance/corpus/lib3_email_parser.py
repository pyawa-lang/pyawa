# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/parser.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.parser

name = email.parser.__name__
surface = str(sorted(n for n in ['BytesFeedParser', 'BytesHeaderParser', 'BytesParser', 'FeedParser', 'HeaderParser', 'Parser', 'StringIO', 'TextIOWrapper', 'compat32'] if hasattr(email.parser, n)))
declared = str(sorted(email.parser.__all__))
