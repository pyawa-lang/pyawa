# 裸 `return`（第 104 轮）：`LOAD_CONST None; RETURN_VALUE`，位点＝`return` 关键字 ✓
# 这条是上游 `importlib` 里到处都是的形态（第 108 行那句就是它 ✓）
def f():
    return

def g(x):
    if x:
        return
    return

f()
g(1)
g(0)
print("ok")
