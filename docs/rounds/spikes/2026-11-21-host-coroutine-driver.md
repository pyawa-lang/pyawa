# Spike A：宿主驱动协程（不改 ABI，2026-11-21）

- **问题**（`ADR-005`）：宿主能否在本机事件循环里驱动脚本生成器/协程，而不改产品 ABI？
- **方法**：`spike_host_coroutine.c`——用现有 `pa_getglobal`／`pa_getfield("send")`／
  `pa_pushnil`／`pa_pushinteger`／`pa_call` 驱动纯生成器的 `send`；yield 值经 `pa_tointeger` 取；
  `StopIteration` 经 `pa_call` 错误路径观察；完成值由脚本写入全局 `result` 供宿主读取。
  运行环境：Linux x86_64，`target/release/libpyawa_runtime.so`（main `db9e9765`）。
- **证据**：
  ```
  PASS step1 yield=10
  PASS step2 yield=5
  OBS step3 rc=1 msg=StopIteration: 12
  PASS completion result=12
  SPIKE-A PASS：宿主经对象协议驱动生成器（yield/完成值/StopIteration）
  ```
- **结论**：现有对象协议**足以驱动生成器**（yield／完成值／异常三态都可达）；但：
  1. 完成值只能从 `pa_errmsg` 字符串（`StopIteration: 12`）解析，或要求脚本另写全局变量；
  2. 异常路径与业务错误共用 `PA_ERR_RUNTIME`，宿主无法区分“协程完成”与“脚本抛错”；
  3. 每次 resume 都要手动重组 `getglobal → getfield(send) → call`，无状态/错误码契约；
  4. async coroutine 与生成器不同（`send(None)` 后可能返回 awaitable），一等 API 必须定义
     “yield 出的 awaitable”与事件源的关系。
- **对 ADR-005 的建议**：`pa_resume` 族应提供**结构化状态**（yielded／completed／raised）、
  返回值通道、异常对象通道，并与 `pa_interrupt` 的取消语义统一；生成器路径可作为兼容基线。
