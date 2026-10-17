# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/token.py`（生成件：常量表 ＋ `tok_name` 反查）。
import token

name = token.tok_name[token.NAME]
terminal = token.ISTERMINAL(token.NAME)
nonterminal = token.ISTERMINAL(token.N_TOKENS)
plus = token.EXACT_TOKEN_TYPES["+"]
