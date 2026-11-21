# 宿主事件循环与协程驱动（ADR-005，accepted）

**状态**：`accepted`（2026-11-21，用户拍板：采用 **L2＝宿主拥有事件循环** 为长期架构目标；
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

## `event` 能力域（L1 底座）

新增独立能力域（与 `fs`/`net`/`clock` 并列；表尾追加，`AB-41`）：

- `wait(entries, timeout_ns)`：就绪/完成事件，条目为 `(opaque key, events)`；
  两段式出参（`out_len` 总就绪数，容量不足由调用方扩容重试）；超时 ⇒ 空集合；
- `notify(key)`／`cancel(key)`：**可选槽**（`None` 即 provider 不支持，`CP-3`）；
- 取消：与 `pa_interrupt` 统一口径；`timeout` 与 `cancel` 必须是不同状态；
- 事件位沿用 POSIX `poll` 规范值（`CP-10` 的 POLLIN 一族）。
- JS（JSPI/Asyncify/回调）、native（epoll/kqueue/WSAPoll/IOCP）、RTOS 都是 provider adapter；
  **内核不认识 OS 原语**。

## `pa_resume` 族（additive ABI，待规格定稿）

- `pa_spawn(state, ...)`：在宿主侧创建可驱动协程（或由脚本返回协程对象，宿主经对象协议持有）；
- `pa_resume(state, nargs, nresults)`：恢复协程；结构化成因：
  - `PA_YIELDED`（带 yield 值），`PA_COMPLETED`（带返回值），`PA_RAISED`（异常经
    `pa_errmsg`/异常对象通道）；错误码与 `PA_OK/PA_YIELDED/PA_COMPLETED` 的最终取值在规格里定；
- `pa_throw(state, exc)`：把宿主异常送回协程（对应 `throw`）；
- host awaitable：宿主可提供 `await` 对象；VM `await` 到时返回 `PA_YIELDED` 并带
  host awaitable 句柄，宿主事件就绪后 `pa_resume` 送回值/异常；
- 取消：`pa_interrupt` 在 resume 边界生效，语义与现有执行入口一致；
- 兼容：现有对象协议（`pa_call` + `getfield("send")`）保留为兼容路径；**不删旧面**。

## 约束与风险

- **GC/引用**：协程挂起期间帧、值、异常必须生根；实例销毁时释放；
- **重入**：宿主回调不得并发进入同一实例；单实例单线程契约沿用 `AB-64`；
- **错误映射**：完成/异常/取消三态不可混用 `PA_ERR_RUNTIME`；需新增状态码；
- **平台**：先 Linux 参考宿主打通，再 Windows/wasm adapter；不提前承诺；
- **ABI**：只增不破（次版本追加）；asyncio 的实现架构不进入内核（ADR-002）。

## 分阶段（PLAN §12.27）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **S1 规格** | `event` vtable ＋ `pa_resume`/`pa_throw`/host awaitable 的 ABI 规格（错误码、栈契约、取消、GC） | 文档评审通过；不写代码 |
| **S2 Spike C** | 宿主 loop 驱动一个 `await` host awaitable 的脚本协程；异常/取消/完成三态；参考 C 宿主 | spike 报告＋证据（不改产品 ABI） |
| **S3 实现** | `event` 域＋`pa_resume` 族（additive）、参考宿主、旧 ABI 宿主不受影响 | `check.py` 20/20；slowcheck 12/12；CI 全绿 |
| **S4 文档/承诺** | 更新 DESIGN/PLAN/RELEASE/宿主指南；asyncio 仍为可选 facade | 文档一致；不承诺 L3 |

## 非目标

- 不在本轮改 `Lib/asyncio`、不扩产品 ABI、不实现 `event`/`pa_resume`；
- 不承诺“宿主 loop 上完整 asyncio”（L3）；
- 不用 asyncio 的实现架构反向定义内核。
