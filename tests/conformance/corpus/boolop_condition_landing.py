# **布尔条件的落点**（第 361 轮真 bug 修）：`emit_test_bare` 的布尔运算分支先前把**外层传进来的**
# `cleanup` 丢掉 ✗ ⇒ 外层那个跳转标签没人落点 ⇒ 收尾断言 `跳转目标标签 N 从未落点` ⇒ **编译期 panic** ✓
# （`import textwrap`／`argparse`／`asyncio` 一族都崩在这里 ✓，上限榜 `-6`（SIGABRT）族的一大半 ✓）。
#
# 口径照参照码元分开 ✓（`if a and b or c` 的参照布局）：
#   * `and`：非末操作数为**假** ⇒ 跳"and 的假出口" —— `jump_if_true == false` 时就是条件出口 ✓；
#     `== true` 时是**本子式之后**（外层还要继续 ✓，即下一个操作数 ✓）。
#   * `or`：非末操作数为**真** ⇒ 跳"本子式之后" ✓；为假 ⇒ 继续下一个操作数 ✓（各给一个落点 ✓）。

def shape(a, b, c, d):
    if a or (not d or a and b == 1) and b <= c:
        return "hit"
    return "miss"


for args in ((0, 1, 2, 3), (1, 1, 2, 3), (0, 0, 2, 3), (0, 1, 9, 3)):
    print(str(shape(*args)))


def shallow(a, b):
    if a or b:
        return "or"
    return "no"


def mid(a, b, c):
    if (a or b) and c:
        return "and"
    return "no"


print(str(shallow(0, 1)), str(shallow(0, 0)), str(shallow(1, 0)))
print(str(mid(1, 0, 1)), str(mid(1, 0, 0)), str(mid(0, 0, 1)))


def nested(a, b, c, d):
    if (a or (b and c)) and d:
        return "nested"
    return "no"


print(str(nested(1, 0, 0, 1)), str(nested(0, 1, 1, 1)), str(nested(0, 1, 1, 0)))
