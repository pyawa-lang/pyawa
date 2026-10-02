# try／except（第 224 轮）：**裸 except**（ABI 实例还没有 builtins ⇒ 用不了 `except ValueError` 这类名字）
path = 0
try:
    x = 1 // 0
except:
    path = 1
y = path
# 嵌套：内层处理块里裸 `raise` 重抛 ⇒ 外层接到
depth = 0
try:
    try:
        x = 1 // 0
    except:
        depth = 1
        raise
except:
    depth = 2
z = depth
# 没有异常发生
clean = 0
try:
    clean = 7
except:
    clean = 9
v = clean
