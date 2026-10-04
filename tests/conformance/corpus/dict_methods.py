# dict 方法面（第 145 轮）：get / keys / values / items
d = {"a": 1, "b": 2}
assert d.get("a") == 1
assert d.get("zzz") is None
assert d.get("zzz", 7) == 7
assert sorted(d.keys()) == ["a", "b"]
assert sorted(d.values()) == [1, 2]
assert list(d.items()) == [("a", 1), ("b", 2)]
assert len(d.keys()) == 2
nums = {1: "x", 2: "y"}
assert nums.get(1) == "x"
assert nums.get(3, "none") == "none"
print("ok")
