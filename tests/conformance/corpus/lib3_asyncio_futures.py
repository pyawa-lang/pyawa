# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/futures.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.futures

name = asyncio.futures.__name__
surface = str(sorted(n for n in ['Future', 'GenericAlias', 'STACK_DEBUG', 'base_futures', 'concurrent', 'contextvars', 'events', 'exceptions', 'format_helpers', 'future_add_to_awaited_by', 'future_discard_from_awaited_by', 'isfuture'] if hasattr(asyncio.futures, n)))
declared = str(sorted(asyncio.futures.__all__))
