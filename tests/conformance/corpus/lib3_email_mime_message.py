# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/mime/message.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.mime.message

name = email.mime.message.__name__
surface = str(sorted(n for n in ['MIMEMessage', 'MIMENonMultipart', 'message'] if hasattr(email.mime.message, n)))
declared = str(sorted(email.mime.message.__all__))
