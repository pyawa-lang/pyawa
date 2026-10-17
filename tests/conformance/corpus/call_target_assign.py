# **调用开头的赋值目标**（第 283 轮）：`f()[k] = v` / `f().attr = v` 是**赋值**，不是表达式语句。
# `multiprocessing/context.py:217` 的 `globals()['reduction'] = reduction` 正是它（23 个模块卡在这条上）。

globals()["g"] = 5
print(str(g))

holder = []


def get():
    return holder


get().append(1)
print(str(holder[0]))


class Box:
    def __init__(self, v):
        self.v = v


boxes = [Box(1)]


def get_box():
    return boxes[0]


get_box().v = 9
print(str(boxes[0].v))
get_box().v = get_box().v + 1
print(str(boxes[0].v))
