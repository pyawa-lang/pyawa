# **位置实参允许裸海象**（第 316 轮，参照口径）：`f(x := 5)` 合法 —— 先前一律走 `parse_expression`
# ⇒ 它不吃裸海象 ⇒ 下一个词素是 `Walrus` ⇒ 报"实参表里出现 Some(Walrus)"
# （`Lib/_py_warnings.py:436` 的 `_is_internal_filename(filename := frame.f_code.co_filename)` 正卡它）。
# 关键字实参那一路**照旧**不吃裸海象（参照里 `f(a=x := 1)` 本身是语法错）。


def identity(value):
    return value


def two(left, right):
    return (left, right)


print(str(identity(x := 5)))
print(str(x))
print(str(two(a := 1, b := 2)))
print(str(a + b))
print(str(identity(y := 2) + y))
print(str([z := 7, z + 1]))
print(str(z))


def closure():
    return (inner := 3) + inner


print(str(closure()))
