#!/usr/bin/env python3
"""**M1 ②：足迹测量驱动**（`PLAN-milestones.md` §6 ②；`DESIGN.md` §13-17 的提示项）。

量的三样（`footprint_host.c` 采数、本脚本驱动与对照）：

1. **VM 引导延迟**：`pa_create` ＋ `pa_destroy`（进程内，N=1000，报 median／min）
2. **宿主整程延迟**：最小宿主从 exec 到退出的墙钟（外部计时，N=30，报 median／min）
   ——这条与 `python3 -c pass` 同一口径，故对照在同一轮里现测
3. **常驻内存**：宿主进程的 RSS（起点／建实例后／执行后）与峰值 `VmHWM`；`python3 -c pass`
   的峰值 RSS 同轮现测（`resource.getrusage` 的 `ru_maxrss`）

`--profile debug|release|both`（默认 `both`）。**缺 `cc` 或 `python3` 即红**：量不出来就说量不出来
（与 `tests/ci/t_ab_1.py` 同一原则：缺前置不跳过）。build profile 分别报——debug 的引导延迟没有
参考价值，release 才是对外可比的那一档。
"""

from __future__ import annotations

import pathlib
import re
import shutil
import statistics
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
HOST = ROOT / "tools" / "footprint_host.c"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"

PROCESS_RUNS = 30
INNER_ITERATIONS = 1000


def fail(message: str) -> int:
    print(f"[足迹] 红：{message}")
    return 1


def machine_line() -> str:
    cpu = "?"
    cpuinfo = pathlib.Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        found = re.search(r"^model name\s*:\s*(.+)$", cpuinfo.read_text(), re.M)
        if found:
            cpu = found.group(1).strip()
    kernel = subprocess.run(["uname", "-srm"], capture_output=True, text=True).stdout.strip()
    return f"{cpu} · {kernel}"


def tool_version(command: list[str]) -> str:
    done = subprocess.run(command, capture_output=True, text=True)
    return (done.stdout or done.stderr).splitlines()[0].strip() if done.returncode == 0 else "?"


def build_and_compile(profile: str) -> pathlib.Path:
    """`cargo build` 那一档 ＋ `cc` 链出宿主可执行文件。"""
    cargo = ["cargo", "build", "-p", "pyawa-abi"] + (["--release"] if profile == "release" else [])
    built = subprocess.run(cargo, cwd=ROOT, capture_output=True, text=True)
    if built.returncode != 0:
        print(built.stdout, built.stderr)
        raise RuntimeError(f"`{' '.join(cargo)}` 失败")
    archive = ROOT / "target" / profile / "libpyawa_abi.a"
    if not archive.is_file():
        raise RuntimeError(f"静态库不在：{archive}")
    directory = pathlib.Path(tempfile.mkdtemp(prefix=f"pyawa-footprint-{profile}-"))
    binary = directory / "footprint_host"
    command = [
        "cc", str(HOST), "-I", str(INCLUDE), str(archive),
        "-lpthread", "-ldl", "-lm", "-o", str(binary),
    ]
    compiled = subprocess.run(command, capture_output=True, text=True)
    if compiled.returncode != 0:
        print(compiled.stderr)
        raise RuntimeError("`cc` 编译／链接失败")
    return binary


def inner_metrics(binary: pathlib.Path) -> dict[str, float]:
    run = subprocess.run([str(binary), str(INNER_ITERATIONS)], capture_output=True, text=True)
    if run.returncode != 0:
        raise RuntimeError(f"footprint_host 退出码 {run.returncode}：{run.stderr}")
    metrics: dict[str, float] = {}
    for field in run.stdout.split():
        key, _, value = field.partition("=")
        metrics[key] = float(value)
    return metrics


def process_wall_ns(binary: pathlib.Path) -> list[int]:
    """最小宿主（iterations=1）的整程墙钟：外部计时，含 exec ＋ ld.so。"""
    import time

    samples = []
    for _ in range(PROCESS_RUNS):
        start = time.perf_counter_ns()
        done = subprocess.run([str(binary), "1"], capture_output=True)
        samples.append(time.perf_counter_ns() - start)
        if done.returncode != 0:
            raise RuntimeError(f"footprint_host 退出码 {done.returncode}")
    return samples


def cpython_wall_ns() -> list[int]:
    import time

    samples = []
    for _ in range(PROCESS_RUNS):
        start = time.perf_counter_ns()
        subprocess.run(["python3", "-c", "pass"], capture_output=True)
        samples.append(time.perf_counter_ns() - start)
    return samples


def cpython_peak_kb() -> int:
    """CPython 裸启动的**峰值 RSS**（`VmHWM`）。

    口径与宿主一样读 `/proc/self/status`，且**不 import 任何模块**——`import resource` 会把
    `ru_maxrss` 抬高一截（实测 15.6 MB vs 9.8 MB），那是测量手段自身的开销，不是解释器的足迹。
    """
    done = subprocess.run(
        ["python3", "-c",
         "print([line.split()[1] for line in open('/proc/self/status')"
         " if line.startswith('VmHWM:')][0])"],
        capture_output=True, text=True,
    )
    return int(done.stdout.strip())


def ms(nanoseconds: float) -> str:
    return f"{nanoseconds / 1e6:.2f} ms"


def us(nanoseconds: float) -> str:
    return f"{nanoseconds / 1e3:.2f} µs"


def main(argv: list[str]) -> int:
    profile = "both"
    if "--profile" in argv:
        profile = argv[argv.index("--profile") + 1]
    profiles = ["debug", "release"] if profile == "both" else [profile]

    for tool in ("cc", "python3", "cargo"):
        if shutil.which(tool) is None:
            return fail(f"本机没有 `{tool}`：量不出来就说量不出来，不跳过")

    print(f"[足迹] 机器：{machine_line()}")
    print(f"[足迹] rustc：{tool_version(['rustc', '--version'])}")
    print(f"[足迹] cc：{tool_version(['cc', '--version'])}")
    print(f"[足迹] python3：{tool_version(['python3', '--version'])}")

    cpython_samples = cpython_wall_ns()
    cpython_peak = cpython_peak_kb()

    for current in profiles:
        binary = build_and_compile(current)
        metrics = inner_metrics(binary)
        wall = process_wall_ns(binary)
        print(f"\n== profile={current} ==")
        print(f"  VM 引导（pa_create+destroy，N={int(metrics['iterations'])}）："
              f"median {us(metrics['create_median_ns'])} · min {us(metrics['create_min_ns'])}")
        print(f"  执行（pa_exec_string(\"x = 1\")）："
              f"median {us(metrics['exec_median_ns'])} · min {us(metrics['exec_min_ns'])}")
        print(f"  宿主整程（N={PROCESS_RUNS}）：median {ms(statistics.median(wall))} · min {ms(min(wall))}")
        print(f"  常驻内存：起点 {int(metrics['rss_before_kb'])} KiB · 建实例后 "
              f"{int(metrics['rss_after_create_kb'])} KiB · 执行后 "
              f"{int(metrics['rss_after_exec_kb'])} KiB · 收尾 {int(metrics['rss_end_kb'])} KiB · "
              f"峰值 {int(metrics['rss_peak_kb'])} KiB")

    print(f"\n== 对照：CPython（{'python3 --version'}，同轮现测）==")
    print(f"  `python3 -c pass` 整程（N={PROCESS_RUNS}）："
          f"median {ms(statistics.median(cpython_samples))} · min {ms(min(cpython_samples))}")
    print(f"  峰值 RSS：{cpython_peak} KiB")
    print("\n[足迹] 绿：三样都量到了")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
