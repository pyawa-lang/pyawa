# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/subprocess.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.subprocess

name = asyncio.subprocess.__name__
surface = str(sorted(n for n in ['DEVNULL', 'PIPE', 'Process', 'STDOUT', 'SubprocessStreamProtocol', 'create_subprocess_exec', 'create_subprocess_shell', 'events', 'logger', 'protocols', 'streams', 'subprocess'] if hasattr(asyncio.subprocess, n)))
declared = str(sorted(asyncio.subprocess.__all__))
