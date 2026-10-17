# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/import_helper.py 第 87–95 行
def _save_and_remove_modules(names):
    orig_modules = {}
    prefixes = tuple(name + '.' for name in names)
    for modname in list(sys.modules):
        if modname in names or modname.startswith(prefixes):
            orig_modules[modname] = sys.modules.pop(modname)
    return orig_modules


import sys

import json

_ = json
saved = _save_and_remove_modules(['json'])
has = 'json' in saved
removed = 'json' not in sys.modules
