# 迭代器对象的消费（第 137 轮）：list(itertools.repeat(5, 3)) 这类此前报"不是可迭代" ✗
# 现走执行器同一处 advance（内建迭代器 + __next__ 协议）
import itertools

assert list(itertools.repeat(5, 3)) == [5, 5, 5]
assert sum(itertools.repeat(1, 4)) == 4
assert tuple(itertools.repeat(2, 2)) == (2, 2)
assert sorted(itertools.repeat(2, 3)) == [2, 2, 2]
assert list(x for x in [1, 2, 3]) == [1, 2, 3]
assert tuple(y * 2 for y in (1, 2)) == (2, 4)
print("ok")
