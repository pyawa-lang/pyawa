# **`str.translate`**（第 351 轮）：`str.maketrans` 第 313 轮就接了 ✓，对拍时才发现 `translate` 缺 ✗。
# 口径照参照逐条量过：表按**码位**（int）查 —— 查不到 ⇒ 原字符留下；查到 `None` ⇒ 删掉；
# 查到 `str` ⇒ 换上去；查到 `int` ⇒ 换成那个码位。

t = str.maketrans("ab", "xy")
print(repr(t))
print(repr("abcab".translate(t)))
t2 = str.maketrans({"a": "X", "b": None})
print(repr(t2))
print(repr("abcab".translate(t2)))
print(repr("abc".translate({})))
t3 = str.maketrans("a", "Z", "b")
print(repr("abcab".translate(t3)))
print(repr("你好".translate({})))
