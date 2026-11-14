# NEXT（会话 13 · TS 注解解析器 M3）

> **本轮落地（2026-11-14，第六轮）**：TS 注解解析器 **M3 求值层**（`PLAN` §12.9）——
> 名称解析（globals→builtins→类型注册表、点号属性链、前向引用 `Deferred`）＋ `typing` 构造
> （`Optional`/`Union`/`Callable`/`Annotated`/`List`/`Dict`/`Set`/`Tuple`）→ `SemanticType`
> （与 `AB-69` 的 `TyExpr` 同形）＋ `to_static_label()`（同 `static_label` 口径）；
> `crates/pyawa-core/tests/type_expression_resolve.rs` 7 passed。上一轮（M1/M2，语法层）：
> - **M1**：`SPEC-type-system.md` 新增 **§16（`TS-51`…`TS-57`）**——语法范围（类型表达式子集，
>   任意表达式归 `__annotate__` 通道）／两阶段（语法层 vs 求值层）／`BC-24` 标签映射／失败语义
>   （语法错 `Syntax`、宿主侧 `PA_ERR_INVALID`）／可逆文本投影／`AB-52`↔`AB-69` **同一表示**；
>   `SPEC-c-abi` 的 `AB-52` 补解析器两阶段与 §16 引用；§13 新增 `T-TS-12/13/14`。
> - **M2**：`pyawa_core::type_expr::TypeExpr` 语法树（名字/点号/联合/下标/列表/前向引用/`...`）＋
>   `pyawa_core::compile::parse_type_expression(&str)`（复用 `compile/parser/types.rs` 的
>   `parse_type_at`，`?` 降级成联合、联合/单实参下标摊平）；`to_source`/`to_python_source`
>   可逆投影；新增 `crates/pyawa-core/tests/type_expression.rs`（语法矩阵/往返/错误）。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`083aea96`（本轮前；本轮提交见 `git log`）
- **工作区**：干净 ✓ · **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **下一步（M4 ABI 接入）**：把 `AB-52` 的 `type_expr` 在 `pa_register`/`pa_func_set_signature`
  处用 `evaluate_type_expression` 求值成与 `AB-69` **同一表示**（`SemanticType` 与 `TyExpr`
  对齐/归一），接入**编译期宿主调用点检查**（`HostFunctionSignature` 的静态标签来自字符串通道），
  失败按 `TS-54` 报 `PA_ERR_INVALID`，`.pyi` 仍用原字符串；验收 `T-TS-14`（字符串/句柄两通道
  等价 + `.pyi` 不变）。
- **已知边界（M3 新增）**：`typing` 构造按**解析路径名**识别（不要求先 `import typing`）；
  `Annotated` 元数据不参与类型判定；`Set`/`Tuple`/宿主任意泛型走 `Apply`（`TS-35` 擦除）；
  前向引用解析不到时给 `Deferred`（M4 在检查/调用时机重试）；`AB-52` 的 ABI 接入仍待 M4
  （`PLAN` §12.4 #1 状态不变）。`T?` 降级成 `T | None`（规范形回写，不保留来源信息）。
- **其他待办（承接上轮）**：`CX-30` 白名单 C 类 18 处转显式（低价值）· ABI `#2` `pa_exec_file`
  （等 IM-15）· ABI **发布**＝用户决定（`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）
  与性能主线（dict 哈希化）· 28 笔本地提交待定 push/合并（需授权）。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `TS-57`／`BC-67`／`AB-72`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
