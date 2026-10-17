# **f-string 的空规格** `f"{x:}"`（第 286 轮真 bug 修复 ✗）：参照发
# `LOAD_CONST ''` ＋ `FORMAT_WITH_SPEC`（`dis` 实测）；先前**什么都不发** ⇒ 栈顶的**值本身**
# 被当成规格 ⇒ `TypeError: format spec must be a str`。

x = 5
print(str(f"{x:}"))
print(str(f"{x!r:}"))
print(str(f"{x:>5}"))
print(str(f"{x!r}"))
print(str(f"{x}"))
