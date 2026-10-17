# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/selector_events.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.selector_events

name = asyncio.selector_events.__name__
surface = str(sorted(n for n in ['BaseSelectorEventLoop', 'SC_IOV_MAX', 'base_events', 'collections', 'constants', 'errno', 'events', 'functools', 'futures', 'itertools', 'logger', 'os'] if hasattr(asyncio.selector_events, n)))
declared = str(sorted(asyncio.selector_events.__all__))
