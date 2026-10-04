#!/usr/bin/env python3
"""**只给一条夹具用例做两侧指令流差分**：临时放开真比对 → 跑夹具 → 打印**首个差异下标**。

为什么要有这个工具：夹具失败时吐的是两串长列表，靠眼睛扫必然看错（第 275 轮因此误判过一次 ✗）。
它把「改一处 → 跑生成器（**查退出码**）→ 跑夹具 → 取首个差异」这条循环固定下来。

用法::

    python3 tools/why_fixture_differs.py $'with a:\n    with b:\n        x = 1'
    python3 tools/why_fixture_differs.py ... --keep     # 保留放开后的状态（调试用）

退出码：0 = 两侧**逐字节一致**；1 = 有差异（打印首个差异及上下文）；2 = 用不了。
"""

from __future__ import annotations

import ast
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
GEN = ROOT / "tools" / "gen_compile_fixture.py"


def escaped(source: str) -> str:
    """文件里那段源码的**文本形态**（换行是 `\\n` 两个字符、反斜杠翻倍），不含两端引号。"""
    return source.replace("\\", "\\\\").replace("\n", "\\n")


def find_entry(lines: list[str], source: str) -> tuple[int, int]:
    """返回该源码所在条目的**行号范围**（多行写法要往上找 `(`、往下找到 `),`）。"""
    needle = escaped(source)
    for index, line in enumerate(lines):
        if needle in line:
            start = index
            while start > 0 and not lines[start].lstrip().startswith("("):
                start -= 1
            end = index
            while not lines[end].rstrip().endswith("),"):
                end += 1
            return start, end
    raise SystemExit("找不到该用例：" + repr(source)[:60])


def render(source: str) -> str:
    return "    (" + repr(source) + ', True, ""),'


def parse_streams(output: str) -> tuple[list, list] | None:
    match = re.search(r"的指令流\n\s*left: (\[.*?\])\n\s*right: (\[.*?\])\n", output, re.S)
    if not match:
        return None

    def to_py(raw: str) -> list:
        return ast.literal_eval(re.sub(r"Some\(([^()]*)\)", r"\1", raw))

    return to_py(match.group(1)), to_py(match.group(2))


def regen() -> int:
    done = subprocess.run(
        ["python3", "tools/gen_compile_fixture.py", "--emit"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=600,
    )
    if done.returncode != 0:
        print("生成器失败（退出码 %d）：" % done.returncode)
        print(done.stdout[-800:], done.stderr[-800:])
    return done.returncode


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    source = sys.argv[1]
    keep = "--keep" in sys.argv
    original = GEN.read_text().splitlines()
    start, end = find_entry(original, source)

    working = list(original)
    working[start : end + 1] = [render(source)]
    GEN.write_text("\n".join(working) + "\n")
    try:
        if regen() != 0:
            return 2
        done = subprocess.run(
            ["cargo", "test", "-p", "pyawa-core", "--test", "compile"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=1200,
        )
        output = done.stdout + done.stderr
        if "test result: ok" in output:
            print("[一致] 两侧指令流逐字节相同：", repr(source)[:70])
            return 0
        streams = parse_streams(output)
        if streams is None:
            print("测试失败但没吐指令流对照；输出末尾：")
            print(output[-1200:])
            return 2
        ours, ref = streams
        print("长度：本层 %d / 参照 %d" % (len(ours), len(ref)))
        shown = 0
        for index in range(max(len(ours), len(ref))):
            left = ours[index] if index < len(ours) else None
            right = ref[index] if index < len(ref) else None
            if left != right:
                low = max(0, index - 2)
                print("差异（第 %d 项）：" % index)
                print("  本层：", ours[low : index + 3])
                print("  参照：", ref[low : index + 3])
                shown += 1
                if shown >= 3:
                    break
        return 1
    finally:
        if not keep:
            GEN.write_text("\n".join(original) + "\n")
            regen()


if __name__ == "__main__":
    sys.exit(main())
