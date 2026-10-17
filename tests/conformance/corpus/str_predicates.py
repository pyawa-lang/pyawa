# **`str` 的一批谓词与变换**（第 344 轮）：`isupper`／`islower`／`isnumeric`／`isdecimal`／
# `isalnum`／`swapcase`／`casefold`／`expandtabs` —— `Lib/` 里到处都是，先前一律 AttributeError。
# **如实登记的偏差**：`isnumeric` 按 Rust 的 `char::is_numeric`（与 Unicode Nd/Nl/No 大致同口径，
# 个别字符可能不同）；`casefold` 按 `to_lowercase` ＋ 补一条最常见的展开 `"ß"` ⇒ `"ss"`
# ⇒ 语料只用常见形态。

print(str("A1".isupper()))
print(str("1".isupper()))
print(str("ABC".isupper()))
print(str("a1".islower()))
print(str("A1".islower()))
print(str("abc".islower()))
print(str("12".isnumeric()))
print(str("1.5".isnumeric()))
print(str("".isnumeric()))
print(str("12".isdecimal()))
print(str("1.5".isdecimal()))
print(str("a1".isalnum()))
print(str("a_1".isalnum()))
print(str("12".isalnum()))
print(str("aB1".swapcase()))
print(str("ABC".swapcase()))
print(str("abc".swapcase()))
print(str("ABC".casefold()))
print(str("Ünïcode".casefold()))
print(repr("a\tb".expandtabs(4)))
print(repr("ab\tc".expandtabs(4)))
print(repr("a\tb".expandtabs()))
print(repr("\t".expandtabs(3)))
print(repr("no-tabs".expandtabs(4)))
