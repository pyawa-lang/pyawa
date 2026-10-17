# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/_collections_abc.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import _collections_abc

name = _collections_abc.__name__
surface = str(sorted(n for n in ['ABCMeta', 'AsyncGenerator', 'AsyncIterable', 'AsyncIterator', 'Awaitable', 'Buffer', 'ByteString', 'Callable', 'Collection', 'Container', 'Coroutine', 'EllipsisType'] if hasattr(_collections_abc, n)))
declared = str(sorted(_collections_abc.__all__))
