# `MS-13` ② 的 `Lib/test/` 语料：正文（自下一行起）**逐字**取自
# `Lib/test/support/refleak_helper.py`（该套 support 模块无 `support/__init__.py`
# ⇒ 无法经加载器整包导入 ⇒ 按"可编译的部分"取逐字片段）。探针只取值。
"""
Utilities for changing test behaviour while hunting
for refleaks
"""

_hunting_for_refleaks = False
def hunting_for_refleaks():
    return _hunting_for_refleaks
