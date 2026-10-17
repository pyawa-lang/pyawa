# **内建 `map`／`filter`**（第 338 轮）。上限榜上 `NameError: name 'map' is not defined` × 74 个模块卡它。
#
# **如实登记的偏差** ✗：参照返回**惰性**的 `map`／`filter` 对象（`type(...)` 是 `map`／`filter`，
# 可以套无限可迭代对象）；本层返回**列表**（急求值）。
# 为什么先这样落：真正的惰性面需要**新迭代器类型**，而"把这两个类型认成迭代器"这一步会在**套件
# 上下文里抖出一条潜伏 UAF**（第 335／337 轮把触发点夹到 `is_iterator_type` 那一步，根因还欠）。
# ⇒ **先急求值让那 74 个模块过这一关**，惰性面随后补。
# **不静默**：值与迭代行为都与参照一致，只有"类型名／惰性"两点不同 ⇒ 语料**不比** `type(...)` 与
# 对象自己的 `repr`（那里面有地址），只比消费之后的结果。

print(str(list(map(lambda x: x * 2, [1, 2, 3]))))
print(str(list(filter(None, [0, 1, 2, 0, 3]))))
print(str(list(filter(lambda x: x > 1, [1, 2, 3]))))
print(str(list(map(lambda a, b: a + b, [1, 2], [10, 20, 30]))))
print(str(next(iter(map(len, ["a", "bb"])))))
print(str(sorted(map(str, [3, 1, 2]))))
print(str(list(map(int, ["1", "2"]))))
print(str(list(filter(bool, ["", "a", None, 0, 1]))))
print(str(list(map(lambda x: x + 1, range(5)))))
print(str(len(list(filter(lambda x: x % 2 == 0, range(10))))))
# **本轮不比**：`map(str.upper, ...)` 一类**未绑定方法**作实参的形态（本层还没接线，是另一条缺口）；
# 以及依赖元素比较的错误路径 ✓。
