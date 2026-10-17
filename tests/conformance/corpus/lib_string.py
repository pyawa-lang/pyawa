# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/string.py`（常量面）。
# 注：`string.capwords` 要 `str.capitalize`，本层尚未接线 ⇒ 不进语料（未实现不是差异，`MS-19`）。
import string

digits = string.digits
letters = string.ascii_letters
punct = string.punctuation
hexd = string.hexdigits
octd = string.octdigits
