# **嵌套块里的终止不能连坐作用域的收尾**（第 357 轮真 bug 修，`FellOffEnd` 族 **104** 个模块的根子）：
# 函数作用域的收尾由 `emitter.epilogue_needed` 判，而 `raise`／`return` 两条语句臂会把它置假
# —— 那是**对"本块"说的** ✓；嵌套块（`if` 的体／`for` 的体／`try` 的套体 …）里的终止**替不了
# 作用域的账** ⇒ 先前不还原 ⇒ 末尾语句的内部终止过 ⇒ 标志留在假 ⇒ **漏发收尾** ⇒ 掉底 ✓。
# 现在 **进出嵌套块时保存／还原**这个标志 ✓（作用域自己的体不还原 ✓）。

def simple(x):
    if x:
        raise TypeError("boom")


print(str(simple(0)))


def nested_loops(x):
    for a in x:
        for b in a:
            if b:
                raise TypeError("boom")


print(str(nested_loops([[0, 0]])))


class C:
    @classmethod
    def m(cls, x):
        for a in x:
            if a:
                raise ValueError("v")

    def n(self):
        return 1


print(str(C.m([0])))
print(str(C().n()))


def with_else(x):
    if x:
        raise TypeError("t")
    else:
        return "else"


print(str(with_else(0)))


def double_nested(x):
    if x:
        if x > 1:
            raise KeyError("k")
    return "end"


print(str(double_nested(0)))
print(str(double_nested(1)))
