# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。

## 现在

- **HEAD**：`5b979b6`（`dev`）——`_sre` 第二块（四个 cased/tolower ✓）
- **工作区**：本轮只落**诊断能力**（见下 ✓），无功能改动 ✓
- **stash**：`stash@{0}` ＝ `sre-raw(575)`（`compile_raw`/`match_raw` ＋ `regex` 依赖 ✓，六例已验证 ✓，**等 ① 修好后再 pop** ✓）
- **判据①**：`tools/lib_import_ratio.py` ＝ **187／628 ≈ 29.8%** ✗（阈值 67%；单次读数 ±1 ⇒ 按**区间**读 ✓）
- **阻塞单点**：**① 的 UAF**（下面定论 ✓）—— 它挡着一切"往命名空间/原生表里加东西"的活 ✓

## ① 第 577 轮的定论（**首次拿到可读崩溃栈** ✓）

**能力**：新增 `PYAWA_SEGV_TRACE=1`（`crates/pyawa-runtime/src/bin/pyawa.rs` ✓）：装 SIGSEGV/SIGBUS/SIGABRT
处理器 ✓ ⇒ 崩溃当场打印**符号化栈** ✓；处理器只在崩溃路径跑 ✓ ⇒ 不拖慢正常执行 ✓。
**前提修复** ✓：`meta_path_shapes` 的失败信息原先只报退出状态 ✗ ⇒ 子进程 stderr 全被丢掉 ✗；
现在把 stdout/stderr 一起带进 `assert!` ✓（**这一格才是栈能拿到的原因** ✓）。

**崩溃栈（8/8 完全一致 ✓，轮次间可复现 ✓）**：
```
DictObject::entries        builtin_objects.rs:3337   ← memcpy（Vec 缓冲区读崩 ✓）★崩溃点
Instance::dict_get         instance/containers.rs:242
executor::super_lookup     executor.rs:209           ← ★读者：**super 对象自己的内联属性字典**
attribute::attribute_lookup / call::call_object_method
builtin_objects::python_level_finalize  :1831        ← 终结器
Instance::release_one      instance.rs:1489 → release_object → format::release → execute
```

**三条硬事实** ✓（都改变了计划 ✓）：
1. **不是** `RefCell` 重入 ✗（`entries()` 里的 `try_borrow()` 探针**一次都没响** ✓）—— 是**读到坏内存** ✓；
2. **`gdb` 一介入就"正常退出"** ✗（`set disable-randomization off` 也一样 ✓）⇒ 只能在**进程内**抓栈 ✓（本轮的器就是这么来的 ✓）；
3. **直跑 0/100+ 全绿 ✗、harness 里 0~100% 红 ✓**（同一脚本/路径/cwd ✓，且复现率随**机器负载**大幅波动 ✓）
   ⇒ 判据① 那种"加名字就红"的现象是**时序/布局敏感** ✓，`PYAWA_QUARANTINE=1` 会让它**消失** ✓ ⇒ heisenbug ✓。

**已排除** ✓（各有实测/读码证据 ✓，别再花轮次 ✗）：类体/命名空间所有权四点 ✓、`previous` 两处替换 ✓、
三处 dict 变更原语 ✓、`values_equal` ✓、`PYAWA_DANGLING` 哨兵 ✓、`type_namespace` 双通道（两份**不同**字典 ⇒ 未见双释放 ✗）✓。

## 下一条命令（① 的下一步：按"读者是谁"往下查 ✓，不插桩 ✓）

读者是 `super_lookup`（`executor.rs:205-214` ✓）读**该 `super` 对象自己的内联属性字典** ✓；
该字典的**建法**已核实为"新建 + 独占"（`builtin_objects.rs:1191-1205`：`AttributeObject::new(…, RefCell::new(Some(new_dict())))` ✓ rc=1 ✓）
⇒ 剩下两种可能 ✓，一条命令就能分开 ✓：

```bash
# 用**已有**开关 `PYAWA_SETDICT_DEBUG=1`（打印"内联属性字典 ← 指针" ✓）＋ `PYAWA_SEGV_TRACE=1`：
#   看崩溃前该 super 对象的字典**是否被别处换过**（`set_attributes` 覆盖 ⇒ 旧值交还但可能仍被引用 ✓）
#   ＋ 看 `mounted_instance_dict`（`executor/protocol.rs:117-141`）是否给**同一对象**又挂了一份 header 槽字典 ✓
cd crates/pyawa-runtime && PYAWA_SETDICT_DEBUG=1 PYAWA_SEGV_TRACE=1 \
  cargo test --test meta_path_shapes -- --nocapture 2>&1 | tail -60
```
⇒ 若字典被换过 ⇒ 修"**换字典时旧的那份还有谁在用**"（`AttributeObject` 的两条通道只留一条 ✓）；
⇒ 若没被换过 ⇒ 转查 `super` 对象**自身**是否被提前释放（读者拿到的是**被复用**的内存里残留的字典指针 ✓）。
**判据（不变 ✓）**：`PYAWA_QUARANTINE=1` 下 `meta_path_shapes` **稳定 20/20 绿** ✓ **且** `stash@{0}` pop 回来仍 20/20 绿 ✓ ⇒ 再提交 `_sre` 那笔 ✓。

## 未决（等你拍 ✓）

1. **`enum` 支**：`Lib/re/__init__.py` 开头 `import enum` ✓ ⇒ `_sre` 通了 `re` 也进不来 ✓（第 564 轮已列两条路的代价 ✓）；
2. **规则**：闸门里**已知间歇**用例的处置（建议"最多重跑 3 次 + 正文写明重跑次数"✓）—— 规则归你 ✓，我不擅自放宽 ✗。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；不接管道看闸门（只看 exit code ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 仪器口径：单次读数 ±1 ⇒ 报"区间" ✓（第 512 轮实测 ✓）。
