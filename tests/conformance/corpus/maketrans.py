# **`str.maketrans`／`bytes.maketrans`**（第 313 轮）：上限榜上
# `'type' object has no attribute 'maketrans'` × 67 个模块的卡点。
# 两个都在**类型对象**上取（静态用法 ⇒ 没有接收者），口径照参照实测：
# 一个字典实参 ⇒ 逐条拷（单字符键折成序号）；两个等长字符串 ⇒ 逐位配对；
# 第三个字符串 ⇒ 映射到 None（删除）；bytes 那支给 256 字节的查表。

print(str(str.maketrans("ab", "cd")))
print(str(str.maketrans({"a": "x", 98: "y"})))
print(str(str.maketrans("ab", "cd", "e")))
# 注：`sorted(...)` 比到 `(int, None)` 那一对时，参照**如实报**
# `TypeError: '<' not supported between instances of ...`，本层的元组比较**还没**
# 透传这个错（另一条线 ✗）⇒ 这一格不进语料。
print(str(len(str.maketrans("ab", "cd", "e"))))

table = bytes.maketrans(b"ab", b"cd")
print(str(len(table)))
print(str(table[97]) + "," + str(table[98]) + "," + str(table[99]))
# `bytes(<range>)` 那支还没接线（另一条线）⇒ 这里用 list 形态 ✓
print(str(bytes.maketrans(b"", b"") == bytes(list(range(256)))))

try:
    str.maketrans()
except TypeError as error:
    print("TypeError: " + str(error))
try:
    str.maketrans("ab", "cde")
except ValueError as error:
    print("ValueError: " + str(error))
try:
    bytes.maketrans(b"ab", b"c")
except ValueError as error:
    print("ValueError2: " + str(error))
