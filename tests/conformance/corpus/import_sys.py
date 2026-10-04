# `import`（第 95／96 轮）：模块表 ＋ 两条 import 臂
# `import sys` 走 `IMPORT_NAME`（模块表＝`sys.modules` 同一份 ✓）；
# `from sys import argv` 走 `IMPORT_FROM`（模块**对象**的属性查找 ✓）——加载器（`P3-12`）仍未接 ✗
import sys
print("ok")
from sys import argv
x = argv
print("ok2")
