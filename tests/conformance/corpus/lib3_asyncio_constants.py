# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/constants.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.constants

name = asyncio.constants.__name__
surface = str(sorted(n for n in ['ACCEPT_RETRY_DELAY', 'DEBUG_STACK_DEPTH', 'FLOW_CONTROL_HIGH_WATER_SSL_READ', 'FLOW_CONTROL_HIGH_WATER_SSL_WRITE', 'LOG_THRESHOLD_FOR_CONNLOST_WRITES', 'SENDFILE_FALLBACK_READBUFFER_SIZE', 'SSL_HANDSHAKE_TIMEOUT', 'SSL_SHUTDOWN_TIMEOUT', 'THREAD_JOIN_TIMEOUT', 'enum'] if hasattr(asyncio.constants, n)))
