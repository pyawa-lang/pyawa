# **f-string 的两件事**（第 286 轮，两侧逐字比）：
# ① **调试形态** `f"{表达式=}"`：正文是段内原文（`=` 前后空白都留）、表达式去掉首尾空白、
#    没写转换时默认 `!r`（参照逐条实测：`f"{x = }"` ⇒ `x = 5`、`f"{s=!s}"` ⇒ `s=hi`）；
# ② **空规格** `f"{x:}"`：参照发 `LOAD_CONST ''` ＋ `FORMAT_WITH_SPEC` —— 先前什么都不发
#    ⇒ 栈顶的值被当成规格 ⇒ `TypeError: format spec must be a str`。

x = 5
s = "hi"
d = {"k": 1}
print(str(f"{x=}"))
print(str(f"{x = }"))
print(str(f"{ x = }"))
print(str(f"{x=  }"))
print(str(f"{x=:>5}"))
print(str(f"{s=!s}"))
print(str(f"{x=} {s=}"))
print(str(f"{x=:}"))
print(str(f"{x=!a}"))
print(str(f"{(x)=}"))
print(str(f"{d['k']=}"))
print(str(f"{x==5=}"))
print(str(f"{x if x else 0=}"))
