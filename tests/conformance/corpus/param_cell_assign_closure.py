# 形参 cell：先被闭包捕获、随后重新赋值 ⇒ 必须是 STORE_DEREF（写回 cell 而不是局部槽）。
def make(x):
    def read():
        return x

    x = 5
    return read()


value = make(1)
