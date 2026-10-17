# `MS-13` ② 的 `Lib/test/` 语料：正文逐字取自
#   Lib/test/support/import_helper.py 第 32–38 行
# 其余为脚手架。
import sys

def unload(name):
    try:
        del sys.modules[name]
    except KeyError:
        pass



import json

_ = json
unload('json')
gone = 'json' not in sys.modules
unload('never_imported_name')
no_error = 'ok'
