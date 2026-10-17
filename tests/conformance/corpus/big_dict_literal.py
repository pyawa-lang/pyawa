# **16 对以上的字典字面量**（第 282 轮）：参照在这里改用**增量**形态 ——
# `BUILD_MAP 0` ＋ 每对 `key; value; MAP_ADD 1`（阈值 15／16 逐条 `dis` 实测）。
# 本层先前一律 `BUILD_MAP n` ⇒ `n > 255` 直接报"尚未接线" ⇒ `Lib/encodings/aliases.py`
# 那个 ~500 对的表被挡在门外（它又压着 `encodings` 一族 123 个模块）。
#
# 15 对及以下**形状不变**（夹具守着逐字节一致）；这条语料守的是**语义**与两边一致。

small = {0: 0, 1: 1, 2: 2, 3: 3, 4: 4, 5: 5, 6: 6, 7: 7, 8: 8, 9: 9, 10: 10, 11: 11, 12: 12, 13: 13, 14: 14}
print(str(len(small)))
print(str(small[14]))

big = {
    0: "a0", 1: "a1", 2: "a2", 3: "a3", 4: "a4", 5: "a5", 6: "a6", 7: "a7",
    8: "a8", 9: "a9", 10: "a10", 11: "a11", 12: "a12", 13: "a13", 14: "a14", 15: "a15",
    16: "a16", 17: "a17", 18: "a18", 19: "a19", 20: "a20", 21: "a21", 22: "a22", 23: "a23",
}
print(str(len(big)))
print(big[0])
print(big[23])

huge = {}
for index in range(400):
    huge[index] = index * 2
print(str(len(huge)))
print(str(huge[399]))
