# 赋值右值同名快路（本会话 46b5f07 第 ③ 处）：右值里的名字是 cell ⇒ 必须 LOAD_DEREF。
def make(cls):
    p = cls

    def read():
        return p

    return read()


value = make("x")
