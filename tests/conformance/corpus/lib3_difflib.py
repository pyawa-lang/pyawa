# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/difflib.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import difflib

name = difflib.__name__
surface = str(sorted(n for n in ['Differ', 'GenericAlias', 'HtmlDiff', 'IS_CHARACTER_JUNK', 'IS_LINE_JUNK', 'Match', 'SequenceMatcher', 'context_diff', 'diff_bytes', 'get_close_matches', 'ndiff', 'restore'] if hasattr(difflib, n)))
declared = str(sorted(difflib.__all__))
