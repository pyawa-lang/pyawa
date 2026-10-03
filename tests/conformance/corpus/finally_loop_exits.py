# try/finally 的循环出口：continue 也必须先跑 finally（`hits[1]` 观测副作用）
hits = [0, 0]
def by_continue():
    n = 0
    while n < 2:
        n = n + 1
        try:
            continue
        finally:
            hits[1] = hits[1] + 1
    return n
c = by_continue()
