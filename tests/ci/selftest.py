#!/usr/bin/env python3
"""tests/ci 的自检——证明每条检查**会红**。

`CONSTRAINTS.md` 的 `CX-15` 禁止用空实现充数。本脚本把仓库里的人读文档与 crate 源码
复制到临时目录，**逐条注入一处违规**，断言对应的检查确实失败；顺带确认干净副本上全绿。

用法::

    python3 tests/ci/selftest.py
"""

from __future__ import annotations

import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable

ROOT = pathlib.Path(__file__).resolve().parents[2]

#: 检查要读的文件（不含 `target/` 之类的构建产物）。
COPY = ("README.md", "AGENTS.md", "docs", "agents-rules", "crates", "tests", "tools")

Mutation = Callable[[pathlib.Path], None]

#: (期望变红的检查项, 相对路径, 注入方式)
CASES: tuple[tuple[str, str, Mutation], ...] = (
    (
        "T-CX-1",
        "docs/SPEC-object-model.md",
        # 故意拆开字符串：本文件也在 `CX-1` 的扫描面里，写成一个整体会把自己判成悬空引用
        lambda path: path.write_text(
            path.read_text(encoding="utf-8")
            + "\n悬空引用 `OM-" + "999`，另见 §13-" + "999。\n",
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-2",
        "README.md",
        lambda path: path.write_text(
            # 完成度计数与 SPEC-INDEX §1 对不上（不写死数字：份数会随文档集推进而变）
            re.sub(r"已写\s*\d+\s*份", "已写 99 份", path.read_text(encoding="utf-8")),
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-3",
        "crates/pyawa-core/src/lib.rs",
        lambda path: path.write_text(
            "static mut COUNTER: u32 = 0;\n" + path.read_text(encoding="utf-8"),
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-4",
        "crates/pyawa-core/src/lib.rs",
        lambda path: path.write_text(
            "use std::fs;\n" + path.read_text(encoding="utf-8"),
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-5",
        "crates/pyawa-core/src/lib.rs",
        lambda path: path.write_text(
            "fn f() -> Rc<u8> { todo!() }\n" + path.read_text(encoding="utf-8"),
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-6",
        "tests/ci/README.md",
        lambda path: path.write_text(
            # 把标"不能"的约束写成"已实现"（CX-14 禁止）
            path.read_text(encoding="utf-8").replace(
                "| `CX-8` | 未实现 |", "| `CX-8` | 已实现 |"
            ),
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-7",
        "docs/SPEC-object-model.md",
        lambda path: path.write_text(
            # 把「尚未写出」节改名 ⇒ 该规格再也看不到自己的缺口（CX-17）
            path.read_text(encoding="utf-8").replace(
                "尚未写出（本规格自己缺的节）", "缺口清单"
            ),
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-1",
        "docs/CONSTRAINTS.md",
        lambda path: path.write_text(
            # 同一编号在同一 owner 文件里定义两次 ⇒ 重复定义
            path.read_text(encoding="utf-8").rstrip()
            + "\n\n- **CX-" + "13** 同号重复定义（自检注入）\n",
            encoding="utf-8",
        ),
    ),
    (
        "T-CX-1",
        "docs/SPEC-INDEX.md",
        lambda path: path.write_text(
            # 把一份规格改回"待写"：它的编号立刻变成"未写规格"，未标注临时假设的引用必须报红
            # ——这正是当初硬编码 UNWRITTEN 造成 138 条假红的那类状态变化
            path.read_text(encoding="utf-8").replace("| `TS-` | v0 |", "| `TS-` | 待写 |"),
            encoding="utf-8",
        ),
    ),
)


def snapshot(destination: pathlib.Path) -> pathlib.Path:
    """把检查要读的文件复制一份，返回副本根目录。"""
    destination.mkdir(parents=True, exist_ok=True)
    for item in COPY:
        source = ROOT / item
        target = destination / item
        if source.is_dir():
            shutil.copytree(source, target)
        else:
            shutil.copy2(source, target)
    return destination


def run_checker(root: pathlib.Path) -> dict[str, str]:
    """在副本里跑检查脚本，返回「检查项 → PASS／FAIL」。"""
    completed = subprocess.run(
        [sys.executable, "tests/ci/check.py"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    statuses: dict[str, str] = {}
    for line in completed.stdout.splitlines():
        if line[:6] in ("[PASS]", "[FAIL]"):
            statuses[line.split()[1]] = line[1:5]
    return statuses


def main() -> int:
    failures: list[str] = []

    with tempfile.TemporaryDirectory(prefix="pyawa-ci-selftest-") as directory:
        workdir = pathlib.Path(directory)

        baseline = run_checker(snapshot(workdir / "baseline"))
        if not baseline:
            print("× 基线：检查脚本没输出任何检查项")
            return 1
        print("基线：" + " ".join(f"{k}={v}" for k, v in sorted(baseline.items())))
        for test_id, status in sorted(baseline.items()):
            if status != "PASS":
                failures.append(f"基线不干净：{test_id}={status}")

        for index, (test_id, relative, mutate) in enumerate(CASES):
            root = snapshot(workdir / f"case-{index}")
            mutate(root / relative)
            status = run_checker(root).get(test_id)
            verdict = "会红" if status == "FAIL" else f"没红（{status}）"
            print(f"注入违规 → {test_id}（{relative}）: {verdict}")
            if status != "FAIL":
                failures.append(f"{test_id} 注入违规后没有变红")

    print()
    if failures:
        for failure in failures:
            print(f"× {failure}")
        return 1
    print(f"√ {len(CASES) + 1} 项断言通过：干净副本全绿，每条检查都能变红")
    return 0


if __name__ == "__main__":
    sys.exit(main())
