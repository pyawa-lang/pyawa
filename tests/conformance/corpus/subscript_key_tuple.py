# **下标里的元组键**（第 283 轮）：`a[i, j]` 等价于 `a[(i, j)]`，尾随逗号 `a[i,]` ⇒ 一项的元组。
# 先前只认单一项 ⇒ `re/_parser.py:335` 的 `_cache2[type(pattern), pattern, flags]` 报
# `` `[` 之后要 `]`，实际 Some(Comma) ``，`re` 那一族 28 个模块都卡在这条上。

d = {(1, 2): 3, (1,): 9, (4, 5): 6}
print(str(d[1, 2]))
print(str(d[1,]))
print(str(d[(4, 5)]))

cache = {}
cache["a", "b"] = 1
print(str(cache["a", "b"]))
print(str(len(cache)))

s = "abcdef"
print(str(s[1:3]))
print(str(s[0]))
