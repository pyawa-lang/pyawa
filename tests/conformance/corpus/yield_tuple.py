# 裸 yield 接元组（第 138 轮）：Lib/os.py:418 的 `yield top, dirs, nondirs` 此前编不过 ✗
def gen(a, b):
    yield a, b
    yield b, a

got = list(gen(1, 2))
assert got == [(1, 2), (2, 1)]
print("ok")
