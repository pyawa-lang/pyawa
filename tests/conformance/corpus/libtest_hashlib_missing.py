# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/hashlib_helper.py 第 26–29 行
# 探针只比是否抛 SkipTest；其余为脚手架。
def _missing_hash(digestname, implementation=None, *, exc=None):
    parts = ["missing", implementation, f"hash algorithm: {digestname!r}"]
    msg = " ".join(filter(None, parts))
    raise unittest.SkipTest(msg) from exc

import unittest


try:
    _missing_hash("sha256", "OpenSSL")
except unittest.SkipTest:
    raised = "skip"
else:
    raised = "none"
