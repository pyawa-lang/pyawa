# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/asyncio/tasks.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import asyncio.tasks

name = asyncio.tasks.__name__
surface = str(sorted(n for n in ['ALL_COMPLETED', 'FIRST_COMPLETED', 'FIRST_EXCEPTION', 'GenericAlias', 'Task', 'all_tasks', 'as_completed', 'base_tasks', 'concurrent', 'contextvars', 'coroutines', 'create_eager_task_factory'] if hasattr(asyncio.tasks, n)))
declared = str(sorted(asyncio.tasks.__all__))
