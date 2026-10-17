# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/encoders.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.encoders

name = email.encoders.__name__
surface = str(sorted(n for n in ['encode_7or8bit', 'encode_base64', 'encode_noop', 'encode_quopri'] if hasattr(email.encoders, n)))
declared = str(sorted(email.encoders.__all__))
