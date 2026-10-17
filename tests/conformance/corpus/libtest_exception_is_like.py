# `MS-13` ② 的 `Lib/test/` 语料（第三批）：正文**逐字**取自
#   Lib/test/support/testcase.py 第 4–28 行
# 其余为脚手架。
class ExceptionIsLikeMixin:
    def assertExceptionIsLike(self, exc, template):
        """
        Passes when the provided `exc` matches the structure of `template`.
        Individual exceptions don't have to be the same objects or even pass
        an equality test: they only need to be the same type and contain equal
        `exc_obj.args`.
        """
        if exc is None and template is None:
            return

        if template is None:
            self.fail(f"unexpected exception: {exc}")

        if exc is None:
            self.fail(f"expected an exception like {template!r}, got None")

        if not isinstance(exc, ExceptionGroup):
            self.assertEqual(exc.__class__, template.__class__)
            self.assertEqual(exc.args[0], template.args[0])
        else:
            self.assertEqual(exc.message, template.message)
            self.assertEqual(len(exc.exceptions), len(template.exceptions))
            for e, t in zip(exc.exceptions, template.exceptions):
                self.assertExceptionIsLike(e, t)
class Checker(ExceptionIsLikeMixin):
    def fail(self, message):
        raise AssertionError(message)

    def assertEqual(self, left, right):
        if left != right:
            raise AssertionError('不相等')


checker = Checker()
result = str(checker.assertExceptionIsLike(ValueError("x"), ValueError("x")))
