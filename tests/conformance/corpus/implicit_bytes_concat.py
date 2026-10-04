# **bytes 的隐式拼接**（第 314 轮）：与字符串同一套口径 —— 相邻 `b"…"` 合成一个常量。
# 动因：`Lib/base64.py:437` 的
#   _b85alphabet = (b"0123456789…"
#                   b"abcdef…")
# 跨行相邻 ⇒ 先前不并 ⇒ 括号那组见到第二个 bytes 字面量 ⇒
# 报"括号没有闭合，实际 Some(Bytes([…]))"（那一族 10 个模块）。

x = (b"0123456789"
     b"abcdefghijklmnopqrstuvwxyz")
print(str(len(x)))
print(str(x[:10]))
print(str(x[10:]))

y = b"a" b"b" b"c"
print(str(y))

print(str(b"x" + b"y"))
print(str((b"abc", b"def")))
print(str(b"abc".decode()))
print(str(len(b"" b"")))

s = ("ab"
     "cd")
print(str(s))
print(str("a" "b"))
