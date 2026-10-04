# 字符串转义（第 250 轮）：常见转义、数字转义、行继续、原始字符串
# 探针一律是**布尔**（值里含换行会打断行式对拍协议）
newline = "a\nb"
newline_ok = newline == "a\nb"
tab = "a\tb"
tab_ok = tab == "a\tb"
hexed = "\x41\x42"
hexed_ok = hexed == "AB"
unicode_name = "\u0041"
unicode_ok = unicode_name == "A"
octal = "\103"
octal_ok = octal == "C"
escaped_quote = "\"q\""
quote_ok = escaped_quote == "\"q\""
continued = "a\
b"
continued_ok = continued == "ab"
raw = r"a\nb"
raw_ok = raw == "a\\nb"
