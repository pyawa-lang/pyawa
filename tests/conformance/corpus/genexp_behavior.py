# 生成器表达式行为（第 142 轮）：带条件、嵌套、作为唯一实参、被内建消费
def evens(values):
    return [v for v in values if v % 2 == 0]


assert evens([1, 2, 3, 4]) == [2, 4]
assert sum(i for i in [1, 2, 3] if i > 1) == 5
assert sum(i for i in [1, 2, 3] if i > 1) == 5
assert list(i * 2 for i in (1, 2, 3) if i != 2) == [2, 6]
assert sorted(i for i in [3, 1, 2]) == [1, 2, 3]
assert tuple(j for j in (i for i in [1, 2])) == (1, 2)
assert max((i for i in [4, 9, 2])) == 9

gen = (i for i in [1, 2, 3])
assert next(gen) == 1
assert next(gen) == 2
assert list(gen) == [3]
print("ok")
