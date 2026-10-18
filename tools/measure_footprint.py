#!/usr/bin/env python3
"""足迹测量驱动：M1 ② 的裸档 ＋ M5 `P3-28` 的实例体量三档实测。

**三档口径**（每档都是真测量；口径标注写死，免得被误引）：

1. **① 裸实例**（C 宿主 `footprint_host.c`，**ABI 级**；**无 site／无导入系统**）：
   `pa_create`＋`pa_destroy` 的 VM 引导延迟、`pa_exec_string("x = 1")` 的执行延迟、
   最小宿主的整程墙钟（外部计时，含 exec＋ld.so）、进程 RSS 曲线（`statm`）与峰值（`VmHWM`）。
   —— 与 `DESIGN.md` §9「Pyawa（M1 最小内核）实测基线」**同一口径的复测**（M1 那节原文不动）。
2. **② 全栈单实例**（`pyawa` CLI，**进程级**；**含 site ＋ 导入系统**）：
   同一 CLI 跑「裸脚本／`+site`／`+site+常用模块`」三种脚本，报整程墙钟（外部计时，同
   `python3 -c pass` 口径）与**进程峰值 RSS**。2 的"每实例增量"＝相对 CLI 裸脚本的 Δ墙钟／ΔRSS
   （**进程级**，与 ① 的实例级数字**不可直接相减**）。
3. **③ 多实例**（两种口径都给）：
   - **③a 单进程多实例**（`footprint_host --multi N M`，裸 ABI）：N=1/2/4/8，逐实例建完后的
     RSS 曲线 ⇒ 每实例边际 RSS／总常驻；建 N 个实例的墙钟＋每个实例各干活的墙钟。
     **顺序执行、不开线程 ⇒ 并行度=1**（这一档只看隔离与内存）。
   - **③b 多进程各一实例**（N 个 `pyawa` 进程并发跑同一份"site＋常用模块＋忙等负载"脚本）：
     每进程峰值 RSS（隔离测量）、并发墙钟、总常驻（N×单实例峰值＝上界；同时刻逐进程采样和＝近似
     实际）、与"顺序墙钟和（＝N×单实例墙钟中位）"之比 ⇒ 并行度。

**进程级峰值 RSS 的采法**（重要）：驱动是 Python，直接 `fork` 再 `exec` 会把**父解释器的 RSS**
算进子进程的 `ru_maxrss`（实测污染到 12.6 MB，而真峰 6.5 MB）⇒ RSS 一律经 `footprint_host --spawn`
采：那个小 C 进程（~2.4 MB）fork/exec 目标，`ru_maxrss` 才等于目标真峰。墙钟仍由驱动外部计时
（含 exec＋ld.so，与 ① 同口径）。

**"常用模块"清单写死**（`COMMON_MODULES`，10 个）：`json, re, datetime, collections, functools,
itertools, math, time, os, io`。依据：① `DESIGN.md` §9 现有 CPython 对照基线用的就是
`json,re,datetime`（同口径可对比）；② 本仓 `Lib/**/*.py` 里被 import 最多的那几个真实面
（`os` 126／`re` 80／`io` 70／`collections` 69／`functools` 48／`itertools` 46／`time` 41 次，
只数 `import`／`from … import` 行）；③ `math` 为首个常用 C 层模块。清单是**口径的一部分**：
改了它就等于换了测量，别悄悄改。

`--profile debug|release|both`（默认 `both`）只影响 ① 的 C 宿主；②③ 用 CLI／宿主都只取 release
（debug 的足迹没有对外意义，与 M1 的报告口径一致）。

**缺 `cc`／`python3`／`cargo` 即红**：量不出来就说量不出来（不跳过、不编造）。`site` 导入若红，
本驱动会打印红字并退回"补 `sys.copyright` 后的诊断口径"（数字仍真，但会显著标注）。
"""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
HOST = ROOT / "tools" / "footprint_host.c"
INCLUDE = ROOT / "crates" / "pyawa-abi" / "include"
SCRATCH_ROOT = ROOT / "target" / "recon"

