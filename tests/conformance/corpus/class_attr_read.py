# 通过类对象读类属性（第 159 轮修的真 bug）：此前 C.x 报 AttributeError: 'type' object has no attribute 'x'
class C:
    x = 2

    def m(self):
        return self.x + 1


class D(C):
    y = 10


assert C.x == 2
assert D.y == 10
assert D.x == 2          # 继承来的类属性
assert C().m() == 3
assert D().m() == 3
assert D.m is not None


class E(metaclass=type):
    z = 7


assert E.z == 7
print("ok")
