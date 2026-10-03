# list 方法面（第 143 轮）：append / extend / pop
items = []
items.append(1)
items.append("two")
assert items == [1, "two"]
items.extend([3, 4])
assert items == [1, "two", 3, 4]
assert items.pop() == 4
assert items == [1, "two", 3]
assert len(items) == 3
extra = []
extra.extend("ab")
assert extra == ["a", "b"]
print("ok")
items2 = [1, 3]
items2.insert(1, 2)
assert items2 == [1, 2, 3]
items2.insert(99, 4)
assert items2 == [1, 2, 3, 4]
items2.insert(-99, 0)
assert items2 == [0, 1, 2, 3, 4]
assert items2.index(2) == 2
assert items2.count(0) == 1
assert items2.count(99) == 0
print("ok2")
