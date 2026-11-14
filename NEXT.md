# NEXT（会话 13 · 解释器缺口收口完成）

> **本轮落地（2026-11-14）**：§12.4 收口时记的**五项既有缺口**全部修完 —
> **①** `asyncio.gather`（`pyawa-core` **类体私有名改写** `BC-66`／`compile/mangle.rs`；
> `__qualname__` 定点修复）· **②** 原生函数 `__module__` 来源模块（每实例旁表＋`install`
> 集中登记；`_csv.*` ⇒ `_csv`）· **③** `_io` `newline=` **读**口径五档（上游两 roundtrip 转 PASS）·
> **④** `inspect.getattr_static`（`getset_descriptor`／`member_descriptor` 真描述符面）·
> **⑤** `StopIteration` 回收 `StackUnderflow`（处理块异常表区间止于出口清理 `POP_EXCEPT`）。
> 三项提交：`7ee2990b`／`e9a8d06d`／`e6c4a9ac`（本页刷新随文档提交）。**ABI 面未动**。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`e6c4a9ac`（本轮代码最后一笔；文档提交见 `git log`）
- **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`
- **闸门**：`check.py` 19/19；`slowcheck --release` 十一项全绿（464 s）；408 条对拍绿；
  `cargo test -p pyawa-core`／`-p pyawa-runtime` 全绿；上游 `test_csv` 127 例：117 通过／0 失败／4 错误／6 跳过。
- **ABI 剩余**：**#1** `AB-52` 的 `type_expr` 求值（等 `TS` 注解解析器；现"原样保留"合规）·
  **#2** `pa_exec_file`（等 `IM-15`；现如实 `PA_ERR_NOTIMPLEMENTED` 合规）· **发布**＝用户决定
  （`DESIGN` §13-22；发布前确认钩子族）。`T-CX-19`（`CX-29`）过渡清单为空，棘轮在。
- **本轮新发现（先于本轮，未修 — 建议单开工单）**：`_sre_module.rs` 进程级 `static Mutex` 表跨
  `Instance` 串对象 ⇒ 同进程第二实例 `import re`／`json` 栈溢出（`posix_module.rs` 有同款已修先例）。
  判据：同一进程建两个 `PaState`、A `import json`、B 再 `import json` ⇒ 不崩且行为一致。
- **已知边界（如实；判据即"给定脚本行为与 3.14.4 一致"）**：
  - 嵌套类/函数内类的 `__qualname__` 仍用 `__name__` 拼（既有偏差）；`nonlocal` 无外层绑定的
    编译期 `SyntaxError` 仍报运行期 `UnboundLocalError`；`__static_attributes__` 显式预改写名歧义；
    `__slots__` 字符串槽名不改写；PEP 695 dunder 类型参数内部 cell 名沿用源码名。
  - `newline=` **写**翻译、`.newlines`、`StringIO(newline=…)` 仍未接；文本 `tell`/`seek` 非 cookie。
  - 类型对象自身的 `__name__` 等不在 `type.__dict__`；`inspect.getattr_static(用户类, "__name__")` 仍 AttributeError。
  - 更早的未修 bug：`iter_value` 丢异常（round 171 撤回）· traceback 帧保活 · dict 哈希化
    （`docs/rounds/04-post-m6-01.md`、`PLAN` §9.2）。
- **口径**：改编译器后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；
  编号现最大 `BC-66`／`AB-72`／`TS-50`／`CP-39`／`CX-29`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十一项全绿才提交。
