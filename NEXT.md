# NEXT（2026-11-21 · alpha 迁移完成；工作流＝main + feat/spike）

- **HEAD**：`a8d6ad8a`（wasm P1 采用；本收口文档提交见 `git log`）· **工作区**：干净
- **当前主线**：`main`（受保护，PR-only，squash-only；既有 m1–m6/alpha 历史与 tag 未改写）
- **分支模型**：`main` ＋ `feat/*`（短命）＋ `spike/*`（永不合并）；见 `docs/WORKFLOW.md`
- **历史**：`archive/pre-alpha-dev` = `cea7e306`；`dev` 已删除（ADR-003）；安全网
  `target/archive/dev-full.bundle`
- **已采用**：portability／frame-stack／windows-platform＋平台补齐／win32-real-semantics／
  wasm E1–E8（见 `docs/migrations/STATUS.md`）
- **仅报告不采用**：JSPI／asyncio-on-wasm／Windows asyncio 后端（`docs/rounds/spikes/`）
- **架构决定（2026-11-21）**：ADR-005 accepted；长期目标＝L2 宿主拥有事件循环，
  L1 event 域为底座，L3 不承诺；详见 `docs/DESIGN-event-source.md`、`PLAN §12.27`。
- **下一步（按序）**：
  1. S1 规格 ✓ 草案：`SPEC-capabilities.md` §9.10（`CP-40`…`CP-46`）＋`SPEC-c-abi.md`
     §15.6（`AB-82`…`AB-87`）；host awaitable 不在 S1（`AB-86`）；
  2. S2 Spike C：宿主 loop 驱动 `await` host awaitable 的协程（不改产品 ABI）；
  3. S3 实现：additive ABI＋参考宿主；asyncio 仍为可选 facade；
  4. 发布面：alpha.2/beta tag 与 ABI 冻结评估（§12.18 B3 仍范围外）。
- **硬线**：代码 PR 须 `check.py` 20/20＋`slowcheck` 十二项全绿；CI 必需检查见 WORKFLOW。
