# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（**前 5 面的真缺口：`dir`**）

**第 529 轮实测** ✓（按新口径"驱动量＝C 模块面 fan-in，前 5＝`sys`／`itertools`／`time`／`errno`／`builtins`" ✓）：
```
import errno／itertools／time／sys／builtins  ⇒ **五个都能导入** ✓（前四个由 VM 提供 ✓，Lib/ 里无对应 .py ✓）
但 `dir` **未定义** ✗ —— NameError: name 'dir' is not defined（`builtins` 面的真缺口 ✓）
```
**注册点已定位** ✓：`crates/pyawa-stdlib/src/builtins_module.rs`
`:20` 的**已实现名字清单**（`"globals"` 在里面 ✓、`"dir"` 不在 ✗）＋ `:58` 的 `(名字, fn as NativeFn)` 注册表 ✓
＋ `:1054 globals_native` 是**同形状样例** ✓（取当前帧 ✓）。

**为什么本轮不落** ✗（如实 ✓）：`dir` 要**两种形态**都与参照一致 ✓——
`dir()`（当前作用域名字，**按参照的排序/去重** ✓）与 `dir(obj)`（**类型 MRO 各命名空间 ＋ 实例字典 ＋ 槽位名**，再去重排序 ✓）；
后者要对齐"合并与排序规则"✗，不是三行能收口的 ✓ ⇒ 硬塞一版会有**新差异**风险 ✗（对拍会红 ✓）。
⇒ 本会话剩余预算不足 ⇒ 按 §4 **留档、不落半成品** ✓；下一条命令就是把它按上面两点实现完 ✓：

```bash
# 1) builtins_module.rs:20 名单加 "dir" ✓；:58 注册 ("dir", dir_native as NativeFn) ✓
# 2) dir_native：0 参 ⇒ 当前帧 `code().varnames`（＋类体/模块的 namespace 键 ✓）去重排序，返回 list[str] ✓
#    1 参 ⇒ 类型对象的命名空间键 ＋ 各 MRO 基类命名空间键 ＋ 实例字典键（`instance_attributes` ✓），去重排序 ✓
# 3) 判据：dir()／dir(1)／dir([])／dir(类实例) 与 `python3` 逐例一致（**排序与去重**都要对 ✓）
#    ⇒ tools/quickcheck.sh ✓ ⇒ tools/slowcheck.sh（十项 ✓）⇒ 提交（代码＋台账＋NEXT.md 同笔 ✓）
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
