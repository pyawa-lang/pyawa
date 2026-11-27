# NEXT（2026-11-21 · alpha 迁移完成；工作流＝main + feat/spike）

- **HEAD**：`2c67f3ab`（IPv6 R3 ＃87；本笔前）· **工作区**：干净
- **当前主线**：`main`（受保护，PR-only，squash-only；既有 m1–m6/alpha 历史与 tag 未改写）
- **分支模型**：`main` ＋ `feat/*`（短命）＋ `spike/*`（永不合并）；见 `docs/WORKFLOW.md`
- **历史**：`archive/pre-alpha-dev` = `cea7e306`；`dev` 已删除（ADR-003）；安全网
  `target/archive/dev-full.bundle`
- **已采用**：portability／frame-stack／windows-platform＋平台补齐／win32-real-semantics／
  wasm E1–E8（见 `docs/migrations/STATUS.md`）
- **已采用（round 282）**：Windows CLI asyncio **默认 Proactor**（P1–P3 数据面全绿后切回；selector 仅作脚本显式逃生口）
- **已采用（round 286）**：Windows CLI asyncio **数字 IPv6**（`CP-47`：4 元组 ⇔ 18B wire；DNS／`scope_id`／`flowinfo` 仍不在）
- **仅报告不采用**：JSPI／asyncio-on-wasm（`docs/rounds/spikes/`）
- **架构决定（2026-11-21）**：ADR-005 accepted；长期目标＝L2 宿主拥有事件循环，
  L1 event 域为底座，L3 不承诺；详见 `docs/DESIGN-event-source.md`、`PLAN §12.27`。
- **下一步（按序）**：
  1. S1 规格 ✓ 草案：`SPEC-capabilities.md` §9.10（`CP-40`…`CP-46`）＋`SPEC-c-abi.md`
     §15.6（`AB-82`…`AB-87`）；host awaitable 不在 S1（`AB-86`）；
  2. S1.1 规格 ✓ 草案：host awaitable 对象协议（`AB-88`…`AB-91`）；
     S2 Spike C ✓：宿主 loop 驱动含 `await` 的协程五态可行（报告已进 main）；
     下一步 S3：`pa_resume`/`pa_throw` ✓、`event` 域 ✓、`await_fn`/VM await ✓（1.17）；
     `await_cancel_fn` 派发 ✓（INTERRUPT/DESTROY/THROW）；参考宿主 await 场景 ✓
     （`reference_host_loop.c`：正常完成/多次 resume/错误/THROW/INTERRUPT 恢复/DESTROY）；
  3. S3 实现 ✓：additive ABI＋参考宿主（参考 loop 宿主 await 六类场景，＃81）；asyncio 仍为可选 facade；
  4. 发布面：alpha.2/beta tag 与 ABI 冻结评估（§12.18 B3 仍范围外）。
- **已知边界（不承诺）**：完整命名管道 transport（服务端 `AcceptEx` 取消／关闭）、IPv6 的 DNS／`scope_id`／`flowinfo`、
  非 CLI 路径的 `inet_pton`／`inet_ntop`（嵌入仍为如实桩）。
- **硬线**：代码 PR 须 `check.py` 20/20＋`slowcheck` 十二项全绿；CI 必需检查见 WORKFLOW。
