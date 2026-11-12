#!/usr/bin/env python3
"""**基准回退闸门**：跑 `tools/bench_hot.py` 四档，与 `tools/bench_baseline.json` 比，回退超阈值即红。

背景（2026-11-12 效率战役）：`bench_hot` 从 **214× CPython 打磨到 ~22×**（5 轮，约 10 倍提速 ✓）。
这个成果必须**不可回退** ✗ —— 否则以后任何一次"顺手重构"都可能把它丢回去。

用法::

    python3 tools/bench_gate.py                  # 用 target/release/pyawa 跑，与基线比
    python3 tools/bench_gate.py --update         # 重录基线（确认变快/可接受后，需在提交信息里说明）
    python3 tools/bench_gate.py --tolerance 0.10
    python3 tools/bench_gate.py --bin target/debug/pyawa

退出码：**0 = 通过** ✓ ／ **99 = 有档回退超阈值** ✗ ／ **2 = 环境问题**（无二进制／无基线）
口径（重要 ✗）：
- 只在**同机、同二进制路径、同口径**下比较 ✓；机器/编译器/探针变了必须 `--update` 并说明 ✓
- 单次读数有噪声 ⇒ 默认容差 **10%**；判趋势请跑两次取小 ✓
- CPython 的绝对比值是**报告级**（CPython 版本会变 ✓），不进闸门 ✗
"""
import argparse
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASELINE = ROOT / "tools/bench_baseline.json"
TIERS = ("baseline", "attribute_read", "attribute_write", "builtin_call")
ROW = re.compile(r"^(%s)\s+([0-9.]+)$" % "|".join(TIERS))


def measure(binary: pathlib.Path) -> dict[str, float]:
    proc = subprocess.run(
        [str(binary), str(ROOT / "tools/bench_hot.py")],
        capture_output=True, text=True, cwd=ROOT,
    )
    out = {}
    for line in (proc.stdout or "").splitlines():
        m = ROW.match(line.strip())
        if m:
            out[m.group(1)] = float(m.group(2))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=str(ROOT / "target/release/pyawa"))
    ap.add_argument("--update", action="store_true")
    ap.add_argument("--tolerance", type=float, default=0.10)
    args = ap.parse_args()

    binary = pathlib.Path(args.bin)
    if not binary.is_file():
        print(f"找不到二进制：{binary}", file=sys.stderr)
        return 2
    now = measure(binary)
    if len(now) < len(TIERS):
        print(f"测量不完整（拿到 {sorted(now)}）✗", file=sys.stderr)
        return 2

    if args.update:
        head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT,
                              capture_output=True, text=True).stdout.strip()
        data = {
            "machine": "AMD Ryzen 5 7640HS (12 逻辑核) / 18 GB / Linux",
            "binary": args.bin,
            "commit": head,
            "date": subprocess.run(["date", "+%Y-%m-%d"], cwd=ROOT,
                                   capture_output=True, text=True).stdout.strip(),
            "tolerance": args.tolerance,
            "tiers": now,
        }
        BASELINE.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"基线已重录 ✓（commit {head}）→ {BASELINE}")
        for k in TIERS:
            print(f"  {k:<16} {now[k]:.3f}s")
        return 0

    if not BASELINE.is_file():
        print(f"没有基线：{BASELINE}（先跑 --update）", file=sys.stderr)
        return 2
    base = json.loads(BASELINE.read_text(encoding="utf-8"))
    tol = args.tolerance or base.get("tolerance", 0.10)

    print(f"基线：{base.get('date')} @ {base.get('commit')}（容差 {tol:.0%}）")
    bad = []
    for tier in TIERS:
        old = float(base["tiers"][tier])
        new = now[tier]
        delta = (new - old) / old
        flag = "✗ 回退" if delta > tol else "✓"
        print(f"  {tier:<16} {old:>7.3f}s → {new:>7.3f}s  ({delta:+.1%})  {flag}")
        if delta > tol:
            bad.append(f"{tier} 回退 {delta:+.1%}（基线 {old:.3f}s → {new:.3f}s）")

    if bad:
        print("\n基准回退超阈值 ✗：", file=sys.stderr)
        for b in bad:
            print(f"  - {b}", file=sys.stderr)
        print("（若是有意变更：确认后跑 --update，并在提交信息里写明原因 ✓）", file=sys.stderr)
        return 99
    print("\n✓ 四档均在容差内")
    return 0


if __name__ == "__main__":
    sys.exit(main())
