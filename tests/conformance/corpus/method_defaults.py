# **方法默认值**（第 302 轮修 `P3-24`）：类体里 `def __init__(self, x, y=2)` 的默认值元组
# 先前**从没入常量池**（类作用域漏了 `flush_deferred`）⇒ `__defaults__` 是 `('Q',)`、
# 缺参调用拿到的是**类名**。这里把几档都钉住：`__defaults__` 本身、缺参调用、关键字调用、
# 仅关键字默认值、以及**嵌套函数**那条（它本来就对，防止回归）。
# **不属于本语料**：`@staticmethod` 取用（那是"在类型对象上取属性不做描述符绑定"的另一格，见 PLAN）。


class Q:
    def __init__(self, x, y=2, z=3):
        self.x = x
        self.y = y
        self.z = z

    def m(self, p=7, *, q=8):
        return str(p) + "," + str(q)


print(str(Q.__init__.__defaults__))
print(str(Q(1).x) + "," + str(Q(1).y) + "," + str(Q(1).z))
print(str(Q(1, 5, 6).y) + "," + str(Q(1, 5, 6).z))
print(str(Q(x=9).x) + "," + str(Q(x=9).y))
print(str(Q(1).m()))
print(str(Q(1).m(1, q=2)))
print(str(Q.m.__defaults__))
print(str(Q.__init__.__code__.co_argcount))
print(str(Q.__init__.__code__.co_varnames))


def outer():
    def inner(x, y=2):
        return x + y

    return inner


print(str(outer().__defaults__))
print(str(outer()(1)))
