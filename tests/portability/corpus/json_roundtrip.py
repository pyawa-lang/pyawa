# json：稳定序列化
import json

obj = {"b": [1, 2, 3], "a": {"x": True, "y": None}, "c": "text"}
encoded = json.dumps(obj, sort_keys=True, separators=(",", ":"))
print("encoded", encoded)
print("roundtrip", json.loads(encoded) == obj)
print("list_order", json.dumps([3, 1, 2]))
