# 元组解包赋值（第 107 轮）：名字／交换／星号／下标／属性五种目标都真跑一遍 ✓
a, b = 1, 2
assert a == 1
assert b == 2

a, b = b, a
assert a == 2
assert b == 1

p, *q, r = [1, 2, 3, 4]
assert p == 1
assert q == [2, 3]
assert r == 4

lst = [0, 0]
lst[0], b = 7, 8
assert lst[0] == 7
assert b == 8

class C:
    pass

c = C()
c.attr, b = 9, 10
assert c.attr == 9
assert b == 10
print("ok")
