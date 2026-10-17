# **字典显示里的 `**` 解包**（第 293 轮）：`{**a, **b}`／`{"z": 0, **a, "w": 3}`。
# 参照 `dis` 实测：每串连续键值对一条 `BUILD_MAP n`、每个 `**` 一条 `LOAD; DICT_UPDATE 1`
# （`Lib/functools.py:345` 的 `{**func.keywords, **keywords}` 就是它，那一族跨 functools／logging／
# statistics／`_py_warnings`）。纯键值对（没有 `**`）的形状**一字不动**。

a = {"x": 1}
b = {"y": 2}
c = {**a, **b}
print(str(len(c)))
print(str(c["x"]) + str(c["y"]))

d = {"z": 0, **a, "w": 3}
print(str(len(d)))
print(str(d["z"]) + str(d["x"]) + str(d["w"]))

e = {}
print(str(len(e)))

f = {**a}
print(str(f["x"]))

big = {
    0: "a0", 1: "a1", 2: "a2", 3: "a3", 4: "a4", 5: "a5", 6: "a6", 7: "a7",
    8: "a8", 9: "a9", 10: "a10", 11: "a11", 12: "a12", 13: "a13", 14: "a14", 15: "a15",
}
print(str(len(big)))
print(str(big[15]))
g = {**big, **a}
print(str(len(g)))

# 后面的键覆盖前面的（参照口径）
h = {**a, "x": 9}
print(str(h["x"]))
