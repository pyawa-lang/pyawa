# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/_hypothesis_stubs/__init__.py 第 85–110 行
# 其余为脚手架。
from enum import Enum

class HealthCheck(Enum):
    data_too_large = 1
    filter_too_much = 2
    too_slow = 3
    return_value = 5
    large_base_example = 7
    not_a_test_method = 8

    @classmethod
    def all(cls):
        return list(cls)


class Verbosity(Enum):
    quiet = 0
    normal = 1
    verbose = 2
    debug = 3


class Phase(Enum):
    explicit = 0
    reuse = 1
    generate = 2
    target = 3
    shrink = 4

has_all = str(hasattr(HealthCheck, 'all'))
health_name = HealthCheck.__name__
has_shrink = str(hasattr(Phase, 'shrink'))
