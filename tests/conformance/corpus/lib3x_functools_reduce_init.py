# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/functools.py`。
import functools

summed = functools.reduce(lambda a, b: a + b, [1, 2, 3], 10)
joined = functools.reduce(lambda a, b: a + b, [], 'seed')
