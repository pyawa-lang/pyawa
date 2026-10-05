# **深递归如实报 `RecursionError`**（第 319 轮）：本层的"调用"是 **Rust 递归** ⇒ 先前不设限，
# Python 层的深递归会直接顶穿**原生栈**（对拍子进程当场 SIGSEGV ✗ —— 上限诊断里那族
# `子进程退出码 -11` × 29 就是这么来的）。现在按参照的做法在 **Python 层**设限 ✓
# （`MAX_CALL_DEPTH`，实测取 64：本层每层调用吃掉的 Rust 栈不少）并在超限时报
# `RecursionError: maximum recursion depth exceeded` ✓。
# 注：参照默认上限 1000、且可用 `sys.setrecursionlimit` 调；本层上限更低、该接口还没接
# （如实登记）⇒ 语料只走"很浅的递归能跑通"与"很深的递归如实报错"两侧。


def depth(n):
    if n == 0:
        return 0
    return depth(n - 1) + 1


print(str(depth(20)))

try:
    depth(100000)
except RecursionError as error:
    print("RecursionError: " + str(error))
