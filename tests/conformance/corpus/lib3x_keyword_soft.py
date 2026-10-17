# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/keyword.py`。
import keyword

soft_match = keyword.issoftkeyword('case')
hard = keyword.iskeyword('while')
soft_list = str(keyword.softkwlist)
