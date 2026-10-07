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

**第 552 轮（本轮 ✓，全部已撤回 ✓）**：
- 给 `DictObject` 的可变借用点打站点后跑 harness ⇒ **panic 前最后一条**是
  `[borrow] entries.borrow_mut ← 行号 3349` ✓（＝`DictObject::insert_raw` ✓）；
  **但它不一定是元凶** ✗ —— 我只在 `builtin_objects.rs` 里打了 5 处 ✓，别的文件里若也持有就漏了 ✗。
- 想把探针扩到**全 crate** 时踩坑 ✗：`core::mem::take(&mut *object.entries.borrow_mut())` 这种形态
  不能内联替换 ✗（17 个编译错 ✗）⇒ 已**全部撤回** ✓（构建 0 错 ✓、护栏 3 次全绿 ✓）。
  **正确做法**：只在**语句形态**（`let mut x = …entries.borrow_mut();` × `x.push/…` ✓）不变量替换 ✓，
  或加一个 `#[inline] fn entries_mut(&self, site: &str) -> RefMut<…>` 统一入口 ✓ 再全局替换 ✓。

**第 553 轮（本轮 ✓，全部已撤回 ✓，树干净 ✓）**：按"只碰语句形态"的正确打法 ✓ 插了 **4 处**
`let mut … = ….entries.borrow_mut();` 的站点打印 ✓（`builtin_objects.rs` ✓，构建 0 错 ✓），
再让测试把子进程 stderr 打出来 ✓ ⇒ **panic 前最后一条**是：
```
[borrow] crates/pyawa-core/src/builtin_objects.rs:3368
RefCell already borrowed                    ← 紧随其后 ✓
```
**`3368` 属于 `DictObject::remove`** ✓（`let mut entries = self.entries.borrow_mut(); … Some(entries.remove(index))` ✓）——
**保留项** ✓：这**可能**就是持有者 ✓（`remove` 的 `RefMut` 在其作用域内触发了一次终结器/回调 ✗ ⇒ 重入 ✗）；
**但**（如实 ✗）stdout/stderr 的**交织顺序**会让"文件里最后一条"≠"时间上最后一条" ✗ ⇒ 需下一轮用**带时间戳**的打印（或把站点数按"进入/离开"成对打印 ✓）来定论 ✓。

**下一轮（定论一步）** ✓：把站点打印改成**成对**（`[borrow+] 3368` / `[borrow-] 3368` ✓）⇒ panic 前**未闭合**的那个就是持有者 ✓，
再顺它找"在 `RefMut` 作用域内调进 Python"的那一步 ✓（若确系 `remove` ✓ ⇒ 把 `Vec::remove` 换成"先 `mem::take` 出条目、结束借用、再释放" ✓）。

**第 554 轮（本轮 ✓，无代码残留 ✓）**：把上一轮的"交织顺序"顾虑**否掉** ✓ —— 我的站点打印与 panic **都走 stderr**
（**无缓冲** ✓）⇒ 文件顺序**就是**时间顺序 ✓ ⇒ 持有者是 `DictObject::remove`（`:3368` ✓）作用域内发生的某次"回调"✓。
**但**顺着查下去，两个候选回调都被否掉 ✓：
```
values_equal  ：文档与实现都是"整数/浮点/字符串**按值**、其余**按身份**" ✓ ⇒ **不调 Python** ✗
Vec::remove   ：纯 Vec 操作 ✓ ⇒ 无回调 ✗
```
⇒ 结论（本轮的关键事实 ✓）：**阻住读者的那个可变借用根本没被插桩** ✗ —— 我只插了 **4 处语句形态**
（`let mut x = ….entries.borrow_mut();` ✓）；**表达式形态**（`self.entries.borrow_mut().push(...)` 等 ✓）没插 ✗，
而 Rust 里 `a.b(c)` 的求值顺序是 **先 `a`（临时 RefMut 生效）再 `c`** ✓ ⇒ 若 `c` 里调进 Python ⇒ **正好重入** ✓✓
—— 这才是真正的形态 ✓（也解释了为什么语句形态的 4 处都干净 ✓）。

**第 555 轮（本轮 ✓，探针已撤 ✓）**：按"统一入口"打成 `entries_mut(site)` ✓（替换 **6 处** ✓、构建 0 错 ✓），
但**两个自身缺陷**让这次没定论 ✗（如实 ✓）：
1. 我把 `site` 写成了**常量** ✗（`site="builtin_objects"` × 6 ✓）⇒ **丢掉了行号** ✗ ⇒ 无法指名 ✓ ——
   正确做法：给每处**手写不同标签**（`"3336"`／`"3349"`／`"3359"`／`"3366"`／`"3368"`／`"3641"` ✓），或用宏 `entries_mut!()` 借 `line!()` ✓；
2. 这次日志 **31 万行** ✗、且 `already` 出现 **0 次** ✗ ⇒ 带探针后**没复现**（时序被改变 ✓）⇒
   下一轮要把探针**限定到"读者那个 dict"**（只对 `dict_get` 读的那个对象打 ✓），否则既淹日志又改变时序 ✓。

