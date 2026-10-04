# 元类那一层（第 232 轮）：元类型上的方法经"类"调用
class M(type):
    def hello(cls):
        return "hi-" + cls.__name__


class Base(metaclass=M):
    x = 1


print(Base.hello())
print(type(Base).__name__)
print(str(Base.x))
