# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（R3：新签名 `StackUnderflow`）

**2026-10-06 第 506 轮已修** ✓：`STORE_NAME` 现在**走映射协议**（命名空间类型自带 `__setitem__` ⇒ 调它 ✓，
否则保持原来的 `DictObject` 直插 ✓）。最小验证 `target/recon/probe_setitem.py` 与参照**逐行一致** ✓
（`setitem __module__`／`__qualname__`／`__firstlineno__`／`__static_attributes__`／…／`A` ✓）；`quickcheck` OK ✓。

**旧墙已过、新墙出现** ✓：`import enum` 不再报 `TypeError: 'NoneType' object is not iterable` ✗，
改报 **`帧操作失败：StackUnderflow`** ✗ ⇒ 这正是 R3 的第一件活（"阻塞最多模块的报错签名"队列的队首 ✓）。

## 下一条命令

```bash
# 用最小脚本把 StackUnderflow 钉到具体语句（枚举模块里哪一步）
printf 'import enum\nprint("ok")\n' > target/recon/en5.py
rm -rf target/recon/__pyawa__; ./target/debug/pyawa target/recon/en5.py 2>&1 | tail -2
# 再按 §105 的四条规则插"打 stdout 的"探针（顶层块边界、跳过 @ 与装饰器目标行）定位到行
# 判据：该脚本打印 ok ⇒ 再试 Lib/ 整包（unittest ＋依赖 ＋ re）⇒ 两道必检 ⇒ SLICE ⇒ --sync ⇒ 报判据①
```

## 未修 bug（各带判据）

| # | 病灶 | 位置 | 判据 |
|---|---|---|---|
| 2 | `__prepare__` 返回值**多一份所有者**（rc=2，应为 1） | `call.rs` 的 `function` 分支（`:449` 起；建帧 `:464–492`；返回值处理在 **`:505` 之后**） | `rc` 探针（`classes.rs` 两处 `println!`）显示 `prepared` 交回时 **rc=1** ✓；`loop_class.py` 跑满 200 次 ✓；两开关不 panic ✓ |
| — | `__prepare__` 实参泄漏（**已修，待提交**） | `classes.rs:150–197` | 每类泄漏两个对象 ⇒ 曾使 K＝80 |

**推迟（不得用轮次磨）**：需要**新载荷类型**的活（`complex` 等 7 变体 × 2 时机已穷尽）、`open`／`_io` 真文件实现（按 `CM-8`／`P3-14` 走 `fs` 域）。

## 纪律提醒

- 红灯不提交；代码＋台账**同笔**（`commit-rule.md` §1）；**先易后难**、判据① 连续 5 轮不动即报停滞。
