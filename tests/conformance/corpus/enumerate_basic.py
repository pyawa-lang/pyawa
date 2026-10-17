# **`enumerate`**（第 347 轮）：上限榜上 `NameError: name 'enumerate' is not defined` × 76 个模块卡它。
#
# **如实登记的偏差**：参照返回**惰性**的 `enumerate` 对象（`type(...)` 是 `enumerate`）；本层返回
# `(下标, 元素)` 的 **`list`** —— 与 `map`／`filter` 同一口径与同一理由（真惰性要新迭代器类型，
# 而"把它认成迭代器"那一步会在套件上下文里抖出一条潜伏 UAF）。值与迭代行为与参照一致，
# 语料**不比** `type(...)`。

print(str(list(enumerate(["a", "b"]))))
print(str(list(enumerate(["a", "b"], 1))))
print(str(list(enumerate([]))))
print(str(list(enumerate("xy", 5))))
print(str(list(enumerate((1, 2, 3)))))
print(str(list(enumerate(range(3), -1))))
for index, value in enumerate(["p", "q"]):
    print(str(index) + ":" + value)
print(str([i for i, _ in enumerate("abc")]))
