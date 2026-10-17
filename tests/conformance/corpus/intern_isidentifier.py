# **`sys.intern` ＋ `str.isidentifier`／`str.isascii`**（第 335 轮）：上限榜上那 70 个模块沿着
# `map` → `sys.intern` → `str.isidentifier` 一路往下走，后两条各挡了它们一程。
# **如实登记的偏差**：`sys.intern` 本层没有驻留池 ⇒ 返回同一个实参对象（不保证参照的
# `sys.intern(a) is sys.intern(b)` 同一性）；`isidentifier` 按"首字符字母或 `_`、其余字母数字或 `_`"
# 判，参照按 Unicode `XID_Start`／`XID_Continue` ⇒ 少数边缘字符会不同 ⇒ 语料只用常见形态。

import sys

print(str(sys.intern("abc")))
print(str(sys.intern("abc") == "abc"))
print(str(type(sys.intern("x")).__name__))
print(str("abc".isidentifier()))
print(str("_a1".isidentifier()))
print(str("1a".isidentifier()))
print(str("".isidentifier()))
print(str("a b".isidentifier()))
print(str("abc".isascii()))
print(str("é".isascii()))
print(str("".isascii()))
