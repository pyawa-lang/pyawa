# zip（第 229 轮）：惰性、取最短；空参数立刻耗尽
print(str(list(zip([1, 2], "ab", [3]))))
print(str(list(zip())))
print(type(zip()).__name__)
print(str(next(zip([1], [2]))))
