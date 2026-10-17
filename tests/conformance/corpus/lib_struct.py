# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/struct.py`（薄封装，真值一直在核心）。
import struct

packed = struct.pack("<i", 5).hex()
size = struct.calcsize("<i")
unpacked = struct.unpack("<H", b"\x01\x00")[0]
