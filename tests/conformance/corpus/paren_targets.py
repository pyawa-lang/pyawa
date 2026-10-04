# 带括号的元组目标（第 111 轮）：`(a, b) = x` ⇒ `UNPACK_SEQUENCE`，目标跨度**含括号** ✓
(a, b) = 1, 2
assert a == 1 and b == 2

(c, d,) = 3, 4
assert c == 3 and d == 4

(x) = 5
assert x == 5

(e,
 f,
 ) = 6, 7
assert e == 6 and f == 7
print("ok")
