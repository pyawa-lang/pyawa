#!/usr/bin/env python3
"""**形参所有权探针**（PLAN §9.4 工单第 3 项）：把台账里 13 处「形参所有权／引用泄漏」的
**历史候选形态**逐条做成**最小复现**，跑一遍看 `sys.getrefcount` 是否回到基线（delta 应为 0）。

口径：
* 每个探针用**用户类实例**（不是小整数／`None`／interned 字符串）当被观察对象 —— 参照实现
  对这些对象是 immortal（`getrefcount` 口径不同），用户对象两边可比 ✓；
* 跑法：`target/release/pyawa tools/param_ownership_probe.py`；对照：
  `python3 tools/param_ownership_probe.py`（两边 delta 应一致；异常回溯一类探针两边可能差 1，
  属实现观测面 ✓）；
* **PASS** ＝ delta == 0；非 0 打印 `FAIL` 并在结尾给出计数（退出码 1）。
* 探针只**报告**，不改任何状态；可用于以后的回归（`check.py` 未接它，按轮手动跑 ✓）。

探针清单（编号 → 台账出处）：
  ① 融合局部存取的被覆盖旧值（round 102 候选 `STORE_FAST_STORE_FAST`）
  ② 属性删除的 name 对象（round 93 `instance_attribute_delete_inner`）
  ③ 比较协议清空操作数（round 93 `evil` 双放）
  ④ 自定义元类建类的命名空间键（round 94）
  ⑤ `unittest` `_AssertRaisesBaseContext.handle` 的 `args` 形参槽（round 85／102）
  ⑥ 生成器形参所有权（round 88）
  ⑦ `__init__` 实例化实参（round 79）
  ⑧ 异常实参／回溯链（round 79／192）
  ⑨ 方法调用实参（round 59 原点）
"""
import gc
import sys
import unittest

results: list[tuple[str, int]] = []


def check(label: str, before: int, after: int) -> None:
    results.append((label, after - before))


class K:
    """普通用户类实例（参照实现里不是 immortal ⇒ 两边 `getrefcount` 可比 ✓）。"""


def probe_fused_store() -> None:
    k = K()

    def fused(args):
        callable_obj, *args = args
        return callable_obj

    holder = (k,)
    before = sys.getrefcount(k)
    for _ in range(50):
        fused(holder)
    check("① fused_store_old", before, sys.getrefcount(k))


def probe_delattr_name() -> None:
    class C:
        pass

    target = C()
    target.attr = 1
    name = "".join(["at", "tr"])  # 非 interned ⇒ 计数可观察
    before = sys.getrefcount(name)
    for _ in range(50):
        target.attr = 1
        delattr(target, name)
    check("② delattr_name", before, sys.getrefcount(name))


def probe_evil_compare() -> None:
    class Evil:
        def __init__(self, box):
            self.box = box

        def __eq__(self, other):
            self.box.clear()
            return False

    box = [1, 2, 3]
    evil = Evil(box)
    other = [Evil(box), 2, 3]
    before = sys.getrefcount(box)
    for _ in range(20):
        box.extend([4, 5, 6])
        evil == other
    check("③ evil_compare", before, sys.getrefcount(box))


def probe_metaclass_class() -> None:
    class Meta(type):
        pass

    key_holder = K()
    before = sys.getrefcount(key_holder)
    for _ in range(50):
        extra = key_holder

        class X(metaclass=Meta):
            a = 1
            b = 2
            c = 3

        del X, extra
    gc.collect()
    check("④ metaclass_class", before, sys.getrefcount(key_holder))


def probe_assert_raises() -> None:
    class T(unittest.TestCase):
        def runTest(self):
            it = iter([1, 2, 3])
            target = K()
            before = sys.getrefcount(target)
            with self.assertRaises(TypeError):
                raise TypeError("x")
            del it
            gc.collect()
            check("⑤ assertRaises_handle", before, sys.getrefcount(target))

    T().runTest()


def probe_generator_param() -> None:
    big = K()

    def gen(x):
        yield 1

    before = sys.getrefcount(big)
    for _ in range(20):
        generator = gen(big)
        del generator
    gc.collect()
    check("⑥ generator_param", before, sys.getrefcount(big))


def probe_init_arg() -> None:
    class B:
        def __init__(self, x):
            self.x = x

    arg = K()
    before = sys.getrefcount(arg)
    for _ in range(20):
        instance = B(arg)
        del instance
    gc.collect()
    check("⑦ init_arg", before, sys.getrefcount(arg))


def probe_exception_payload() -> None:
    payload = K()
    before = sys.getrefcount(payload)
    for _ in range(20):
        try:
            raise ValueError(payload)
        except ValueError:
            pass
    gc.collect()
    check("⑧ exception_payload", before, sys.getrefcount(payload))


def probe_method_arg() -> None:
    class M:
        def method(self, a):
            return 0

    receiver = M()
    obj = K()
    before = sys.getrefcount(obj)
    for _ in range(50):
        receiver.method(obj)
    check("⑨ method_arg", before, sys.getrefcount(obj))


def main() -> int:
    for probe in (
        probe_fused_store,
        probe_delattr_name,
        probe_evil_compare,
        probe_metaclass_class,
        probe_assert_raises,
        probe_generator_param,
        probe_init_arg,
        probe_exception_payload,
        probe_method_arg,
    ):
        probe()
    failed = 0
    for label, delta in results:
        flag = "PASS" if delta == 0 else "FAIL"
        if delta != 0:
            failed += 1
        print(f"{flag} {label:<24} delta {delta:+d}")
    print(f"\n{len(results) - failed}/{len(results)} PASS（delta 应为 0）")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
