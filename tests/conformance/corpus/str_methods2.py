# 字符串方法第二批（第 145 轮）：find / count / isdigit / isalpha / zfill / splitlines /
# removeprefix / removesuffix
assert "a,b,c".find("b") == 2
assert "abc".find("z") == -1
assert "a,b,c".count(",") == 2
assert "123".isdigit() is True
assert "12a".isdigit() is False
assert "abc".isalpha() is True
assert "a1".isalpha() is False
assert "7".zfill(3) == "007"
assert "abc".zfill(2) == "abc"
assert "a\nb".splitlines() == ["a", "b"]
assert "abcdef".removeprefix("abc") == "def"
assert "abcdef".removeprefix("zzz") == "abcdef"
assert "abcdef".removesuffix("def") == "abc"
print("ok")
