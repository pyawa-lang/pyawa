# posix 函数面（第 204 轮起逐条落地）：`Lib/os.py` 一导入就 `from posix import *`
# 如实记：`stat` 返回的对象只带我们真有的字段（`st_mode`／`st_size`／`st_dev`／`st_ino`）
import posix

print(str(hasattr(posix, "stat")))
print(str(hasattr(posix, "open")))
print(str(hasattr(posix, "close")))
print(str(hasattr(posix, "read")))
print(str(hasattr(posix, "write")))
