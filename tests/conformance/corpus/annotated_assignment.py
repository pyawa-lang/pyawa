# **带注解的赋值／裸注解**（第 340 轮）：`x: int = 1`、`x: int`、类体里的注解 —— 先前完全没有这一支，
# 在 `:` 上报 `语句结尾多出了 Some(Colon)`（上限榜上 `annotationlib` 一族的**语法面**根子，另有
# 一大片模块里零散带注解）。
#
# **如实登记的偏差**：注解表达式**只解析、不求值、不保存** —— 本层还没有 `__annotations__`／
# `__annotate__`（PEP 649 那一套）。参照 3.14 是**惰性**求值 ⇒ "不求值"这一条不改执行期行为，
# 但 `__annotations__` 查不到 ⇒ 语料**不比**它。

x: int = 1
y: str
z: "T" = 3
print(str(x))
print(str(z))


def f():
    a: int = 5
    b: list = []
    c: dict = {}
    return (a, b, c)


print(str(f()))


class C:
    v: int = 7
    w: str
    items: list = [1, 2]


print(str(C.v))
print(str(C.items))
print(str(x + 1))
