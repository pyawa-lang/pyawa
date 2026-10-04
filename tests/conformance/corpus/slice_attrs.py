# slice 的属性面（第 151 轮）：start / stop / step（省略的那段是 None）
s = slice(1, 3)
assert s.start == 1
assert s.stop == 3
assert s.step is None
t = slice(None, None, -1)
assert t.start is None
assert t.stop is None
assert t.step == -1
u = slice(5)
assert u.start is None
assert u.stop == 5
assert u.step is None
assert [1, 2, 3, 4][slice(1, 3)] == [2, 3]
assert [1, 2, 3, 4][slice(None, None, 2)] == [1, 3]
print("ok")
