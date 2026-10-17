# **多个 `*` / `**` 实参**（第 326 轮，照参照实测）：`f(*a, *b)` ⇒
#   `BUILD_LIST 0; LOAD a; LIST_EXTEND 1; LOAD b; LIST_EXTEND 1; CALL_INTRINSIC_1 6`；
# 有前置位置实参时 `BUILD_LIST` 的个数换成它们。先前那一支直接报"多个 `*` 实参尚未接线"
# （上限榜上 `functools` 那一族 11 个模块卡它）。
# 注：程序里**不排序**（本层的元组比较是另一条独立的缺口，会干扰这一格）。


def show(*args, **kwargs):
    return (args, kwargs)


a = [1, 2]
b = (3, 4)
print(str(show(*a, *b)))
print(str(show(0, *a, *b)))
print(str(show(*a)))
print(str(show(9, *a)))
kw = {"x": 1}
kw2 = {"y": 2}
print(str(show(**kw)))
print(str(show(1, **kw)))
print(str(show(*a, **kw, **kw2)))
print(str(show(*a, *b, **kw)))
print(str(show(1, 2, 3)))
print(str(show()))
print(str(show(**{})))
