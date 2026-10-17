# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/base_events.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.base_events

name = asyncio.base_events.__name__
surface = str(sorted(n for n in ['BaseEventLoop', 'MAXIMUM_SELECT_TIMEOUT', 'Server', 'collections', 'concurrent', 'constants', 'coroutines', 'errno', 'events', 'exceptions', 'futures', 'heapq'] if hasattr(asyncio.base_events, n)))
declared = str(sorted(asyncio.base_events.__all__))
