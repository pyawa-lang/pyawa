# NEXT（会话 13 · TS 注解解析器收口）

> **项目落地（2026-11-14，M1…M6 全部收口；`PLAN` §12.9／§12.10）**：全口径「TS 注解解析器」——
> ①**规格**：`SPEC-type-system.md` §16 的 `TS-51`…`TS-57`（语法子集／两阶段／`BC-24` 映射／
> 失败语义／可逆投影／`AB-52`↔`AB-69` 同一性／验收）；`AB-52` 的"由 Pyawa 自己的注解解析器
> 求值"已成可执行义务；②**语法层**：`TypeExpr`＋`compile::parse_type_expression`（复用既有
> `parse_type_at`，`?` 降级/联合摊平，可逆投影）；③**求值层**：`SemanticType`＋名称解析
> （globals→builtins→类型注册表、点号链、前向引用 `Deferred`）＋`typing` 构造＋`BC-24` 静态
> 标签；④**ABI**：`AB-52` 注册时求值成 `AB-69` 的 `TyExpr`（同一账本 ⇒ 边界运行期＋编译期调用点
> 检查；`Any`/`Platform`；坏串 ⇒ `PA_ERR_INVALID`；`.pyi` 原串）；⑤**编译期**：标注覆盖率/档位
> 报告（`TS-23`/`24`/`25`/`39`）＋`pa_newtype` sig 校验；⑥**收口**：`§12.4 #1` 状态清零。
> 测试：`type_expression` 3 · `type_expression_resolve` 7 · `annotation_strings` 7 · `annotation_coverage` 4。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`313806af`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿（代码态 766 s 于 M5 提交前）；
  `cargo check --workspace --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2。
- **范围外／未做（诊断证据，见 `PLAN` §12.10）**：`BC-25`①（需全程序调用点信息；现保守发射）·
  `TS-38`/`TS-39` 的 `.pyac` 内嵌/独立导出（需 `IM-18`/`IM-19` 产物格式变更窗口）· `TS-34`
  诊断缓存（工具链步骤）。均不影响"注解解析器"本身。
- **其他待办（承接）**：`CX-30` 白名单 C 类 18 处转显式（低价值）· ABI `#2` `pa_exec_file`
  （等 IM-15）· ABI **发布**＝用户决定（`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）
  与性能主线（dict 哈希化）· 34+ 笔本地提交待定 push/合并（需授权）。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `TS-57`／`BC-67`／`AB-72`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
