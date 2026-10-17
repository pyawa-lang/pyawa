# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/token.py`。
import token

eq = token.EXACT_TOKEN_TYPES['==']
name = token.tok_name[token.OP]
ordered = str(sorted(token.EXACT_TOKEN_TYPES)[:6])
