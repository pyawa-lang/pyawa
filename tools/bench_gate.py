#!/usr/bin/env python3
"""**基准回退闸门**：跑 `tools/bench_hot.py` 四档，与 `tools/bench_baseline.json` 比，回退超阈值即红。

背景（2026-11-12 效率战役）：`bench_hot` 从 **214× CPython 打磨到 ~22×**（5 轮，约 10 倍提速 ✓）。
这个成果必须**不可回退** ✗ —— 否则以后任何一次"顺手重构"都可能把它丢回去。

用法::

    python3 tools/bench_gate.py                  # 用 target/release/pyawa 跑，与基线比（默认跑 2 次取小 ✓）
    python3 tools/bench_gate.py --runs 1         # 只跑一次（快，但噪声大）
    python3 tools/bench_gate.py --update         # 重录基线（确认变快/可接受后，需在提交信息里说明）
    python3 tools/bench_gate.py --tolerance 0.10
    python3 tools/bench_gate.py --bin target/debug/pyawa

退出码：**0 = 通过** ✓ ／ **99 = 有档回退超阈值** ✗ ／ **2 = 环境问题**（无二进制／无基线）
口径（重要 ✗）：
- 只在**同机、同二进制路径、同口径**下比较 ✓；机器/编译器/探针变了必须 `--update` 并说明 ✓
- 单次读数有噪声 ⇒ 默认容差 **10%**；**默认跑 4 次、逐档取较小值** ✓（`--runs 1` 可关；
  `builtin_call` 档的噪声源见台账 round 253：ASLR/GC 布局 ⇒ 单次可漂 10%+）；
  首轮超阈**自动 5 s 后复测两次**再判（真回退不会因复测变绿 ✓）
- CPython 的绝对比值是**报告级**（CPython 版本会变 ✓），不进闸门 ✗
"""
import argparse
import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASELINE = ROOT / "tools/bench_baseline.json"
TIERS = ("baseline", "attribute_read", "attribute_write", "builtin_call")
ROW = re.compile(r"^(%s)\s+([0-9.]+)$" % "|".join(TIERS))


def canonical_env() -> dict[str, str]:
    """**干净环境**（round 253）：实测同一二进制在**多一个环境变量**时 `builtin_call` 档会慢 ~13%
    （`env FOO=1` ⇒ 1.29 s vs 1.13 s；空的 `CARGO_HOME=` 也一样）——这是**环境布局敏感**，
    不是代码回退 ⇒ 闸门取"干净环境"与"继承环境"两次读数的**较小值**，避免被调用方的环境拖红 ✓。
    只留给运行必需的三项（pyawa 找自己的 `Lib` 靠可执行文件路径，不靠环境 ✓）。"""
    env = {"PATH": "/usr/bin:/bin:/usr/local/bin"}
    for key in ("HOME", "LANG", "LC_ALL", "TMPDIR"):
        value = os.environ.get(key)
        if value:
            env[key] = value
    return env


def machine_load() -> tuple[float, int]:
    """当前 1 分钟负载与可用核数（并发测量无效 ⇒ 高负载下不判 ✓）。"""
    try:
        load = float(pathlib.Path("/proc/loadavg").read_text().split()[0])
    except Exception:  # noqa: BLE001
        return 0.0, 1
    try:
        cores = len(os.sched_getaffinity(0))
    except Exception:  # noqa: BLE001
        cores = os.cpu_count() or 1
    return load, cores


def measure(binary: pathlib.Path, env: dict[str, str] | None = None) -> dict[str, float]:
    proc = subprocess.run(
        [str(binary), str(ROOT / "tools/bench_hot.py")],
        capture_output=True, text=True, cwd=ROOT, env=env,
    )
    out = {}
    for line in (proc.stdout or "").splitlines():
        m = ROW.match(line.strip())
        if m:
            out[m.group(1)] = float(m.group(2))
    return out


