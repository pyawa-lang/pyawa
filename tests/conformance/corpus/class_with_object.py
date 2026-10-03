# object 作为基类（第 132 轮）：_bootstrap.py 里常见 `class X(object)` 写法 ✓
class Base(object):
    def m(self):
        return 7

assert Base().m() == 7
print("ok")
