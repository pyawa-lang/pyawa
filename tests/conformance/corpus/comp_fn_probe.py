# 临时对照：函数作用域里的推导式（迭代器是局部变量）
def scale(factor):
    values = [1, 2]
    return [v * factor for v in values]
scaled = scale(3)
total = 0
for v in scaled:
    total = total + v
