# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（R1：修 bug 2）

```bash
sed -n '505,570p' crates/pyawa-core/src/executor/call.rs      # function 分支的返回值处理
tools/quickcheck.sh target/recon/loop_class.py                # 快档：期望「200 次跑满、退出码 0」
PYAWA_QUARANTINE=1 ./target/debug/pyawa target/recon/loop_class.py   # 期望**不 panic**
PYAWA_DANGLING=1   ./target/debug/pyawa target/recon/loop_class.py   # 期望**不 panic**
```

## 未修 bug（各带判据）

| # | 病灶 | 位置 | 判据 |
|---|---|---|---|
| 2 | `__prepare__` 返回值**多一份所有者**（rc=2，应为 1） | `call.rs` 的 `function` 分支（`:449` 起；建帧 `:464–492`；返回值处理在 **`:505` 之后**） | `rc` 探针（`classes.rs` 两处 `println!`）显示 `prepared` 交回时 **rc=1** ✓；`loop_class.py` 跑满 200 次 ✓；两开关不 panic ✓ |
| — | `__prepare__` 实参泄漏（**已修，待提交**） | `classes.rs:150–197` | 每类泄漏两个对象 ⇒ 曾使 K＝80 |

**推迟（不得用轮次磨）**：需要**新载荷类型**的活（`complex` 等 7 变体 × 2 时机已穷尽）、`open`／`_io` 真文件实现（按 `CM-8`／`P3-14` 走 `fs` 域）。

## 纪律提醒

- 红灯不提交；代码＋台账**同笔**（`commit-rule.md` §1）；**先易后难**、判据① 连续 5 轮不动即报停滞。