def compare(now: dict, base: dict, tol: float) -> list[str]:
    """返回回退超阈值的说明（空 ⇒ 全绿 ✓）。"""
    bad = []
    for tier in TIERS:
        old = float(base["tiers"][tier])
        new = now[tier]
        delta = (new - old) / old
        print(f"  {tier:<16} {old:>7.3f}s → {new:>7.3f}s  ({delta:+.1%})  {'✗ 回退' if delta > tol else '✓'}")
        if delta > tol:
            bad.append(f"{tier} 回退 {delta:+.1%}（基线 {old:.3f}s → {new:.3f}s）")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=str(ROOT / "target/release/pyawa"))
    ap.add_argument("--update", action="store_true")
    ap.add_argument("--tolerance", type=float, default=0.10)
    ap.add_argument("--runs", type=int, default=4,
                    help="每档取 N 次测量的**较小值**（默认 4 ✓：`builtin_call` 档受 ASLR/GC 布局影响，单次可漂 10%%+）")
    ap.add_argument("--inherit-env", action="store_true",
                    help="只跑**继承环境**（默认交替跑「干净环境／继承环境」取小 ✓）")
    args = ap.parse_args()

    binary = pathlib.Path(args.bin)
    if not binary.is_file():
        print(f"找不到二进制：{binary}", file=sys.stderr)
        return 2
    runs = max(1, args.runs)
    clean = canonical_env()
    # **交替环境**（round 253）：第 0 次用干净环境、第 1 次用继承环境 ⇒ 取小值；
    # `--inherit-env` 时全部用继承环境（调试用）。
    def sample(index: int) -> dict[str, float]:
        env = None if args.inherit_env or index % 2 == 1 else clean
        return measure(binary, env)

    # **负载守门**（round 262 教训）：并发测量无效 ⇒ 高负载下不判（不误红 ✗，也绝不假装通过 ✓）
    if os.environ.get("CI"):  # CI＝共享 runner，负载不可控 ⇒ 不判性能（只报数）
        _ = measure(binary)
        print("⚠ CI 环境：共享 runner 负载不可控 ⇒ **跳过基准判定**（请在本地空载复测 ✓）")
        return 0

    load, cores = machine_load()
    if load > cores * 0.7:
        print(f"⚠ 机器负载 {load:.2f}／{cores} 核 ⇒ **跳过判定**（bench 对负载敏感、并发测量无效；"
              f"强制判会误红 ✗）。请空载时重跑 ✓")
        return 0

    now = sample(0)
    if len(now) < len(TIERS):
        print(f"测量不完整（拿到 {sorted(now)}）✗", file=sys.stderr)
        return 2
    # **取多次较小值**（round 253）：墙钟读数只会被负载／降频**抬高** ⇒ 取小最接近真实能力 ✓。
    for _ in range(runs - 1):
        again = sample(_ + 1)
        if len(again) < len(TIERS):
            print(f"测量不完整（拿到 {sorted(again)}）✗", file=sys.stderr)
            return 2
        for tier in TIERS:
            now[tier] = min(now[tier], again[tier])

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
    bad = compare(now, base, tol)
    if bad:
        # **首轮红 ⇒ 5 s 后复测一次**（round 253）：墙钟读数会被**瞬时负载／降频**抬高，
        # 同一提交在安静时复测常常是绿的 ✓（实测 slowcheck 里 builtin_call 两次 1.286s，
        # 单独复测 1.134s）。真回退不会因为复测变绿 ✓。
        import time
        for attempt in range(2):
            print(f"\n首轮有回退 ⇒ 5 s 后复测（第 {attempt + 1}/2 次；噪声／布局漂移 ✓）", file=sys.stderr)
            time.sleep(5)
            again = sample(runs + attempt)
            if len(again) < len(TIERS):
                print(f"复测不完整（拿到 {sorted(again)}）✗", file=sys.stderr)
                return 2
            for tier in TIERS:
                now[tier] = min(now[tier], again[tier])
            print("复测后：")
            bad = compare(now, base, tol)
            if not bad:
                break

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
