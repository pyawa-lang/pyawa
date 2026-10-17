# `_thread` 的锁（第 280 轮；用户裁定 A：**VM 侧最小面**）。
#
# 依据：`SPEC-capabilities.md` §9.9 的 `ipc` 行自己写着"`thread_*` …线程语义归 VM 侧"；
# `DESIGN.md` §5 的挂起是**协作式**的 ⇒ 本层每实例单线程 ⇒ 锁就是 VM 内的记账。
# 这一条是 `importlib` 的前置：`_bootstrap._setup` 要 `_thread.RLock`／`allocate_lock`／`get_ident`。
#
# **不进语料**的部分（`MS-19` 的适用范围：没做的不进语料）：`start_new_thread` 一类**线程创建**
# （本层按 `CM-6` 如实报未实现）；`get_ident()` 的**取值**属 `MS-17` 的实现观测面（只断言"是整数、
# 两次一致"，不断言具体数字）。

import _thread

lock = _thread.allocate_lock()
print("unlocked" if not lock.locked() else "no")
print("acquired" if lock.acquire() else "no")
print("locked" if lock.locked() else "no")
print("nonblocking" if not lock.acquire(False) else "no")
lock.release()
print("released" if not lock.locked() else "no")
try:
    lock.release()
    print("no")
except RuntimeError as error:
    print("unlocked_release" if str(error) == "release unlocked lock" else "no")

rlock = _thread.RLock()
print("enter" if rlock.__enter__() is True else "no")
print("depth_one" if rlock._recursion_count() == 1 else "no")
print("owned" if rlock._is_owned() else "no")
rlock.acquire()
print("depth_two" if rlock._recursion_count() == 2 else "no")
saved = rlock._release_save()
print("saved_depth" if saved[0] == 2 else "no")
print("saved_unlocked" if not rlock.locked() else "no")
rlock._acquire_restore(saved)
print("restored" if rlock._recursion_count() == 2 else "no")
rlock.release()
print("after_release" if rlock._recursion_count() == 1 else "no")
print("exit_none" if rlock.__exit__(None, None, None) is None else "no")
print("depth_zero" if rlock._recursion_count() == 0 else "no")

print("type_lock" if type(lock).__name__ == "lock" else "no")
print("lock_type" if _thread.LockType is type(lock) else "no")
print("type_rlock" if _thread.RLock.__name__ == "RLock" else "no")
print("error_type" if _thread.error is RuntimeError else "no")
print("ident_int" if isinstance(_thread.get_ident(), int) else "no")
print("ident_stable" if _thread.get_ident() == _thread.get_ident() else "no")
print("timeout" if _thread.TIMEOUT_MAX > 0 else "no")
