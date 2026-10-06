# NEXT.md —— 续做状态（单页；长复盘进台账）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**。本轮开工先写"要落什么 ＋ 验收命令"。

## 现在

- **HEAD**：`3e5c70f`（`dev`，已推送）
- **工作区**：`M crates/pyawa-core/src/classes.rs`（**bug 1 修复**：`__prepare__` 调用后交还 `name_value`／`bases_value`；
  构建 0 错；**未提交** ⇒ 必须与 bug 2 的修复**同笔提交**）
- **判据**：`tools/lib_import_ratio.py` ＝ **184/628 ＝ 29.3%** ✗（阈值 67%）

## 下一条命令（R3 队首：`enum.py:1106` 的 `class Enum(metaclass=EnumType)` ⇒ StackUnderflow）

**第 507 轮已修** ✓：`type_new_native` 原先按**类型身份**要求命名空间"恰好是 dict" ✗ ⇒ `_EnumDict`
（dict 子类 ✓）被拒 ⇒ 与参照不符 ✓。已放宽为**子类即可** ✓（`is_subtype` ✓）；最小样例
`target/recon/su/d.py`（把 `__prepare__` 的 `dict` 子类**本身**交给 `super().__new__`）本层 `D-ok` ✓ ＝ 参照 ✓。

**仍是墙** ✗：`import enum` 在 **`enum.py:1106`（`class Enum(metaclass=EnumType)`）** 报
**`帧操作失败：StackUnderflow`** ✓（顶层探针实测 ✓；同族的最小样例 a／b／c 都已排除 ✓）。

## 下一条命令

```bash
# ① 在 EnumType.__new__ 内部继续二分（§105 规则）：先试它用到的剩余构造
#    —— `classdict['_hashable_values_'] = []`（STORE_SUBSCR ✓）、`del e.__notes__`、
#    `@property` / `staticmethod` 混用、`for name in member_names` 的**解包/切片**
grep -n "class _EnumDict" -A 60 target/lib-full/enum.py | grep -nE "def |return|del |for |:" | head -20
# ② 或直接对 `class Enum(metaclass=EnumType)` 那一句做"最小复现"：把 EnumType.__new__ 里
#    第 481 行起的语句**逐段**搬进仿制类，看哪一段触发 StackUnderflow
# 判据：`import enum` 打印 ok ⇒ 再试 Lib/ 整包（unittest ＋依赖 ＋ re）⇒ 两道必检 ⇒ SLICE ⇒ --sync ⇒ 报判据①
```

## 未修 bug（各带判据）

| # | 病灶 | 位置 | 判据 |
|---|---|---|---|
| 2 | `__prepare__` 返回值**多一份所有者**（rc=2，应为 1） | `call.rs` 的 `function` 分支（`:449` 起；建帧 `:464–492`；返回值处理在 **`:505` 之后**） | `rc` 探针（`classes.rs` 两处 `println!`）显示 `prepared` 交回时 **rc=1** ✓；`loop_class.py` 跑满 200 次 ✓；两开关不 panic ✓ |
| — | `__prepare__` 实参泄漏（**已修，待提交**） | `classes.rs:150–197` | 每类泄漏两个对象 ⇒ 曾使 K＝80 |

**推迟（不得用轮次磨）**：需要**新载荷类型**的活（`complex` 等 7 变体 × 2 时机已穷尽）、`open`／`_io` 真文件实现（按 `CM-8`／`P3-14` 走 `fs` 域）。

## 纪律提醒

- 红灯不提交；代码＋台账**同笔**（`commit-rule.md` §1）；**先易后难**、判据① 连续 5 轮不动即报停滞。
