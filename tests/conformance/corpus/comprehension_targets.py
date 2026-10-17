# 推导式的**目标**（第 281 轮真 bug ✗）：`[a for a, b in …]` 与 `[a for (a, b) in …]` 先前**都**报
# "推导式的 `for <目标>` 后面要 `in`" —— 清单推导式那条把"目标只有一个词元"写死了 ✗
# （集合／字典推导式那两条用的是解析器交回的游标 ✓ ⇒ 同一形状在 `{…}` 里能跑、在 `[…]` 里不能 ✓）。
# 同一轮还补了两处：**带圆括号的目标**（`Lib/weakref.py:537` 的 `[(f,i) for (f,i) in …]` ✓）与
# **任意项数**的元组目标（先前写死"只接线两项" ✗）。
#
# 分界照参照实测：`(x)` 是**名字**、`(x,)` 是**一项的元组**（要拆包 ✓）。

pairs = [(1, 2), (3, 4)]
print(str([f for f, i in pairs]))
print(str([f for (f, i) in pairs]))
print(str([f for (f, i) in pairs if i > 2]))
print(str([x for (x,) in [(7,)]]))
print(str([a + b + c for a, b, c in [(1, 2, 3)]]))
print(str([a + d for (a, b, c, d) in [(1, 2, 3, 4)]]))
print(str({k for k, v in pairs}))
print(str(sorted({k: v for (k, v) in pairs})))
print(str(list(k for (k, v) in pairs)))
print(str([k for (k, v) in pairs if v > 2]))
