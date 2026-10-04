#!/usr/bin/env python3
"""**找出"现在就能同步"的模块**（判据① 的分子就是这么长的）。

口径：一个模块能同步 ⇔ 把它按上游**逐字节**放进 `Lib/` 之后，**只有 `Lib/` ＋ 内建**也能
`import` 成功（参照侧本来就成功；`MS-10` 的三分类不变）。

做法（不动已有文件 ✓、失败即**撤回** ✓）：
1. 拿上游全量 `Lib/**/*.py` 的模块名（与 `lib_import_ratio.py` 同一口径）；
2. 逐个（`--jobs` 路并行）：**拷进** `Lib/` ⇒ 用对拍 runner 跑 `import <模块>` ⇒ 成功**留下**、
   失败**删掉**；
3. 反复几轮直到没有新增（一个模块的依赖可能也是这一轮刚同步进来的 ✓）。

输出：本轮新增的模块清单（写进 `tools/sync_lib.py` 的 `SLICE` 就靠它 ✓）。
"""

from __future__ import annotations

import argparse
import concurrent.futures
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LIB = ROOT / "Lib"
UPSTREAM = pathlib.Path(sys.executable).parent.parent / "lib" / f"python{sys.version_info.major}.{sys.version_info.minor}"


def upstream_prefix() -> pathlib.Path:
    import sysconfig

    return pathlib.Path(sysconfig.get_paths()["stdlib"])


