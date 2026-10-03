# 切片赋值（第 175 轮）：两段 ⇒ STORE_SLICE；三段 ⇒ BUILD_SLICE 3 + STORE_SUBSCR
xs = [0, 1, 2, 3, 4]
xs[1:3] = [9, 9]
assert xs == [0, 9, 9, 3, 4]

ys = [0, 1, 2, 3, 4, 5]
ys[0:4:2] = [7, 8]
assert ys == [7, 1, 8, 3, 4, 5]

zs = [1, 2, 3]
zs[1:] = [5]
assert zs == [1, 5]

tail = [1, 2, 3, 4]
tail[:2] = []
assert tail == [3, 4]


# async def：只**定义**、不调用（本层把 async 当透明修饰符 ⇒ 只有调用才看得出差别，已登记）
async def _c(): pass


def _d(): return 11


assert _d() == 11
print("ok")
