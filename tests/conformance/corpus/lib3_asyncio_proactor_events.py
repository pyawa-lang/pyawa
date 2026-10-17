# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/proactor_events.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.proactor_events

name = asyncio.proactor_events.__name__
surface = str(sorted(n for n in ['BaseProactorEventLoop', 'base_events', 'collections', 'constants', 'exceptions', 'futures', 'io', 'logger', 'os', 'protocols', 'signal', 'socket'] if hasattr(asyncio.proactor_events, n)))
declared = str(sorted(asyncio.proactor_events.__all__))
