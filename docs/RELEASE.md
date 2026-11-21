# alpha 发行说明（v0.1.0-alpha.1）

**本版是 alpha**：功能可供自用宿主接入，但 **ABI 未冻结** —— 后续 alpha 之间仍可能做破坏性变更
（每次都会在 `CHANGELOG.md` 列出）。ABI 的冻结点随**首个稳定发行版**（`AB-81`）。

**范围**：宿主是自己写的宿主——C／C++（`pa.h`）、CPython 经 ctypes、**JS（浏览器，WASM；实验性）**。
**不承诺**第三方跨语言生态，以及本文件"已知限制"里列出的项；稳定版的平台矩阵承诺见
`PLAN-milestones.md` §12.20（Windows）／§12.21（Web）。

## 版本

- 发行版：**v0.1.0-alpha.1**（SemVer pre-release；打 tag 前可改）
- ABI：**1.14**（`SPEC-c-abi`）；指令集：**2**
- 冻结：**未冻结**；冻结随首个稳定发行版（`AB-81`）
- 目标平台：`x86_64-unknown-linux-gnu`（已验证）· `wasm32-unknown-unknown`（**实验性**，浏览器／JS 宿主）

## 构建

```bash
tools/cargo.sh build --release                    # CLI：target/release/pyawa
tools/cargo.sh build --release -p pyawa-abi       # 最小 ABI 库（libpyawa_abi.a/.so）
tools/cargo.sh build --release -p pyawa-runtime   # 电池版组合根（含 stdlib/能力域/paL_newstate）

# Web（浏览器，WASM；实验性）：先装目标，再构建库（宿主 bin 不是 wasm 产物）
rustup target add wasm32-unknown-unknown
tools/cargo.sh build --workspace --lib --target wasm32-unknown-unknown --release

# Windows x86_64（W1 最小构建；MSVC 工具链）——libs ＋ bins 可编译（能力域 provider 属 W3–W5）
cargo build --workspace
```

## 集成（宿主三步）

1. `#include "pa.h"` —— **唯一入口头**（它 include 了 `pa_capabilities.h`）。
2. 建实例：电池版 `paL_newstate()`；或最小 ABI `pa_create(&host, &state)`（`PA_ABI_SIZE`／`PA_ABI_VERSION` 协商）。
3. 注册宿主函数／类型（`pa_register`；`pa_newtype` ＋ `pa_set_object_ops`），再 `pa_exec_string` 执行。

完整示例见 `docs/REFERENCE-HOST.md`（七项演示），由 `tests/ci/t_ref_host.py` **真编译真运行**验收。

### Web（浏览器，WASM）宿主（实验性）

1. 按上面的"构建"产出 `target/wasm32-unknown-unknown/release/pyawa_runtime.wasm`。
2. JS 侧 instantiate，并提供 import 模块 **`pyawa_host`** 的 4 条函数：`pa_host_slot_call(slot, state)`
   （宿主函数回调）＋ `clock_now_ns()`／`clock_monotonic_ns()`／`clock_sleep_ns(ns)`（JS 提供的时钟）。
   协议与槽位表见 `crates/pyawa-runtime/src/wasm_bridge.rs`（`PLAN §12.21` E2／E4）。
3. 建实例：电池版 `paL_newstate()`，或最小 ABI `pa_create(&host, &state)`——wasm32 上 `pa_host`
   是 **12 字节**（32 位 `usize`），JS 侧按 32 位布局填写。
4. 注册宿主函数：`pa_register` 的 `pa_host_fn` 传 **`pyawa_wasm_trampoline(slot)` 的返回值**（wasm 函数表
   索引；JS 造不出 C 函数指针），签名用 `pyawa_wasm_sig_basic(nparams)`。
5. 可执行样例（也是 E1–E5 的验收）：`tests/wasm/e2_host_function.js`／`e3_abi_surface.js`／
   `e4_js_clock.js`／`e5_smoke.js`。

## 支持矩阵（如实）

| 项 | 状态 |
|---|---|
| 平台 | **Linux x86_64（已验证）**；**Web／浏览器（`wasm32-unknown-unknown`，实验性）**：E1–E8 库嵌入档已进 main（`4f9490cc`／`a8d6ad8a`），E8 子集同结果；**Windows x86_64：W1–W6＋原生 CLI（§12.23）＋runtime 运行矩阵（必需）＋§12.24 真实语义＋平台面补齐**——CI `windows` job **全绿**；**脚本可移植性（Linux×Windows）**：24/24 一致（main `7d1b0f0e`）。内存口径＝Windows 不做 memcheck |
| 宿主语言 | C／C++（`pa.h`，含异步宿主驱动：`pa_resume`／`event`／host awaitable）；CPython 经 ctypes（已验证）；**JS（WASM，实验性）** |
| ABI | **1.17**，**alpha 期未冻结**（1.15 `pa_resume`/`pa_throw`；1.16 `event` 域；1.17 host awaitable） |
| 指令集 | 2 |

## 已知限制（本版不承诺）

