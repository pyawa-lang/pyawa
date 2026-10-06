# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（**P3-26 的真实前置：`re`**）

**口径已定（2026-10-07 用户裁决 ✓）**：空 cell 那支（xml.sax 族 6 个 ✓）按 §4 **弃支** ✓，不烧轮次；
当轮改取 **P3-26 成批同步** ✓（§9.4：成批同步 ＞ 单簇语义修复 ✓）；其后 P3-27（`Lib/test` 语料化 ✓）。

**本轮把 P3-26 做成了实测探底** ✓（六个整包逐个搬进来做**两道必检** ✓：逐个导入 ＋ 无关脚本 `startup ok` ✓）：
```
asyncio(35)        ✗ 缺 logging ⇒ logging 缺 re ✗
multiprocessing(23)✗ 缺 threading ⇒ threading 缺 functools ⇒ eval／_getframe ✗
unittest(13)       ✗ 缺 traceback ⇒ traceback 缺 re ✗（补 textwrap ✓ 后仍缺 re ✗）
json(6)            ✗ 缺 re ✗
http(5)            ✗ 缺 enum ✗（enum 仍 `StackUnderflow` ⇒ 属**已弃支** ✓）
logging(3)         ✗ 缺 re ✗
⇒ 六个整包**无一可搬** ✗；`linecache.py` 单独 ✓ 通过（已留在 `Lib/` ✓，随下一笔入账 ✓）
```
**⇒ 关键事实（改变 P3-26 的可行性判断）** ✓：这些整包的共同基座是 **`re`** ✗
（`re` 自身缺 **`_sre`** ✗）——`textwrap`／`traceback`／`unittest`／`json`／`logging`／`asyncio`
以及 `email` 那 20 个，**全部**压在它上面 ✓（合计 **≈77 个模块** ⇒ 若解得，判据① **+12 点量级** ✓）。

## 下一条命令（二选一，请裁决）

```bash
# A（最高杠杆，工程量最大）：实现 `_sre` 的最小可用面（Rust 正则引擎 ⇒ `re` 能 import ✓ 且能用 ✓）
#    判据：`import re` ✓ ⇒ `re.match/search/sub/split/findall` 与参照逐例一致 ✓
#    ⇒ 再按 P3-26 整包搬：textwrap／traceback／unittest／json／logging／asyncio ⇒ 两道必检 ⇒ SLICE ⇒ --sync ⇒ 报判据①
# B（工程小、收益中）：先把**不依赖 `re`** 的整包/单文件按 P3-26 扫出来搬（如 `linecache` ✓ 这类）✓
#    —— 先用 `tools/find_syncable.py` 的整包改造版量出"现在就能搬的清单" ✓，再决定要不要投 `_sre` ✓
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
