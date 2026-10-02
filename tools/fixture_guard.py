#!/usr/bin/env python3
"""夹具生成脚本的**可执行守卫**：探测前后接收者快照必须相同，不同就**拒绝该用例**。

为什么**不**做成 `tests/ci/check.py` 的静态检查：静态判据要求理解夹具语义，只能退化成
"看调用顺序"的启发式 ⇒ **会误报也会漏报**，那正是 `CX-15`（禁假检查）反对的——
假检查比没有更坏。这里比较的是**真实状态**，零假阳性。

用法：

* **进程内生成器**：``with unchanged("用例名", receiver):``……探测……（退出 `with` 时比对）；
  生成器脚本所在目录本来就在 `sys.path[0]`，直接 ``from fixture_guard import unchanged`` 即可。
* **子进程探针脚本**：生成器把 `tools/` 放进子进程的 `PYTHONPATH`（见
  `tools/gen_slice_fixture.py` 的 `probe_environment()`），探针里同样
  ``from fixture_guard import unchanged``。

**故意**要改接收者的探针（例如 `itertools.islice` 消费迭代器、`__setitem__` 的写入探测）
**不要**套这个守卫——要么换一个**新**接收者，要么只守卫"值要被记录"的那些接收者，
并在生成器里写明原因。守卫不是"所有探测都不许改东西"，而是"**被记录的值**不许被别的探测改掉"。

自检：``python3 tools/fixture_guard.py``（会绿一条、红一条——证明它不是假检查）。
"""

from __future__ import annotations

import contextlib
import copy
from collections.abc import Iterator


def snapshot(value: object) -> object:
    """接收者快照（**深拷贝**：容器的内容也要进快照，`is` 相同不算数）。"""
    return copy.deepcopy(value)


def same(first: object, second: object, _seen: set[tuple[int, int]] | None = None) -> bool:
    """快照是否"没变"（**NaN 感知** ＋ **自引用安全**的结构比较）。

    两条边界都是接上守卫时被夹具当场抓出来的：
    - `float("nan") != float("nan")` ⇒ 不能用 `!=`（`marshal` 的 `float_nan` 用例命中）；
    - 自引用容器（`l.append(l)`）⇒ 递归必须有**环检测**（否则 `RecursionError`）。
    `dict` 比键序与值，集合按 `repr` 排序后比，其余落到 `==`。
    """
    seen = _seen if _seen is not None else set()
    if isinstance(first, float) and isinstance(second, float):
        # NaN 与自身"相同"（快照语义关心的是**有没有变**，不是 IEEE 相等）
        if first != first and second != second:
            return True
        return first == second
    if type(first) is not type(second):
        return False
    if isinstance(first, (list, tuple, dict, set, frozenset)):
        pair = (id(first), id(second))
        if pair in seen:
            # 这一对已经在比较栈上 ⇒ 认为此路相同（环不必展开）
            return True
        seen.add(pair)
        try:
            if isinstance(first, (list, tuple)):
                return len(first) == len(second) and all(
                    same(left, right, seen) for left, right in zip(first, second)
                )
            if isinstance(first, dict):
                if list(first.keys()) != list(second.keys()):
                    return False
                return all(same(first[key], second[key], seen) for key in first)
            return sorted(map(repr, first)) == sorted(map(repr, second))
        finally:
            seen.discard(pair)
    return bool(first == second)


def ensure_unchanged(before: object, after: object, label: str) -> None:
    """两次快照必须相同；不同就抛 `AssertionError`（生成器随即不产出该夹具）。"""
    if not same(before, after):
        raise AssertionError(
            f"用例「{label}」：探测期间接收者被改动了——\n"
            f"    探测前：{before!r}\n"
            f"    探测后：{after!r}\n"
            f"  ⇒ 被记录的值与用例名不再对应（`PLAN-milestones.md` §9.4 复核清单）；"
            f"请让写入类探测用**新**接收者，或把取值的探测移到改动之前。"
        )


@contextlib.contextmanager
def unchanged(label: str, *receivers: object) -> Iterator[None]:
    """守卫一段探测：退出时逐个比对快照（探测体抛异常时不比对，原异常照常上抛）。"""
    before = [snapshot(receiver) for receiver in receivers]
    yield
    after = [snapshot(receiver) for receiver in receivers]
    for index, (first, second) in enumerate(zip(before, after)):
        ensure_unchanged(first, second, f"{label}#{index}")


def self_check() -> None:
    """证明守卫**会绿也会红**（`CX-15`：每条检查必须会红会绿）。"""
    items = [1, 2, 3]
    with unchanged("不变的情形", items):
        _ = items[1:]
    try:
        with unchanged("被改的情形", items):
            items[1:3] = [9]
    except AssertionError:
        pass
    else:
        raise AssertionError("守卫没能拦住就地改动 ⇒ 它就是个假检查")
    if items != [1, 9]:
        raise AssertionError(f"自检自己污染了接收者：{items!r}")
    # **NaN**：`nan != nan`，但快照语义上"没变" ⇒ 不许误报（这条是 `marshal` 的 `float_nan`
    # 用例第一次接上守卫时抓出来的真 bug）
    nan = float("nan")
    with unchanged("NaN 不变", nan):
        _ = nan + 1.0
    # 嵌套容器里的 NaN 同样不误报
    with unchanged("嵌套 NaN 不变", [nan, {"x": nan}]):
        _ = 1
    # 自引用容器不许把比较器递归爆掉
    cyclic: list[object] = [1]
    cyclic.append(cyclic)
    with unchanged("自引用不变", cyclic):
        _ = len(cyclic)


if __name__ == "__main__":
    self_check()
    print("fixture_guard 自检通过：不变 ⇒ 绿；就地改动 ⇒ 红")
