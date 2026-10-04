# 第 202 轮回归：**函数帧里的 `except … as 名字`**（真 bug 修复）
# 先前处理器绑定与收尾都写死 `STORE_NAME`／`DELETE_NAME` ⇒ 函数里直接中止 ✗
#
# 第 203 轮：**处理器里 `return <表达式>`** 的收尾已修（参照是
#   `[值] → SWAP 2 → POP_EXCEPT → 名字清理 → RETURN_VALUE`）⇒ 这里把它守住 ✓。
# ⚠ 比较用**同一性／类型名** ✓ —— 不用 `str(exc)`（那是另一处已知缺口：`str()` 还不认 `__str__` ✗）。


def assigned():
    x = 0
    try:
        raise KeyError("k")
    except KeyError as exc:
        x = 1
    return x


def constant_return():
    try:
        raise ValueError("v")
    except ValueError as exc:
        return 1
    return 0


def reraise_path():
    out = []
    try:
        out.append(1)
    except ValueError as exc:
        out.append(2)
    finally:
        out.append(3)
    return out


def no_match():
    try:
        raise KeyError("k")
    except ValueError as exc:
        return "wrong"
    except KeyError:
        return "right"


def returns_object():
    try:
        raise ValueError("v")
    except ValueError as exc:
        return exc
    return None


def returns_expression():
    try:
        raise KeyError("k")
    except KeyError as exc:
        return 20 + 1
    return None


assert type(returns_object()).__name__ == "ValueError"
assert returns_expression() == 21
assert assigned() == 1
assert constant_return() == 1
assert reraise_path() == [1, 3]
assert no_match() == "right"
print("ok")
