# Spike C：宿主 loop 驱动 `await` 协程（2026-11-21）

- **问题**（`ADR-005` S2）：宿主能否在自己的循环里驱动一个含 `await` 的脚本协程，
  并观察到 yield token、恢复值、完成值、异常、取消五态？不改产品 ABI。
- **方法**：`spike_host_awaitable.c`——脚本侧 `Token.__await__` 把宿主 token `yield` 出来；
  宿主用现有对象协议（`pa_getglobal`→`pa_call`）执行 `start`/`resume`；宿主 loop 维护
  “token → 恢复值”的映射；异常由脚本包装器转成 `error` 状态；取消用 `pa_interrupt`。
  运行环境：Linux x86_64，main `6ad16341` 的 `target/release/libpyawa_runtime.so`。
- **证据**：
  ```
  PASS loop yield token=io1
  PASS loop completed result=io1:A
  PASS loop exception type=ValueError
  OBS cancel resume rc=4 msg=(null)
  PASS loop cancel rc=4
  SPIKE-C PASS：宿主 loop 驱动 await 协程（yield/恢复/完成/异常/取消）
  ```
- **结论**：
  1. **可行**：现有对象协议足以让宿主循环驱动 `await` 协程，五态可达；
  2. `await` 的对象目前是**脚本侧 wrapper（`Token`）**，宿主只送值——真正“宿主提供
     awaitable 对象（`__await__`/迭代器在宿主）”**尚未验证**，需要 S1.1 对象协议；
  3. 异常路径需要**脚本包装器**转成可读状态（现有 ABI 无 `pa_throw` 的结构化通道）；
  4. 取消：`pa_interrupt` 后下一次 resume 返回 `PA_ERR_INTERRUPT`（4），`pa_errmsg` 为空；
     取消后协程仍挂起，生命周期/销毁语义需在 S1.1 定；
  5. 建议 S1.1 定义 host awaitable 的 object op（await/next/resume）与取消/错误码，
     并明确“取消后协程可销毁、不可再 resume”。