**第 556 轮（本轮 ✓，无代码改动 ✓）**：以本会话剩余预算，①**无法安全收口** ✗（每次探针都要"改—跑—撤"三步 ✓，
且已两次踩到自身缺陷 ✗）。⇒ 把**可逐字套用**的探针补丁写死在这里 ✓（下一轮或专项直接应用 ✓）：

```rust
// ① builtin_objects.rs 模块级：记住"读者正在读的那个 dict"（读者失败时写入 ✓）
pub(crate) static WATCHED_DICT: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

// ② DictObject::entries()：把 borrow() 换成 try_borrow()，失败即登记自己再 panic ✓
pub fn entries(&self) -> Vec<(NonNull<Header>, NonNull<Header>)> {   // ← 按其真实签名照改 ✓
    match self.entries.try_borrow() {
        Ok(items) => items.clone(),
        Err(_) => {
            WATCHED_DICT.store(self as *const _ as usize, core::sync::atomic::Ordering::Relaxed);
            panic!("entries(): RefCell 已被可变借用（dict={:p}）", self);
        }
    }
}

// ③ 统一入口：**只对"被登记的那个 dict"**打印 ✓（避免 31 万行＋改时序 ✗），且站点逐处手写不同标签 ✓
#[inline]
pub fn entries_mut(&self, site: &'static str) -> core::cell::RefMut<'_, Vec<(NonNull<Header>, NonNull<Header>)>> {
    if crate::diag::flag("PYAWA_BORROW_DEBUG")
        && WATCHED_DICT.load(core::sync::atomic::Ordering::Relaxed) == self as *const _ as usize
    {
        eprintln!("[borrow+] site={site}");      // ← 调用处逐处写 "3336"/"3349"/"3359"/"3366"/"3368"/"3641" ✓
    }
    self.entries.borrow_mut()
}
```
⇒ 应用后重跑 harness（`PYAWA_QUARANTINE=1 PYAWA_BORROW_DEBUG=1 cargo test -p pyawa-runtime --test meta_path_shapes -- --nocapture` ✓）
⇒ `[borrow+]` 里**未被释放**的那一条就是持有者 ✓ ⇒ 按"**先求值、后借用**"重排它 ✓。

**第 558 轮（本轮 ✓，全部已撤回 ✓）**：按补丁实打了一次 ✓（`entries.borrow()`→`try_borrow` **4 处** ✓、
`borrow_mut()`→`entries_mut("L<行号>")` **6 处** ✓、`id` 与 stderr 打印就位 ✓），
但**我自己又踩一个坑** ✗：把 `WATCHED_DICT` 静态声明**前置到了模块 `//!` 文档注释之前** ✗ ⇒
`E0753: expected outer doc comment` ×6 ✗ ⇒ 已**全部撤回** ✓（构建 0 错 ✓）。
**下一轮照补丁打时**：静态声明要放在**模块 `//!` 注释与 `use` 之后** ✓（不要前置 ✓）；
其余步骤（`entries()` 的 `try_borrow` 登记 ＋ `entries_mut(site)` 只对被登记 dict 打印 ＋ 站点写 `line!()` 行号 ✓）都验证过可编译 ✓。

**第 559 轮（本轮 ✓，全部已撤回 ✓）**：按正确位置打补丁（静态放在 `use` 之后 ✓）⇒ **构建 0 错** ✓，
但跑 harness 时 **`already` 与 `[borrow+]` 都没出现** ✗ ⇒ **测试没崩** ✓ —— 即**探针把 bug 藏起来了** ✗
（`try_borrow`＋统一入口这层间接改变了时序 ✓）⇒ 典型 **heisenbug** ✓✓。

**⇒ 这条事实很关键** ✓（给专项/下一轮）：**①不能靠"插桩—复现"来收** ✗，
必须**从借用图推理**：`entries()` 在 panic 时说明"同一个 dict 的 `RefMut` 还活着" ✗ ⇒
在 `builtin_objects.rs`／`classes.rs`／`executor/*` 里逐条审"**谁在 `RefMut` 作用域内可能调进 Python**"✓
（本轮已排除：`values_equal` 不调 Python ✓、`insert_raw`/`replace_value`/`remove` 三处原语干净 ✓ ⇒
⇒**剩下只有"表达式形态下参数求值"与"跨函数的借用传递"两类** ✓）。

# ⛔ 目标收口（第 560 轮 ＝ 目标轮次上限，2026-10-07 02:2x）

