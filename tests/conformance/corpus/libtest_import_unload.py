# `MS-13` ② 的 `Lib/test/` 语料（第三批）：正文**逐字**取自
#   Lib/test/support/import_helper.py 第 32–38 行
#   Lib/test/support/import_helper.py 第 87–95 行
# 其余为脚手架。
def unload(name):
    try:
        del sys.modules[name]
    except KeyError:
        pass


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
unload('json')
after_unload = 'json' not in sys.modules
import json

saved = _save_and_remove_modules(['json'])
saved_has_json = 'json' in saved
after_save = 'json' not in sys.modules
