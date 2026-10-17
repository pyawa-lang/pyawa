# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/stat.py`。
import stat

link = stat.S_ISLNK(0o120777)
sock = stat.S_ISSOCK(0o140755)
dir_text = stat.filemode(0o40755)
regular = stat.filemode(0o100644)
