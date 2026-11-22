# 宿主事件循环与协程驱动（ADR-005，accepted）

**状态**：`accepted`＋**S 系列已实现**（2026-11-21，用户拍板：采用 **L2＝宿主拥有事件循环** 为长期架构目标；
L1 是其底座，L3 不承诺）。本文件是目标设计；实现须按 `PLAN §12.27` 分阶段走，spike 通过前
不写产品 ABI。

## 决策

- **主推模式 A（宿主驱动，Lua 式）**：宿主拥有事件循环；Pyawa 提供 VM/协程与
  `event` 能力域（事件源 provider）＋ `pa_resume` 族（additive ABI）；宿主在自己的循环里
  resume 脚本协程。
- **保留模式 B（Pyawa 循环，CPython 兼容）**：现有 `pa_exec_*` 同步阻塞语义不变；
  `Lib/asyncio` 作为可选 CPython 语义 facade，在 `event`/`fs`/`net` 能力上运行（Linux 桌面）。
- **L3（宿主 loop 上完整 asyncio）不承诺**：不重做 asyncio 的 transports/subprocess/signals；
  `Lib/` 不改（`CX-8`）。

## 执行模型（双模式）

```
模式 A（宿主驱动）：
  宿主循环 ──> pa_resume(coroutine, value) ──> Pyawa VM
      ^                                           │
      │  yielded(value) / completed(value) / raised(exc)
      └────────────── 宿主 event provider（readiness/定时/取消）

模式 B（Pyawa 循环）：
  脚本 pa_exec_* ──> Lib/asyncio loop ──> select/selectors ──> event/fs/net provider
```

两模式可同时存在于一个宿主进程的不同实例；同一实例不并发进入（`AB-64`）。

## `event` 能力域（L1 底座）——S1 规格见 `SPEC-capabilities.md` §9.10（`CP-40`…`CP-46`）

新增独立能力域（与 `fs`/`net`/`clock` 并列；表尾追加，`AB-41`）：

- `wait(entries, timeout_ns)`：就绪/完成事件，条目为 `(opaque key, events)`；
  两段式出参（`out_len` 总就绪数，容量不足由调用方扩容重试）；超时 ⇒ 空集合；
- `notify(key)`／`cancel(key)`：**可选槽**（`None` 即 provider 不支持，`CP-3`）；
- 取消：与 `pa_interrupt` 统一口径；`timeout` 与 `cancel` 必须是不同状态；
- 事件位沿用 POSIX `poll` 规范值（`CP-10` 的 POLLIN 一族）。
- JS（JSPI/Asyncify/回调）、native（epoll/kqueue/WSAPoll/IOCP）、RTOS 都是 provider adapter；
  **内核不认识 OS 原语**。

## `pa_resume` 族（additive ABI，S1 规格见 `SPEC-c-abi.md` §15.6／`AB-82`…`AB-87`）

- `pa_spawn(state, ...)`：在宿主侧创建可驱动协程（或由脚本返回协程对象，宿主经对象协议持有）；
- `pa_resume(state, nargs, nresults)`：恢复协程；结构化成因：
  - `PA_YIELDED`（带 yield 值），`PA_COMPLETED`（带返回值），`PA_RAISED`（异常经
    `pa_errmsg`/异常对象通道）；错误码与 `PA_OK/PA_YIELDED/PA_COMPLETED` 的最终取值在规格里定；
- `pa_throw(state, exc)`：把宿主异常送回协程（对应 `throw`）；
- host awaitable（S1.1，`SPEC-c-abi.md` §11.1／`AB-88`…`AB-91`）：宿主类型可装
  `await_fn`（返回 token）与可选 `await_cancel_fn`（INTERRUPT／THROW／DESTROY）；
  脚本 `await` 时 VM 返回 `PA_YIELDED`＋token，宿主事件就绪后 `pa_resume` 送值、
  `pa_throw` 送异常；取消后协程不可再 resume；未装槽 ⇒ 行为同 S1；
- 取消：`pa_interrupt` 在 resume 边界生效，语义与现有执行入口一致；
- 兼容：现有对象协议（`pa_call` + `getfield("send")`）保留为兼容路径；**不删旧面**。

## 约束与风险

- **GC/引用**：协程挂起期间帧、值、异常必须生根；实例销毁时释放；
- **重入**：宿主回调不得并发进入同一实例；单实例单线程契约沿用 `AB-64`；
- **错误映射**：完成/异常/取消三态不可混用 `PA_ERR_RUNTIME`；需新增状态码；
- **平台**：先 Linux 参考宿主打通，再 Windows/wasm adapter；不提前承诺；
- **ABI**：只增不破（次版本追加）；asyncio 的实现架构不进入内核（ADR-002）。

## 实现状态（2026-11-21）

S1 规格、S1.1 规格、S2 Spike C、S3 实现、S4 文档/承诺均已收口：
`pa_resume`/`pa_throw`（1.15）、`event` 域（1.16）、host awaitable `await_fn`/`await_cancel_fn`
＋VM await 派发（1.17）；参考 C 宿主（`examples/reference_host_async.c`）覆盖 resume/event/await；
旧 ABI 宿主零回归。**宿主 loop 参考适配器**（`examples/reference_host_loop.c`）用真实
`poll(2)` 作 event provider，演示任务表与超时；**CLI asyncio 仍为模式 B**（Pyawa 循环、
Linux 可用）；**L3 不承诺**。

## 分阶段（PLAN §12.27）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **S1 规格 ✓ 草案（2026-11-21）** | `event` vtable ＋ `pa_resume`/`pa_throw`（状态码、栈契约、取消、GC）；host awaitable 不在 S1 | `SPEC-capabilities.md` §9.10（`CP-40`…`CP-46`）＋`SPEC-c-abi.md` §15.6（`AB-82`…`AB-87`） |
| **S1.1 规格 ✓ 草案（2026-11-21）** | host awaitable 对象协议：`await_fn`/`await_cancel_fn`、token/恢复/异常/取消生命周期、追加式 ABI | `SPEC-c-abi.md` §11.1（`AB-88`…`AB-91`）；T-AB-24…26 |
| **S2 Spike C ✓（2026-11-21）** | 宿主 loop 驱动含 `await` 的脚本协程；yield/恢复/完成/异常/取消五态；参考 C 宿主 | 报告：`docs/rounds/spikes/2026-11-21-host-awaitable-loop.md`；**真正 host awaitable 对象协议未验证**（S1.1） |
| **S3 实现** | `event` 域＋`pa_resume` 族（additive）、参考宿主、旧 ABI 宿主不受影响 | `check.py` 20/20；slowcheck 12/12；CI 全绿 |
| **S4 文档/承诺** | 更新 DESIGN/PLAN/RELEASE/宿主指南；asyncio 仍为可选 facade | 文档一致；不承诺 L3 |

## 非目标

- 不在本轮改 `Lib/asyncio`、不扩产品 ABI、不实现 `event`/`pa_resume`；
- 不承诺“宿主 loop 上完整 asyncio”（L3）；
- 不用 asyncio 的实现架构反向定义内核。
