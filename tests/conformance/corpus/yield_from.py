# **`yield from`**（第 315 轮）：先前**根本没接** —— `from` 被当**名字**、随后的 `[` 被当**下标**
# ⇒ 报"``[`` 之后要 `]`，实际 `Some(For)`"（`Lib/traceback.py:1258` 的
# `yield from [indent + l + '\n' for l in formatted]` 正卡它，那一族 15 个模块）。
# 发射形状照参照实测：`GET_YIELD_FROM_ITER; [L1] LOAD_CONST None; SEND <L2>; YIELD_VALUE 1;
# RESUME 2; POP_TOP; JUMP_BACKWARD_NO_INTERRUPT <L1>; [L2] END_SEND`（语句形态末尾再补 POP_TOP）。


def inner():
    yield 1
    yield 2
    return "inner-done"


def delegating():
    result = yield from inner()
    yield result


def chain():
    yield from delegating()
    yield from [3, 4]
    yield from (5,)
    yield from "ab"


def empty():
    yield from ()
    yield 6


print(str(list(chain())))
print(str(list(empty())))


# 注：`yield from [<推导式>]`（`Lib/traceback.py:1258` 的**确切形状**）本层仍会
# `StackUnderflow` ✗（内联推导式后面接 SEND 那一串时栈对不上）⇒ 这一格**不进语料**，
# 已如实记在台账里；其余形态（生成器／列表／元组／字符串／空／返回值）都与参照逐字同 ✓。
