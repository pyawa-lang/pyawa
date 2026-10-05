# **链式比较里的 `is`／`in`**（第 327 轮）：这两个族**没有** `COMPARE_OP` 的 oparg，而链式比较的
# 两条发射路径都直接 `.expect()` 取 oparg ⇒ **内部 panic** ✗（实测 `if a == b is c:` ✓）——
# 上限诊断里"状态 1、无 errmsg"那一族 29 个模块的真身就是它 ✓（本轮给 panic 文本做了回填，
# 它才现出原形 ✓）。口径与 `emit_compare` 一致：`is`⇒`IS_OP 0`、`is not`⇒`IS_OP 1`、
# `in`⇒`CONTAINS_OP 0`、`not in`⇒`CONTAINS_OP 1`。
# 注：**带 `else` 的链式比较条件**是另一条独立的缺口（会把后面的语句整段吞掉 ✗）⇒ 不进语料，
# 已如实记在台账里。

a = 1
b = 1
c = 1
x = [1, 2]

if a == b is c:
    print("eq-is ok")
if x is x and a in x:
    print("and ok")
if a is b is c:
    print("is-is-is ok")
if a is not b:
    print("isnot ok")
if a not in []:
    print("notin ok")
if 1 < 2 < 3 < 4:
    print("chain4 ok")
print(str(a))
