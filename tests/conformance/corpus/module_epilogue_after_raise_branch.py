# 模块收尾漏发（第 199 轮真 bug 修复）：else 终止而 then 能落下来 ⇒ 收尾照样要发
a = 1
if a:
    x = 1
elif a:
    x = 2
else:
    raise ValueError('x')
print(str(x))
