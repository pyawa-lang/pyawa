# 海象 `:=`（第 105／106 轮）：括号形式与**条件里的裸形式**都要 ✓
y = (z := 1)
assert z == 1
if (o := "x"):
    print("ok")
if p := "q":
    print(p)
