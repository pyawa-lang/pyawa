# **星号形参上的注解**（第 299 轮接）：`def f(*args: int, **kw: str)`。
# 先前只认 `*名字`／`**名字` ⇒ 报"形参表里出现 Some(Colon)"（`Lib/test/support/__init__.py`
# 那一族 26 个模块的首个卡点）。注解本身解析掉、不登记（与 `**kw` 同口径）。


def f(*args: int, **kwargs: str) -> None:
    print(str(len(args)) + str(len(kwargs)))


f(1, 2, a=3)
f()
f(*[1, 2, 3], **{"z": 1})


def g(x: int, *rest: str, y: float = 1.0, **kw: object) -> int:
    return x


print(str(g(1)))
print(str(g(2, "a", "b", y=3.0, z=None)))
