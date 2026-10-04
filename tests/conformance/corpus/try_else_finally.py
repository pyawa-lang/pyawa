# try/except/else/finally（第 246 轮）：三条路都要对
def run(flag):
    events = 0
    try:
        events = events + 1
        if flag:
            x = 1 // 0
    except:
        events = events + 10
    else:
        events = events + 100
    finally:
        events = events + 1000
    return events
ok = run(0)
bad = run(1)
def fin():
    try:
        return 5
    finally:
        x = 1
five = fin()
def only_finally():
    total = 0
    try:
        total = total + 1
    finally:
        total = total + 10
    return total
after = only_finally()
