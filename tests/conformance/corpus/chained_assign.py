# 链式赋值（第 112 轮）：值只求一次、`COPY 1` 给每个非末尾目标 ✓，从左到右存 ✓
a = b = 1
assert a == 1 and b == 1

x = y = z = 2
assert x == 2 and y == 2 and z == 2

class C:
    pass

c = C()
v = c.attr = 3
assert v == 3 and c.attr == 3

lst = [0]
w = lst[0] = 4
assert w == 4 and lst[0] == 4

def f():
    p = q = 5
    return p + q

assert f() == 10
print("ok")
