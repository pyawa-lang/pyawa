# 闭包里的海象目标（第 119 轮）：目标是 cell／自由变量 ⇒ `STORE_DEREF` ✓
def outer():
    x = 0
    def inner():
        return x
    if (x := 5):
        y = 1
    return inner()

assert outer() == 5
print("ok")