def runner_path() -> str:
    found = sorted(
        (ROOT / "target" / "debug" / "deps").glob("conformance-*"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for path in found:
        if path.is_file() and path.suffix != ".d":
            return str(path)
    raise SystemExit("找不到对拍 runner（先 `cargo test -p pyawa-abi --test conformance --no-run`）")


def modules_of(prefix: pathlib.Path) -> list[str]:
    names: set[str] = set()
    for path in prefix.rglob("*.py"):
        if "__pycache__" in path.parts:
            continue
        relative = path.relative_to(prefix)
        parts = list(relative.parts)
        if parts[-1] == "__init__.py":
            parts = parts[:-1]
        else:
            parts[-1] = path.stem
        if parts:
            names.add(".".join(parts))
    return sorted(names)


COPY_LOCK = __import__("threading").Lock()


def copy_in(prefix: pathlib.Path, module: str) -> pathlib.Path | None:
    """把一个模块拷进 `Lib/`（包则整目录）；返回**记下来要撤回的**路径。

    **加锁** ✓：并行时同一个包会被两条模块名一起试到（`pkg` 与 `pkg.sub`）✓ ⇒ 先查后拷会撞 ✓。
    """
    relative = pathlib.Path(*module.split("."))
    source_file = prefix / relative.with_suffix(".py")
    target_file = LIB / relative.with_suffix(".py")
    source_package = prefix / relative
    target_package = LIB / relative
    with COPY_LOCK:
        return copy_in_locked(prefix, relative, source_file, target_file, source_package, target_package)


def copy_in_locked(
    prefix: pathlib.Path,
    relative: pathlib.Path,
    source_file: pathlib.Path,
    target_file: pathlib.Path,
    source_package: pathlib.Path,
    target_package: pathlib.Path,
) -> pathlib.Path | None:
    del prefix
    if (source_package / "__init__.py").is_file():
        if target_package.exists():
            return None
        shutil.copytree(source_package, target_package, ignore=shutil.ignore_patterns("__pycache__"))
        return target_package
    if source_file.is_file():
        if target_file.exists():
            return None
        target_file.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_file, target_file)
        return target_file
    return None


def wait(prefix: pathlib.Path) -> int:
    """把 `Lib/` 与上游同步（CX-8）并报告这次新增了多少。"""
    copied = 0
    for path in sorted(LIB.rglob("*.py")):
        if "__pycache__" in path.parts:
            continue
        relative = path.relative_to(LIB)
        source = prefix / relative
        if not source.is_file():
            continue
        if not path.read_bytes() == source.read_bytes():
            print(f"  ✗ {relative} 与上游不一致")
            return 1
        copied += 1
    print(f"  ✓ `Lib/` 共 {copied} 个文件，逐字节与上游一致")
    return 0


def probe(runner: str, module: str, scratch: pathlib.Path) -> bool:
    source = scratch / (module.replace(".", "_") + ".py")
    source.write_text(f"import {module}\n")
    environment = {
        **__import__("os").environ,
        "PYAWA_CONFORMANCE_SOURCE": str(source.resolve()),
        "PYAWA_CONFORMANCE_PROBES": "0",
    }
    try:
        done = subprocess.run(
            [runner, "--exact", "pyawa_side_runner", "--nocapture"],
            env=environment,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return False
    return "exit=0" in done.stdout


def prune_noncompiling(rounds: int = 4) -> int:
    """**编不过的删掉** ✓：`cargo test -p pyawa-core --test lib_compile` 是那道闸门（`Lib/` 里
    **每个**文件都要过编译期不变量 ✓ ⇒ 判据① 要的"能 import"必须先"能编" ✓）。

    工具量出来的模块是**能 import** 的 ✓，但**包**会连带拷进一批子模块 ✗ —— 那些子模块既没被 import、
    也可能编不过 ✗ ⇒ 这里按闸门报出来的清单**逐个删掉** ✓（删到闸门绿为止 ✓）。
    """
    removed: list[str] = []
    for _ in range(rounds):
        environment = {**__import__("os").environ, "PYAWA_TRACE_COMPILE_FILE": "1"}
        done = subprocess.run(
            ["cargo", "test", "-p", "pyawa-core", "--test", "lib_compile", "--", "--nocapture"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=1800,
            env=environment,
        )
        if done.returncode == 0:
            break
        output = done.stdout + done.stderr
        failing = sorted(
            {
                line.strip().strip('",')
                for line in output.splitlines()
                if line.strip().startswith('"') and line.strip().endswith('",')
            }
        )
        if not failing:
            # **编译器自己 panic** 的那一类（`跳转目标标签 … 从未落点` ✓）：门不会给出清单 ✗ ⇒
            # 用测试里那行 `[编译扫描] <文件>` 的**最后一条**（就是崩的那个 ✓）。
            scanned = [
                line.split("] ", 1)[1].strip()
                for line in output.splitlines()
                if line.startswith("[编译扫描] ")
            ]
            if scanned:
                failing = [scanned[-1]]
                print(f"  闸门 panic 在 {failing[0]} ⇒ 删掉它 ✓")
        if not failing:
            print("闸门红了但没解析出文件名 ⇒ 手工看一眼")
            return 1
        for relative in failing:
            path = LIB / relative
            if path.is_file():
                path.unlink()
                removed.append(relative)
        print(f"  删掉 {len(failing)} 个编不过的文件（例：{failing[0]}）")
    print(f"共删 {len(removed)} 个编不过的文件")
    return 0


def present(prefix_module: str) -> bool:
    relative = pathlib.Path(*prefix_module.split("."))
    return (LIB / relative.with_suffix(".py")).is_file() or (LIB / relative).is_dir()


def remove_module(module: str) -> None:
    """删掉**这个模块自己的文件**（包目录里别的文件仍各算各的 ✓）。"""
    relative = pathlib.Path(*module.split("."))
    for candidate in [LIB / relative.with_suffix(".py"), LIB / relative / "__init__.py"]:
        if candidate.is_file():
            candidate.unlink()


def batch(prefix: pathlib.Path, runner: str, scratch: pathlib.Path, rounds: int) -> int:
    """**整批同步**：先把上游全量拷进来，再反复"探不通过的删掉"直到不动点。

    为什么要有这条：单模块模式（一次只拷一个）探不出**互相依赖**的那些 ✗ —— `re`＋`fnmatch`＋`glob`
    这种"一家子"必须**一起**在场才 import 得动 ✓。可见集合随在场文件**单调** ⇒ 反复删到不动点
    就是最大那一个 ✓（删一个只会让别的更容易失败 ⇒ 收敛 ✓）。
    """
    modules = modules_of(prefix)
    copied = 0
    with COPY_LOCK:
        for module in modules:
            if copy_in(prefix, module) is not None:
                copied += 1
    print(f"先整批拷入 {copied} 个模块")
    for round_index in range(1, rounds + 1):
        alive = [module for module in modules if present(module)]
        failed: list[str] = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=args_jobs) as pool:
            for module, ok in zip(alive, pool.map(lambda m: probe(runner, m, scratch), alive)):
                if not ok:
                    failed.append(module)
        if not failed:
            print(f"第 {round_index} 轮：没有要删的 ⇒ 到不动点")
            break
        for module in failed:
            remove_module(module)
        print(f"第 {round_index} 轮：删掉 {len(failed)} 个（例：{failed[0]}）")
    return prune_noncompiling()


args_jobs = 8


def main() -> int:
    parser = argparse.ArgumentParser(description="找出现在就能同步的模块")
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument(
        "--batch",
        action="store_true",
        help="整批同步：先全拷进来，再反复删到「能 import」的不动点（探互相依赖的那些）",
    )
    parser.add_argument(
        "--prune",
        action="store_true",
        help="跑 `lib_compile` 闸门、把编不过的文件删掉（包连带拷进来的子模块）",
    )
    args = parser.parse_args()

    global args_jobs
    prefix = upstream_prefix()
    runner = runner_path()
    args_jobs = args.jobs
    scratch = ROOT / "target" / "syncable"
    scratch.mkdir(parents=True, exist_ok=True)
    candidates = modules_of(prefix)
    print(f"上游 {len(candidates)} 个模块；`Lib/` 现有 {len(list(LIB.rglob('*.py')))} 个文件")

    if args.batch:
        scratch.mkdir(parents=True, exist_ok=True)
        return batch(prefix, runner, scratch, args.rounds)

    copied_all: list[str] = []
    for round_index in range(1, args.rounds + 1):
        # 只试"还没在 Lib/ 里"的
        pending = []
        for module in candidates:
            relative = pathlib.Path(*module.split("."))
            if (LIB / relative.with_suffix(".py")).exists() or (LIB / relative).is_dir():
                continue
            pending.append(module)
        if not pending:
            break
        print(f"第 {round_index} 轮：试 {len(pending)} 个")
        copied: list[str] = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            futures = {
                pool.submit(copy_in, prefix, module): module for module in pending
            }
            for future in concurrent.futures.as_completed(futures):
                module = futures[future]
                target = future.result()
                if target is None:
                    continue
                if probe(runner, module, scratch):
                    copied.append(module)
                elif target.is_dir():
                    shutil.rmtree(target)
                else:
                    target.unlink()
        copied.sort()
        if not copied:
            print("本轮没有新增")
            break
        copied_all.extend(copied)
        print(f"本轮新增 {len(copied)} 个：{', '.join(copied[:12])}{' …' if len(copied) > 12 else ''}")

    print(f"\n合计新增 {len(copied_all)} 个模块：")
    for module in copied_all:
        print(f"  {module}")
    # **编不过的删掉**（`lib_compile` 那道闸门 ✓）：包会连带拷进没被 import 的子模块 ✗
    if args.prune:
        return prune_noncompiling()
    return wait(prefix)


if __name__ == "__main__":
    sys.exit(main())
