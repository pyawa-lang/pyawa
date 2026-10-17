# 空 `*args` 的绑定（第 277 轮真 bug ✗）：没有多余位置实参时，`*p` 必须绑成**空元组** ✓
# 先前 `locals[varargs_slot]` 的赋值整块写在 `if !extra.is_empty()` **里面** ✗ ⇒ 那一格从不绑 ✓
# ⇒ 函数体里一读就报"未绑定局部" ✓（`Lib/posixpath.py` 的 `join(a, *p)` 与 `import site` 都撞在它上面 ✓）。


def count_extra(a, *p):
    return len(p)


def fold(a, *p):
    for item in p:
        a = a + item
    return a


print("zero" if count_extra(1) == 0 else "no")
print("three" if count_extra(1, 2, 3) == 3 else "no")
print("ab" if fold("a", "b") == "ab" else "no")
print("a" if fold("a") == "a" else "no")
