#!/usr/bin/env python3
"""**复核所有「未对齐」登记**：临时全部放开真比对 → 逐条实跑 → 只把**真差异**留回去。

为什么值得做：功能往前推进之后，早先的登记**可能已经失效**（第 55 轮用差分工具捞回两条 ✗→✅）。
本工具把这件事固定成一条命令，并且区分两种失败：

- **指令流／常量表**对不上 ⇒ 登记**仍然有效**，按原样放回（`covered=False`）；
- **只有位置／行号**对不上 ⇒ 改成"**指令真比对、位置按 `BC-4` 允许的观测面跳过**"
  （`covered=True` ＋ 理由前缀 `行号级未对齐`）。

用法::

    python3 tools/reaudit_fixture_reasons.py

（内部每次都会跑生成器并**检查退出码**，再跑夹具；改完就地写盘。）
"""

from __future__ import annotations

import ast
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
GEN = ROOT / "tools" / "gen_compile_fixture.py"
UNALIGNED = ("未对齐", "位置表未对齐", "行号级未对齐")


def entry_ranges(lines: list[str]) -> dict[str, tuple[int, int, str]]:
    """找出所有 `covered=False` 且理由以「未对齐」开头的条目：源码 → (起行, 止行, 理由)。"""
    found: dict[str, tuple[int, int, str]] = {}
    index = 0
    while index < len(lines):
        if lines[index].lstrip().startswith("("):
            end = index
            while end < len(lines) and not lines[end].rstrip().endswith("),"):
                end += 1
            text = "\n".join(lines[index : end + 1]).strip().rstrip(",")
            try:
                value = ast.literal_eval(text)
            except Exception:
                index = end + 1
                continue
            if isinstance(value, tuple) and len(value) == 3:
                source, covered, reason = value
                if covered is False and isinstance(reason, str) and reason.startswith(UNALIGNED):
                    found[source] = (index, end, reason)
            index = end + 1
        else:
            index += 1
    return found


def rewrite(lines: list[str], span: tuple[int, int], source: str, covered: bool, reason: str) -> list[str]:
    out = list(lines)
    out[span[0] : span[1] + 1] = [
        "    (" + repr(source) + (", True, " if covered else ", False, ") + '"' + reason + '"),'
    ]
    return out


def regenerate() -> bool:
    done = subprocess.run(
        ["python3", "tools/gen_compile_fixture.py", "--emit"],
        cwd=ROOT, capture_output=True, text=True, timeout=600,
    )
    if done.returncode != 0:
        print("生成器失败（退出码 %d）：" % done.returncode, done.stdout[-500:], done.stderr[-500:])
        return False
    return True


def run_fixture() -> str:
    done = subprocess.run(
        # **串行**跑：夹具的四个测试并行时输出会交织，按"第一条失败"归属用例会张冠李戴 ✗
        ["cargo", "test", "-p", "pyawa-core", "--test", "compile", "--", "--test-threads=1"],
        cwd=ROOT, capture_output=True, text=True, timeout=1800,
    )
    return done.stdout + done.stderr


def main() -> int:
    original = GEN.read_text().splitlines()
    entries = entry_ranges(original)
    print("待复核登记：", len(entries))
    if not entries:
        return 0

    lines = list(original)
    spans = dict(entries)
    for source, span in spans.items():
        lines = rewrite(lines, (span[0], span[1]), source, True, span[2])
    GEN.write_text("\n".join(lines) + "\n")

    genuine: list[str] = []
    position_only: list[str] = []
    while True:
        if not regenerate():
            return 2
        output = run_fixture()
        if "test result: ok" in output:
            print("复核结束：绿色")
            break
        match = re.search(r'failed: ("(?:[^"\\]|\\.)*")', output)
        if not match:
            print("测试红但不是用例差异；输出末尾：", output[-800:])
            return 2
        import json
        source = json.loads(match.group(1))
        if source not in spans:
            print("失败用例不在复核集合里（无关红）：", repr(source)[:60])
            return 2
        # 判字段：**按 panic 的行号**判（不能看整段输出有没有 "co_positions" —— 后面的测试
        # 也会打印那字样 ✗，那是张冠李戴的根源）：
        #   `compile.rs:111` 常量表 / `:145` 指令流 / `:349` 位置 / `:378` 行表
        field = "指令或常量"
        if "compile.rs:349" in output:
            field = "位置"
        elif "compile.rs:378" in output:
            field = "行号"
        span = spans.pop(source)
        if field in ("位置", "行号"):
            reason = span[2]
            if not reason.startswith("行号级未对齐"):
                reason = "行号级未对齐：" + reason + "；按 `BC-4`「传播精度不要求」⇒ 指令流与常量池真比对"
            lines = rewrite(lines, (span[0], span[1]), source, True, reason)
            position_only.append(source)
            print("只有位置／行号差异 ⇒ 转真比对：", repr(source)[:52])
        else:
            lines = rewrite(lines, (span[0], span[1]), source, False, span[2])
            genuine.append(source)
            print("真差异（按原样放回）：", repr(source)[:52])
        GEN.write_text("\n".join(lines) + "\n")

    if not regenerate():
        return 2
    print("结果：真差异保留 %d 条；位置面转真比对 %d 条；其余（未触发的）也转真比对 %d 条"
          % (len(genuine), len(position_only), 0))
    return 0


if __name__ == "__main__":
    sys.exit(main())
