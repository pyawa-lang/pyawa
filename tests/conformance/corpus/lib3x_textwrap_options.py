# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/textwrap.py`。
import textwrap

wrapped = str(textwrap.wrap('alpha beta gamma delta', 10, subsequent_indent='  '))
short = textwrap.shorten('a b c d e f', width=7, placeholder='...')
