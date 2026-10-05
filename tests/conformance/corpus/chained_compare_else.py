# **链式比较条件带 `else`**（第 328 轮）：`if a < b <= c: … else: …` 之后再接语句 —— 先前
# 假出口会直接落到"**收尾副本**"（`POP_TOP; LOAD_CONST None; RETURN_VALUE`）⇒ 后面的语句被整段
# 吞掉（什么都不打印、退出码 0）。根因是"链式比较当条件"那条特化无条件启用，而它**只在
# `if` 处于尾位且没有 `else`** 时才成立。现在用已有的 `collect_condition_exits` 当门。

a = 1
b = 1
c = 1
if a < b <= c:
    print("then")
else:
    print("else")
print("after")

x = [1, 2]
if 0 < a in x:
    print("chain-in then")
else:
    print("chain-in else")
print("after2")

if a < b < c:
    print("then3")
print("after3")
