# 变更日志

## v0.1.0-alpha.1（首个 alpha；ABI 未冻结）

### 嵌入 ABI（1.14；alpha 期未冻结，后续 alpha 仍可能破坏）
- 宿主对象：属性/方法派发（`AB-73`）、统一对象钩子（`AB-75`）、真实类型身份（`AB-76`）、基类（`AB-77`）。
- 协议唯一入口：**75** 条协议清单（以 `pyawa_core::PROTOCOL_CASES` 为准）＋ 记录式 oracle（`T-AB-19`）—— 宿主对象 `print()` 与原生一致。
- 异常：宿主持久异常映射（`AB-74`）；模块对象可上栈（`AB-79`）；字节一次性写回（`AB-80`）。
- 能力域 vtable 的 C 布局入 `pa_capabilities.h`（`fs`／`clock`／`net` 三域）。
- 电池版组合根：`paL_newstate`／`paL_openlibs`（含 stdlib/能力域）。

### 语言
- `x = yield`（`SEND` 取值形态）已修；`_signal` 记账式最小面（解锁 `asyncio.run`／`await`／`async for`）；
  `_csv` 实现并同步 `Lib/csv.py`。

### 质量
- 新 ABI 路径 valgrind 0 错误；参考宿主七项演示；闸门 20/20；slowcheck 十二项。
