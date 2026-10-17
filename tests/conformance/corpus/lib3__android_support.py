# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/_android_support.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import _android_support

name = _android_support.__name__
surface = str(sorted(n for n in ['BUCKET_SIZE', 'BinaryLogStream', 'Logcat', 'MAX_BYTES_PER_SECOND', 'MAX_BYTES_PER_WRITE', 'MAX_CHARS_PER_WRITE', 'PER_MESSAGE_OVERHEAD', 'RLock', 'TextLogStream', 'init_streams', 'io', 'sleep'] if hasattr(_android_support, n)))
