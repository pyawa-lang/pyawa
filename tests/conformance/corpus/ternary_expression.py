# 三元表达式：两个分支都要能被选到（语义；本层用 JUMP_FORWARD 汇合，参照复制余部）
a = True
b = False
x = 5 if a else 7
y = 5 if b else 7
z = 1 if a else (2 if b else 3)
w = (10 if b else 20) if a else 30
