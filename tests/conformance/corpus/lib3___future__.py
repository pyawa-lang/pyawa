# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/__future__.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import __future__

name = __future__.__name__
surface = str(sorted(n for n in ['CO_FUTURE_ABSOLUTE_IMPORT', 'CO_FUTURE_ANNOTATIONS', 'CO_FUTURE_BARRY_AS_BDFL', 'CO_FUTURE_DIVISION', 'CO_FUTURE_GENERATOR_STOP', 'CO_FUTURE_PRINT_FUNCTION', 'CO_FUTURE_UNICODE_LITERALS', 'CO_FUTURE_WITH_STATEMENT', 'CO_GENERATOR_ALLOWED', 'CO_NESTED', 'absolute_import', 'all_feature_names'] if hasattr(__future__, n)))
declared = str(sorted(__future__.__all__))
