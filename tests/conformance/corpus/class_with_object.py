# object 作为基类（第 132 轮）＋ 类型对象的 __name__（第 133 轮）
class Base(object):
    def m(self):
        return 7

    def who(self):
        return Base.__name__

assert Base().m() == 7
assert Base.__name__ == "Base"
assert Base().who() == "Base"
print("ok")
