# Spike B：`event` provider 形状原型（不接 VM，2026-11-21）

- **问题**（`ADR-005`）：拟议的 `event` 能力域（宿主事件源 provider）用怎样的 wait/notify/cancel
  形状可以表达就绪、超时、取消、两段式出参与宿主循环？
- **方法**：`spike_event_source.c`——C mock provider（两个 key、读写就绪位、可取消、mock 时钟）
  ＋ host loop 骨架；不接 VM、不改产品 ABI、不进 CI。运行环境：Linux x86_64。
- **证据**：
  ```
  PASS timeout out_len=0 now_ns=1000
  PASS ready key=1 revents=0x1
  PASS two-phase out_len=1
  PASS cancel
  PASS host-loop done=1,1
  SPIKE-B PASS：event provider 形状（ready/timeout/cancel/two-phase/host-loop）可表达
  ```
- **结论**：形状可表达；建议定案时明确：
  1. `wait(entries, timeout_ns)` 的两段式出参（`out_len` 总就绪数、容量不足由调用方扩容重试）；
  2. **timeout 与 cancel 必须是不同状态**（超时返回空集合，取消走 `pa_interrupt` 口径）；
  3. `notify`／`cancel` 是可选槽：`None` 即该 provider 不支持（`CP-3`），宿主不得假设；
  4. 事件位与现有 `select`／`poll` 映射保持 POSIX 值（`CP-10` 的 POLLIN 一族），不新造语义；
  5. 宿主循环骨架：wait→dispatch→resume，不把循环写进内核；JS/native/RTOS 都是 adapter。
- **对 ADR-005 的建议**：`event` 域作为独立能力域表尾追加；先在 spike 里用 C mock 固定语义，
  再写正式 vtable 规格与 provider 适配器清单。
