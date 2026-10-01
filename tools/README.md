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

## 数据生成器（不是上表那四个工具）

| 脚本 | 作用 |
|---|---|
| `gen_opcode_tables.py` | 向本机 CPython 运行时探测指令表，拟合并校验后生成 **`crates/pyawa-core/src/opcode_metadata.rs`**（归属 `pyawa-core`，见 `BC-38`）；**数值不落进脚本** |
| `gen_opcode_fixture.py` | 导出 **`crates/pyawa-core/tests/fixture-opcode-3.14.json`**——对拍用的**期望值**，不是实现 |
| `gen_jump_fixture.py` | 编译若干小片段，导出 **`crates/pyawa-core/tests/fixture-jump-3.14.json`**（`co_code` 的 hex ＋ 每条跳转的 `dis` 偏移／`argval`）——供 `T-BC-17` 用**参照实现产出的字节**验证 `BC-55` 的跳转算术 |

两者都要求本机能 `import _opcode`／`_opcode_metadata`（基线 CPython 3.14）；参照实现升补丁版本时重生成。

## 状态

**部分就位**：数据生成器已入库，**产物路径已随依赖边裁决落在 `pyawa-core`**（`BC-38`）；
`pyawa-stdlib` 只做转发。
`DESIGN.md` §10 的四个工具（REPL／调试器／profiler／LSP）仍是占位，M6 之后才有内容。
