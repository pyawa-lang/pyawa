# **序列重复 `*`**（第 305 轮接）：`str`／`list`／`tuple`／`bytes` 乘整数，**两个方向都认**；
# 次数为 0／负数 ⇒ **空序列**；整数乘整数仍走整数那条路（这里是回归哨）。
# 动因：`Lib/` 里 `"-" * 40` 这类遍地都是，实测第一处撞上的是 `traceback.py` 的 `f"{'a' * 3}"`
# （先前报 `unsupported operand type(s) for *: 'str' and 'int'`）。

print(str("ab" * 3))
print(str(3 * "ab"))
print(str("-" * 10))
print(str([1, 2] * 2))
print(str(2 * [3]))
print(str((1,) * 3))
print(str(() * 5))
print(str(b"ab" * 2))
print(str(2 * b"xy"))
print(str("x" * 0))
print(str("x" * -3))
print(str([1] * 0))
print(str(len("ab" * 5)))
print(str(2 * 3))
print(str(2 * -3))
s = "z" * 3
print(s)
