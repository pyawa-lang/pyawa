# 循环体末尾"体不落到末尾的 if"（第 248 轮）：取反＋回边换边后语义不变
def find_first_even(items):
    for item in items:
        if item % 2 == 0:
            return item
    return -1
a = find_first_even([1, 3, 4, 5])
b = find_first_even([1, 3, 5])
def stop_at(items, limit):
    total = 0
    for item in items:
        if item > limit:
            break
        total = total + item
    return total
c = stop_at([1, 2, 9, 3], 5)
def skip_odd(items):
    total = 0
    for item in items:
        if item % 2 == 1:
            continue
        total = total + item
    return total
d = skip_odd([1, 2, 3, 4])
def while_return(flag):
    n = 0
    while flag:
        if n > 2:
            return n
        n = n + 1
    return -1
e = while_return(1)
f = while_return(0)
