# **`print` 接受任意对象**（第 330 轮）：`print(1)`、`print([1, 2])`、`print(None)` 这些是 `Lib/` 里
# 遍地都是的写法，先前一律如实拒绝（消息还写着"`str()` 落地前"，而 `str()` 早就落地了）。
# 现在非 `str` 实参走**核心的同一处渲染**（`Instance::object_str_native`，与 `str(x)` 同一条路）。

print(1)
print(None)
print(True)
print(False)
print([1, 2])
print({"a": 1})
print(1.5)
print(1.0)
print((1, 2))
print()
print("x", 1)
print(str([1, 2]) == "[1, 2]")
print(-0.0)
