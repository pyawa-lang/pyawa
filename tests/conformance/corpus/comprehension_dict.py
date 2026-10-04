# 字典推导式 ＋ 多重 for（第 237 轮）
base = [1, 2, 3]
doubled = {v: v * 2 for v in base}
two = doubled[2]
a = 1
b = 2
pairs = [(a, 10), (b, 20)]
swapped = {y: x for x, y in pairs}
twenty = swapped[20]
low = [1, 2]
high = [10, 20]
multi = [m + n for m in low for n in high]
total = 0
for v in multi:
    total = total + v
