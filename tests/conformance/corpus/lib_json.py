# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/json/__init__.py`（序列化 ＋ 反序列化）。
import json

text = json.dumps({"a": 1, "b": [1, 2]})
loaded = json.loads('{"a": 1, "b": [1, 2]}')["a"]
mixed = json.dumps([1, "x", None, True])
