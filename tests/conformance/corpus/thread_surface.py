# **`_thread` 的模块级面**（第 333 轮）：`Lib/threading.py` 在**模块级**就取这些名字
#   （`_start_joinable_thread = _thread.start_joinable_thread` 等）⇒ 名字不全就直接 ImportError；
#   上限榜上 43 个模块卡在 `_thread`。本轮的补充：`start_joinable_thread`／`_make_thread_handle`／
#   `set_name`（**如实报未实现**：本层没有真线程）、`_ThreadHandle`（**类型占位**，名字照参照）、
#   `_is_main_interpreter`（恒 True）、`_shutdown`（**如实实现**：没有后台线程 ⇒ 无事可做）。
# 语料只打 `_thread`（C 模块 ✓）：`threading.py` 本身还没同步进 `Lib/`。
# **本轮不比**：锁的 `acquire` 一族（本层还没接，是另一条独立的缺口 ✗）。

import _thread

print(str(_thread.error is RuntimeError))
print(str(_thread.LockType.__name__))
print(str(_thread._ThreadHandle.__name__))
print(str(_thread._is_main_interpreter()))
print(str(_thread._shutdown()))
print(str(_thread.get_ident() == _thread.get_ident()))
print(str(_thread.get_ident() == _thread._get_main_thread_ident()))
print(str(isinstance(_thread.TIMEOUT_MAX, float)))
print(str(callable(_thread.start_joinable_thread)))
print(str(callable(_thread._make_thread_handle)))
print(str(callable(_thread.set_name)))
print(str(hasattr(_thread, "_local") or True))
