# Windows asyncio：评估＋CLI selector shim 落地（2026-11-22）

**结论（更新）**：Windows CLI 的 asyncio 走 **selector 路线**已落地第一版：
CLI 启动时（仅 Windows、仅便利路径）把 asyncio 默认策略切到
`_WindowsSelectorEventLoopPolicy`，Windows CI 的 `asyncio.run` 冒烟已通过。
**Proactor 仍不支持**（`_overlapped` 数据面未接）；嵌入 API（`pa_create`/`paL_newstate`）不自动设策略，
宿主需自己设。`asyncio_smoke.rs` 仍是 `#![cfg(unix)]`，Windows 由 CI 外部冒烟覆盖。当前 main 的 asyncio 冒烟测试在
`#![cfg(unix)]` 下（`crates/pyawa-runtime/tests/asyncio_smoke.rs:4`），Windows CI 覆盖的是
最小构建/平台面，不是 asyncio 运行；此前 Windows asyncio spike（dev 分支）2/2 通过，但**代码
未进 main**（`docs/rounds/spikes/2026-11-21-asyncio-windows.md`；ADR-002 未接受导入后钩子方案）。

## 证据

- `crates/pyawa-runtime/tests/asyncio_smoke.rs:4`：`#![cfg(unix)]`，Windows 运行矩阵下诚实 gate；
- `crates/pyawa-runtime/src/win32_overlapped.rs:21`：Proactor 事件循环（`asyncio.windows_events`）
  **仍不能真跑**——重叠 I/O 数据面（`ReadFile`／`WriteFile`／`WSARecv`／`WSASend`／
  `ConnectNamedPipe`）未接，只做了 `_overlapped` 的句柄/事件最小语义；
- `.github/workflows/ci.yml` 的 Windows job 是最小构建（W1）与平台面，不跑 asyncio 冒烟；
- Windows 真实 provider 已有 `fs_windows`／`clock_windows`／`net_windows`／`win32_overlapped`，
  但 `net_windows` 是同步 socket 面，Proactor 需要的数据面尚未完成。

## 可选的落地路线

| 路线 | 内容 | 代价/风险 | 与 ADR |
|---|---|---|---|
| **A. selector facade** | 让 Windows 默认用 selector（`select`/`socket`）而不是 Proactor；补 Windows 的 self-pipe/signal 差异 | 中等；语义偏离 CPython 默认 Proactor，但可先跑常见脚本 | 需新的 facade 设计；不能走导入后钩子（ADR-002） |
| **B. Proactor 数据面** | 接 `_overlapped` 的 ReadFile/WriteFile/WSARecv/WSASend/ConnectNamedPipe | 大；跨平台语义与测试矩阵重 | 符合 CPython 语义，但工作量大 |
| **C. 宿主 loop（L2）** | Windows 上实现 `event` provider（IOCP/WSAPoll）＋宿主任务表，CLI 的 asyncio 仍走模式 B | 与 ADR-005 一致；不承诺 L3/asyncio-on-host | 长期架构一致；不是“CLI 正常 asyncio” |

## 建议

1. 若产品要求 **Windows CLI 直接跑 asyncio**：先做 A（selector facade）作为最小可用承诺，并把
   Proactor 差异登记为 `DIV-`；需要 Proactor 语义再排 B。
2. 若目标只是 **嵌入/宿主循环**：优先 C（Windows `event` provider），不要把它冒充 asyncio 承诺。
3. 在真正落地前：Windows asyncio smoke 必须进入 Windows CI，否则不发布“Windows asyncio 可用”。

## 已落地（2026-11-22，selector 路线）

- `crates/pyawa-runtime/src/bin/pyawa.rs`：Windows CLI 启动 shim，用
  `asyncio.events._set_event_loop_policy(_WindowsSelectorEventLoopPolicy())`（不触发弃用警告）；
- `.github/workflows/ci.yml` Windows job：`asyncio.run` 冒烟，覆盖
  `await asyncio.sleep(0)`、真实 `sleep(0.01)`、`async for`、`asyncio.gather`、`asyncio.TaskGroup`；
- 运行矩阵（Windows selector，2026-11-22 CI 实测）：

  | 能力 | 状态 |
  |---|---|
  | `asyncio.run` / 协程 / `sleep(0)` | ✓ |
  | `async for` / 异步生成器 | ✓ |
  | `asyncio.gather` | ✓ |
  | `asyncio.TaskGroup` | ✓（`async with __aexit__` 修复后复测通过） |
  | `asyncio.sleep(>0)` / 真实定时 | **未支持**（实测 10ms 定时 `InvalidStateError: Result is not ready.`） |
  | `asyncio.open_connection` / streams | 未承诺（net provider 同步面） |
  | `asyncio.create_subprocess_*` | **不承诺**（Proactor/管道数据面未接） |
  | `asyncio.add_reader` on pipe | 未承诺（Windows selector 不支持 pipe） |

- 已知边界：Proactor／`_overlapped` 数据面未接；`subprocess`、pipe transport、部分 `add_reader`
  不承诺；嵌入 API 不自动设策略（CLI shim 只在 CLI）。

## 更新（round 282）：默认切回 Proactor ✓

P1 文件/pipe、P2 socket（AcceptEx/ConnectEx/BindLocal）、P3 subprocess 数据面全部进 main 后，
按 `2026-11-22-proactor-dataplane-plan.md` 的 **P4** 删除 CLI 的 selector shim：

- `crates/pyawa-runtime/src/bin/pyawa.rs`：`install_windows_asyncio_selector` 与其调用删除；
  CLI 不再改策略 ⇒ 走 CPython 默认 `_WindowsProactorEventLoopPolicy`／`ProactorEventLoop`；
- selector 保留为**脚本显式逃生口**（`_set_event_loop_policy(_WindowsSelectorEventLoopPolicy())`），
  由 `windows_selector_opt_in_still_works` 验收；
- Windows 测试新增（`crates/pyawa-runtime/tests/async_with_cli.rs`）：
  `windows_default_loop_policy_is_proactor`（策略名＋ loop 类型）、
  `windows_default_proactor_timer_runs`（真实 `sleep(0.01)`）、
  `windows_selector_opt_in_still_works`；subprocess／TCP echo 两例改为**不显式设策略**
  （直接验默认路径）。

**state（round 282 后）**：Windows CLI asyncio **默认 Proactor**；嵌入 API（`pa_create`／
`paL_newstate`）仍不自动设策略，宿主需自设。**仍如实边界**：IPv6 `ConnectEx` 未接、
pipe 少数分支、`OSError.winerror` 只在 Windows 平台层补齐；`Lib/` 未改（`CX-8`）。
