# **`br'…'`／`rb'…'`（原始 bytes）**（第 291 轮）：反斜杠原样留下、**不解码**。
# `Lib/glob.py:283` 的 `magic_check_bytes.sub(br'[\1]', pathname)` 就是它 ——
# 先前 `br` 被当成**名字**、后面那个字符串落进实参表 ⇒ 报"实参表里出现 `Some(Str(…))`"
# （`glob` 那一族 15 个模块）。

print(str(br'[\1]' == b'[\\1]'))
print(str(rb'[\1]' == b'[\\1]'))
print(str(len(br'[\1]')))
print(str(len(br"a\tb")))
print(str(br'a\tb' == b'a\\tb'))
print(str(len(b'A\x42')))
print(str(b'caf\xc3\xa9'))
