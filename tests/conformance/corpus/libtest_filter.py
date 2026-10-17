# `MS-13` ② 的 `Lib/test/` 语料：`Lib/test/libregrtest/filter.py`（逐字同步件）。
# 取 `_is_full_match_test` 的两个分支（`.` 判定 ＋ 通配符排除）。
# 注：该文件的 `set_match_tests` 那条路要 `itertools.groupby`，本层尚未接线 ⇒ 本语料不收它
# （未实现不是差异，`MS-19`；它会红，故不进语料）。
import test.libregrtest.filter

f = test.libregrtest.filter
full = f._is_full_match_test("test.test_os.FileTests.test_access")
glob = f._is_full_match_test("test_access*")
plain = f._is_full_match_test("test_x")
bracket = f._is_full_match_test("test[abc]")
