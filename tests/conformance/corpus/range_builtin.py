# range（第 148 轮）：本层先给迭代器（count ＋ islice 拼），for / list / 推导式这些常见用法一致
assert list(range(3)) == [0, 1, 2]
assert list(range(2, 5)) == [2, 3, 4]
assert list(range(0, 10, 3)) == [0, 3, 6, 9]
assert list(range(0)) == []
assert list(range(5, 2)) == []
assert sum(range(4)) == 6
assert sorted([i * 2 for i in range(3)]) == [0, 2, 4]
total = 0
for i in range(4):
    total = total + i
assert total == 6
print("ok")
