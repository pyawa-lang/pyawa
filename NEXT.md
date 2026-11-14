# NEXT（会话 13 · TS 注解解析器 M4）

> **本轮落地（2026-11-14，第七轮）**：TS 注解解析器 **M4 ABI 接入**（`PLAN` §12.9）——
> `pa_register`/`pa_module_addfunction` 注册时用 `evaluate_type_expression` 把 `AB-52` 的
> `type_expr` 求值成 `AB-69` 的 `TyExpr`（新增 `Any`/`Platform`），登记进同一
> `host_typed_signatures` ⇒ **边界运行期检查**＋**编译期宿主调用点检查**对字符串通道生效；
> 句柄签名优先不覆盖；坏字符串/未知名 ⇒ `PA_ERR_INVALID`，显式前向引用 ⇒ 平台类型；`.pyi`
> 仍原串。`crates/pyawa-abi/tests/annotation_strings.rs` 6 passed（含两通道同行为/`.pyi`）。
> 前两轮：M3 求值层（名称解析/typing 构造/`SemanticType`/标签投影，7 passed）；M1/M2 规格与
> 语法层（`TS-51`…`TS-57`、`TypeExpr`、`parse_type_expression`）。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`b202def0`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **下一步（M5 编译期注解面）**：①`BC-25`① 的"只在标注/未标注交界处发射"（跨模块静态信息／
  调用点标注覆盖；现按"带标注保守发射"）②`TS-47` 注解面接线（形参/返回/变量注解在两种模式下的
  静态检查与运行期 `CHECK_ASSIGNMENT` 口径）③`TS-32`…`TS-39` 覆盖率/安全档位报告按规格收口；
  `pa_newtype` 的 `sig` 字符串仍只作 `.pyi` 文本（M5 接线面）；边界如实登记。
- **已知边界（M4）**：`AB-52` 求值在**注册时**；显式前向引用未解析 ⇒ 平台类型（`TS-54` 唯一例外，
  不检查）；同一 C 函数已有 `AB-69` 句柄签名时不覆盖；`PLAN` §12.4 #1 的"待"留到 M6 收口。
  `typing` 构造按解析路径名识别（不要求 import）；`Annotated` 元数据不参与；`Set`/`Tuple`/
  宿主任意泛型走 `Apply`（擦除）；`T?` 降级成 `T | None`（规范形回写）。
- **其他待办（承接）**：`CX-30` 白名单 C 类 18 处转显式（低价值）· ABI `#2` `pa_exec_file`
  （等 IM-15）· ABI **发布**＝用户决定（`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）
  与性能主线（dict 哈希化）· 31+ 笔本地提交待定 push/合并（需授权）。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `TS-57`／`BC-67`／`AB-72`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
