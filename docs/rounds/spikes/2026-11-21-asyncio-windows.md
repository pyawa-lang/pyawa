# Spike：Windows asyncio 默认后端（2026-11-20）

- **问题**：Windows 上让 `Lib/asyncio` 默认走 selector 后端、而非 Windows CPython 的 Proactor。
- **实验**（dev `1e3bd26b`＋`22d9ea73`）：核心导入后钩子 + `pyawa-runtime::win32_asyncio` 设
  `asyncio.events._set_event_loop_policy`；Windows asyncio smoke 2/2 通过。
- **裁决（2026-11-21）**：**不采用代码进 `main`**（`ADR-002` 未接受该架构方向）。
  若未来需要“CPython 语义 facade”，应在宿主事件源 provider 上重新设计，而非在核心加导入后钩子。
