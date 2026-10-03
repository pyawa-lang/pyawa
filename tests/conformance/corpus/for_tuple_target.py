# `for` 的元组目标（第 118 轮）：`UNPACK_SEQUENCE`，位点＝整段目标 ✓
# 顺带守"裸名字当表达式语句"（函数体里一句 a ✓）
pairs = [(1, 2), (3, 4)]
total = 0
for a, b in pairs:
    total = total + a + b
assert total == 10

for x, y, z in [(1, 2, 3)]:
    total = total + x + y + z
assert total == 16

def f(items):
    out = 0
    for a, b in items:
        a
        out = out + a + b
    return out

assert f([(1, 2)]) == 3
print("ok")
