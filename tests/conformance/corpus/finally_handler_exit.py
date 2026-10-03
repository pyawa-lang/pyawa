# try/except/finally：处理块里的 return 也必须先跑 finally
hits = [0]
def by_handler():
    try:
        x = 1 // 0
    except:
        return 1
    finally:
        hits[0] = hits[0] + 1
r = by_handler()
