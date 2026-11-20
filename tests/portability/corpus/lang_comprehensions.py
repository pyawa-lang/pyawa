# 语言：推导式、生成器、排序后的集合输出
print("list", [x * x for x in range(5)])
print("dict", sorted({x: x + 1 for x in range(3)}.items()))
print("set", sorted({x % 3 for x in range(10)}))
print("gen", sum(x for x in range(10)))
print("nested", [y for x in [[1, 2], [3]] for y in x])
