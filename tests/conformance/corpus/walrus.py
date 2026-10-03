# 海象 `:=`（第 105 轮）：值留在栈上、同时写目标 ✓（`COPY 1; STORE_NAME`，位点＝海象跨度／目标名 ✓）
y = (z := 1)
assert z == 1
if (o := "x"):
    print("ok")
