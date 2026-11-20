#!/usr/bin/env python3
"""**P0-2 可移植门：比较两个平台的 canonical JSON，判"同脚本同结果"**（`PLAN §12.26`）。

用法：`python3 tests/portability/compare.py linux.json windows.json`
退出码：0 ＝ 全部一致（或已在 `known_diffs.json` 登记）；1 ＝ 有未登记差异。
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent


def load(path: pathlib.Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_known(path: pathlib.Path | None) -> dict[str, str]:
    if path is None or not path.is_file():
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    # 允许两种写法：{"script.py": "理由"} 或 {"scripts": {"script.py": "理由"}}
    return data.get("scripts", data)


def show_diff(name: str, left: dict, right: dict) -> None:
    print(f"--- {name}: 两平台结果不同 ---")
    if left["exit"] != right["exit"]:
        print(f"  exit: {left['exit']} != {right['exit']}")
    if left["stdout"] != right["stdout"]:
        import difflib
        diff = difflib.unified_diff(
            left["stdout"], right["stdout"], fromfile="linux/stdout", tofile="windows/stdout", lineterm=""
        )
        for line in diff:
            print("  " + line)
    if left["stderr"] != right["stderr"]:
        import difflib
        diff = difflib.unified_diff(
            left["stderr"], right["stderr"], fromfile="linux/stderr", tofile="windows/stderr", lineterm=""
        )
        for line in diff:
            print("  " + line)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("left", type=pathlib.Path, help="平台 A 的 canonical JSON")
    parser.add_argument("right", type=pathlib.Path, help="平台 B 的 canonical JSON")
    parser.add_argument("--known-diffs", type=pathlib.Path, default=HERE / "known_diffs.json")
    args = parser.parse_args()

    left = load(args.left)
    right = load(args.right)
    known = load_known(args.known_diffs)

    left_results = left.get("results", {})
    right_results = right.get("results", {})
    names = sorted(set(left_results) | set(right_results))
    mismatches = []
    known_hits = []
    for name in names:
        if name not in left_results or name not in right_results:
            mismatches.append(name)
            print(f"--- {name}: 只在一侧出现 ---")
            continue
        if name in known:
            known_hits.append((name, known[name]))
            continue
        if left_results[name] != right_results[name]:
            mismatches.append(name)
            show_diff(name, left_results[name], right_results[name])

    for name, reason in known_hits:
        print(f"KNOWN {name}: {reason}")
    print(
        f"portability compare：{len(names)} 个脚本，{len(names) - len(mismatches)} 一致"
        f"（其中已知差异 {len(known_hits)}），未登记差异 {len(mismatches)}"
    )
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())
