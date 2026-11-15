# `pyawa-lsp` —— Pyawa 的 LSP 服务（PLAN §11 梯②：诊断 ＋ hover）

**零第三方依赖**（手写 JSON-RPC 2.0／`Content-Length` 帧）；只依赖 `pyawa-core` 的编译期 API。
**只编译不执行**：本服务不建 VM 实例、不跑字节码 ✓。

## 编辑器接入

编辑器按 LSP 的 **stdio** 传输启动本程序（构建产物：`target/<profile>/pyawa-lsp`）：

```sh
cargo build -p pyawa-lsp --bin pyawa-lsp
./target/debug/pyawa-lsp          # 编辑器会接管 stdin/stdout
```

（`stdout` 只放协议帧；诊断/日志走 stderr 或 LSP 通知 ✓。）

## v1 支持的协议子集

| 方向 | 方法／能力 | 说明 |
|---|---|---|
| 客户端 → 服务 | `initialize`／`initialized` | 能力：`textDocumentSync = {openClose: true, change: 1}`（**全量同步**）、`hoverProvider: true` |
| 客户端 → 服务 | `textDocument/didOpen`／`didChange`／`didClose` | 每次重新编译，发 `textDocument/publishDiagnostics`；`didClose` 清空诊断 |
| 客户端 → 服务 | `textDocument/hover` | 对**已标注的名字**返回 `name: 类型` 的 markdown；找不到 ⇒ `null` |
| 客户端 → 服务 | `textDocument/completion` | **文档内符号**补全（注解名／`def`／`class`／顶层赋值；kind 3／6）；v1 无上下文敏感、无成员／标准库补全 |
| 客户端 → 服务 | `textDocument/definition` | 光标下名字 ⇒ 首个定义处（注解／`def`／`class`／顶层赋值）`Location`；找不到 ⇒ `null` |
| 客户端 → 服务 | `textDocument/rename` | **单文件整词**重命名，返回 `WorkspaceEdit`（客户端应用）；不做跨文件／作用域分析 |
| 客户端 → 服务 | `shutdown`／`exit` | 正常退出 |
| 服务 → 客户端 | `textDocument/publishDiagnostics` | 字段 `range`（LSP 0 基 line/character）、`severity: 1`、`code`（`syntax`／`type`／`unsupported`）、`source: "pyawa"`、`message` |

诊断来源＝`pyawa_core::compile::check_source`（与 `pyawa check` **同一处真相**）：
`.py` **只语法类**（`TS-26`／`MS-19`）、`.pyawa` 语法＋`TS-47` 类型；**批量**（不遇首错即止）。
模式判定：URI 以 `.pyawa` 结尾或 `languageId == "pyawa"` ⇒ 扩模式，否则纯 Python 模式。

## 位置口径（已按 LSP 严格换算）

- **`character` 一律按 UTF-16 code unit**：入参（hover／definition／rename／completion 的位置）与
  出参（诊断 `range`、definition `Location`、rename `WorkspaceEdit`）都在 LSP 单位与内部 char 下标
  之间换算；非 BMP 字符（emoji 等）占 2 个 unit ✓（验收见 `tests/lsp_stdio.rs` 的
  `unicode_positions_use_utf16_code_units`）。
- 内部 `Diagnostic.line/col` 来自 lexer 的 **1 基 char 下标**（`pyawa check` 同源），LSP 层换算成
  0 基 UTF-16；`publish` 的 range 宽度＝该字符的 UTF-16 宽度（通常 1）。

## 范围外（明确不做，理由在行内 ✗——不是"待办"）

- **语法诊断无位点时 `line/col = 0`**：既有 parser 只有**部分**错误消息带「第 L 行，列 C」；
  统一给 151 处 parser 错误点补 span 的成本/风险与 LSP 需求不成比例（`pyawa check` 同口径）。
- **hover 只覆盖已标注的名字**：类型推断违反 `TS-2`「完全不推断」的既有决策；不做 docstring 渲染。
- **completion／definition／rename 只支持当前文档**：跨文件需 workspace／import 图与会话状态，
  PLAN §11 的第③梯原文即"**按需**"；成员补全／标准库符号同理范围外。
- **增量同步／`workspace/*`／文档符号／语义高亮**：LSP MVP 只需全量同步；这些是后续独立需求。
- **rename 作用域**：单文件整词替换（无作用域分析）；跨文件改名需先有 workspace 语义。

## 依赖隔离（`CX-27`／`T-CX-17`）

第三方依赖只**允许**出现在本 crate；当前实现**一个第三方也没用**。`pyawa-core`／`pyawa-abi`／
`pyawa-stdlib`／`pyawa-runtime`／`pyawa-capabilities` **不依赖**本 crate（`T-CX-17` 反向依赖检查 ✓）。
