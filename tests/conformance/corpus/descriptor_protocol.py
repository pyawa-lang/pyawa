# 描述符协议（第 192 轮）：__get__ / __set__ / __delete__（数据描述符优先于实例字典）
class D:
    def __init__(self):
        self.value = 1

    def __get__(self, instance, owner=None):
        return self.value

    def __set__(self, instance, value):
        self.value = value

    def __delete__(self, instance):
        self.value = 0


class C:
    x = D()


obj = C()
assert obj.x == 1
obj.x = 7
assert obj.x == 7
assert C.x == 7
del obj.x
assert obj.x == 0


# 只读描述符（只有 __get__）
class R:
    def __get__(self, instance, owner=None):
        return "readonly"


class E:
    r = R()


assert E().r == "readonly"
assert E.r == "readonly"
print("ok")
