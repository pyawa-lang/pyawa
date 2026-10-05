# **`str` 的查找／切分四件**（第 349 轮）：`rfind`／`index`／`rindex`／`rpartition` ——
# 与既有的 `find`／`partition` 同源，`Lib/` 里常用，先前一律 AttributeError。


print(str("banana".rfind("an")))
print(str("banana".find("an")))
print(str("banana".index("an")))
print(str("banana".rindex("an")))
print(str("banana".rfind("zz")))
print(str("banana".find("zz")))
print(repr("a-b-c".rpartition("-")))
print(repr("abc".rpartition("-")))
print(repr("a-b-c".partition("-")))
print(repr("".rpartition("-")))
try:
    "banana".index("zz")
except ValueError as error:
    print("index: " + str(error))
try:
    "banana".rindex("zz")
except ValueError as error:
    print("rindex: " + str(error))
