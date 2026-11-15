#!/usr/bin/env python3
"""**解释器热点微基准**（round 175）：属性读／属性写／内建调用／空循环四档。

用法：
    ./target/release/pyawa tools/bench_hot.py          # 本体是 pyawa 脚本，直接跑
    python3 tools/bench_hot.py --emit                  # 只打印下面的探针源码

口径：**同一台机器上相对比较**（绝对值无意义 ✓）；每档跑 2 000 000 次，报告墙钟。
为什么是 pyawa 脚本而不是 python 脚本：要量的是**本层**的 `LOAD_ATTR`／`CALL`／`BINARY_OP`
派发成本 ✓（用宿主 python 跑就量不到 ✓）。
"""

PROBE = '''import time

class C:
    def __init__(self):
        self.x = 1

o = C()

def measure(title, body):
    start = time.time()
    body()
    print(title, round(time.time() - start, 3))

def baseline():
    n = 0
    for _ in range(2000000):
        n += 1
    return n

def attribute_read():
    n = 0
    for _ in range(2000000):
        n += o.x
    return n

def attribute_write():
    for _ in range(2000000):
        o.x = 2

def builtin_call():
    n = 0
    for _ in range(2000000):
        n += len([1])
    return n

measure('baseline        ', baseline)
measure('attribute_read  ', attribute_read)
measure('attribute_write ', attribute_write)
measure('builtin_call    ', builtin_call)
'''

if __name__ == "__main__":
    import sys

    if "--emit" in sys.argv:
        print(PROBE)
    else:
        exec(compile(PROBE, "tools/bench_hot.py", "exec"), {"__name__": "__main__"})