PROCESS_RUNS = 30  # ① 最小宿主整程的样本数（与 M1 同）
INNER_ITERATIONS = 1000  # ① 内部循环次数（与 M1 同）
CLI_RUNS = 20  # ② 每个脚本的墙钟样本数
RSS_RUNS = 10  # ②③ 每个脚本的峰值 RSS 样本数（经 --spawn 采）
SEQUENTIAL_RUNS = 5  # ③b 单实例墙钟样本数（顺序基线）
PARALLEL_REPEATS = 3  # ③b 每个 N 的并发重复次数
MULTI_INSTANCES = (1, 2, 4, 8)  # ③ 的 N
MULTI_EXEC_ITERATIONS = 100  # ③a 每个实例干活次数
MULTI_RUNS = 3  # ③a 每个 N 的重复次数
WORK_LOOP_ITERATIONS = 20000  # ③b 每实例的忙等负载（纯 Python 整数加法轮数）

#: ②③ 的"常用模块"清单 —— 写死，是口径的一部分（依据见模块 docstring）。
COMMON_MODULES = (
    "json",
    "re",
    "datetime",
    "collections",
    "functools",
    "itertools",
    "math",
    "time",
    "os",
    "io",
)

MIB = 1024.0

#: `build_and_compile` 建的临时目录（成功收工时清掉；失败时留着便于诊断）。
BUILD_DIRECTORIES: list[pathlib.Path] = []


def fail(message: str) -> int:
    print(f"[足迹] 红：{message}")
    return 1


def machine_line() -> str:
    cpu = "?"
    logical = os.cpu_count() or 1
    physical = None
    cpuinfo = pathlib.Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        text = cpuinfo.read_text()
        found = re.search(r"^model name\s*:\s*(.+)$", text, re.M)
        if found:
            cpu = found.group(1).strip()
        # 物理核数：按 (physical id, core id) 去重（`nproc` 是逻辑核数，SMT 会翻倍）。
        pairs = set(re.findall(r"^physical id\s*:\s*(\d+)$.*?^core id\s*:\s*(\d+)$", text, re.M | re.S))
        if pairs:
            physical = len(pairs)
    kernel = subprocess.run(["uname", "-srm"], capture_output=True, text=True).stdout.strip()
    cores = f"{logical} 逻辑核" + (f"／{physical} 物理核" if physical else "")
    return f"{cpu} · {kernel} · {cores}"


def tool_version(command: list[str]) -> str:
    done = subprocess.run(command, capture_output=True, text=True)
    return (done.stdout or done.stderr).splitlines()[0].strip() if done.returncode == 0 else "?"


def load_line() -> str:
    try:
        return pathlib.Path("/proc/loadavg").read_text().strip()
    except OSError:
        return "?"


def median(values: list[float]) -> float:
    return float(statistics.median(values))


def run_checked(command: list[str], what: str) -> subprocess.CompletedProcess[str]:
    done = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if done.returncode != 0:
        print(done.stdout, done.stderr)
        raise RuntimeError(f"`{' '.join(command)}` 失败（{what}）")
    return done


def absolute(program: str) -> str:
    """`execv` 不查 PATH ⇒ 把 `python3` 这类名字解析成绝对路径（找不到就原样，交给失败路径报）。"""
    return shutil.which(program) or program


# --------------------------------------------------------------------------- ① 裸实例


def build_and_compile(profile: str) -> pathlib.Path:
    """`cargo build` 那一档 ＋ `cc` 链出 C 宿主可执行文件（它同时是 ②③ 的 `--spawn` 脚手架）。"""
    cargo = ["cargo", "build", "-p", "pyawa-abi"] + (["--release"] if profile == "release" else [])
    run_checked(cargo, f"profile={profile}")
    archive = ROOT / "target" / profile / "libpyawa_abi.a"
    if not archive.is_file():
        raise RuntimeError(f"静态库不在：{archive}")
    directory = pathlib.Path(tempfile.mkdtemp(prefix=f"pyawa-footprint-{profile}-", dir=SCRATCH_ROOT))
    BUILD_DIRECTORIES.append(directory)
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


