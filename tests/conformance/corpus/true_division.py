# `/` 的结果是 **float**（参照实测 `7/2 == 3.5`）——本轮的表达式面 ＋ harness 的浮点渲染
a = 7
b = 2
x = a / b
y = a // b
z = a % b
w = a ** b
v = ~a
