# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/mime/base.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.mime.base

name = email.mime.base.__name__
surface = str(sorted(n for n in ['MIMEBase', 'email', 'message'] if hasattr(email.mime.base, n)))
declared = str(sorted(email.mime.base.__all__))