def parse_metrics(stdout: str) -> dict[str, float]:
    metrics: dict[str, float] = {}
    for field in stdout.split():
        key, _, value = field.partition("=")
        if not value:
            continue
        try:
            metrics[key] = float(value)
        except ValueError:  # `mode=spawn` 这类非数值字段
            continue
    return metrics


def inner_metrics(binary: pathlib.Path) -> dict[str, float]:
    run = subprocess.run([str(binary), str(INNER_ITERATIONS)], capture_output=True, text=True)
    if run.returncode != 0:
        raise RuntimeError(f"footprint_host 退出码 {run.returncode}：{run.stderr}")
    return parse_metrics(run.stdout)


def multi_instance_metrics(binary: pathlib.Path, instances: int, iterations: int) -> dict[str, float]:
    run = subprocess.run(
        [str(binary), "--multi", str(instances), str(iterations)],
        capture_output=True, text=True,
    )
    if run.returncode != 0:
        raise RuntimeError(f"footprint_host --multi 退出码 {run.returncode}：{run.stderr}")
    return parse_metrics(run.stdout)


def process_wall_ns(binary: pathlib.Path) -> list[int]:
    """最小宿主（iterations=1）的整程墙钟：外部计时，含 exec ＋ ld.so。"""
    samples = []
    for _ in range(PROCESS_RUNS):
        start = time.perf_counter_ns()
        done = subprocess.run([str(binary), "1"], capture_output=True)
        samples.append(time.perf_counter_ns() - start)
        if done.returncode != 0:
            raise RuntimeError(f"footprint_host 退出码 {done.returncode}")
    return samples


# -------------------------------------------------- 进程级采数（②③b 与 CPython 对照）


def spawn_and_wait(argv: list[str]) -> int:
    """直接 fork/exec 一个子进程，返回**外部计时**的墙钟 ns（含 exec＋ld.so）。"""
    pid = os.fork()
    if pid == 0:  # 子进程：只做 exec，绝不回 Python 解释器（`os._exit` 防止重复 flush）
        try:
            null = os.open(os.devnull, os.O_WRONLY)
            os.dup2(null, 1)
            os.dup2(null, 2)
            os.execv(argv[0], argv)
        except BaseException:  # noqa: BLE001 - 子进程里任何失败都只能退出
            os._exit(127)
    start = time.perf_counter_ns()
    _, status, _ = os.wait4(pid, 0)
    elapsed = time.perf_counter_ns() - start
    code = os.waitstatus_to_exitcode(status)
    if code != 0:
        raise RuntimeError(f"`{' '.join(argv)}` 退出码 {code}：{explain_failure(argv)}")
    return elapsed


def external_wall_samples(argv: list[str], runs: int) -> list[int]:
    return [spawn_and_wait(argv) for _ in range(runs)]


def explain_failure(argv: list[str]) -> str:
    done = subprocess.run(argv, capture_output=True, text=True)
    lines = (done.stderr or done.stdout).strip().splitlines()
    return lines[-1] if lines else f"退出码 {done.returncode}"


def helper_spawn(helper: pathlib.Path, argv: list[str]) -> tuple[int, int]:
    """经 C 脚手架跑目标：返回（墙钟 ns，目标峰值 RSS KiB）。

    墙钟是脚手架内部的单调计时（fork 前 → wait4），与外部计时差 <1 ms；峰值 RSS 才是这里的关键——
    从 Python 父进程 fork 会污染 `ru_maxrss`（见模块 docstring／`footprint_host.c` 文件头）。
    """
    done = subprocess.run(
        [str(helper), "--spawn", *argv], capture_output=True, text=True
    )
    if done.returncode != 0:
        raise RuntimeError(f"`footprint_host --spawn` 退出码 {done.returncode}：{done.stderr[-400:]}")
    for line in done.stdout.splitlines():
        if line.startswith("mode=spawn "):
            metrics = parse_metrics(line)
            code = int(metrics["exit_code"])
            if code != 0:
                tail = (done.stdout + done.stderr).strip().splitlines()
                raise RuntimeError(
                    f"`{' '.join(argv)}` 退出码 {code}：{tail[-1] if tail else ''}"
                )
            return int(metrics["wall_ns"]), int(metrics["maxrss_kb"])
    raise RuntimeError(f"`footprint_host --spawn` 没打 `mode=spawn` 行：{done.stdout[-300:]}{done.stderr[-300:]}")


