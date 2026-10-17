# boolop 直接操作数快路（本会话 46b5f07 第 ⑤ 处）：`and` 的操作数是 cell ⇒ 必须 LOAD_DEREF。
def make(x):
    def read():
        return x

    return read() and x


value = make(1)
