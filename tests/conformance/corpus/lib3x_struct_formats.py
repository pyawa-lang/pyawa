# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/struct.py`。
import struct

packed = struct.pack('<2sIf', b'ab', 5, 1.5).hex()
size = struct.calcsize('<2sIf')
unpacked = str(struct.unpack('<2sIf', struct.pack('<2sIf', b'ab', 5, 1.5)))
