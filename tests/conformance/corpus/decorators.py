# 装饰器（第 102／103 轮）：编译面逐字节对齐（夹具四条 ✓），这里守**运行期**真跑 ✓
# 形态：单条装饰器 ⇒ 建函数 ⇒ 逆序 `CALL 0` 裹上 ⇒ 再存储；内层单元的起始行取装饰器那行 ✓
def deco(function):
    return function

@deco
def f():
    return 1

assert f() == 1
print("ok")
