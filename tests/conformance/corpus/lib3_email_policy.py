# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/policy.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.policy

name = email.policy.__name__
surface = str(sorted(n for n in ['Compat32', 'EmailMessage', 'EmailPolicy', 'HTTP', 'HeaderRegistry', 'Policy', 'SMTP', 'SMTPUTF8', 'compat32', 'default', 'linesep_splitter', 'raw_data_manager'] if hasattr(email.policy, n)))
declared = str(sorted(email.policy.__all__))
