# try/finally 的循环出口：continue／break 都必须先跑 finally（`hits` 观测副作用）
hits = [0, 0, 0]
def by_continue():
    n = 0
    while n < 2:
        n = n + 1
        try:
            continue
        finally:
            hits[1] = hits[1] + 1
    return n
def by_break():
    n = 0
    while n < 5:
        n = n + 1
        try:
            break
        finally:
            hits[0] = hits[0] + 1
    return n
def nested():
    try:
        try:
            return 7
        finally:
            hits[2] = hits[2] + 2
    finally:
        hits[2] = hits[2] + 1
c = by_continue()
b = by_break()
d = nested()
