# `if` 里的 try 吞掉作用域余部（第 202 轮真 bug 修复）
a = 1
if a:
    try:
        b = 2
    except ImportError:
        pass
print("done")
