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
