# tools — 工具链

`DESIGN.md` §10：REPL、调试器、profiler、LSP／类型检查器。
四个工具**共享同一套核心**，模式只是参数——不存在两套工具链（`DESIGN.md` §2.1）。

| 工具 | 依赖 | 备注 |
|---|---|---|
| REPL | `code`／`_pyrepl` | `_pyrepl` 是 6,946 行纯 Python，可直接移植 |
| 调试器 | `sys.monitoring` | 3.12+ 事件模型；**VM 架构约束**（`BC-19`…`BC-21`） |
| profiler | `sys.setprofile`／`sys.monitoring` | 同上；`sys.settrace`／`setprofile` 允许排在 `sys.monitoring` 之后（`BC-22`） |
| LSP／类型检查器 | 类型系统 | 在"完全不推断"前提下**只覆盖标注代码**（`DESIGN.md` §6 结构性局限） |

⚠ 事件点**不是后加功能**：`BC-19` 要求主循环里预留，`BC-20` 要求可整体关闭且关闭时不改变语义。

## 状态

占位目录，M6 之后才有内容。