**目标本身：未达成** ✗（**不得声称完成** ✓，`AGENTS.md`「完成度如实」✓）。
```
判据①：**187／628 ＝ 29.8%** ✗（阈值 67% ✓）—— 起点 184（29.3%）⇒ 本会话**净 +3~4**（在 ±1 噪声带之上 ✓）
进度指标（不作判据 ✓）：Lib/ 已同步子集 294 个文件 ⇒ 能 import 172 个（58.5% ✓）
`HEAD=3bf8a61` ✓；本 goal 有效落地 **19 笔**（十项闸门每笔全绿 ✓）＋**撤回 2 笔**（均闸门红 ✓）
```
**三因（未变 ✓，按优先级）**：
1. **`re`／`_sre`**（≈**77 个模块** ✓，最高杠杆 ✓）—— `re` 之下是 C 面 `_sre` ✗；
2. **RefCell 重入**（builtins 命名空间 +1 即触发 ✗ ⇒ 挡住"补 `builtins` 面"这条新口径前排 ✓）：
   证据链完整 ✓、**已定性为 heisenbug** ✓ ⇒ 收法＝**按借用图推理**（见上 ✓），不要再插桩 ✗；
3. **闭包链**（空 cell ✗）—— 已在 (b) 弃支 ✓（`xml.sax` 族 6 个 ✓）。
**下一手（不烧轮次的做法 ✓）**：先定 1 或 2 之一开专项（`_sre` ≈77 模块 / 借用图 audit ✓）；
工具与规则侧本会话已交付齐全 ✓（见本文件其它小节与 `docs/rounds/03-m3-progress-02.md` ✓）。

# ✅ 第 561 轮：`_sre` 方案定稿（**只落地"面"的事实，不动代码** ✓）

**依赖通路已打通** ✓（`e9ae84b`：`CARGO_HOME=target/cargo-home` ＋ `tools/cargo.sh` ＋ 闸门脚本同步导出 ✓；
`regex v1.13.1` 拉取＋编译 15 s ✓）⇒ 用户口径"允许依赖、不手写一切"**可执行** ✓。

**`_sre` 的**确切面**（从我们本地上游 3.14 副本读出 ✓，非印象 ✗）**：
```
re/_constants.py:16   MAGIC = 20230612          ← `_compiler.py:18` 会 assert _sre.MAGIC == MAGIC ✓
re/_constants.py:18   from _sre import MAXREPEAT, MAXGROUPS     ← 两个名字都要在 ✓
re/_compiler.py:397   _CODEBITS = _sre.CODESIZE * 8             ← CODESIZE 要 ✓（参照＝4 ✓）
re/_compiler.py:52-57 _sre.unicode_iscased／unicode_tolower／ascii_iscased／ascii_tolower  ← 四个函数 ✓
re/_compiler.py:778   _sre.compile(pattern, flags|state.flags, code, groups-1, groupindex, tuple(indexgroup))
re/__init__.py:315    Pattern = type(_compiler.compile('', 0))  ← **导入时就调** ⇒ 没有 compile ⇒ `re` 永远进不来 ✓
re/__init__.py:377    _sre.template(pattern, _parser.parse_template(repl, pattern))       ← sub 的 repl ✓
```
**关键设计决定** ✓（本轮最大的"事实"）：`_sre.compile` 拿到的是 `_compiler` 生成的 **SRE 字节码** ✗，
而我们**不打算解释那份字节码** ✗ ⇒ **用 `regex` crate 直接编译 `pattern` 源串** ✓（flags 映射 IGNORECASE/MULTILINE/DOTALL/VERBOSE ✓），
**忽略 `code` 参数** ✓（语义是子集 ✓，如实登记 ✓）。
为避免"新载荷类型"那类风险 ✗（`complex` 的教训 ✓），**Pattern/Match 用 Python 层小类包一个不透明 id** ✓：
`_sre` = 原生模块（常量 ＋ `compile_raw(pattern, flags) -> int` ＋ `match_raw(id, s, pos, endpos) -> tuple|None` ✓），
`Lib/_sre.py`?? ✗ 不能同名 ✗ ⇒ 原生模块直接**返回由原生工厂造出的 Python 类实例**（或让 `re` 侧用这些原始函数自己包 ✓）
—— 具体形状下一轮定，**原则是"不新增载荷类型"** ✓。

**参照的精确值（本地 `python3` 实测 ✓，逐值照抄即可 ✓）**：
```
_sre.MAGIC      = 20230612
_sre.CODESIZE   = 4
_sre.MAXREPEAT  = 4294967295            （= 2**32 - 1 ✓）
_sre.MAXGROUPS  = 1073741823            （= 2**30 - 1 ✓）
_sre.unicode_iscased(cp:int)->bool ／ _sre.ascii_iscased(cp:int)->bool
_sre.unicode_tolower(cp:int)->int   ／ _sre.ascii_tolower(cp:int)->int     ← **收整数码点** ✓（我先前误传 str ⇒ TypeError ✗）
```
**下一轮第一步（可执行）** ✓：先落 `_sre` 的**常量与四个 cased/tolower 函数** ✓（`crates/pyawa-stdlib/src/_sre_module.rs` ✓，
照 `sys_module.rs`／`errno_module.rs` 的注册形状 ✓），判据＝与参照**逐值一致** ✓；
再把 `compile` 接上 `regex` ✓，判据＝`re.match/search/sub/split/findall` **逐例**一致 ✓。

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
