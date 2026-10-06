# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（**口径已改**：同步是结果，不是手段 ⇒ 按 C 模块 fan-in 取活）

**2026-10-07 用户修订** ✓：「整包批量同步」**不是手段** ✓（实测 **0 个可搬** ✓）；
判据① 的驱动量是 **C 模块缺口** ✓ ⇒ 按 `CM-14` 的 fan-in（`DESIGN.md` §9 曲线：前 5 个 ⇒ 67%、20 个 ⇒ 80% ✓）
取活 ⇒ 当前靶子＝把前 5 个（`sys`／`itertools`／`time`／`errno`／`builtins` ✓）的面补到能支撑 `Lib/` 导入 ✓；
继续按 `tools/next_work.py` 的**报错签名**队列取活 ✓。空 cell 支（xml.sax 族 6 个 ✓）**(b) 弃支** ✓、不烧轮次 ✓。

### 那 0 个整包的卡点性质（第 525 轮实测 ✓，供 §9.4 收敛）
**三类都有，但有明确主次** ✓：
```
① 先卡在"尚未同步的纯 Python 模块" ✗（不是 C 面本身）：
   unittest→traceback→re ／ asyncio→logging→re ／ json→re ／ pathlib→glob ／
   zipfile→importlib.util ／ zoneinfo→sysconfig ／ ensurepip→subprocess ／ dbm→struct
② 它们下面压着 **C 模块缺口** ✓（与 §9 的 fan-in 靶子同族 ✓）：
   `_sre`（re ⇒ ≈77 个模块 ✓）／`_struct`／`_string`／`_curses`／`_sqlite3`／`_multiprocessing`
③ **少数编译器/语义缺口** ✓：`tomllib` ⇒ 我们自己的 `SyntaxError: Some(Star)`（第 96 行 ✓）；
   `http`→`enum`（`StackUnderflow` ✓ 属弃支 ✓）；`multiprocessing`→`threading`→`functools`→`eval`/`_getframe` ✓；
   `ctypes` 直接 `-11` ✗
⇒ **没有一个是"只差同步"就能过的** ✗ ⇒ 与"同步是结果、不是手段"一致 ✓。
```

## 仪器口径（第 512 轮实测 ✓，必须记住）

`tools/lib_import_ratio.py` **同一棵干净树连跑两次**给出：**184／628**（29.3%）与 **183／628**（29.1%）✓
⇒ 该仪器**单次读数有 ±1 的噪声** ✗（子集里有个别模块本身不稳 ✓）。
⇒ 判据"连续 N 轮未动"要按**区间**读（183–184 ✗），不要把 ±1 当成涨跌 ✓；要判涨跌必须**多跑几次取众数** ✓。

## 未修 bug（各带判据）

| # | 病灶 | 位置 | 判据 |
|---|---|---|---|
| 2 | `__prepare__` 返回值**多一份所有者**（rc=2，应为 1） | `call.rs` 的 `function` 分支（`:449` 起；建帧 `:464–492`；返回值处理在 **`:505` 之后**） | `rc` 探针（`classes.rs` 两处 `println!`）显示 `prepared` 交回时 **rc=1** ✓；`loop_class.py` 跑满 200 次 ✓；两开关不 panic ✓ |
| — | `__prepare__` 实参泄漏（**已修，待提交**） | `classes.rs:150–197` | 每类泄漏两个对象 ⇒ 曾使 K＝80 |

**推迟（不得用轮次磨）**：需要**新载荷类型**的活（`complex` 等 7 变体 × 2 时机已穷尽）、`open`／`_io` 真文件实现（按 `CM-8`／`P3-14` 走 `fs` 域）。

## 纪律提醒

- 红灯不提交；代码＋台账**同笔**（`commit-rule.md` §1）；**先易后难**、判据① 连续 5 轮不动即报停滞。
