# **`round` 接上浮点面**（第 337 轮）：先前只接整数 ⇒ `round(2.5)` 直接 TypeError ✗
# （第 336 轮做 `math` 时自己撞上的 ✓）。
# 口径**逐条量过**（3.14 实测）：不给 `ndigits` ⇒ 返回 `int`（半数取偶：2.5⇒2、3.5⇒4、-0.5⇒0 ✓）；
# 给了 `ndigits` ⇒ 返回 `float`；`int` 给了 `ndigits` 仍返回 `int`；
# **按正确的十进制舍入**（`round(2.675, 2)` ⇒ `2.67` ✓ —— 不是"先乘 100 再取偶" ✗，那样会得到 2.68 ✗）。

print(str(round(2.5)))
print(str(round(3.5)))
print(str(round(-0.5)))
print(str(round(0.5)))
print(str(round(1.5)))
print(str(round(2.675, 2)))
print(str(round(1.005, 2)))
print(str(round(1234.5678, -2)))
print(str(round(2.5, 0)))
print(str(round(7)))
print(str(round(7, 2)))
print(str(round(True)))
print(str(round(-2.5)))
print(str(round(1.2345, 3)))
print(str(type(round(2.5)).__name__))
print(str(type(round(2.5, 1)).__name__))
try:
    round(2.5, 1.5)
except TypeError as error:
    print("ndigits: " + str(error))
try:
    round("x")
except TypeError as error:
    print("str: " + str(error))