def helper_rss_samples(helper: pathlib.Path, argv: list[str], runs: int) -> list[int]:
    return [helper_spawn(helper, argv)[1] for _ in range(runs)]


def run_concurrent(argv: list[str], count: int) -> dict[str, object]:
    """并发跑 `count` 个同一命令；返回并发墙钟、同时刻逐进程采样总 RSS 的峰值。

    "同时刻总和"用 `/proc/<pid>/statm` 的**当前** RSS（exec 后就是目标自己的内存，父进程页已换掉）；
    共享只读页在每个进程里都计一份 ⇒ 是**上界**（真实物理占用会更少）。
    """
    children: list[int] = []
    start = time.perf_counter_ns()
    for _ in range(count):
        pid = os.fork()
        if pid == 0:
            try:
                null = os.open(os.devnull, os.O_WRONLY)
                os.dup2(null, 1)
                os.dup2(null, 2)
                os.execv(argv[0], argv)
            except BaseException:  # noqa: BLE001
                os._exit(127)
        children.append(pid)
    sampled_peak = 0
    pending = list(children)
    while pending:
        total = 0
        for pid in pending:
            rss = read_rss_kb(pid)
            if rss is not None:
                total += rss
        sampled_peak = max(sampled_peak, total)
        for pid in list(pending):
            try:
                finished, status, _ = os.wait4(pid, os.WNOHANG)
            except ChildProcessError:
                finished = pid
                status = 0
            if finished == pid:
                pending.remove(pid)
                code = os.waitstatus_to_exitcode(status)
                if code != 0:
                    raise RuntimeError(f"并发子进程退出码 {code}：{explain_failure(argv)}")
        time.sleep(0.001)
    return {
        "wall": time.perf_counter_ns() - start,
        "sampled_peak_kb": sampled_peak,
    }


