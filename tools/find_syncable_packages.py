#!/usr/bin/env python3
"""**整包判一次**（`P3-26`）：上游"整包缺席"的包里，哪些**现在就能搬**。

`tools/find_syncable.py` 判的是**单个模块**（一次一枚 ✗）⇒ 顶层单文件 43/157 之后就上不去了 ✓
—— 缺的是"整包"这一档 ✓（`asyncio` 35／`multiprocessing` 23／`unittest` 13／`json` 6／`http` 5／`logging` 3 ✓）。

本工具把判据改成**整包**：
1. 枚举上游**整包**（目录里有 `__init__.py` ✓）且**本仓 `Lib/` 里缺席**的 ✓；
2. 逐个把整包拷进 `Lib/` ✓ ⇒ 跑 `import <包>` ✓；
3. **再过 `round-rule.md` §6.4 的第二道必检**：无关脚本 `print("startup ok")` 仍要绿 ✓
   （`find_syncable` 的"通过"**不算** ✓）；
4. 两道都绿 ⇒ **留下**（写进 `SLICE` 的候选 ✓）；否则 ⇒ **撤出**并把"卡在哪"报出来 ✓。

用法::

    python3 tools/find_syncable_packages.py            # 只判、留下通过者
    python3 tools/find_syncable_packages.py --dry-run  # 判完全部撤出（不留改动）
"""

from __future__ import annotations

import argparse
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LIB = ROOT / "Lib"
BIN = ROOT / "target" / "debug" / "pyawa"
PROBE = ROOT / "target" / "recon" / "pkg_sync"
STARTUP = ROOT / "target" / "recon" / "s0.py"


def upstream_prefix() -> pathlib.Path:
    sys.path.insert(0, str(ROOT / "tools"))
    from sync_lib import upstream_prefix as prefix  # noqa: PLC0415

    return prefix()


def packages(prefix: pathlib.Path) -> list[str]:
    found = []
    for init in prefix.rglob("__init__.py"):
        if "__pycache__" in init.parts or "site-packages" in init.parts:
            continue
        rel = init.parent.relative_to(prefix)
        parts = list(rel.parts)
        if not parts or not all(part.isidentifier() for part in parts):
            continue
        if len(parts) == 1:  # 顶层包（子包随父包一起搬 ✓）
            found.append(parts[0])
    return sorted(found)


def absent(name: str) -> bool:
    return not (LIB / name).exists() and not (LIB / f"{name}.py").exists()


def run_import(name: str) -> str:
    PROBE.mkdir(parents=True, exist_ok=True)
    script = PROBE / f"probe_{name}.py"
    script.write_text(f"import {name}\nprint('OK')\n")
    cache = PROBE / "__pyawa__"
    if cache.exists():
        shutil.rmtree(cache, ignore_errors=True)
    try:
        done = subprocess.run(
            [str(BIN), str(script)], capture_output=True, text=True, timeout=180, cwd=ROOT
        )
    except subprocess.TimeoutExpired:
        return "<超时>"
    if done.returncode == 0 and "OK" in done.stdout:
        return ""
    tail = [line.strip() for line in (done.stderr + done.stdout).splitlines() if line.strip()]
    return tail[-1][:110] if tail else f"退出码 {done.returncode}"


def startup_ok() -> bool:
    cache = ROOT / "target" / "recon" / "__pyawa__"
    if cache.exists():
        shutil.rmtree(cache, ignore_errors=True)
    done = subprocess.run([str(BIN), str(STARTUP)], capture_output=True, text=True, cwd=ROOT)
    return done.returncode == 0 and "startup ok" in done.stdout


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true", help="判完一律撤出（不留改动）")
    args = parser.parse_args()

    prefix = upstream_prefix()
    all_packages = packages(prefix)
    todo = [name for name in all_packages if absent(name)]
    print(f"上游顶层包 {len(all_packages)} 个 ⇒ 本仓缺席 **{len(todo)}** 个（逐个整包判 ✓）")
    movable, blocked = [], []
    for name in todo:
        target = LIB / name
        shutil.copytree(prefix / name, target, dirs_exist_ok=True)
        failure = run_import(name)
        if not failure and startup_ok():
            if args.dry_run:
                shutil.rmtree(target, ignore_errors=True)
            movable.append(name)
            print(f"  ✓ {name} ⇒ 两道必检都过（{'演练：已撤出' if args.dry_run else '已留在 Lib/'} ✓）")
        else:
            shutil.rmtree(target, ignore_errors=True)
            reason = failure or "无关脚本 `startup ok` 变红 ✗"
            blocked.append((name, reason))
            print(f"  ✗ {name} ⇒ {reason}")
    print(f"—— 可搬 **{len(movable)}** 个：{'、'.join(movable) if movable else '（无 ✓：说明整包这一档被基座挡住 ✗）'}")
    if blocked:
        print("—— 卡住的（按报错归并）——")
        tally: dict[str, list[str]] = {}
        for name, reason in blocked:
            tally.setdefault(reason, []).append(name)
        for reason, names in sorted(tally.items(), key=lambda item: -len(item[1])):
            print(f"  {len(names):3d}  {reason}")
            print(f"       例：{'、'.join(sorted(names)[:8])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
