# 零参 super()（第 233 轮）：沿 MRO 取"定义类之后"的那个同名方法
class A:
    def hi(self):
        return "A"


class B(A):
    def hi(self):
        return super().hi() + "B"


class C(B):
    def hi(self):
        return super().hi() + "C"


print(C().hi())
