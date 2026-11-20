# 语言：字符串、Unicode、编码与格式化
s = "héllo, 世界"
print("len", len(s))
print("plain", s)
print("slice", s[1:5])
print("upper", s.upper()[:5])
print("fmt", "{}|{:>4}|{!r}".format(1, "ab", "q"))
print("enc", s.encode("utf-8").hex())
print("dec", s.encode("utf-8").decode("utf-8") == s)
print("repr", repr("a\tb"))
