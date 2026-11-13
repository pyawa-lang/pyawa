# `pyawa-lsp` —— Pyawa 的 LSP 服务（PLAN §10 梯②：诊断 ＋ hover）

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

## 如实边界（v1 ✗，后续梯次再补）

- **LSP 子集**：未实现 `workspace/*`、增量同步、文档符号、语义高亮等；协议版本按 LSP 3.17 的
  最小面自洽，不声明未实现的能力。
- **hover 只覆盖"已标注的名字"**：显示声明的类型文本（如 `x: int`）；不推断表达式类型、
  不做文档字符串渲染。
- **列口径近似**：诊断与 hover 的 `character` 按 `char` 计，**不是严格 UTF-16 code unit**；
  纯 ASCII 等价，非 BMP 字符（如 emoji）会偏（已在 `server.rs` 的 `publish` 文档里标注）。
- **`publishDiagnostics` 的 range 为 1 字符宽**（起止同行）；精确到 token 的 span 要等 parser
  统一带位点（梯②之后的补齐面）。
- **completion／definition／rename 只做"文档内符号"的浅层版本**：不做类型推断、成员补全、
  标准库符号、跨文件跳转；rename 是**单文件整词替换**（同名单文件内全部替换，无作用域分析）。
- 语法诊断在既有 parser **无位点标记**时 `line/col = 0`（与 `pyawa check` 同口径，如实记录）。

## 依赖隔离（`CX-27`／`T-CX-17`）

第三方依赖只**允许**出现在本 crate；当前实现**一个第三方也没用**。`pyawa-core`／`pyawa-abi`／
`pyawa-stdlib`／`pyawa-runtime`／`pyawa-capabilities` **不依赖**本 crate（`T-CX-17` 反向依赖检查 ✓）。
