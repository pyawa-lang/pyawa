def outer():
    x = 40
    def inner():
        return x + 2
    return inner()
a = outer()

def make(n):
    def add(m):
        return n + m
    return add
b = make(1)(2)

def counter():
    total = 0
    def bump():
        nonlocal total
        total = total + 1
        return total
    bump()
    bump()
    return total
c = counter()

f = lambda: 7
d = f()
