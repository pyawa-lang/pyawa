# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/_hypothesis_stubs/__init__.py 第 67–75 行（assume／reject）
def assume(condition):
    if not condition:
        raise unittest.SkipTest("Unsatisfied assumption")
    return True


def reject():
    assume(False)

import unittest

ok = assume(True)
try:
    reject()
except unittest.SkipTest:
    rejected = 'skip'
else:
    rejected = 'none'
