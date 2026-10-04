# **`time` 模块**（第 317 轮）：经 **`clock` 域**（`SPEC-capabilities.md` §4 第四项）取钟 ——
# 本段落地 `time()`／`time_ns()`／`monotonic()`／`monotonic_ns()`／`perf_counter*`。
# 动因：上限诊断里 `ModuleNotFoundError: No module named 'time'` × 54 个模块（本层最大一族）。
# 这一格只比**性质**（类型、非负、单调不减），不比具体数值（两侧当然不同）；`timezone`／`tzname`
# 本层按 UTC 假定给值（**已登记的偏差**），`sleep` 的 `sleep_ns` 槽位未提供 ⇒ 如实报未实现，
# 两者都**不进语料**。

import time

now = time.time()
print(str(isinstance(now, float)))
print(str(now > 0))

nano = time.time_ns()
print(str(isinstance(nano, int)))
print(str(nano > 0))
# 注：本想比 `abs(nano / 1e9 - now) < 5`，但本层的**浮点四则**是另一条独立的缺口
# （`1.5 + 0.5` 报 `unsupported operand type(s) for +: 'float' and 'float'`，用 stash 验证过
# 它**先于本轮**就坏 ✓）⇒ 那一格不进语料，已如实记在台账里。
print(str(nano // 1_000_000_000 >= 1))

first = time.monotonic()
second = time.monotonic()
print(str(isinstance(first, float)))
print(str(second >= first))

print(str(isinstance(time.monotonic_ns(), int)))
print(str(time.monotonic_ns() >= 0))
print(str(isinstance(time.perf_counter(), float)))
print(str(time.perf_counter_ns() >= 0))
