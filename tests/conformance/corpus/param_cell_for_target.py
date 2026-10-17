# `for` 目标名被闭包捕获 ⇒ cell：循环变量必须走 STORE_DEREF（本会话 46b5f07 的第 ④ 处口径）。
def make():
    def read():
        return i

    for i in range(3):
        pass
    return read()


value = make()
