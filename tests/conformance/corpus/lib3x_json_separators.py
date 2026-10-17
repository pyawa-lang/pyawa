# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/json/__init__.py`。
import json

text = json.dumps({'b': 2, 'a': 1}, sort_keys=True, separators=(',', ':'))
loaded = json.loads('[1, 2, {"x": 3}]')
nested = str(loaded)
