# 第 202 轮回归：**函数帧里的 `except … as 名字`**（真 bug 修复）
# 先前处理器绑定与收尾都写死 `STORE_NAME`／`DELETE_NAME` ⇒ 函数里直接中止 ✗
#
# ⚠ **不放进本用例**：处理器里 `return <表达式>`（如 `return str(exc)`）✗ ——
#   那是另一处已知缺口（见台账第 146 轮：参照在处理器里 `return` 前有 `SWAP 2`、
#   我们缺这条 ⇒ **返回错值** ✗），修好后要把它补回来 ✓。


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


assert assigned() == 1
assert constant_return() == 1
assert reraise_path() == [1, 3]
assert no_match() == "right"
print("ok")
