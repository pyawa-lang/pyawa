# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/stat.py`（位掩码判定 ＋ `filemode`）。
import stat

is_dir = stat.S_ISDIR(0o40755)
is_reg = stat.S_ISREG(0o100644)
mode = oct(stat.S_IMODE(0o100644))
kind = oct(stat.S_IFMT(0o100644))
text = stat.filemode(0o100644)
