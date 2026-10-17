# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/streams.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.streams

name = asyncio.streams.__name__
surface = str(sorted(n for n in ['FlowControlMixin', 'StreamReader', 'StreamReaderProtocol', 'StreamWriter', 'collections', 'coroutines', 'events', 'exceptions', 'format_helpers', 'logger', 'open_connection', 'open_unix_connection'] if hasattr(asyncio.streams, n)))
declared = str(sorted(asyncio.streams.__all__))
