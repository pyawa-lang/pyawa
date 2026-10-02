# break／continue（第 223 轮）：语义按参照（指令流的块结构对齐待模型）
total = 0
for i in [1, 2, 3, 4, 5]:
    if i == 4:
        break
    if i == 2:
        continue
    total += i
x = total
# `break` 跳过 `else`；正常耗尽才跑 `else`
hit = 0
for i in [1, 2]:
    break
else:
    hit = 1
y = hit
# `while` 里的 continue（回到条件）
n = 0
seen = 0
while n < 6:
    n += 1
    if n == 3:
        continue
    seen += n
z = seen
