# CALL_FUNCTION_EX（第 149 轮）：* 解包此前只认 tuple，列表／生成器都报未接线 ✗
def add(a, b):
    return a + b


def add3(a, b, c=0):
    return a + b + c


def kwonly(**kw):
    return kw["x"]


args = [1, 2]
assert add(*args) == 3
assert add(*(1, 2)) == 3
assert add(*(i for i in (1, 2))) == 3
assert add3(*[1, 2], **{"c": 3}) == 6
assert add3(*args) == 3
assert kwonly(x=1) == 1
assert add(1, **{"b": 2}) == 3
assert add3(*args, **{"c": 4}) == 7
assert kwonly(**{"x": 3}) == 3
print("ok")
