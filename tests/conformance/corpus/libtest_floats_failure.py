# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/testcase.py 第 1、31–53 行（`FloatsAreIdenticalMixin` 的失败路径）
# 其余为脚手架。
from math import copysign, isnan
class FloatsAreIdenticalMixin:
    def assertFloatsAreIdentical(self, x, y):
        """Fail unless floats x and y are identical, in the sense that:
        (1) both x and y are nans, or
        (2) both x and y are infinities, with the same sign, or
        (3) both x and y are zeros, with the same sign, or
        (4) x and y are both finite and nonzero, and x == y

        """
        msg = 'floats {!r} and {!r} are not identical'

        if isnan(x) or isnan(y):
            if isnan(x) and isnan(y):
                return
        elif x == y:
            if x != 0.0:
                return
            # both zero; check that signs match
            elif copysign(1.0, x) == copysign(1.0, y):
                return
            else:
                msg += ': zeros have different signs'
        self.fail(msg.format(x, y))

class Checker(FloatsAreIdenticalMixin):
    def fail(self, message):
        raise AssertionError(message)


checker = Checker()
try:
    checker.assertFloatsAreIdentical(1.0, 2.0)
except AssertionError as error:
    message = str(error)
else:
    message = 'no error'
