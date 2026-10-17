# `MS-13` ② 的 `Lib/` 语料：被测代码即逐字同步的 `Lib/this.py`
# （模块体在导入时打印 ROT13 解码后的 Zen；两侧 stdout 必须逐行一致）。
import this

n_lines = len(this.s.splitlines())
first_word = this.d["n"]
