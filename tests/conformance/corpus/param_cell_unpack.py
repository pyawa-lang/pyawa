# 形参 cell 一族的程序级回归（本会话 46b5f07 修）：元组解包给**形参名**（被闭包捕获 ⇒ cell）
# 必须走 STORE_DEREF，不能按 varnames 判成 STORE_FAST。
def make(cls):
    a, cls = (7, 8)

    def read():
        return cls

    return a, read


a, read = make(1)
first = a
second = read()
