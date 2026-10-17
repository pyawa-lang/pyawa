# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/encodings/cp1006.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import encodings.cp1006

name = encodings.cp1006.__name__
surface = str(sorted(n for n in ['Codec', 'IncrementalDecoder', 'IncrementalEncoder', 'StreamReader', 'StreamWriter', 'codecs', 'decoding_table', 'encoding_table', 'getregentry'] if hasattr(encodings.cp1006, n)))
