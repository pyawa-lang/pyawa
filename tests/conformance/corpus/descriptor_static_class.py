# **`staticmethod`／`classmethod` 的取用**（第 303 轮修 `P3-25`）：本层这两个包装对象先前
# **不参与描述符协议** ⇒ `Q.s` 拿到的是包装对象本身 ⇒ `'staticmethod' object is not callable`；
# 现在按参照语义拆开：`staticmethod` ⇒ 交回被包的函数；`classmethod` ⇒ 绑到**那个类**。


class Q:
    @staticmethod
    def s(a=1):
        return a + 10

    @classmethod
    def c(cls, b=2):
        return str(cls.__name__) + "," + str(b)

    def m(self, p=7):
        return p


print(str(Q.s()))
print(str(Q.s(5)))
print(str(Q().s(6)))
print(str(Q.c()))
print(str(Q.c(9)))
print(str(Q().c(9)))


class Sub(Q):
    pass


print(str(Sub.c(1)))
print(str(Q().m()))
print(str(Q().m(3)))
