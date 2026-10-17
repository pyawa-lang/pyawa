# `MS-13` ② 的 `Lib/test/` 语料（第三批）：正文**逐字**取自
#   Lib/test/libregrtest/filter.py 第 52–77 行
# 其余为脚手架。
def _compile_match_function(patterns):
    patterns = list(patterns)

    if all(map(_is_full_match_test, patterns)):
        # Simple case: all patterns are full test identifier.
        # The test.bisect_cmd utility only uses such full test identifiers.
        return set(patterns).__contains__
    else:
        import fnmatch
        regex = '|'.join(map(fnmatch.translate, patterns))
        # The search *is* case sensitive on purpose:
        # don't use flags=re.IGNORECASE
        regex_match = re.compile(regex).match

        def match_test_regex(test_id, regex_match=regex_match):
            if regex_match(test_id):
                # The regex matches the whole identifier, for example
                # 'test.test_os.FileTests.test_access'.
                return True
            else:
                # Try to match parts of the test identifier.
                # For example, split 'test.test_os.FileTests.test_access'
                # into: 'test', 'test_os', 'FileTests' and 'test_access'.
                return any(map(regex_match, test_id.split(".")))

        return match_test_regex
import re

matcher = _compile_match_function(['test_os', 'test_sys'])
full_match = matcher('test_os')
no_match = matcher('test_other')
