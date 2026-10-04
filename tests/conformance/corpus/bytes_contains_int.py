# **`bytes` 的 `in` 与 `__contains__` 认整数那一档**（第 284 轮，照参照实测）：
# `98 in b"b"` ⇒ `True`；`300 in b"ab"`／`(-1) in b"ab"` ⇒ `ValueError: byte must be in range(0, 256)`；
# `"a" in b"ab"` ⇒ `TypeError: a bytes-like object is required, not 'str'`。
# 先前只有"字节 × 字节"那一档 ⇒ 整数一律报 TypeError ✗（`Lib/` 里按字节判的地方真的会踩 ✓）。

print(str(98 in b"b"))
print(str(98 in b"abc"))
print(str(0 in b"\x00"))
print(str(98 not in b"b"))
print(str(b"ab".__contains__(b"a")))

try:
    print(str(300 in b"ab"))
except ValueError as error:
    print(str(error))

try:
    print(str((-1) in b"ab"))
except ValueError as error:
    print(str(error))

try:
    print(str("a" in b"ab"))
except TypeError as error:
    print(str(error))
