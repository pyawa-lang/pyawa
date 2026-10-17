# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/_pylong.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import _pylong

name = _pylong.__name__
surface = str(sorted(n for n in ['compute_powers', 'decimal', 'int_divmod', 'int_from_string', 'int_to_decimal', 'int_to_decimal_string', 're', 'str_to_int'] if hasattr(_pylong, n)))
