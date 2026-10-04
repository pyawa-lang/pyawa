# 数值内建（第 152 轮）：pow / divmod / round（整数面）
assert pow(2, 10) == 1024
assert pow(3, 0) == 1
assert pow(0, 5) == 0
assert divmod(7, 2) == (3, 1)
assert divmod(-7, 2) == (-4, 1)
assert divmod(7, -2) == (-4, -1)
assert divmod(-7, -2) == (3, -1)
assert round(7) == 7
assert divmod(10, 5) == (2, 0)
q, r = divmod(17, 5)
assert q == 3 and r == 2
print("ok")
