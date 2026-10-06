# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（**①RefCell 重入：建议按 §4 同样处置 —— 弃支或另开专项**）

**到第 541 轮为止，① 已耗 7 轮**（532 发现 → 533 定性内存类 → 534 复刻环境 → 535／536 拿到两处 panic ＋ 部分链
→ 537 子进程回溯拿不到 → 538 抽取器修正 → 539 点名失败用例 → 540 钉到子名 `nested_list` ＋ 行 `2852` ＋ 调用者名单 ✓）。
**当前状态** ✓：
```
失败子名：nested_list（脚本 `import sys; sys.meta_path = [[1, 2]]; …; import os` ✓）
panic  ：RefCell already borrowed @ builtin_objects.rs:2852（AttributeObject::set_attributes 的 borrow_mut ✗）
另一处 ：RefCell already mutably borrowed @ :3336（DictObject::entries ✗），部分链
         dict_get ← super_lookup ← attribute_lookup ← call_object_method ✓
调用者 ：protocol.rs:139（mounted_instance_dict 惰性挂载 ✓）／:256（obj.__dict__ = … ✓）／builtin_objects.rs:2869（清理 ✓）
```
⇒ 性质**不是**"某个函数少释放"✗，而是**"持有 RefCell 借用期间又调进 Python"**✗ —— 属**跨模块重入**类，
每推进一步都要新探针 ✓，且修法要动"读路径不许在借用期间回调"✗（牵动 `attribute_lookup`／`super_lookup`／
`dict_get` 三处 ✓）。

**⇒ 我按 §4 不再自作主张烧轮次** ✓，把 ① 与 ③（闭包链）一样**交你拍板**：**弃支**（记档、不烧轮次 ✓）
／**另开专项**（给足预算与更强手段，如 `valgrind`／`-Zsanitizer=address` 对照 ✓）。

**若你选择继续**，下一轮的最小一步已经写死 ✓：
```bash
# 在测试 `run()` 里打印"实际命令行 ＋ PYAWA_* env ＋ CWD"（临时 3 行 ✓，跑完即撤 ✓）
# ⇒ 逐字照抄手工跑（必然复现 ✓）⇒ RUST_BACKTRACE=full ⇒ 拿"持有 borrow 的那个函数" ✓
# ⇒ 把它的"读路径"改成"先出快照 ⇒ 结束借用 ⇒ 再回调" ✓
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
