# 语言：dict 插入序与更新，集合输出前排序
d = {}
for key in ["b", "a", "c"]:
    d[key] = len(key)
d["a"] = 99
print("keys", list(d))
print("values", list(d.values()))
print("items", list(d.items()))
print("pop", d.pop("b"), list(d))
print("sorted", sorted(d))
print("get", d.get("missing", "default"))