- `pa_exec_file`／`paL_dofile` **需要 `fs` provider**：电池版 `paL_newstate` 自带；最小 ABI 产物需宿主注册 `fs`，否则如实 `PA_ERR_NOTIMPLEMENTED`。
- `paL_where` 是**尽力口径**（当前帧/`f_back` 的 `code:line`），**只提供 `code:line` 视图**（traceback 数据面见 `OM-42`，链语义见 `BC-60`／`T-BC-22`；完整 `traceback` 模块面不在本版）；`paL_errorf` 仅为 C 头内联格式化（不新增导出符号）；`paL_requiref` 依赖模块系统（无模块表的裸实例如实未提供）。
- 能力域：仅 `fs`／`clock`／`net` 三域有 C 布局；其余 6 域调用时"未实现"（`CP-2`／`CP-3`）。
- buffer 面只到"一次性写回"（`pa_writetobytes`）；无 `memoryview`、无长期可写视图、无多维 buffer。
- **Web（浏览器，WASM）的限制（如实）**：① **无 fs 读／无 net**——浏览器没有文件读取／网络能力；P1 的 fs 只提供**宿主 stdout 写**
  （`fs_write` handles 1／2），`open()` 等调用时如实报"未实现"（`CP-2`）；② **单线程**：没有 wasm 线程，
  CLI／LSP 不是 wasm 产物（wasm 侧只出库）；③ `clock` 由 JS 宿主提供（`now_ns`／`monotonic_ns`／
  `sleep_ns`），**日历／时区三槽未接** ⇒ `localtime`／`strftime`／`mktime` 如实未实现；④ `random`
  的种子在 wasm 上退化为**进程内计数器**（不是熵源）；⑤ wasm32 强制 `panic=abort` ⇒ ABI 的
  `catch_unwind` 兜底**失效**（Rust panic ＝ trap，不可恢复）；⑥ 纯 Python `Lib/` 的 `import` 依赖
  `fs` ⇒ **浏览器内不可用**（内置 C 层模块可用）；⑦ `sys.platform` 仍报 `"linux"`（未按目标改写）；
  ⑧ wasm32 的 `usize` 是 32 位 ⇒ ABI 值线格式按 **wasm32 布局**（`pa_host` **12 字节**），JS 侧不得
  按 64 位宿主布局读写。
- **Windows 上不做 memcheck（W6 口径）**：Windows 无 valgrind，Rust 1.98 stable 也无可用 ASan ⛔
  工具链 ⇒ 内存错误闸门仍以 **Linux `slowcheck`／valgrind** 为准；Windows CI job 只做**功能测试**
  （构建／provider／core＋lsp／参考宿主），**不声称内存验证**。
- Windows：**W1–W6 已完成** ＋ 原生 CLI（§12.23）＋ runtime 必需矩阵 ＋ `winreg` 真实语义首批
  ＋ `_winapi`/`_overlapped` 最小真实语义 ＋ **平台面补齐第一批**（`clock` 日历三槽、`fs.pipe`、
  `net.socketpair`、`dev`/`ino` 真值，§12.20 W2–W4）。conformance 在 Windows **是必需步骤且绿**
  （408 例、0 新差异；`DIV-12`＝guest 路径表现）。**仍如实未接**：`_overlapped` 数据面（Proactor
  不能真跑；asyncio 的 Windows 默认后端**未承诺**（`ADR-002`，dev 上的 selector 后端实验仅报告、代码不进 main）；`errno` 数值表仍由 Linux
  探测生成；`ntpath` 的 `splitroot`/`normpath` 盘符语义有差；`symlink` 需特权；Windows 不做 memcheck。
- **脚本可移植性（生死线）**：可移植子集内脚本无修改、可观测行为一致；平台查询如实回答；后端
  选型不属于脚本契约。承诺目标含 Linux/Windows（运行档）、wasm/Android/iOS（库嵌入，分阶段
  验收）；验收门与阶段见 `PLAN §12.26`；偏离一律登记 `DIV-`。
- **宿主事件循环（ADR-005，S 系列）**：宿主可用 `event` 能力域＋`pa_resume`/`pa_throw`＋
  host awaitable 驱动脚本协程；参考 C 宿主 `examples/reference_host_async.c`。**不承诺** L3
  （宿主 loop 上完整 asyncio）；CLI asyncio 仍走 Pyawa 循环（模式 B）。

## 验证证据

- 闸门 `tests/ci/check.py` **20/20**；`tools/slowcheck.sh --release` 十二项。
- 新 ABI 路径（`AB-73`…`AB-80`）valgrind **0 错误**；参考宿主七项演示绿。
- 布局一致性机检 `tests/ci/check_capability_layout.py` **PASS**（C↔Rust 56 行）。
- **Web（WASM）**：E1–E8 已进 main：`e2_host_function.js`／`e3_abi_surface.js`（ABI 面与错误码）／
  `e4_js_clock.js`（JS clock；fs 只提供宿主 stdout 写）／`e5_smoke.js`（纯计算＋协议派发）／
  `e6_recursion.js`（显式帧栈深递归）／`e7_embedding.js`（多实例／宿主函数／stdout）／
  `e8_portability.js`（可移植子集，10/10 与 native 一致；CI `portability-compare` 的 Linux×WASM 步骤）。
  asyncio-on-wasm 仅报告、不承诺（`ADR-002`）。
- **Windows（W1）**：本机交叉 `cargo check --workspace --target x86_64-pc-windows-msvc` **通过**
  （仅 type-check，不链接／不运行）；CI `windows` job 已接线（`cargo build --workspace` ＋
  `check.py`；`M3-COVERAGE` 显式 SKIP，原因见 `PLAN` §12.20 W1），GitHub runner 待首次实跑。
