# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/_header_value_parser.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email._header_value_parser

name = email._header_value_parser.__name__
surface = str(sorted(n for n in ['ASPECIALS', 'ATOM_ENDS', 'ATTRIBUTE_ENDS', 'AddrSpec', 'Address', 'AddressList', 'AngleAddr', 'Atom', 'Attribute', 'BareQuotedString', 'CFWSList', 'CFWS_LEADER'] if hasattr(email._header_value_parser, n)))
