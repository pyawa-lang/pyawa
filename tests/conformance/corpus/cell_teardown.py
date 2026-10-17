# **cell 的所有权**（第 334 轮真 bug 修）：`MAKE_CELL` 先前用 `raw_local` **借用**同号局部槽的值 ✗
# ⇒ 同一份引用**两边都算持有**（局部数组一份、新 cell 一份）⇒ 帧收尾释放局部那份 ＋
# `cell_clear` 释放 cell 那份 ⇒ **同一份引用被减两次** ✗ ⇒ 释放后使用 ✓
# （实测：`import threading` ⇒ `对已释放对象 decref：类型 list` ✓，回溯落点 `cell_clear` ✓；
#  而本文件回退那一处改动后，两个 `print` 打出来的是 `g` / `h` —— 读出已被释放的对象 ✓）。
# 修法：`MAKE_CELL` 把局部那份**取走**（`set_local(slot, None)` ✓）⇒ 所有权只剩一份 ✓。

def outer(data):
    def inner():
        return data
    return inner


g = outer([1, 2])
print(str(g()))
h = outer({"a": 1})
print(str(h()))
print(str(outer("text")()))
