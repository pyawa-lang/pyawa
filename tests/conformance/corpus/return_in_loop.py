# 循环体内 return（第 247 轮）：常量先丢迭代器、非常量"值＋SWAP/POP_TOP"，嵌套每个 for 丢一次
def first(items):
    for item in items:
        return item
    return 0
a = first([7, 8])
b = first([])
def const_in_loop(items):
    for item in items:
        return 5
    return 0
c = const_in_loop([1])
def nested(outer, inner):
    for i in outer:
        for j in inner:
            return i + j
    return 0
d = nested([1, 2], [10, 20])
e = nested([], [1])
def while_loop(flag):
    while flag:
        return 9
    return 0
f = while_loop(1)
g = while_loop(0)
