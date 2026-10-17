# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/email/generator.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import email.generator

name = email.generator.__name__
surface = str(sorted(n for n in ['BytesGenerator', 'BytesIO', 'DecodedGenerator', 'Generator', 'HeaderWriteError', 'NEWLINE_WITHOUT_FWSP', 'NEWLINE_WITHOUT_FWSP_BYTES', 'NL', 'NLCRE', 'StringIO', 'UNDERSCORE', 'deepcopy'] if hasattr(email.generator, n)))
declared = str(sorted(email.generator.__all__))
