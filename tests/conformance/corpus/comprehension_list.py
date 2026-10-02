# 推导式（第 235 轮）：元素／if 条件／变量不外泄（模块级；函数内的那条见 PLAN 待查）
a = 1
b = 2
values = [a, b, a + b, b + b]
doubled = [v * 2 for v in values]
doubled_sum = 0
for v in doubled:
    doubled_sum = doubled_sum + v
picked = [v for v in values if v > 2]
picked_sum = 0
picked_count = 0
for v in picked:
    picked_sum = picked_sum + v
    picked_count = picked_count + 1
def scale(factor):
    return [v * factor for v in values]
scaled = scale(3)
scaled_sum = 0
for v in scaled:
    scaled_sum = scaled_sum + v
v = 99
eggs = [v for v in values]
outside = v
