# `MS-13` ② 的 `Lib/test/` 语料（第三批）：正文**逐字**取自
#   Lib/test/support/_hypothesis_stubs/__init__.py 第 67–83 行
# 其余为脚手架。
def assume(condition):
    if not condition:
        raise unittest.SkipTest("Unsatisfied assumption")
    return True


def reject():
    assume(False)


def register_random(*args, **kwargs):
    pass  # pragma: no cover


def settings(*args, **kwargs):
    return lambda f: f  # pragma: nocover

import unittest


class Recorder:
    def __init__(self):
        self.calls = []

    def __call__(self, *args, **kwargs):
        self.calls.append(args)


recorder = Recorder()
decorated = settings(max_examples=5)(recorder)
settings_same = decorated is recorder
assumed_ok = assume(True)
register_random("x", "y")
try:
    reject()
except Exception:
    rejected = "raised"
else:
    rejected = "not raised"
