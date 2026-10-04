class C:
    def __enter__(self):
        return 7
    def __exit__(self, kind, value, tb):
        return False
def f():
    with C() as tag:
        x = tag
    return x
p = f()
