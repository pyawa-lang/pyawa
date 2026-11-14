# NEXT（会话 13 · TS 注解解析器 M1/M2）

> **本轮落地（2026-11-14，第五轮）**：全口径 **TS 注解解析器**的第一笔 —— **M1 规格与模型** ＋
> **M2 语法层**（`PLAN` §12.9）：
> - **M1**：`SPEC-type-system.md` 新增 **§16（`TS-51`…`TS-57`）**——语法范围（类型表达式子集，
>   任意表达式归 `__annotate__` 通道）／两阶段（语法层 vs 求值层）／`BC-24` 标签映射／失败语义
>   （语法错 `Syntax`、宿主侧 `PA_ERR_INVALID`）／可逆文本投影／`AB-52`↔`AB-69` **同一表示**；
>   `SPEC-c-abi` 的 `AB-52` 补解析器两阶段与 §16 引用；§13 新增 `T-TS-12/13/14`。
> - **M2**：`pyawa_core::type_expr::TypeExpr` 语法树（名字/点号/联合/下标/列表/前向引用/`...`）＋
>   `pyawa_core::compile::parse_type_expression(&str)`（复用 `compile/parser/types.rs` 的
>   `parse_type_at`，`?` 降级成联合、联合/单实参下标摊平）；`to_source`/`to_python_source`
>   可逆投影；新增 `crates/pyawa-core/tests/type_expression.rs`（语法矩阵/往返/错误）。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`e4fea98f`（本轮前；本轮提交见 `git log`）
- **工作区**：干净 ✓ · **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **下一步（M3 求值层）**：在 `TypeExpr` 上加求值入口——名称解析（模块 globals → builtins →
  宿主注册类型；点号按属性链；字符串前向引用**延迟**到求值）＋ `typing` 构造识别
  （`Callable`/`Optional`/`Union`/`Annotated`/`TypeVar`）→ 语义类型/`BC-24` 标签（`TS-52`/`TS-53`，
  `T-TS-13`）。依赖：`Lib/typing.py` 已在同步集（`import typing` 可用）；宿主类型经 `pa_newtype`
  注册。**M4** 再把 `AB-52` 字符串接进 `pa_register`/`pa_func_set_signature`（与 `AB-69` 同一表示、
  编译期调用点检查、`.pyi` 原串不变）。
- **已知边界（承接）**：M2 只做**语法层**（无实例/无求值）；`AB-52` 的 `type_expr` 求值仍"待"
  （M4 才接，`PLAN` §12.4 #1 状态不变）；`T?` 在语法层**降级成 `T | None`**（回写给规范形，
  不保留"用 `?` 写的"来源信息）；前向引用/点号名在 M3 才解析。
- **其他待办（承接上轮）**：`CX-30` 白名单 C 类 18 处转显式（低价值）· ABI `#2` `pa_exec_file`
  （等 IM-15）· ABI **发布**＝用户决定（`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）
  与性能主线（dict 哈希化）· 28 笔本地提交待定 push/合并（需授权）。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `TS-57`／`BC-67`／`AB-72`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
