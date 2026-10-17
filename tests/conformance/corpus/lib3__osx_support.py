# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/_osx_support.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import _osx_support

name = _osx_support.__name__
surface = str(sorted(n for n in ['compiler_fixup', 'customize_compiler', 'customize_config_vars', 'get_platform_osx', 'os', 're', 'sys'] if hasattr(_osx_support, n)))
declared = str(sorted(_osx_support.__all__))
