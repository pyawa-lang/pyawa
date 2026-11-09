#!/usr/bin/env python3
"""**以"导入名"跑一份 `test_*.py`**（官方 regrtest 的做法；给 `libtests_survey.py` 的复试用）。

为什么需要：官方套件里的 doctest 期望文本带**模块限定名**（`test.test_extcall.f()`、
`test.test_unpack_ex.f()`），而"直接跑脚本"时模块名是 `__main__` ⇒ 期望对不上 ✗。
本文件把测试文件 `exec` 进一个**具名模块**（`test.test_xxx`）再跑它的 `load_tests`／`test_main`
（与 regrtest 的 `unittest` 派发同一形状 ✓）。

用法（由 pyawa CLI 执行）::

    pyawa tools/libtests_named_runner.py <测试文件路径> <导入名>

退出码：0 = 通过；1 = 有失败／错误；2 = 该文件没有可跑入口（**不据此改判** ✓）。
"""
import sys
import types
import unittest


def main() -> int:
    path = sys.argv[1]
    name = sys.argv[2]
    module = types.ModuleType(name)
    module.__file__ = path
    # **父包上也要挂一份**（round 180）：`exec` 进具名模块绕过了 import 机制 ⇒
    # `from test import test_pdb`（测试文件常自引用本模块做 subprocess 用例）会走
    # `from package import name` 的**属性取法** ⇒ 父包上没有这个属性就 `ImportError` ✗。
    parent_name, _, short_name = name.rpartition(".")
    parent = sys.modules.get(parent_name) if parent_name else None
    if parent is not None:
        setattr(parent, short_name, module)
    sys.modules[name] = module
    source = open(path).read()
    # `__name__` 是导入名 ⇒ 文件自己的 `if __name__ == "__main__":` 不触发 ✓。
    exec(compile(source, path, "exec"), module.__dict__)

    loader = unittest.defaultTestLoader
    if hasattr(module, "load_tests"):
        suite = module.load_tests(loader, unittest.TestSuite(), None)
        result = unittest.TextTestRunner(stream=open("/dev/null", "w"), verbosity=0).run(suite)
        return 0 if result.wasSuccessful() else 1
    if hasattr(module, "test_main"):
        try:
            module.test_main()
        except BaseException:
            return 1
        return 0
    # **没有 `load_tests`／`test_main`**：按 `unittest` 的默认发现规则跑模块内 `TestCase`
    # （官方 regrtest 对普通 `test_*.py` 就是这么派发的 ✓）——没有测试类才返回 2（**不**改判 ✓）。
    suite = loader.loadTestsFromModule(module)
    if suite.countTestCases() > 0:
        result = unittest.TextTestRunner(stream=open("/dev/null", "w"), verbosity=0).run(suite)
        return 0 if result.wasSuccessful() else 1
    return 2


if __name__ == "__main__":
    sys.exit(main())
