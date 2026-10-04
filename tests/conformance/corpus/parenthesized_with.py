# **带括号的 `with`**（第 299 轮接）：`with (a as x, b as y):` —— 3.10 起合法，
# 括号只是分组；`Lib/test/support/__init__.py:2943` 正是它（那一族 26 个模块）。
# 含多行、尾随逗号两种写法。


class Ctx:
    def __init__(self, name):
        self.name = name

    def __enter__(self):
        print("enter " + self.name)
        return self

    def __exit__(self, *args):
        print("exit " + self.name)


with (Ctx("a") as x, Ctx("b")):
    print("body " + x.name)

with (
    Ctx("c") as y,
    Ctx("d"),
):
    print("body2 " + y.name)

with (Ctx("e"),):
    print("body3")
