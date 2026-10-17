# `MS-13` ② 的 `Lib/test/` 语料：`_is_full_match_test` 正文**逐字**取自
# `Lib/test/libregrtest/filter.py`（第 20–27 行）。探针只调用它。
import re

def _is_full_match_test(pattern):
    # If a pattern contains at least one dot, it's considered
    # as a full test identifier.
    # Example: 'test.test_os.FileTests.test_access'.
    #
    # ignore patterns which contain fnmatch patterns: '*', '?', '[...]'
    # or '[!...]'. For example, ignore 'test_access*'.
    return ('.' in pattern) and (not re.search(r'[?*\[\]]', pattern))
