# 链式比较（第 262 轮）：结果语义（不放函数调用——带调用/下标的形态在当前实现下会 StackUnderflow，已立案）
a = 1
b = 2
c = 3
ok = a < b < c
bad_left = 5 < b < c
bad_right = a < b < 1
mixed = a == 1 != c
two = a < b
