# ③ 导入路径进对拍（第 212 轮）：对拍器只按「脚本」跑 ⇒ 用"自己会 import"的语料把导入路径也纳入
import os

print(str(hasattr(os, "sep")))
print(str(hasattr(os, "curdir")))
