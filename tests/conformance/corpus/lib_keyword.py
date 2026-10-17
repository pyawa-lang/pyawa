# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/keyword.py`（生成件，纯数据 ＋ 两个判定函数）。
import keyword

is_for = keyword.iskeyword("for")
is_match = keyword.iskeyword("match")
soft_match = keyword.issoftkeyword("match")
n = len(keyword.kwlist)
first = keyword.kwlist[0]
