# NEXT（2026-11-21 · alpha 迁移完成；工作流＝main + feat/spike）

- **HEAD**：`a8d6ad8a`（wasm P1 采用；本收口文档提交见 `git log`）· **工作区**：干净
- **当前主线**：`main`（受保护，PR-only，squash-only；既有 m1–m6/alpha 历史与 tag 未改写）
- **分支模型**：`main` ＋ `feat/*`（短命）＋ `spike/*`（永不合并）；见 `docs/WORKFLOW.md`
- **历史**：`archive/pre-alpha-dev` = `cea7e306`；`dev` 已删除（ADR-003）；安全网
  `target/archive/dev-full.bundle`
- **已采用**：portability／frame-stack／windows-platform＋平台补齐／win32-real-semantics／
  wasm E1–E8（见 `docs/migrations/STATUS.md`）
- **仅报告不采用**：JSPI／asyncio-on-wasm／Windows asyncio 后端（`docs/rounds/spikes/`）
- **下一步（待用户定）**：
  1. 文档同步：按 ADR-001/002 回填 PLAN/DESIGN/REQUIREMENTS/RELEASE（需授权）；
  2. 架构面：宿主事件源 provider／协程驱动 ABI（ADR-002 的后续设计）；
  3. 发布面：alpha.2/beta tag 与 ABI 冻结评估（§12.18 B3 仍范围外）。
- **硬线**：代码 PR 须 `check.py` 20/20＋`slowcheck` 十二项全绿；CI 必需检查见 WORKFLOW。
