# f-string 字面段里的转义（第 251 轮）：源偏移映射后位点与语义都要对
# 探针一律布尔（值里含换行会打断行式协议）
name = "x"
one = f"a\n{name}"
one_ok = one == "a\nx"
two = f"\t{name}\t"
two_ok = two == "\tx\t"
three = f"\x41{name}"
three_ok = three == "Ax"
raw = rf"a\n{name}"
raw_ok = raw == "a\\nx"
