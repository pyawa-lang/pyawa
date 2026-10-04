# lambda（第 232 轮）：无参／带默认值／仅关键字／当实参传递
add = lambda a, b: a + b
x = add(40, 2)
double = lambda v: v * 2
y = double(x)
with_default = lambda v, step=3: v + step
z = with_default(1)
kw = lambda v, *, k=5: v + k
w = kw(1)
apply = lambda f, v: f(v)
u = apply(double, 3)
