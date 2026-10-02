# 后缀链与链式赋值目标（第 221／222 轮）
class Box:
    pass
items = [Box()]
items[0].v = [10, 20]
x = items[0].v[1]
y = items[0].v
items[0].v[0] = 99
z = items[0].v[0]
