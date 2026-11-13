# **`x = yield`（SEND 取值形态）**（`PLAN-milestones.md` §12.1 的语料）：
# `yield` 当表达式时，所在函数必须被判成生成器；`send(v)` 的 `v` 就是该表达式的值。
# 修因：`statements_have_yield` 先前只下探 `return`／裸表达式 ⇒ `v = yield` 漏判 ⇒
# 生成器标志（`CO_GENERATOR`）没置 ⇒ 运行期报"非生成器函数不该让出"。
# 这里只放**参照与实现都确定一致**的形态；语料按 `manifest.tsv` 的模式 `python` 跑。


def take():
    v = yield
    print("v", str(v))
    return "done"


g = take()
print("start", str(g.send(None)))
try:
    print("resumed", str(g.send(7)))
except StopIteration as stop:
    print("stop", str(stop.value))

# 链式赋值目标同样要看右值里的 `yield`（目标按左到右存）。
def chained():
    a = b = yield
    print("chained", str(a), str(b))


c = chained()
c.send(None)
try:
    c.send(11)
except StopIteration:
    print("chained stop")

# 三元表达式的分支、f-string 插值、比较的操作数都在同一作用域。
def branches():
    t = (yield) if True else 0
    print("ternary", str(t))
    s = f"<{(yield)}>"
    print("fstring", s)
    u = (yield) < 3
    print("compare", str(u))


d = branches()
d.send(None)
d.send("branch")
d.send("value")
try:
    d.send(2)
except StopIteration:
    print("branches stop")

# 推导式的**最外层可迭代对象**在外层求值（3.12+ 内联形态同口径）。
def doubles():
    r = [x * 2 for x in (yield)]
    print("listcomp", str(r))


e = doubles()
e.send(None)
try:
    e.send([1, 2, 3])
except StopIteration:
    print("doubles stop")

# 嵌套 `def` 的形参默认值在外层求值：`inner` 的默认值就是一个 `yield`。
def nested():
    def inner(a=(yield)):
        return a

    value = yield
    print("nested", str(inner(value)))


f = nested()
f.send(None)
try:
    f.send(5)
except StopIteration:
    print("nested stop")

# `yield from` 的表达式形态：送入值一路委派。
def delegated():
    v = yield from [1, 2]
    print("delegated", str(v))


h = delegated()
print("one", str(h.send(None)))
print("two", str(h.send(None)))
try:
    print("three", str(h.send(None)))
except StopIteration:
    print("delegated stop")

# `try/finally` 里的赋值让出：`close()` 要走到 `finally`（`GeneratorExit` 不泄漏、不崩）。
closed = []


def closing():
    try:
        v = yield 1
        print("never", str(v))
    finally:
        closed.append("finally")


i = closing()
print("suspended", str(i.send(None)))
i.close()
print("closed", str(len(closed)))
