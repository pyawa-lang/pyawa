# **嵌套 `try` 之后的外层余部**（第 353 轮修，`P3-20`）：处理块路径先前**不分深度**一律走
# "余部＋收尾" ✗ ⇒ 在**处理块里**发出一条 `LOAD_CONST None; RETURN_VALUE` ✗ ⇒ **模块提前返回** ✓
# ⇒ 其后的语句全丢 ✓（第 293 轮定位、第 351／352 轮把病灶收到码元级 ✓，本轮修好 ✓）。
# 它就是 `Lib/types.py` 被截断的根子（`except ImportError:` 里嵌 `try/except TypeError as exc:` ⇒
# 其后 30+ 个定义全丢），上限榜上那一族 **101** 个模块压在它上面。

try:
    raise ValueError
except ValueError:
    print("in handler")
    try:
        raise TypeError
    except TypeError as exc:
        b = 2
    print("after nested")
print("after")


def guarded():
    try:
        raise KeyError("k")
    except KeyError:
        try:
            raise IndexError("i")
        except IndexError as exc:
            inner = 1
        return ("handler", inner)
    return ("body",)


print(str(guarded()))


def two_levels():
    out = []
    try:
        raise ValueError
    except ValueError:
        try:
            raise TypeError
        except TypeError:
            out.append("deep")
        out.append("middle")
    out.append("outer")
    return out


print(str(two_levels()))
