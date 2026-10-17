# **`operator.itemgetter`**（第 308 轮）：返回一个可调用对象 —— 本层用"绑定方法"形态承载那几个
# key（与 `[].append` 同一套机制）。参照语义：一个 item ⇒ 单个值；多个 ⇒ 元组；
# **零个 ⇒ 构造时就报** `TypeError: itemgetter expected 1 argument, got 0`。
# 动因：上限诊断里 `ImportError: cannot import name 'itemgetter' from 'operator'` × 31 个模块。

import operator

data = [{"k": 1, "n": 2}, {"k": 3, "n": 4}]
getter = operator.itemgetter("k")
print(str(getter(data[0])))
print(str(operator.itemgetter("k", "n")(data[1])))
print(str(operator.itemgetter(0)([9, 8])))
print(str([getter(row) for row in data]))
print(str(operator.itemgetter(0, 1)("ab")))
print(str(operator.itemgetter(-1)([1, 2, 3])))

try:
    operator.itemgetter()
except TypeError as error:
    print("TypeError: " + str(error))
