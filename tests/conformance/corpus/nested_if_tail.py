# 嵌套 `if` 的**尾位**（第 279 轮真 bug ✗）：块里最后一条 `if` **后面还有代码**时，
# 分支不许补隐式 `return` —— 补了就是**提前返回** ✗（实测 `pick(1, 1)` 先前给 `None`）。
# 同一形状在 `Lib/importlib/_bootstrap.py` 的 `_spec_from_module` 里 ⇒ 挡在 M3 的 import 链上。
#
# 第二条覆盖的是同轮的第二个缺陷：**非尾块里的 `if` 也给条件出口建独立落点**
# ⇒ 跳转落到收尾副本（`LOAD_CONST None; RETURN_VALUE`）上 ⇒ 空栈或假 `None` ✗。


def pick(flag, other):
    value = None
    if flag:
        if other:
            value = "both"
    return value


def fallback(origin, location):
    if not origin and location is not None:
        origin = location
    return origin


def nested_fallback(loader, location):
    origin = None
    if origin is None:
        if loader is not None:
            origin = "loaded"
        if not origin and location is not None:
            origin = location
    return origin


print("both" if pick(1, 1) == "both" else "no")
print("none" if pick(1, 0) is None else "no")
print("none" if pick(0, 1) is None else "no")
print("/x" if fallback(None, "/x") == "/x" else "no")
print("loaded" if fallback("loaded", "/x") == "loaded" else "no")
print("/x" if nested_fallback(None, "/x") == "/x" else "no")
print("loaded" if nested_fallback("loaded", "/x") == "loaded" else "no")
