# 宿主事件源与协程驱动（ADR-005，提案）

**状态**：`proposed`（2026-11-21，用户授权「可以进行 1 2」中的架构设计项）。
本文件是设计提案，不是已承诺 ABI；实现前须先完成 spike 并接受 ADR-005。

## 问题

Pyawa 的定位是 Lua 式**可嵌入内核**（`ADR-001`）：I/O 与事件循环属于宿主。当前缺口：

1. 脚本执行入口 `pa_exec_*` 是**阻塞**的；宿主事件循环无法在脚本 `await` 时接管控制权；
2. `Lib/asyncio` 作为 CPython 语义 facade（`ADR-002`）目前通过 `selectors`/`select` 的
   **阻塞等待**实现，事件源隐含在 `fs`/`net`/`select` 里，没有独立的宿主契约；
3. 宿主若想“驱动脚本协程”（Lua 的 `lua_resume` 角色），只能手动走对象协议：
   `pa_call` 取协程对象 → `pa_getfield("send")` → `pa_call` 逐次驱动；缺一等 API 与错误/值契约。

## 约束

- 不改 `Lib/`（`CX-8`）；不破坏现有 ABI（新面只能**表尾追加/新增符号**，走 `AB-41` 的次版本规则）；
- 不把 asyncio 的实现架构（selector/proactor/self-pipe/JSPI）写进内核（`ADR-002`）；
- 平台差异只在 provider；内核保持平台无关。

## 提案（两件正交的事）

### 1. `event` 能力域（宿主事件源 provider）

新增一个**独立能力域**（与 `fs`/`net`/`clock` 并列；`SPEC-capabilities.md` 表尾追加）：

- `wait(entries, timeout_ns)`：就绪/完成事件（条目含 opaque key 与事件位），可取消（沿用
  `pa_interrupt`）；超时如实返回空；
- `notify(key)`／`cancel(key)`：宿主侧唤醒/取消（可选，按 provider 能力如实缺失）；
- `clock` 仍由 clock 域提供；`event` 不解释 OS 原语。

`select`/`selectors`/`Lib/asyncio` 的兼容外壳优先映射到 `event`；`fs`/`net` 的 poll 可逐步
收敛为 `event` 的 provider 实现（不是本轮强制）。JS 宿主的 JSPI/Asyncify/回调都是 `event`
provider 的**适配器实现**，不进入内核。

### 2. 协程驱动 ABI（`pa_resume` 族，**additive**）

宿主事件循环需要：

- 在栈顶放一个协程/生成器对象；
- `pa_resume(state, nargs, nresults)`：恢复执行；返回 `PA_OK` 时用出参/状态区分
  **yield（带值）** 与 **完成**；异常映射为 Python 异常（`PA_ERR_RUNTIME`＋`pa_errmsg`）；
- `pa_throw(state, exc)`：把宿主异常送回协程（对应生成器 `throw`）；
- 已有对象协议（`pa_call`＋`send`）保留为兼容路径；新 API 只补“一等驱动”，不删旧面。

**执行模型**：不改变普通 `pa_exec_*` 的阻塞语义；只在宿主显式使用 `pa_resume` 时进入
“宿主驱动”模式。宿主事件源回调 → `pa_resume` → 脚本 yield/完成 → 宿主继续自己的循环。

### 3. 与 asyncio 的关系

- `Lib/asyncio` 仍是**可选 facade**：在提供 `event`/`fs`/`net` 能力的宿主上投影 CPython 语义；
- 不承诺 wasm/移动端“全平台 asyncio 同语义”（`ADR-002`）；
- asyncio-on-wasm 的 JSPI/Asyncify 结论只作 spike 报告（`docs/rounds/spikes/`）。

## 非目标

- 不在本轮改 `Lib/asyncio`、不扩 ABI 版本、不实现 `event` 域或 `pa_resume`；
- 不用 asyncio 的实现架构反向定义内核；
- 不为旧 WebView 伪造 POSIX 行为。

## Spike 计划（ADR 接受前）

1. **Spike A（不改 ABI）**：用 C/ctypes 宿主按现有对象协议驱动 Python 生成器/协程
   （`pa_call` 取对象 → `pa_getfield("send"/"throw")` → `pa_call`），记录 yield/完成/异常
   的值与错误路径；产出 `docs/rounds/spikes/` 报告。
2. **Spike B（provider 原型）**：在 `tests/spikes/` 用现成 vtable 形态做最小 `event.wait`
   原型（不注册进产品、不进 CI）；验证宿主可唤醒等待、取消与超时语义。
3. 依据两份 spike 写 ADR-005 的最终 ABI 形状、错误码与版本策略；`accepted` 后才实现。

## Spike 结果（2026-11-21，已完成）

**Spike A（宿主驱动生成器，不改 ABI）**：`spike_host_coroutine.c` 用现有
`pa_getglobal`→`pa_getfield("send")`→`pa_call` 驱动纯生成器；yield 值、完成值、`StopIteration`
三态实测可达。**发现**：完成值只能从 `pa_errmsg`（`StopIteration: 12`）解析或写全局变量；
完成与业务异常共用 `PA_ERR_RUNTIME`；async coroutine 与生成器不同。⇒ `pa_resume` 必须给
**结构化状态**（yielded／completed／raised）与返回值/异常通道。

**Spike B（`event` provider 形状）**：`spike_event_source.c` 的 mock provider＋host loop 验证
了就绪集合、timeout、cancel、两段式出参（容量不足时返回总长）与循环骨架。**发现**：
timeout 与 cancel 必须是不同状态；`notify`／`cancel` 应为可选槽（如实 `None`）；事件位沿用
POSIX `POLLIN` 一族；宿主循环（wait→dispatch→resume）属于 adapter，不写进内核。

**下一步**：据 A/B 证据写 `event` vtable 规格与 `pa_resume` 状态/错误码，提交 ADR-005
review；**仍为 proposed**，不实现产品代码。

## 验收（ADR 接受判据）

- spike A 证明“宿主驱动协程”的值/异常/完成三态契约可实现且无卡死；
- spike B 证明 `event` provider 可在现有能力域规则下表尾追加、不破坏旧宿主；
- 明确 `pa_resume` 状态/错误码、与 `pa_interrupt` 的取消语义、以及 `Lib/asyncio` facade 的映射；
- 通过 `check.py` 与文档评审；实现阶段再满足 slowcheck 与 CI 全绿。
