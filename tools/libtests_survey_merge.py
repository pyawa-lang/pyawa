#!/usr/bin/env python3
"""**合并分片 survey 结果**（round 140）。

本会话单条命令 ~600 s 上限 ⇒ 官方全量 survey 常按 `--start/--end` 分片跑（每片各自写
`target/libtests_survey.tsv`，跑完把那份 cp 到 `target/recon/<名字>.tsv` ✓）。本工具把这些分片
逐文件合并成**一份** `target/libtests_survey.tsv`（同文件后来者覆盖 ✓）并打印与正式口径一致的统计 ✓。

用法：
    python3 tools/libtests_survey_merge.py target/recon/p341slow/sv_*.tsv \
        [--extra 名字:kind:理由] [--out target/libtests_survey.tsv]

`--extra` 用于补"分片跑不了的单文件"（例如 `test_ntpath` 超慢，单独按已知口径记 timeout ✓）。
"""
from __future__ import annotations

import argparse
import pathlib
import sys

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_OUT = WORKSPACE / "target/libtests_survey.tsv"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("parts", nargs="+", help="分片 tsv 路径（同文件后来者覆盖）")
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    ap.add_argument(
        "--extra",
        action="append",
        default=[],
        metavar="NAME:KIND:REASON",
        help="补一个分片外的文件（KIND 用 ok/runtime/unimplemented/timeout/syntax）",
    )
    args = ap.parse_args()

    rows: dict[str, list[str]] = {}
    for part in args.parts:
        path = pathlib.Path(part)
        if not path.is_file():
            sys.exit(f"找不到分片：{path}")
        for line in path.read_text(encoding="utf-8").splitlines()[1:]:
            fields = line.split("\t")
            if len(fields) >= 2 and fields[0].endswith(".py"):
                rows[fields[0]] = fields
    for extra in args.extra:
        name, kind, reason = extra.split(":", 2)
        rows[name] = [name, kind, reason]
    if not rows:
        sys.exit("没有任何行可合并 ✗")

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as fh:
        fh.write("file\tkind\treason\n")
        for name in sorted(rows):
            fh.write("\t".join(rows[name]) + "\n")

    kinds = ["ok", "syntax", "unimplemented", "runtime", "timeout"]
    counts = {kind: sum(1 for row in rows.values() if row[1] == kind) for kind in kinds}
    total = len(rows)
    compilable = total - counts["syntax"]
    print(f"共 {total} 个 test_*.py（分片 {len(args.parts)} 份）")
    print(f"  可编译（非语法错）：{compilable}/{total} = {100 * compilable / max(total, 1):.1f}%")
    print(f"  跑通（退出 0）：  {counts['ok']}/{total} = {100 * counts['ok'] / max(total, 1):.1f}%")
    print(
        f"  语法错 {counts['syntax']} ｜ 未实现 {counts['unimplemented']}"
        f" ｜ 运行期错 {counts['runtime']} ｜ 超时 {counts['timeout']}"
    )
    print(f"  明细 → {out}")


if __name__ == "__main__":
    main()
