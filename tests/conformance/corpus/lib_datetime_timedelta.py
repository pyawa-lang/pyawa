# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/datetime.py`（`timedelta` 的字段与格式化）。
# 注：`timedelta + timedelta` 本层尚未接线 ⇒ 不收它（未实现不是差异，`MS-19`）。
import datetime

delta = datetime.timedelta(days=1, hours=2, minutes=30)
seconds = delta.total_seconds()
text = str(delta)
