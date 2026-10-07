# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。

## 现在

- **HEAD**：`a0607f3`（`dev`）＋本轮未提交的**真修**（见下 ✓）
- **stash**：`stash@{0}` ＝ `sre-raw(575)`（`compile_raw`/`match_raw` ＋ `regex` 依赖 ✓，六例已验证 ✓，**现在可以 pop 了** ✓）
- **判据①**：`tools/lib_import_ratio.py` ＝ **187／628 ≈ 29.8%** ✗（阈值 67%；单次读数 ±1 ⇒ 按**区间**读 ✓）
- **① 的状态**：**可观测失败已消失** ✓（重压 6×6 并发：修前 **27/36 红** ✗ ⇒ 修后 **0/36 绿** ✓，同一环境同一命令 ✓）；
  **但底层"字典被重复释放"仍未被证明不存在** ✗（见下"如实" ✓）。

## 本轮的真修（两处，都有参照依据 ✓）

1. **`__del__` 改用特殊查找** ✓（`builtin_objects.rs::python_level_finalize`）：
   参照取 `__del__` 走 `_PyObject_LookupSpecial` ✓ —— **只扫 `type(self)` 的 MRO** ✓，
   不碰实例字典 ✗、不走 `__getattribute__`／`__getattr__` ✗。旧写法走**完整 `getattr`** ✗
   ⇒ 对 `super` 对象会绕进 `super_lookup` ✓ 读它**自己正在释放**的属性字典 ✓
   ⇒ 第 578 轮抓到的崩溃读者正是它 ✓（剥掉这层绕行后，重压 27/36 ⇒ 0/36 ✓）。
2. **`incref` 的断言放宽到终结器窗口** ✓（`header.rs`）：`release_one`（`OM-20` ①）在**计数归零后**
   才调终结器 ✓，而终结器要把 `__del__` **绑到 `self`** ✓ ⇒ 从 0 incref 是**合法复活** ✓
   （`release_one` 随后正是用 `refcount != 0` 判复活 ✓）。旧断言 `> 0` ✗ ⇒ 让**任何带 `__del__` 的脚本**
   在 debug 下当场中止 ✓（实测：`class A: def __del__(self): print("bye")` ✓ 参照 `bye/end` ✓、我们 panic ✗）。
   现在收紧成"只有 `FINALIZING` 窗口才允许" ✓ ⇒ 真正的越界 incref 照样报 ✓。

**护栏（新文件 ✓）**：`crates/pyawa-runtime/tests/finalize_shapes.rs`（两条：`__del__` 在重绑定时跑 ✓、
终结器**复活**语义 ✓，均与参照逐字一致 ✓）。

**如实（不得含糊 ✓）**：直跑里 `[free-dict]` 同一指针出现两次的检查**不能作数** ✗
（分配器复用地址也会这样 ✓）；**"谁提前释放"仍未落到具体一行** ✗ —— 归零的是**可观测失败**，
不是"证明底层 UAF 不存在" ✗。

## 下一条命令（回到判据①主线 ✓）

```bash
git stash pop                      # 取回 `_sre` 底层（现在 ① 不再随机红 ✓）
# 然后：Lib/_sre 包装层 —— `compile(pattern, flags, code, groups, groupindex, indexgroup)`（忽略 code ✓）
#   返回带 match/search/fullmatch/split/findall/finditer/sub/subn 的 Python 层对象（不新增载荷类型 ✓）
cd crates/pyawa-runtime && cargo test --test finalize_shapes --test meta_path_shapes   # 先确认两族护栏 ✓
tools/slowcheck.sh                 # 十项闸门（只看 exit code ✓）
```
**另需你拍** ✓（第 564 轮列过代价 ✓）：`re/__init__.py` 开头 `import enum` ✗ ⇒ `_sre` 通了 `re` 也进不来 ✓
（重启 `enum` 支 ✓ 或给 `Lib/` 放最小自有 `enum.py` ✓ 二选一）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；闸门**只看 exit code**（不接管道 ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 仪器口径：单次读数 ±1 ⇒ 报"区间" ✓（第 512 轮实测 ✓）。
