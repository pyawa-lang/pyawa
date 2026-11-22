# Proactor 数据面实施计划（Windows asyncio 剩余项，2026-11-22）

**状态**：计划，**未实现**。Windows CLI asyncio 当前走 selector 子集；Proactor
（`asyncio.windows_events`、subprocess／pipe transport）仍不承诺。

## 当前证据

- `crates/pyawa-runtime/src/win32_overlapped.rs`：`_winapi` 的事件／IOCP 句柄与
  `_overlapped.Overlapped` 的最小真实语义已有；但 `ReadFile`／`WriteFile`／
  `WSARecv`／`WSASend`／`ConnectNamedPipe` 仍如实 `NotImplementedError`
  （文件头“如实边界”）。
- `pyawa-stdlib` 的 Windows 桩由 `paL_newstate`／CLI 覆盖；裸 `pa_create` 不自动安装
  Proactor 相关模块。
- `asyncio.windows_events` 的 Proactor 路径需要：
  - `_overlapped`：`Overlapped.ReadFile/WriteFile/WSARecv/WSASend/WSARecvInto/
    WSASendInto/WSARecvFrom/WSARecvFromInto/WSASendTo/ConnectNamedPipe/
    GetOverlappedResult/cancel`；
  - `_winapi`：`CreatePipe`、`ConnectNamedPipe`、`CreateFile`、`GetOverlappedResult`
    等（视 subprocess/pipe 路径）。
- Windows selector smoke 已覆盖 `run`／协程／`sleep`／`async for`／`gather`／`TaskGroup`；
  Proactor 不在这条覆盖内。

## 分阶段

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P1 文件/pipe 重叠 I/O** | `ReadFile`／`WriteFile`／`GetOverlappedResult`＋IOCP 完成包；`CreatePipe`/`ConnectNamedPipe` 最小面 | Windows 单测：真实 pipe 读写、cancel、GC 后句柄回收 |
| **P2 socket 重叠 I/O** | `WSARecv`／`WSASend`（含 Into 变体）、`WSARecvFrom`／`WSASendTo` | 本机 loopback 连接读写；与 selector 结果对拍 |
| **P3 subprocess/pipe transports** | `asyncio.create_subprocess_exec` 的 stdin/stdout/stderr pipes | `asyncio.run` 跑子进程 echo；取消与超时 |
| **P4 默认策略** | 若 P1–P3 全绿，才评估把 Windows 默认从 selector 切回 Proactor；否则保持 selector | 文档更新 + Windows CI 全绿 |

## 风险

- OVERLAPPED 内存必须活到 IOCP 完成包；GC 早回收 ⇒ 悬垂指针/崩溃；
- 缓冲区（`bytearray`）生命周期与 `GetOverlappedResult` 的“已完成/待完成”两态；
- `ConnectNamedPipe` 的 `ERROR_PIPE_CONNECTED` 等 Win32 细节；
- 取消：`CancelIoEx`＋完成包顺序，不能泄漏挂起事件；
- Windows CI 无本地复现环境，必须靠 Windows runner 逐步加测试。

## 测试入口（Windows CI）

- `crates/pyawa-runtime/tests/windows_overlapped.rs`（现有形状测试扩展）；
- 新增 `windows_proactor_async.rs`：`asyncio.run` 跑真实 pipe／子进程；
- 每个阶段只加一个测试面，红了先修再推进。

## 不做

- 不修改 `Lib/`（CX-8）；
- 不在 selector 子集已够用时盲目切默认策略；
- 不把 Proactor 作为 Windows asyncio 承诺，直到 P1–P3 全绿并进入 RELEASE。
