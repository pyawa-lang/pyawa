# **类模式的子模式**（第 301 轮）：`case Point(x=0, y=0):`／`case Point(x=n, y=m):`／嵌套
# `case Box(inner=Point(x=1)):`。形状照参照 `dis` 实测：`COPY 1; <类>; LOAD_CONST <关键字名元组>;
# MATCH_CLASS <位置个数>; COPY 1; POP_JUMP_IF_NONE <不命中>; NOT_TAKEN; UNPACK_SEQUENCE <总数>`
# ＋逐个子模式判定；**每个失败点各自清理**（顶层失败时主语要留着给下一条 case）。
# 注意：本语料的构造一律**写全实参** —— 方法默认值那条另有缺陷（见 PLAN 的 P3-24），不混进来。


class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y


def f(p):
    match p:
        case Point(x=0, y=0):
            return "origin"
        case Point(x=1):
            return "x-one"
        case Point(y=2):
            return "y-two"
        case Point(x=n, y=m):
            return "point " + str(n) + " " + str(m)
        case _:
            return "other"


print(f(Point(0, 0)))
print(f(Point(1, 9)))
print(f(Point(7, 2)))
print(f(Point(3, 4)))
print(f(7))


class Box:
    def __init__(self, inner):
        self.inner = inner


def g(b):
    match b:
        case Box(inner=Point(x=1)):
            return "box-x-one"
        case Box(inner=Point(x=0, y=0)):
            return "box-origin"
        case Box(inner=Point(x=n, y=m)):
            return "box-point " + str(n) + " " + str(m)
        case Box():
            return "box"
        case _:
            return "other"


print(g(Box(Point(1, 5))))
print(g(Box(Point(0, 0))))
print(g(Box(Point(4, 6))))
print(g(Box(9)))
print(g(3))
