# alpha 迁移后的主线状态（2026-11-21）

## 已采用进 main（选择性采用）

| 组 | 内容 | main 提交 |
|---|---|---|
| portability | 同脚本跨平台同结果（24 语料＋CI 矩阵/compare） | `7d1b0f0e` |
| frame-stack | §12.22 显式帧栈 S1–S4（深递归／wasm E6） | `09800d64` |
| windows-platform | W1–W6 平台面＋原生 CLI＋参考宿主；平台面补齐（clock/fs/net/dev·ino） | `4b034ffa`＋`09b5c10a` |
| win32-real-semantics | §12.24 winreg／`_winapi`／`_overlapped` 最小真实语义 | `0cbb1ed0` |
| wasm-embedding | E1–E6 平台门控/ABI/clock；P1 库嵌入 E7/E8＋子集同结果 | `4f9490cc`＋`a8d6ad8a` |

## 仅报告、不采用代码

- JSPI 栈切换（`docs/rounds/spikes/2026-11-20-jspi.md`）
- asyncio-on-wasm 纯协程（`docs/rounds/spikes/2026-11-20-asyncio-wasm.md`）
- Windows asyncio 默认后端（`docs/rounds/spikes/2026-11-21-asyncio-windows.md`）
- 依据：`docs/DECISIONS.md` ADR-002。

## 历史

- `main` 的 m1–m6／alpha 历史与 tag **未改写**；
- `archive/pre-alpha-dev` = `cea7e306`（alpha 前 dev 尖端）；
- `dev` 已按 ADR-003 删除（迁移前 bundle：`target/archive/dev-full.bundle`，非仓库产物）。

## 文档同步待办（不在本次迁移范围）

主线的 `PLAN`／`DESIGN`／`REQUIREMENTS`／`RELEASE` 仍是 alpha 版本；把已采用组的规格/状态
回填、并按 ADR-001/002 重写 asyncio 相关表述，是下一轮“文档同步”工作（需单独授权）。
