# try/finally：finally 必须在 return 之前跑（语义；`hits[0]` 观测副作用）
hits = [0]
def f():
    try:
        return 1
    finally:
        hits[0] = hits[0] + 1
r = f()
