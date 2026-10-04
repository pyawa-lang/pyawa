# 行内体与单元素元组解包（第 188 轮）


def f(n):
    if n > 0: return "pos"
    else: return "neg"


print(f(1))
print(f(-1))
x, = [7]
print(str(x))
if 1: print("inline")
