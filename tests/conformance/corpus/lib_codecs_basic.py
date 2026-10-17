# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/codecs.py`（编码查找面）。
# 注：`codecs.encode`／`decode` 直调在 `_codecs` 那边尚未接线 ⇒ 不收它们（`MS-19`）。
import codecs

name = codecs.lookup("utf-8").name
ascii_name = codecs.lookup("ascii").name
