#!/usr/bin/env python3
"""按**夹具**增量刷新 `tools/compile-positions-census.tsv`（位置差异的唯一事实在夹具里）。

**只追加缺的条目、只改头部计数**，不整体重写（整体重写会级联丢条目 ✗）。
"""
import json
import pathlib

FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "crates/pyawa-core/tests/fixture-compile-3.14.json"
CENSUS = pathlib.Path(__file__).resolve().parent / "compile-positions-census.tsv"


def escaped(source: str) -> str:
    return '"' + source.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def main() -> int:
    cases = json.loads(FIXTURE.read_text())["cases"]
    lines = CENSUS.read_text().splitlines()
    header = [line for line in lines if line.startswith("#")]
    entries = [line for line in lines if line.strip() and not line.startswith("#")]
    have = {line.split("\t")[0] for line in entries}

    position_bad = [(s, e.get("positions_uncovered_because", "")) for s, e in cases.items()
                    if e["covered"] and not e.get("positions_covered", True)]
    line_bad = [s for s, e in cases.items() if e["covered"] and not e.get("lines_covered", True)]

    added = 0
    for source, why in position_bad:
        key = escaped(source)
        if key not in have:
            entries.append(key + "\t" + (why or "位置表未对齐：待补测").replace("\t", " "))
            have.add(key)
            added += 1

    # 头部**只改计数行**（其余保持提交版原文 ✓）
    counts = (f"# 现状（刷新时自动更新）：**列跨度差异 {len(position_bad)} 条**、"
              f"**行号级差异 {len(line_bad)} 条**（两者可重叠）；")
    for index, line in enumerate(header):
        if line.startswith("# 现状"):
            header[index] = counts
            break
    else:
        header.append(counts)
    CENSUS.write_text("\n".join(header + entries) + "\n")
    print(f"新增 {added} 条；合计 {len(entries)} 条（列跨度差异 {len(position_bad)}、行号级 {len(line_bad)}）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
