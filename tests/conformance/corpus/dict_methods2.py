# dict 方法第二批（第 147 轮）：update / setdefault / pop
d = {"a": 1}
d.update({"a": 2, "b": 3})
assert d.get("a") == 2
assert d.get("b") == 3
assert len(d) == 2
e = {}
assert e.setdefault("k", 10) == 10
assert e.setdefault("k", 20) == 10
assert len(e) == 1
f = {"x": 5}
assert f.pop("x") == 5
assert len(f) == 0
assert f.pop("missing", 7) == 7
g = {"z": 1}
g.update({})
assert len(g) == 1
print("ok")
