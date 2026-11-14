# NEXT（会话 13 · TS 注解解析器 M5）

> **本轮落地（2026-11-14，第八轮）**：TS 注解解析器 **M5 编译期扩展（部分）**（`PLAN` §12.9）——
> ①**标注覆盖率与严格档位报告**（`TS-23`/`TS-24`/`TS-25`/`TS-39`）：`compile::annotation_coverage`
> ＋`AnnotationCoverage::to_report()`，按分列口径统计位置参数/仅关键字/`*args`/`**kwargs`/返回/
> 变量（`x: T = v` 只算一格）＋档位＋模式；`tests/annotation_coverage.rs` 4 passed。
> ②`pa_newtype` 的 `sig` 字符串注册时**校验**（坏表达式 ⇒ `PA_ERR_INVALID`；`annotation_strings.rs`
> 7 passed）。**边界如实登记（未做＋理由）**：③`BC-25`① 需**全程序信息**（`BC-23` 在被调用方
> 序言发射，"两侧已标注 ⇒ 禁止发射"要知道全部调用点/跨模块；单编译单元不可判定）④`TS-37`
> 批量诊断（`typecheck` 现首错即止，重构诊断收集属另一窗口）⑤`TS-38` 产物内嵌／`TS-39` 独立
> 导出（需改 `IM-18`/`IM-19` 产物要素与黄金字节；运行期仍不读 fs）。
> 前几轮：M4 ABI 接入（`AB-52` 求值/两通道同表示，6 passed）· M3 求值层 · M1/M2 规格与语法层。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`755fbd49`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **下一步（M6 收口）**：①更新 `PLAN` §12.4 #1 的状态（`type_expr` 求值**已落地**：M4 注册时求值
  入 `AB-69` 同一账本）②`AB-52`/`AB-69`/`§12.4` 的交叉引用与 `NEXT`/`PLAN` 同步（"待"清零）③复核
  M1…M5 的边界清单（`BC-25`①/`TS-37`/`TS-38`/`TS-39`）仍如实登记在 `PLAN` §12.9 与 `TS` §10/§16。
- **已知边界（M5）**：覆盖率报告由**编译入口**给出（运行期不读伴随文件）；产物内嵌/独立导出与
  `BC-25`① 的全程序分析、`TS-37` 批量化是**未做项**（理由已登记），不是"默认关闭"；`pa_newtype`
  的字符串只校验、不参与边界检查（构造调用面）。
- **其他待办（承接）**：`CX-30` 白名单 C 类 18 处转显式（低价值）· ABI `#2` `pa_exec_file`
  （等 IM-15）· ABI **发布**＝用户决定（`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）
  与性能主线（dict 哈希化）· 32+ 笔本地提交待定 push/合并（需授权）。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `TS-57`／`BC-67`／`AB-72`／`CP-39`／`CX-30`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
