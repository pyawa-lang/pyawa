# 评估：Windows asyncio（2026-11-22，只读评估，无代码落地）

**结论**：Windows CLI 的 asyncio **仍不是承诺**。当前 main 的 asyncio 冒烟测试在
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

## 未做

- 未改任何代码/CI/承诺；本报告只是评估，`asyncio_smoke` 仍是 Unix-only。
