# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/datetime.py`（纯 Python 回退实现 `_pydatetime` 的出口）。
import datetime

d = datetime.date(2024, 2, 29)
iso = d.isoformat()
weekday = d.weekday()
delta = str(datetime.timedelta(days=1, hours=2))
