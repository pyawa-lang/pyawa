# 行内 suite 的程序级用例（本会话 6f53a26 修了"源末无换行"的 panic；corpus harness 会补换行，
# 所以这里测的是行内体本身的语义，不是那一格 panic）。
total = 0
for i in range(3): total = total + i
after = total
if total == 3:
    flag = "three"
else:
    flag = "other"
