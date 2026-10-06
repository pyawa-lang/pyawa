#!/usr/bin/env python3
"""选活机械化：对 `Lib/` 里已入库的模块**逐个导入**，按"最后一行错误签名"聚类排行。

用法：
    python3 tools/next_work.py [--jobs N] [--top N] [--lib Lib]

为什么：`round-rule.md` §6 要求"取活按阻塞最多模块的报错签名队列，而不是按 fan-in 想象"。
本工具产出那张队列：哪一条报错压着最多的模块 ⇒ 下一条活就取它。

判据口径：
- 只探**顶层的** `Lib/*.py`（包内部的模块不单独算 ✓）；
- 用 `python3 -S`／我们的 CLI **两个**都跑？不 —— 本工具只探**我们**这一侧 ✓；
- "通过"＝退出码 0；"失败"＝取 stderr/stdout 的**最后一行**做签名 ✓（去掉路径/行号噪声）。
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "debug" / "pyawa"
PROBE_DIR = ROOT / "target" / "recon" / "next_work"

NOISE = [
    (re.compile(r"\d+"), "N"),                      # 行号/列号/计数
    (re.compile(r"0x[0-9a-fA-F]+"), "0xP"),         # 指针
    (re.compile(r"'[^']*'"), "'S'"),                # 单引号里的具体名字
    (re.compile(r'"[^"]*"'), '"S"'),
]


def signature(text: str) -> str:
    line = ""
    for raw in reversed(text.splitlines()):
        raw = raw.strip()
        if raw:
            line = raw
            break
    if "ModuleNotFoundError" in line or "ImportError" in line:
        # **保留缺失的名字** ✓：它才是可执行的队列键（"谁的缺席压着多少模块"）
        return line[:160]
    for pattern, repl in NOISE:
        line = pattern.sub(repl, line)
    return line[:160] or "<空输出>"


def probe(module: str, timeout: float) -> tuple[str, str]:
    PROBE_DIR.mkdir(parents=True, exist_ok=True)
    script = PROBE_DIR / f"probe_{module}.py"
    script.write_text(f'import {module}\nprint("OK")\n')
    env_cache = PROBE_DIR / "__pyawa__"
    if env_cache.exists():
        for child in env_cache.iterdir():
            if child.is_dir():
                import shutil

                shutil.rmtree(child, ignore_errors=True)
    try:
        done = subprocess.run(
            [str(BIN), str(script)],
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=ROOT,
        )
    except subprocess.TimeoutExpired:
        return module, "<超时>"
    if done.returncode == 0:
        return module, ""
    return module, signature(done.stderr + done.stdout)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--top", type=int, default=12)
    ap.add_argument("--timeout", type=float, default=30.0)
    ap.add_argument("--lib", default="Lib")
    args = ap.parse_args()

    lib = ROOT / args.lib
    def dotted(path: pathlib.Path):
        rel = path.relative_to(lib).with_suffix("")
        parts = list(rel.parts)
        if parts and parts[-1] == "__init__":
            parts = parts[:-1]
        if not parts or not all(part.isidentifier() for part in parts):
            return None
        return ".".join(parts)

    modules = sorted(
        name for name in (dotted(p) for p in lib.rglob("*.py")) if name
    )
    if not modules:
        print(f"没找到模块：{lib}", file=sys.stderr)
        return 1

    tally: collections.Counter[str] = collections.Counter()
    examples: dict[str, list[str]] = collections.defaultdict(list)
    ok = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for module, sig in pool.map(lambda m: probe(m, args.timeout), modules):
            if not sig:
                ok += 1
                continue
            tally[sig] += 1
            examples[sig].append(module)

    print(f"探测 {len(modules)} 个顶层模块：通过 {ok} ✗ {len(modules) - ok}")
    print(f"—— 报错签名排行（前 {args.top}）——")
    for sig, count in tally.most_common(args.top):
        sample = "、".join(examples[sig][:6])
        print(f"{count:4d}  {sig}")
        print(f"      例：{sample}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
