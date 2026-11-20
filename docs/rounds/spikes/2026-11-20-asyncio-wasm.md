# Spike：asyncio-on-wasm（无真实 I/O）可行性（2026-11-20）

- **问题**：同一份 `Lib/asyncio` 能否在 wasm 宿主上跑通 `asyncio.run(gather/sleep)`？
- **实验**（dev `55803542`＋`66b93438`＋`fe23ed78`）：wasm 只读 fs（宿主文件表）＋
  `pa_setpath(["/Lib"])` ⇒ `import asyncio`；内存 net `socketpair`／poll 供 self-pipe；
  `posix.getpid` 虚拟 pid=1；JSPI 入口；E9 跑与 native 同一份 `asyncio_basic.py` 输出一致
  （run `44738394468` wasm job E9 PASS）。
- **结论**：纯协程面可行；真实网络/文件 I/O、浏览器兼容矩阵仍未解决。
- **裁决（2026-11-21）**：**仅报告，不采用代码进 `main`**。asyncio 是可选 CPython 语义
  facade（`ADR-002`），其平台支持由宿主能力声明；不得反向定义内核架构。
