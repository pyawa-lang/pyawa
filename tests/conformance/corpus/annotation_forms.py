# **注解的形状**（第 287 轮，两侧逐字比）。参照把注解编进 `__annotate__` 单元，逐条 `dis` 实测：
# ① `X[a]` 一个实参**不**发 `BUILD_TUPLE`（`list[int]`）；② `X[a, b]` 发 `BUILD_TUPLE n`（`dict[str, object]`）；
# ③ `A | B` 发 `BINARY_OP 7`；④ `...` 发 `LOAD_CONST Ellipsis`；⑤ `A.B` 发 `LOAD_ATTR`；
# ⑥ 字符串（前向引用）发 `LOAD_CONST`。先前只认"一层、一个实参"⇒ `Lib/test/support` 那一族（26 个模块）
# 卡在 `dict[str, object] | None` 上。


def f1(a: dict[str, object]):
    return a


def f2(a: list[int]):
    return a


def f3() -> int | None:
    return None


def f4(a: tuple[int, ...]):
    return a


def f5(a: list[tuple[int, str]]):
    return a


def f6(a: "X"):
    return a


def f7(a: int):
    return a


def f8(a: dict[str, list["X"]] | None = None):
    return a


class Outer:
    class Inner:
        pass


def f9(a: Outer.Inner):
    return a


print(str(f1({})))
print(str(f2([])))
print(str(f3()))
print(str(f4((1,))))
print(str(f5([])))
print(str(f6(1)))
print(str(f7(2)))
print(str(f8()))
print(str(f9(3)))
