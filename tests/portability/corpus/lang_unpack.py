# 语言：解包、星号参数、默认参数
def f(a, b=2, *rest, **kw):
    return (a, b, rest, tuple(sorted(kw.items())))

print("pack", f(1))
print("star", f(1, 2, 3, 4, x=5, y=6))
first, *middle, last = [1, 2, 3, 4]
print("unpack", first, middle, last)
(a, b), c = (1, 2), 3
print("nested", a, b, c)
print("swap", (lambda x: x)((c, a, b)))
