# `del`（第 106 轮）：四种目标各来一次 ✓ —— `DELETE_NAME`／`DELETE_FAST`／`DELETE_SUBSCR`／`DELETE_ATTR`
x = 1
del x

d = {}
d["k"] = 1
assert d["k"] == 1
del d["k"]

class C:
    pass

c = C()
c.attr = 1
del c.attr

def f():
    local = 1
    del local

f()
print("ok")
