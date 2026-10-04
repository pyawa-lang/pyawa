# **`for` 的嵌套元组目标**（第 289 轮，照参照 `dis` 实测）：
# `for a, (b, c) in x:` ⇒ `UNPACK_SEQUENCE 2`（整段目标）＋ `STORE a`
# ＋ `UNPACK_SEQUENCE 2`（`(b, c)` 那一段）＋ `STORE b` ＋ `STORE c`。
# 先前只认**名字** ⇒ `Lib/test/support/__init__.py:1887` 的
# `for report_type, (old_mode, old_file) in …` 报"`for` 的元组目标后面要名字"（那一族 26 个模块）。

pairs = [("a", ("b", "c")), ("a", ("d", "e"))]
for a, (b, c) in pairs:
    print(str(a) + str(b) + str(c))

for (p, q) in pairs:
    print(str(p) + str(q))

for a, [b, c] in pairs:
    print(str(a) + str(b) + str(c))

for single in [1, 2]:
    print(str(single))

for x, y in [(1, 2), (3, 4)]:
    print(str(x + y))

for (one,) in [(7,), (8,)]:
    print(str(one))

for a, (b, (c, d)) in [("a", ("b", ("c", "d")))]:
    print(str(a) + str(b) + str(c) + str(d))

# 函数里（局部槽那条路）也要对
def walk(items):
    total = 0
    for key, (left, right) in items:
        total = total + left + right
    return total


print(str(walk([("k", (1, 2)), ("m", (3, 4))])))
