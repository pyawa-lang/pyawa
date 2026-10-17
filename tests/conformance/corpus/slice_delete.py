# **切片删除**（第 311 轮）：`del x[a:b]`／`del x[a:b:c]` —— 与**切片写**同一套边界口径。
# 动因：`Lib/asyncio/base_events.py:173` 的 `del addrinfos_lists[0][:first - 1]`（那一族 35 个模块），
# 先前落到"下标必须是整数"那条错误上。

x = [1, 2, 3, 4, 5]
del x[1:3]
print(str(x))

y = [0, 1, 2, 3, 4, 5, 6]
del y[::2]
print(str(y))

z = [0, 1, 2, 3, 4, 5, 6]
del z[5:1:-1]
print(str(z))

w = [1, 2, 3]
del w[:]
print(str(w))

v = [1, 2, 3, 4]
del v[1:]
print(str(v))
del v[-1:]
print(str(v))

nested = [[1, 2, 3], [4, 5, 6]]
del nested[0][1:]
print(str(nested))

d = {"a": 1, "b": 2}
del d["a"]
print(str(d))

# **动态界**（`BUILD_SLICE` 那一档，`asyncio/base_events.py:173` 的形状）
x2 = [1, 2, 3, 4, 5]
n = 3
del x2[1:n]
print(str(x2))

nested2 = [[1, 2, 3], [4, 5, 6]]
m = 2
del nested2[0][:m - 1]
print(str(nested2))
del nested2[1][1:3:1]
print(str(nested2))

text = "abcdef"
try:
    del text[1:2]
except TypeError as error:
    print("TypeError: " + str(error))
