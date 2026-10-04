# Rust 侧已实现的模块进模块表（第 134 轮）：此前只登记了 sys ⇒ 其余都掉进"按 sys.path 找不到" ✗
# 名字一律用各模块自己的 NAME（_imp 而不是 imp —— imp 在 3.14 已被移除 ✓）
import _imp
import itertools
import marshal
import operator
import sys

assert operator.add(2, 3) == 5
assert operator.mul(3, 4) == 12
assert itertools.repeat is not None
assert sys.modules is not None
print("ok")
