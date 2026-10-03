# 字符串方法面（第 143 轮）：Lib/ 里每个文件都在用；此前整片是空的 ✗
assert "a".upper() == "A"
assert "A".lower() == "a"
assert " a ".strip() == "a"
assert "ab".startswith("a") is True
assert "ab".startswith("b") is False
assert "ab".endswith("b") is True
assert ",".join(["a", "b"]) == "a,b"
assert ",".join([]) == ""
assert "a,b".split(",") == ["a", "b"]
assert "a b".split() == ["a", "b"]
assert "ab".replace("a", "c") == "cb"
assert "os".upper().startswith("O") is True
assert "/a/b".split("/") == ["", "a", "b"]
print("ok")
