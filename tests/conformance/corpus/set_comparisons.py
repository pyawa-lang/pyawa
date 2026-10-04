# 集合比较（第 205 轮）：子集／超集运算符（`Lib/os.py` 的 `_have_functions` 用它）
a = {1, 2}
b = {1, 2, 3}

print(str(a <= b))
print(str(a < b))
print(str(b >= a))
print(str(b > a))
print(str(a == {2, 1}))
print(str(a != b))
