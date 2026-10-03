# set 方法面（第 146 轮）：add / discard / update / copy
s = set()
s.add(1)
assert len(s) == 1
s.add(1)
assert len(s) == 1
s.add(2)
assert sorted(s) == [1, 2]
assert 1 in s
s.discard(1)
assert sorted(s) == [2]
s.discard(99)
assert sorted(s) == [2]
s.update([3, 3, 4])
assert sorted(s) == [2, 3, 4]
c = s.copy()
c.add(5)
assert sorted(s) == [2, 3, 4]
assert sorted(c) == [2, 3, 4, 5]
print("ok")
