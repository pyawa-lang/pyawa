# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/argparse.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import argparse

name = argparse.__name__
surface = str(sorted(n for n in ['Action', 'ArgumentDefaultsHelpFormatter', 'ArgumentError', 'ArgumentParser', 'ArgumentTypeError', 'BooleanOptionalAction', 'FileType', 'HelpFormatter', 'MetavarTypeHelpFormatter', 'Namespace', 'ONE_OR_MORE', 'OPTIONAL'] if hasattr(argparse, n)))
declared = str(sorted(argparse.__all__))
