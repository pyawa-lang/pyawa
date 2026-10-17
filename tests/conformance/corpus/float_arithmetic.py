# **浮点四则与一元面**（第 318 轮）：先前**整段缺失** —— `1.5 + 0.5`／`2.0 * 3.0`／`-1.5`
# 都报 `unsupported operand type(s) for …: 'float' and 'float'`（`/` 与比较是好的）。
# 这一格用 `git stash` 验证过**先于本轮**就坏。参照口径：**任一侧是 float ⇒ 结果就是 float**；
# `//` 取 floor、`%` 取**除数**的符号、`/`／`//`／`%` 的零除都报 `ZeroDivisionError: division by zero`；
# 负底数配非整数指数的幂参照给**复数** ⇒ 本层如实报未实现（不静默给 NaN）。
# 顺带放开两处挡住它的：实参开头的**一元 `+`／`-`**（`f(-1.5)` 参照合法）与
# `INTRINSIC_UNARY_POSITIVE` 的浮点那一格。

print(str(1.5 + 0.5))
print(str(1.5 - 0.5))
print(str(2.0 * 3.0))
print(str(7.0 / 2.0))
print(str(7.0 // 2.0))
print(str(-7.0 % 3.0))
print(str(2.0 ** 3.0))
print(str(2.0 ** -1.0))
print(str(-1.5))
print(str(+1.5))
print(str(abs(-1.5)))
print(str(1 + 2.0))
print(str(2.5 * 2))
print(str(3 - 0.5))
print(str(1.0 == 1))
print(str(0.1 + 0.2))
print(str(1_000_000.0 * 1_000_000.0))
try:
    print(str(1.0 / 0.0))
except ZeroDivisionError as error:
    print("ZeroDivisionError: " + str(error))
try:
    print(str(1.5 + "x"))
except TypeError as error:
    print("TypeError: " + str(error))
