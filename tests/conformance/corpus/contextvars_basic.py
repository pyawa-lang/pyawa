# **`_contextvars`**（第 332 轮）：`Lib/contextvars.py` 只做
#   `from _contextvars import Context, ContextVar, Token, copy_context`
#   ＋ `_collections_abc.Mapping.register(Context)`
# ⇒ 这一组名字就够它 import。上限榜上 `No module named '_contextvars'` × 49 个模块的卡点。
# 语料直接打 **`_contextvars`**（C 模块 ✓）：`contextvars.py` 本身还没同步进 `Lib/`。
#
# **如实登记的偏差与未接面**：
#   * 没有真正的上下文隔离（值存在**变量自己**身上）；`ContextVar` 的默认值只认**位置**写法
#     （`ContextVar("v", None)`），关键字 `default=` 还没接（`new` 槽看不到 kwargs）⇒
#     **本层比参照更宽**：`ContextVar("w", 7)` 参照会 `TypeError`、本层接受 ✗（如实登记，语料不比它）；
#   * `get()` 无值且无默认 ⇒ 本层按参照报 `LookupError`，但消息是近似（不含地址）；
#   * `Token.old_value` 在"原本没值"时给的是默认值，参照给 `<Token.MISSING>` 哨兵 ⇒ 语料不比它；
#   * `Context` 的方法面（`get`／`run`／`copy`／`__contains__`）第一版实现会崩 ⇒ 本轮如实报
#     `NotImplementedError`（不把崩溃留在树里）。

import _contextvars

var = _contextvars.ContextVar("v")
print(var.name)
try:
    var.get()
except LookupError:
    print("LookupError ok")
print(str(var.get("fallback")))
token = var.set(42)
print(str(var.get()))
print(str(type(token).__name__))
var.reset(token)
print(str(var.get(None)))
ctx = _contextvars.copy_context()
print(str(type(ctx).__name__))
print(str(isinstance(ctx, _contextvars.Context)))