def read_rss_kb(pid: int) -> int | None:
    try:
        fields = pathlib.Path(f"/proc/{pid}/statm").read_text().split()
        return int(fields[1]) * (os.sysconf("SC_PAGE_SIZE") // 1024)
    except (OSError, IndexError, ValueError):
        return None


# --------------------------------------------------------------------- ②③ 的脚本与 CLI


def build_cli() -> pathlib.Path:
    """全栈测量主体＝`pyawa` CLI（组合根装配了 `sys`／内建／`Lib/` 的 `sys.path`）。"""
    run_checked(["cargo", "build", "-p", "pyawa-runtime", "--release"], "CLI")
    binary = ROOT / "target" / "release" / "pyawa"
    if not binary.is_file():
        raise RuntimeError(f"CLI 不在：{binary}")
    return binary


def script_texts(site_ready: bool) -> dict[str, str]:
    site = "import site\n"
    if not site_ready:
        # 诊断口径：`site.py:467-469` 的 `setcopyright()` 取 `sys.copyright`；本仓该属性缺失时
        # `import site` 红。补上后再导入（数字仍真，但**不是**"原样实例"的口径，会红字标注）。
        site = 'import sys\nsys.copyright = ""\nimport site\n'
    imports = "".join(f"import {module}\n" for module in COMMON_MODULES)
    workload = (
        "busy = 0\n"
        f"for index in range({WORK_LOOP_ITERATIONS}):\n"
        "    busy += index\n"
    )
    # 文件名**不能**与标准库模块同名（`site.py` 会被 sys.path[0] 抢先命中 ⇒ 量到的不是真 site ✗）。
    return {
        "bare": "x = 1\n",
        "site": site,
        "full": site + imports,
        "work": site + imports + workload,
    }


def probe_site(cli: pathlib.Path, scratch: pathlib.Path) -> tuple[bool, str]:
    """真 `import site` 绿不绿（②的前置）。返回（是否绿，失败时的一行诊断）。"""
    probe = scratch / "probe_site.py"
    probe.write_text("import site\n")
    done = subprocess.run([str(cli), str(probe)], capture_output=True, text=True)
    if done.returncode == 0:
        return True, ""
    lines = (done.stderr or done.stdout).strip().splitlines()
    return False, lines[-1] if lines else f"退出码 {done.returncode}"


def prepare_scripts(cli: pathlib.Path, scratch: pathlib.Path, site_ready: bool) -> dict[str, pathlib.Path]:
    """写脚本并**预热一次**（首次运行会编译并写 `.pyac` 产物 ⇒ 预热后计时才是稳态）。"""
    paths: dict[str, pathlib.Path] = {}
    for name, text in script_texts(site_ready).items():
        path = scratch / f"footprint_{name}.py"
        path.write_text(text)
        warm = subprocess.run([str(cli), str(path)], capture_output=True, text=True)
        if warm.returncode != 0:
            lines = (warm.stderr or warm.stdout).strip().splitlines()
            raise RuntimeError(
                f"脚本 `{path.name}` 预热失败（退出码 {warm.returncode}）："
                f"{lines[-1] if lines else ''}\n脚本内容：\n{text}"
            )
        paths[name] = path
    return paths


def profile_script(helper: pathlib.Path, cli: pathlib.Path, path: pathlib.Path) -> dict[str, float]:
    argv = [str(cli), str(path)]
    walls = external_wall_samples(argv, CLI_RUNS)
    rss = helper_rss_samples(helper, argv, RSS_RUNS)
    return {
        "wall_median": median(walls),
        "wall_min": float(min(walls)),
        "rss_median": median(rss),
    }


# ----------------------------------------------------------------------------- 输出辅助


def ms(nanoseconds: float) -> str:
    return f"{nanoseconds / 1e6:.2f} ms"


def us(nanoseconds: float) -> str:
    return f"{nanoseconds / 1e3:.2f} µs"


def mib(kilobytes: float) -> str:
    return f"{kilobytes / MIB:.2f} MiB"


def delta_ms(current_ns: float, base_ns: float) -> str:
    return f"{(current_ns - base_ns) / 1e6:+.2f} ms"


def delta_mib(current_kb: float, base_kb: float) -> str:
    return f"{(current_kb - base_kb) / MIB:+.2f} MiB"


# ------------------------------------------------------------------------------- 各档打印


def print_tier1(profiles: list[str]) -> dict[str, pathlib.Path]:
    binaries: dict[str, pathlib.Path] = {}
    print("\n== ① 裸实例（C 宿主／ABI 级；无 site、无导入系统；M1 同口径复测）==")
    for current in profiles:
        binary = build_and_compile(current)
        binaries[current] = binary
        metrics = inner_metrics(binary)
        wall = process_wall_ns(binary)
        print(f"  profile={current}")
        print(
            f"    VM 引导（pa_create+destroy，N={int(metrics['iterations'])}）："
            f"median {us(metrics['create_median_ns'])} · min {us(metrics['create_min_ns'])}"
        )
        print(
            f"    执行（pa_exec_string(\"x = 1\")）："
            f"median {us(metrics['exec_median_ns'])} · min {us(metrics['exec_min_ns'])}"
        )
        print(
            f"    宿主整程（N={PROCESS_RUNS}）：median {ms(median(wall))} · min {ms(min(wall))}"
        )
        print(
            f"    常驻内存：起点 {int(metrics['rss_before_kb'])} KiB · 建实例后 "
            f"{int(metrics['rss_after_create_kb'])} KiB · 执行后 "
            f"{int(metrics['rss_after_exec_kb'])} KiB · 收尾 {int(metrics['rss_end_kb'])} KiB · "
            f"峰值 {int(metrics['rss_peak_kb'])} KiB"
        )
    return binaries


def print_tier2(
    helper: pathlib.Path, cli: pathlib.Path, scripts: dict[str, pathlib.Path], site_ready: bool
) -> dict[str, dict[str, float]]:
    print("\n== ② 全栈单实例（pyawa CLI／进程级；含 site 与导入系统；release）==")
    if site_ready:
        print("  site：`import site` 真导入绿 ✓")
    else:
        print(
            "  site：**红** ✗（`import site` 在 HEAD 失败）⇒ 以下 site 相关数字是"
            "「补 `sys.copyright` 后的诊断口径」，见上方红字"
        )
    measured = {
        name: profile_script(helper, cli, path)
        for name, path in scripts.items()
        if name != "work"
    }
    bare = measured["bare"]
    print(f"  {'场景':<26}{'整程墙钟(median/min)':<30}{'进程峰值 RSS':<18}{'Δ墙钟':<16}ΔRSS")
    labels = {
        "bare": "裸脚本（x = 1）",
        "site": "+ site",
        "full": "+ site + 常用模块（②口径）",
    }
    for name in ("bare", "site", "full"):
        values = measured[name]
        wall_delta = "—" if name == "bare" else delta_ms(values["wall_median"], bare["wall_median"])
        rss_delta = "—" if name == "bare" else delta_mib(values["rss_median"], bare["rss_median"])
        print(
            f"  {labels[name]:<26}"
            f"{ms(values['wall_median']) + ' / ' + ms(values['wall_min']):<30}"
            f"{mib(values['rss_median']):<18}"
            f"{wall_delta:<16}{rss_delta}"
        )
    print("  常用模块清单（写死，10 个）：" + ", ".join(COMMON_MODULES))
    print(
        "  依据：DESIGN §9 的 CPython 对照基线同用 json,re,datetime；其余取 "
        "Lib/**/*.py 里 import 次数最高的真实面（os/re/io/collections/functools/itertools/time）"
        "＋首个常用 C 层模块 math。"
    )
    print(
        f"  口径：整程＝外部计时、含 exec ＋ ld.so ＋ CLI 组合根（与 `python3 -c pass` 同口径，N={CLI_RUNS}）；"
        f"峰值 RSS＝经 `footprint_host --spawn` 采的 `ru_maxrss`（进程级，N={RSS_RUNS} 取 median）。"
        "Δ 列＝相对 CLI 裸脚本的**进程级增量**，与 ① 的**实例级**数字不可混算。"
    )
    return measured


def print_cpython_comparison(helper: pathlib.Path, measured: dict[str, dict[str, float]]) -> None:
    print("\n== 对照：CPython（同轮、同方法现测）==")
    python = absolute("python3")
    imports = "\n".join(["import site"] + [f"import {module}" for module in COMMON_MODULES])
    cases = [
        ("`python3 -c pass`（含 site）", [python, "-c", "pass"]),
        ("`python3 -S -c pass`（去 site）", [python, "-S", "-c", "pass"]),
        ("`python3 -c`：site ＋ 常用模块", [python, "-c", imports]),
    ]
    for label, argv in cases:
        walls = external_wall_samples(argv, CLI_RUNS)
        rss = helper_rss_samples(helper, argv, RSS_RUNS)
        print(
            f"  {label:<30}"
            f"median {ms(median(walls))} · min {ms(min(walls))}"
            f" · 峰值 RSS {mib(median(rss))}"
        )
    full = measured["full"]
    print(
        f"  ⇒ 本仓 ② 口径（+site+常用模块）vs CPython 同导入面（上表第三行）："
        f"整程 {ms(full['wall_median'])} vs 上表第三行；峰值 RSS {mib(full['rss_median'])} vs 上表第三行。"
    )


def print_tier3a(release_binary: pathlib.Path) -> None:
    print(
        "\n== ③a 单进程多实例（C 宿主 --multi／裸 ABI；N 个实例同时存活，顺序执行 ⇒ 并行度=1）=="
    )
    print(
        f"  {'N':<4}{'每实例边际 RSS':<18}{'总常驻 RSS':<16}{'建 N 个墙钟':<18}"
        f"{'各自干活墙钟':<18}墙钟合计"
    )
    curves: dict[int, list[float]] = {}
    rss_starts: dict[int, float] = {}
    for instances in MULTI_INSTANCES:
        runs = [
            multi_instance_metrics(release_binary, instances, MULTI_EXEC_ITERATIONS)
            for _ in range(MULTI_RUNS)
        ]
        rss_before = median([run["rss_before_kb"] for run in runs])
        rss_all = median([run["rss_after_all_kb"] for run in runs])
        marginal = (rss_all - rss_before) / instances
        create_total = median([run["create_total_ns"] for run in runs])
        work_total = median([run["work_total_ns"] for run in runs])
        curves[instances] = [
            median([run[f"rss_after_{step}_kb"] for run in runs])
            for step in range(1, instances + 1)
        ]
        rss_starts[instances] = rss_before
        print(
            f"  {instances:<4}{mib(marginal):<18}{mib(rss_all):<16}"
            f"{ms(create_total):<18}{ms(work_total):<18}{ms(create_total + work_total)}"
        )
    largest = max(MULTI_INSTANCES)
    curve = " → ".join(f"{int(value)}" for value in curves[largest])
    print(
        f"  RSS 曲线（N={largest}，KiB，起点 {int(rss_starts[largest])}）：{curve}"
    )
    print(
        "  口径：每实例边际=(建 N 个后的 RSS − 起点)/N；总常驻＝建 N 个后 RSS（**不含**执行期峰值，"
        "见 ① 的 VmHWM）；墙钟＝N 次 `pa_create`／每实例各 "
        f"{MULTI_EXEC_ITERATIONS} 次 `pa_exec_string(\"x = 1\")` 之和（median，N={MULTI_RUNS}）。"
        "**这一档无并行**：单线程顺序建／顺序跑。"
    )


def print_tier3b(helper: pathlib.Path, cli: pathlib.Path, work_script: pathlib.Path) -> None:
    print(
        "\n== ③b 多进程各一实例（N 个 pyawa 进程并发；全栈＝site＋常用模块＋忙等负载）=="
    )
    argv = [str(cli), str(work_script)]
    single_walls = external_wall_samples(argv, SEQUENTIAL_RUNS)
    single_wall = median(single_walls)
    single_peak = median(helper_rss_samples(helper, argv, RSS_RUNS))
    print(
        f"  单实例基线（隔离，墙钟 N={SEQUENTIAL_RUNS}／RSS N={RSS_RUNS}）："
        f"墙钟 median {ms(single_wall)} · min {ms(min(single_walls))} · "
        f"峰值 RSS {mib(single_peak)}"
    )
    print(f"  忙等负载：{WORK_LOOP_ITERATIONS} 轮纯 Python 整数加法（每实例同一份脚本）")
    print(
        f"  {'N':<4}{'并发墙钟':<14}{'顺序墙钟和':<14}{'并行度':<9}"
        f"{'每进程峰值 RSS':<16}{'峰值之和(总常驻上界)':<22}同时刻采样总 RSS"
    )
    for instances in MULTI_INSTANCES:
        # 顺序基线**按同一个 N 现测**（N 次独立顺序跑之和）⇒ 与并发批同处同一负载条件下，
        # 免得拿"几分钟前的单实例中位"当分母（共享工作区里别的线会让基线漂）。
        sequential_sum = sum(external_wall_samples(argv, instances))
        repeats = [run_concurrent(argv, instances) for _ in range(PARALLEL_REPEATS)]
        wall = median([repeat["wall"] for repeat in repeats])
        sampled = max(repeat["sampled_peak_kb"] for repeat in repeats)
        speedup = sequential_sum / wall if wall else 0.0
        print(
            f"  {instances:<4}{ms(wall):<14}{ms(sequential_sum):<14}{speedup:<9.2f}"
            f"{mib(single_peak):<16}{mib(instances * single_peak):<22}{mib(sampled)}"
        )
    print(
        "  口径：并发墙钟＝N 个子进程同时起跑→全部退出（外部计时，含 exec）；并行度＝"
        "**同 N 现测**的顺序墙钟和(N 次独立顺序跑之和)÷并发墙钟；每进程峰值 RSS＝**隔离测量**"
        f"（同一脚本、N={RSS_RUNS} 取 median，经 `--spawn` 避父进程 RSS 污染）；峰值之和＝N×单实例峰值"
        "（上界：共享只读页按 N 份重复计）；同时刻采样总 RSS＝并发运行中 1 ms 逐进程读 "
        "`/proc/<pid>/statm` 的当前 RSS 之和取峰值。并行度受**物理核数**与本机其他负载限制"
        "（逻辑核数见机器行）。"
    )


# ---------------------------------------------------------------------------------- main


def main(argv: list[str]) -> int:
    profile = "both"
    if "--profile" in argv:
        profile = argv[argv.index("--profile") + 1]
    if profile not in ("debug", "release", "both"):
        return fail(f"`--profile` 只接 debug／release／both（收到 {profile}）")
    profiles = ["debug", "release"] if profile == "both" else [profile]

    for tool in ("cc", "python3", "cargo"):
        if shutil.which(tool) is None:
            return fail(f"本机没有 `{tool}`：量不出来就说量不出来，不跳过")

    print(f"[足迹] 机器：{machine_line()}")
    print(f"[足迹] rustc：{tool_version(['rustc', '--version'])}")
    print(f"[足迹] cc：{tool_version(['cc', '--version'])}")
    print(f"[足迹] python3：{tool_version(['python3', '--version'])}")
    print(f"[足迹] 载入（开跑）：{load_line()}")
    print(
        "[足迹] 三档口径：①＝C 宿主裸 ABI（无 site／导入系统）· ②＝CLI 进程级（含 site＋导入系统）· "
        "③＝单进程多实例（裸 ABI）＋ 多进程各一实例（全栈 CLI）。"
    )
    print(
        "[足迹] 时间用外部计时（perf_counter，含 exec ＋ ld.so）；内存用 statm／VmHWM／"
        "`--spawn` 采的 ru_maxrss（避父进程 RSS 污染）；②③ 与 CPython 对照都只取 release"
        "（debug 足迹无对外意义）。共享工作区里别的时间在跑：同一命令复跑会有波动，看 median／min "
        "与报告里的复跑区间。"
    )

    SCRATCH_ROOT.mkdir(parents=True, exist_ok=True)
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="pyawa-footprint-", dir=SCRATCH_ROOT))
    succeeded = False
    try:
        binaries = print_tier1(profiles)
        helper = binaries.get("release")
        if helper is None:
            helper = build_and_compile("release")

        cli = build_cli()
        site_ready, site_error = probe_site(cli, scratch)
        if not site_ready:
            print(f"\n[足迹] 红：`import site` 失败 ⇒ {site_error}")
            print(
                "[足迹] ② 的 site 数字退回「补 `sys.copyright` 后的诊断口径」"
                "（真测量，但不是原样实例；M5 判据③ 引用前必须先修掉这一条）"
            )
        scripts = prepare_scripts(cli, scratch, site_ready)
        measured = print_tier2(helper, cli, scripts, site_ready)
        print_cpython_comparison(helper, measured)

        print_tier3a(helper)
        print_tier3b(helper, cli, scripts["work"])
        succeeded = True
    except (RuntimeError, OSError) as error:
        print(f"\n[足迹] 红：{error}")
        print(f"[足迹] 现场保留在：{scratch}（脚本与产物都在这里，便于诊断）")
        return 1

    if succeeded:
        shutil.rmtree(scratch, ignore_errors=True)
        for directory in BUILD_DIRECTORIES:
            shutil.rmtree(directory, ignore_errors=True)
    print(f"\n[足迹] 载入（收工）：{load_line()}")
    print("[足迹] 绿：三档都量到了（① 裸实例 ② 全栈单实例 ③ 单进程多实例＋多进程各一实例）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
