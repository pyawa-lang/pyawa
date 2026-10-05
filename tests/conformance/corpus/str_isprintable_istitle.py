# **`str.isprintable` 与 `str.istitle`**（第 350 轮）：`Lib/` 里常用，先前一律 AttributeError。
# `istitle` 的口径照参照实测：**无大小写的字符（含数字）一律清掉"上一个有大小写"** ⇒
#   `"1A".istitle()` ⇒ True、`"A1b".istitle()` ⇒ False（第一版把数字当"不清"⇒ 误判 True，实测抓到）。
# **如实登记的偏差**：`isprintable` 按 Rust 的控制／空白判定，与参照的 Unicode 口径大致同，
# 个别字符（如 U+2028）可能不同。

print(str("".isprintable()))
print(str("a b".isprintable()))
print(str("a\tb".isprintable()))
print(str("\n".isprintable()))
print(str("A B".istitle()))
print(str("Ab Cd".istitle()))
print(str("ab cd".istitle()))
print(str("A1b".istitle()))
print(str("1A".istitle()))
print(str("Hello World".istitle()))
print(str("".istitle()))
print(str("123".istitle()))
