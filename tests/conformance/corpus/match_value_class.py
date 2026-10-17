# **`match` 的值模式与类模式**（第 300 轮）：`case Color.RED:` 与 `case str():`。
# 判定形状照参照 `dis` 实测：值模式与字面量模式同形；类模式走
# `COPY 1; <类>; LOAD_CONST (); MATCH_CLASS 0; COPY 1; POP_JUMP_IF_NONE <清理>;
#  NOT_TAKEN; UNPACK_SEQUENCE 0`，**不命中那条路要另起清理块**（否则下一条 case 会拿着 None 去比）。


class Color:
    RED = 1
    GREEN = 2


def f(x):
    match x:
        case Color.RED:
            return "red"
        case Color.GREEN:
            return "green"
        case str():
            return "str"
        case int():
            return "int"
        case float():
            return "float"
        case _:
            return "other"


print(f(Color.RED))
print(f(Color.GREEN))
print(f("hi"))
print(f(7))
print(f(1.5))
print(f(None))
print(f(True))


class Point:
    pass


class Sub(Point):
    pass


def g(x):
    match x:
        case Sub():
            return "sub"
        case Point():
            return "point"
        case _:
            return "not"


print(g(Point()))
print(g(Sub()))
print(g(3))
