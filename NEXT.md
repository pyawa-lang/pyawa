# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（R3：`sys._getframe` 的 `f_back` 链 ⇒ 再放 `threading` 一族）

**第 512 轮（本轮）** ✓：
- 队列第 3 档做实了：`base64.py`／`logging/`／`threading.py` **都不在 `Lib/`** ✓（上游都有 ✓）⇒ 补进来后逐层推进；
- `threading` 的闭包：`_threading_local` → `contextlib` → `functools` → ❌ **`NameError: name 'eval' is not defined`** ✗；
- 取回被撤回的 `80977af`（eval/exec ✓）⇒ `eval("1 + 2")`＝**3** ✓、`eval("max(3, 4)")`＝**4** ✓ ⇒ 墙前移到
  **`sys._getframe 目前只接 depth＝0（f_back 链随后补）`** ✗（`functools`／`contextlib`／`threading`／`_threading_local` 都卡这里 ✓）；
- **但取回的那份 eval/exec 会让 `meta_path_shapes` 红** ✗：在场 **2/5 红** ⇒ 只留三参 `type` 时 **0/5 红** ✓
  ⇒ 结论：**那份旧实现不能原样入账** ✗，需要重做（或先定位它为何扰动元路径那一格 ✓）。
- **本轮入账的是"三参 `type(name, bases, ns)`"** ✓（`type_call` 原先只有"随后补"✗ ⇒ 现交给 `type_new_native` ✓，
  `type("X", (), {"a": 1}).a`＝**1** ✓）——它是上面那条链的第 2 道墙 ✓。

## 下一条命令

```bash
# ① 实现 `sys._getframe(depth)` 的 f_back 链（`Frame` 需要父帧指针 ＋ 进出栈时维护 ✓）
grep -rn "f_back\|_getframe" crates/pyawa-core/src/ | head
# ② 让 eval/exec 能入账：先查它为何扰动 meta_path_shapes（在场 2/5 红 ⇒ 间歇性 ✓）
git show 80977af -- crates/pyawa-core/src/compile.rs | head -60      # 新编译入口是否有布局/全局副作用
# 判据：`functools`／`contextlib`／`threading`／`_threading_local` 四个都导入 ✓ ⇒ 两道必检 ⇒ SLICE ⇒ --sync ⇒ 报判据①
```

## 未修 bug（各带判据）

| # | 病灶 | 位置 | 判据 |
|---|---|---|---|
| 2 | `__prepare__` 返回值**多一份所有者**（rc=2，应为 1） | `call.rs` 的 `function` 分支（`:449` 起；建帧 `:464–492`；返回值处理在 **`:505` 之后**） | `rc` 探针（`classes.rs` 两处 `println!`）显示 `prepared` 交回时 **rc=1** ✓；`loop_class.py` 跑满 200 次 ✓；两开关不 panic ✓ |
| — | `__prepare__` 实参泄漏（**已修，待提交**） | `classes.rs:150–197` | 每类泄漏两个对象 ⇒ 曾使 K＝80 |

**推迟（不得用轮次磨）**：需要**新载荷类型**的活（`complex` 等 7 变体 × 2 时机已穷尽）、`open`／`_io` 真文件实现（按 `CM-8`／`P3-14` 走 `fs` 域）。

## 纪律提醒

- 红灯不提交；代码＋台账**同笔**（`commit-rule.md` §1）；**先易后难**、判据① 连续 5 轮不动即报停滞。
