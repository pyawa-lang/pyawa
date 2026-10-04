# 类级描述符访问（第 235 轮）：C.x 要走 __get__(None, C)
class R:
    def __get__(self, instance, owner=None):
        return "cls" if instance is None else "inst"


class E:
    r = R()


print(E.r)
print(E().r)
