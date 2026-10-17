# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/unix_events.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.unix_events

name = asyncio.unix_events.__name__
surface = str(sorted(n for n in ['EventLoop', 'SelectorEventLoop', 'base_events', 'base_subprocess', 'can_use_pidfd', 'constants', 'coroutines', 'errno', 'events', 'exceptions', 'futures', 'io'] if hasattr(asyncio.unix_events, n)))
declared = str(sorted(asyncio.unix_events.__all__))
