# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/textwrap.py`（`TextWrapper` 的纯语义面）。
import textwrap

wrapped = str(textwrap.wrap("The quick brown fox jumps over the lazy dog", 15))
filled = textwrap.fill("The quick brown fox jumps over the lazy dog", 15)
short = textwrap.shorten("The quick brown fox jumps over the lazy dog", width=20)
dedented = str(textwrap.dedent("  a\n  b\n"))
