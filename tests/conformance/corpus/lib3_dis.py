# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/dis.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import dis

name = dis.__name__
surface = str(sorted(n for n in ['ArgResolver', 'BINARY_OP', 'Bytecode', 'CACHE', 'CALL_INTRINSIC_1', 'CALL_INTRINSIC_2', 'COMPILER_FLAG_NAMES', 'CONTAINS_OP', 'CONVERT_VALUE', 'END_ASYNC_FOR', 'ENTER_EXECUTOR', 'EXTENDED_ARG'] if hasattr(dis, n)))
declared = str(sorted(dis.__all__))
