# 决策记录（ADR）

规则：状态 `proposed` → `accepted` / `rejected` / `superseded`；规范文档只引用已接受 ADR。
2026-11-21 的迁移由用户授权「全部可以进行」执行。

## ADR-001 定位与语义（accepted）

- **定位＝Lua 式可嵌入内核**：核心只提供 VM／对象模型／协程原语／宿主能力接口；
  I/O、事件循环、平台差异属于宿主 provider。
- **语义＝CPython 3.14 可观察行为**：`Lib/` 逐字节一致；但平台能力无法支撑的面如实缺失，
  不伪造。
- 推论：内核不得为某个标准库的实现架构（包括 asyncio）反向定制。

## ADR-002 asyncio 是可选的 CPython 语义 facade（accepted）

- `Lib/asyncio` 作为**语义 facade**：在宿主提供 fs／net／clock／事件源能力时投影 CPython 可观察语义；
- **不**允许 asyncio 的实现架构（selector/proactor、self-pipe、导入后钩子、JSPI/Asyncify）
  成为内核或平台承诺的驱动；
- wasm/移动端的 async 支持按**宿主能力**声明；不承诺“全平台 asyncio 同语义”；
- 相关探索（JSPI、asyncio-on-wasm、Windows Proactor/selector 后端）只作
  `docs/rounds/spikes/` 报告，代码不采用。

## ADR-003 alpha 迁移与历史（accepted）

- alpha 前的 dev 已在 `main` 的 m1–m6/alpha 合并父链中；显式保留 `archive/pre-alpha-dev`
  （`cea7e306`）。
- alpha 后的 dev（88 提交）**逐组选择性采用**进 `main`：portability、frame-stack、
  windows-platform、win32-real-semantics、wasm-embedding（E1–E8）已采用；
  asyncio-windows／JSPI／asyncio-on-wasm 仅报告、不采用代码。
- **不建** `archive/alpha-dev`；`dev` 在台账全部裁决后删除（迁移前已导出
  `target/archive/dev-full.bundle` 安全网）；不重写 `main` 历史与既有 tag。
- 采用提交均带 `Ported-from:`／`(cherry picked from …)` 可追溯。

## ADR-004 工作流（accepted）

- 采用 `main` + `feat/*` + `spike/*` 的最小模型；PR 门禁 + squash-only + tag 发布；
- 以后按发布需要增量引入 `develop`／`release/*`／`hotfix/*`（见 `docs/WORKFLOW.md`）；
- 不引入完整 Git Flow 的仪式，但保留其发布纪律的扩展点。

## ADR-005 宿主事件循环与协程驱动（accepted，2026-11-21）

- **用户拍板：长期架构目标＝ L2“宿主拥有事件循环”**（Lua 式）；L1 `event` 能力域是其底座；
  L3（宿主 loop 上完整 asyncio）**不承诺**。
- 双模式并存：
  - 模式 A（宿主驱动）：宿主循环 + `event` provider + `pa_resume` 族（additive ABI），
    Pyawa 提供 VM/协程；普通 `pa_exec_*` 阻塞语义不变；
  - 模式 B（Pyawa 循环）：现有 `Lib/asyncio` 作为可选 CPython 语义 facade，Linux 桌面可用。
- 详情、状态机、风险与分阶段见 `docs/DESIGN-event-source.md`；实现跟踪见 `PLAN §12.27`。
- **accepted 只表示目标架构**：S1 规格/S2 Spike C 通过后才写产品 ABI；不得跳过 spike 直接实现。
