# Proactor 数据面实施计划（Windows asyncio 剩余项，2026-11-22）

**状态**：**P1–P4 已落地 ✓**（round 282）；**P5 规格＋provider＋guest 面已落地 ✓**（round 283／285／286）。P1 文件/pipe（#69／#72）、P2 socket
（AcceptEx/ConnectEx/BindLocal，#79）、P3 subprocess（#72／#79）已进 main；P4 已把 Windows CLI
默认策略从 selector 切回 **CPython 默认 Proactor**（round 282）。selector 仅保留为脚本显式逃生口。

## 当前证据

- `crates/pyawa-runtime/src/win32_overlapped.rs`：`ReadFile`／`WriteFile`／`WSARecv`／
  `WSASend`／`AcceptEx`／`ConnectEx`／`BindLocal`／`ConnectNamedPipe`／`GetOverlappedResult`
  已接真实语义（P1–P3）；`ERROR_BROKEN_PIPE` 按 CPython 分读/写口径抛 `BrokenPipeError`、
  `OSError.winerror` 已在 Windows 平台层补齐。
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
| **P1 文件/pipe 重叠 I/O（进行中）** | **文件部分 ✓**：`CreateFile`＋`ReadFile`／`WriteFile`／`GetOverlappedResult`／`getresult`（Windows 测试 `windows_overlapped.rs`）；**pipe 部分待做**：`CreatePipe`／`ConnectNamedPipe` | 文件读写 ✓；pipe/cancel/GC 待补 |
| **P2 socket 重叠 I/O** | `WSARecv`／`WSASend`（含 Into 变体）、`WSARecvFrom`／`WSASendTo` | 本机 loopback 连接读写；与 selector 结果对拍 |
| **P3 subprocess/pipe transports** | `asyncio.create_subprocess_exec` 的 stdin/stdout/stderr pipes | `asyncio.run` 跑子进程 echo；取消与超时 |
| **P4 默认策略 ✓（round 282）** | P1–P3 全绿后删除 CLI 的 selector shim ⇒ 走 CPython 默认 **Proactor**；selector 保留为脚本显式逃生口（`_set_event_loop_policy(_WindowsSelectorEventLoopPolicy())`） | 文档更新 ✓；Windows CI（`asyncio.run` 冒烟＋运行矩阵）全绿 ✓；新增 4 个 Windows 测试（默认策略／真实定时器／selector 逃生口／默认 Proactor 下 subprocess＋TCP echo）|

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

- 不修改 `Lib/`（`CX-8`）；
- 不在 P1–P3 未全绿时切默认策略（`P4` 的前置条件；2026-11-22–round 281 期间保持 selector）。
- **已补（round 284）**：`_overlapped.ConnectPipe` 客户端接线＋ `OSError.winerror` 默认属性
  （类属性 `None`，Win32 错误仍由实例属性覆盖）。**仍不做**：完整命名管道 transport
  （`start_serving_pipe` 服务端 `AcceptEx` 的取消／关闭路径；照抄 e2e 会挂）。
- **P5 ✓（round 286 完成 guest 面）**：IPv6（`CP-47` 规格 ✓、`R2` Windows provider 数据面 ✓（round 285：canonical 10 ⇒ Winsock 23 映射、28B `SOCKADDR_IN6`、128B storage、`IPV6_V6ONLY` 26⇒27、`ConnectEx`/`BindLocal`/`inet_pton`/`inet_ntop`）、`R3` guest `_socket` 地址线 ✓（4 元组／18B wire、`getaddrinfo` 数字 IPv6）＋ asyncio `::1` e2e ✓（round 286）；**边界**：只接数字 IPv6、不带 `scope_id`／`flowinfo`、DNS 不在 v1，`inet_pton`／`ntop` 真实实现只在 Windows CLI）。
