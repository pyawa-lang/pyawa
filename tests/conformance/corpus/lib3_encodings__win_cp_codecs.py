# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/encodings/_win_cp_codecs.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import encodings._win_cp_codecs

name = encodings._win_cp_codecs.__name__
surface = str(sorted(n for n in ['codecs', 'create_win32_code_page_codec'] if hasattr(encodings._win_cp_codecs, n)))
