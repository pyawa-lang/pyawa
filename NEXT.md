# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

**第 547 轮补充：完整链已从落盘日志读全** ✓（`target/recon/canary_empty.log` ✓，`RUST_BACKTRACE=full` ✓）：
```
<DictObject>::entries            builtin_objects.rs:3336   ← panic（读到"已被可变借用"的 dict ✗）
<Instance>::dict_get             instance/containers.rs:242
executor::super_lookup           executor.rs:212
attribute::attribute_lookup      attribute.rs:85
call::call_object_method         call.rs:87
builtin_objects::python_level_finalize  builtin_objects.rs:1831   ← 终结器
<Instance>::release_one          instance.rs:1481             ← 调终结器的那一行
<Instance>::release_object
executor::format::release
executor::execute::{closure#1}   ★ 释放发生在 execute 的指令处理里
executor::execute
call::call_callable → call::call_value → classes::build_class_native   ★★ 落在**类创建**路径
call::call_callable → executor::execute → …
```
⇒ 定位 ✓：**类创建路径**（`classes::build_class_native` ✓）里某个 dict 仍被 `borrow_mut()` 持有时，
其条目的 refcount 掉到 0 ⇒ `release_one` 跑终结器 ⇒ 终结器回调 Python（属性查找→`super()`→`dict_get`）✗
⇒ 重入借用 panic ✓。下一轮只要在 `classes.rs` 里找"**持有 `borrow_mut()` 期间触发释放**"的那一段
（典型：边 `entries.borrow_mut()` 边替换/移除条目并把旧值 `release` 掉 ✗）⇒ 把 `release` 挪到借用作用域之外 ✓。

**第 548 轮补充（三处"最基本"的 dict 变更原语都是干净的 ✗）** ✓：
```
DictObject::insert_raw    :3263  self.entries.borrow_mut().push(…)      ⇒ 借用随语句结束 ✓（无 release ✓）
DictObject::replace_value :3353  返回旧值 ⇒ 借用随语句结束 ✓（调用方**借用外**释放 ✓）
DictObject::remove        :3365  具名 borrow_mut ⇒ 但函数内无 release ✓（返回元组，调用方释放 ✓）
在 builtin_objects.rs／executor/protocol.rs／classes.rs 里，**"具名 borrow_mut ＋ 14 行内 release"⇒ 0 处** ✗
```
⇒ 持有者**不是**"在这些原语里顺手 release"这种形态 ✗，而更可能是：
**某处把某个 dict 的 `borrow_mut()` 拿着不放（跨过一次 Python 回调）**✗，且那个 dict 正是后来 `dict_get` 要读的同一个 ✓。

## 下一条命令（**直接问"谁持有"**，一次到位）

```bash
# 给 DictObject 的**可变借用点**加"记录持有人"：拿 `borrow_mut()` 时存一份 backtrace/现场（`PYAWA_BORROW_DEBUG=1` ✓）
#   ⇒ panic 发生时（`entries()` 里）把"上一次取可变借用的现场"打出来 ✓ ← 这才是我们缺的最后一条信息 ✓
# 具体：在 `builtin_objects.rs` 里给 `DictObject` 加 `holder: Cell<Option<&'static str>>`（或存 `std::backtrace::Backtrace`✗ 太重）
#   ＋ 在每处 `.entries.borrow_mut()`（3348／3353／3365／3336 一带 ✓）写站点名 ✓
#   ＋ 在 `entries()` 的 `borrow()` 失败路径（用 `try_borrow().unwrap()` 改成 `match` ✓）打印 holder ✓
# 判据不变（三条）：放回 id ⇒ QUARANTINE 20/20 绿 ／ 不带 QUARANTINE 20/20 绿 ／ 六件 builtins 一起补 ⇒ slowcheck 全绿 ⇒ 提交 ✓
```

## 下一条命令（**★ 持有者已拿到：`python_level_finalize`（终结器里回调 Python ✗）**）

**第 546 轮（本轮 ✓，探针已撤 ✓）**：让测试给子进程显式 `RUST_BACKTRACE=full` 并把 stderr 落盘后 ✓，
**子进程自己的回溯**终于到手 ✓（`target/recon/canary_empty.log` ✓；注意本轮失败的子名是 `empty` ✓ ——
**子名会变** ✗ ⇒ 更印证是"时序/回收时机"类 ✓）：
```
panic: RefCell already mutably borrowed @ builtin_objects.rs:3336（DictObject::entries ✗）
  20: <DictObject>::entries                                  builtin_objects.rs:3336
  21: <Instance>::dict_get                                   instance/containers.rs:242
  22: executor::super_lookup                                 executor.rs:212
  23: executor::attribute::attribute_lookup                  attribute.rs:85
  24: executor::call::call_object_method                     call.rs:87
  25: builtin_objects::python_level_finalize                 builtin_objects.rs:1831   ★ **持有者**
  26: <Instance>::release_one                                instance.rs:1481
```
⇒ **病灶定案** ✓：`Instance::release_one`（`instance.rs:1481` ✓）在**还持有 `DictObject` 的
`borrow_mut()`（清条目 ☆）**时，调用了 `python_level_finalize`（`builtin_objects.rs:1831` ✓）⇒
终结器回调进 Python（属性查找 → `super()` → `dict_get` ✓）⇒ **重入借用** ⇒ panic ✓✓
—— 正是"**持有 RefCell 借用期间又调进 Python**"那一类 ✓（与我在 541 轮的推断一致 ✓）。

## 下一条命令（改法很小，一次到位）

```bash
sed -n '1470,1495p' crates/pyawa-core/src/instance.rs     # release_one：看清条目在哪一格被 borrow_mut 持有
# 改法（口径）：**先把条目取出/结束借用 ⇒ 再调 finalize** ✓（或把 finalize 排到借用作用域之外 ✓）
#   典型形态：`let removed = { let mut e = entries.borrow_mut(); e.remove(index) };`  ⇒ 再 `finalize(removed)` ✓
# 判据（三条同时）：
#   ① builtins 里放回 "id" ⇒ PYAWA_QUARANTINE=1 下 meta_path_shapes **20/20 绿** ✓
#   ② 不带 QUARANTINE 也 20/20 绿 ✓（当前 6/20 ✗）
#   ③ 再把 vars／hash／ascii／format／dir 一起补 ⇒ tools/slowcheck.sh 十项稳定全绿 ⇒ 提交（代码＋台账＋NEXT.md 同笔 ✓，不接管道 ✗）
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
