# **`dict.setdefault` 的引用账**（第 297 轮真 bug 修复）：返回值那一份要**自己 retain**。
# 先前 `setdefault(key, 默认值)` 直接返回借来的实参 ⇒ 调用方释放结果时把**字典里那一份**也放掉
# ⇒ 值在仍在字典里时就被释放（`PYAWA_QUARANTINE=1` 会当场报"对已释放对象 incref"）。
# `Lib/enum.py` 的 `classdict.setdefault('_ignore_', []).append('_ignore_')` 正是这一手。

d = {}
d.setdefault("k", [])
print(str(len(d["k"])))
d["k"].append(1)
print(str(d["k"]))

e = {}
v = e.setdefault("m", [])
v.append(2)
print(str(e["m"]))
print(str(e.setdefault("m", [99])))
print(str(e["m"]))

f = {"a": 1}
print(str(f.setdefault("a", 5)))
print(str(f.setdefault("b", 6)))
print(str(len(f)))
