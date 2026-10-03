# 第三批方法（第 154 轮）：str 补位/切分 + dict copy/clear/popitem + list clear
# 期望值全部先经参照核对（center 的补空格方向我一开始写反了，已按参照改正）
assert "a".rjust(3) == "  a"
assert "a".ljust(3) == "a  "
assert "a".center(5) == "  a  "
assert "ab".center(5) == "  ab "
assert "a".center(2) == "a "
assert "a-b".partition("-") == ("a", "-", "b")
assert "ab".partition("-") == ("ab", "", "")
assert "a,b,c".rsplit(",") == ["a", "b", "c"]
assert "a,b,c".rsplit(",")[0] == "a"
assert len("a,b,c".rsplit(",")) == 3
d = {"a": 1, "b": 2}
c = d.copy()
c["z"] = 9
assert len(d) == 2
assert len(c) == 3
assert d.get("z") is None
assert d.popitem() == ("b", 2)
assert len(d) == 1
d.clear()
assert len(d) == 0
items = [1, 2, 3]
items.clear()
assert items == []
print("ok")
