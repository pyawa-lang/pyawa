> 本卷是 `docs/ROUNDS.md` 分卷台账之一（**非规范**、只增不改；卷目见该索引）✓

#### 第 369 轮：✅ 对照实验一**判因清楚** —— `-11` 只在 `--jobs 8` 下出现（与并发有关 ✓），**不是**深度修复引入的

**① 对照（`--ceiling --jobs 1` ✓）** ✓：
```
上限诊断：能 import **162** 个（25.8%）      ← 与 jobs 8 **一致** ✓
     118  TypeError: 'NoneType' object is not iterable
      77  NameError: name 'eval' is not defined        ← **回到了 77** ✓（jobs 8 里的 76 是假象 ✗）
      28  annotationlib ／ 19 _struct ／ 9 binascii ／ 8 complex ／ 8 xml.dom ／ 7 warnoptions ／ …
       6  TypeError: … unary -: 'timedelta'
       6  ImportError: cannot import name 'open' from 'builtins'
       6  SyntaxError：加载模块 'typing'：`class` 后面要冒号
**没有任何 `-11` 行** ✓
```
**② 结论（本轮 ✓）**：
* **`-11` 只在 `--jobs 8` 出现** ✓ ⇒ 与**并发／环境**有关 ✗，**不是**第 167 轮"异常表深度"修复的确定性后果 ✓
  ⇒ 按纪律：**保留该修复** ✓（它已由 9 行复现"由崩转对"证明有价值 ✓）；
* 而且 `jobs 1` 与 `jobs 8` 的**上限、判据完全一致** ✓ ⇒ 那族 `-11` **不影响**头条数字 ✓
  ⇒ 它更像是**本会话 (a) 族**（内存缺陷 ✓：同一对象按两种角色计数／模块字典与命名空间同一份 ✓）
  在**并发子进程**下的表现 ✓ —— **但这一条仍是推断** ✗（要坐实得用 `PYAWA_QUARANTINE` 一类诊断 ✓，
  排进后续 ✓，**不**在没证据时当结论 ✓）。
**③ 下一个候选（下一轮 ✓，回到主线 ✓）**：既然 118 族的**下一层**仍在（`NoneType` 不可迭代 ✗ ✓），
就继续追它 ✓ —— 第 359／363 轮已知它来自 `getattr(x, n, None)` 的**三参吞异常**那条路 ✓
⇒ 从**最小复现** `target/repro_forelse.py` 出发 ✓（它现在还返回 `None` ✗ ✓），
把它**继续二分**（第 356 轮那套：显式写变体、规范判定 ✓）⇒ 找到"三参 `getattr` 吞异常"的形状 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（本轮实测 ✓，
`jobs 1` 与 `jobs 8` 一致 ✓）；**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只跑对照 ✓、树干净 ✓）。

#### 第 367 轮：复现手法修正 ✓ —— CLI 要**手动把 `target/lib-full` 放进 `sys.path`**（否则只是找不到模块 ✗）

**① 本轮踩到的 ✓**：直接 `./target/debug/pyawa target/imp_ctxlib.py` 给的是
`ModuleNotFoundError: No module named 'contextlib'` ✓ —— **不是崩溃** ✗ ⇒ 因为
`tools/lib_import_ratio.py --ceiling` 是把**上游全量语料**（`target/lib-full` ✓）放进 `sys.path` 跑的 ✓
⇒ 复现必须照做 ✓（本会话早先的探针就是这么写的 ✓，这次我漏了 ✓）。
**② 顺带一个**我自己的小错**（如实 ✓）**：上一轮我用
`timeout … | tail …; echo "退出码=$?"` 量退出码 ✗ —— `$?` 拿到的是 **`tail` 的**状态 ✗、不是 `pyawa` 的 ✗
⇒ 这一轮改成**先重定向到文件、再单独看 `$?`** ✓（并已在命令里这么做 ✓）。
**③ 下一轮（就一件 ✓，判因）** ✓：用带 `sys.path` 的复现跑 `contextlib` ✓（本轮已跑 ✓，结果见命令输出 ✓）：
* 若**崩（-11）** ✗ ⇒ **撤掉第 167 轮的深度修复**再跑 ✓：
  * 不崩 ✓ ⇒ 我的修复引入的 ⇒ 按纪律**撤回** ✓、台账写明 ✓；
  * 照样崩 ✗ ⇒ 与它无关 ⇒ 再查 `__class__`（第 144 轮 ✓）／更早改动 ✓；
* 若**不崩** ✓ ⇒ 说明那族**只在 `--ceiling` 的并发/长跑下出现** ✗（`--jobs 8` 子进程里 ✓）
  ⇒ 那就改用 `--ceiling --jobs 1`（或单模块驱动 ✓）复现 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 365 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（树干净 ✓）。

#### 第 368 轮：`contextlib` **单独跑不崩** ✓（给的是 `eval` 的 NameError ✓）⇒ `-11` 只在 `--jobs 8` 环境里出现

**① 对照结果** ✓（`target/ctxlib.out` ✓，先重定向再取 `$?` ✓）：
```
带 sys.path 单独跑：退出码=1 ✗（不是 -11 ✓）
    pyawa: 未捕获（状态 1）：NameError: name 'eval' is not defined     ← 已知族 ✓
```
⇒ 同一个模块：**单独跑**是 `eval` 族的 `NameError` ✓；**`--ceiling --jobs 8`** 里是 `-11` ✗
⇒ 说明 `-11` 与**并发/长跑环境**相关 ✓，**不是**"这个模块必崩" ✗。
**② 据此**降低"我的修复引入回归"的可能性**（但**不下结论** ✓）**：
* 若第 167 轮的深度修复引入的是**确定性**错误 ✓，那么单独跑同一模块**也**该崩 ✗ —— 而它不崩 ✓；
* 所以 `-11` 更可能来自**本会话 (a) 族**（内存缺陷 ✓：按两种角色计数／模块字典与命名空间同一份 ✓）
  在**并发子进程**下的表现 ✓，或 `--jobs 8` 的资源因素 ✓；
* **但**这仍是**推断** ✗ ⇒ 按纪律**必须做那个对照实验**（下一轮 ✓）。
**③ 下一轮（就一件 ✓，两个对照各一次 ✓）** ✓：
1. `--ceiling --jobs 1` ✓ ⇒ 若 `-11` **消失** ✓ ⇒ 与并发有关 ✓（记入 (a) 族账 ✓）；
2. **撤掉第 167 轮修复**（把两行改回 `handler_depth` ／ `handler_depth + 1` ✓）⇒ 再跑 `--jobs 8` ✓：
   * 若 `-11` **照样出现** ✗ ⇒ 与我的修复无关 ✓ ⇒ **保留**修复 ✓（它已由 9 行复现证明有价值 ✓）；
   * 若 `-11` **消失** ✓ ⇒ 是我的修复引入的 ✗ ⇒ **撤回**并在台账写明 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 365 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（树干净 ✓）。

#### 第 365 轮：重测结果**如实** —— 上限/判据未动 ✗；且出现**新的 `-11` 族（6 个模块）** ✗（疑回归，下轮追）

**① 受管作业实测（03:59／04:02 ✓）** ✓：
```
上限诊断：能 import **162** 个（25.8%）（**未动** ✗）
     118  TypeError: 'NoneType' object is not iterable        ← **没动** ✗
      76  NameError: name 'eval' is not defined               （77 → 76 ✓ 微动）
      28  SyntaxError：annotationlib（第 327 行 列 18-20）
      19  _struct ／ 9 binascii ／ 8 complex ／ 8 xml.dom ／ 7 warnoptions ／ 7 _codecs_jp ／ 7 _codecs_iso2022
       6  TypeError: bad operand type for unary -: 'timedelta'
       6  **子进程退出码 -11**                                  ← 🚨 **新族**（SIGSEGV ✗）
       6  ImportError: cannot import name 'open' from 'builtins'
判据①：**172 ÷ 628 ⇒ 27.4%**（未动 ✗）／进度指标 **55.1%**（未动 ✗）
```
**② 两条如实结论** ✓：
1. **第四个真 bug 的修复没有推动这两项指标** ✗ —— 9 行复现确实修好了 ✓（它由崩转对 ✓），
   但 118 族卡的**下一层**（`NoneType` 不可迭代 ✗）依然在 ✓ ⇒ 说明这条链还有**至少一层** ✓
   （第 359 轮已经知道：`getattr(x, n, None)` 的**三参吞异常**那条路 ✓）；
2. 🚨 **新出现的 `-11`（6 个模块）** ✗ —— 前几次重测（`r128`、`r135`）**都没有**它 ✓
   ⇒ 所以它是在**这两次之间**引入的 ✗ ⇒ 嫌疑：`__class__` 能力缺口（第 144 轮 ✓）
   或**异常表深度修复**（第 167 轮 ✓）。
**③ 下一轮（就一件 ✓，**先处置回归** ✓ —— 纪律要求：红了要查清、必要时撤 ✓）**：
* 从 `target/ratio-r184.txt` 的同一次运行里拿到那 **6 个模块的名字** ✗（我的窄 grep 没抓例子 ✗）
  ⇒ 重跑一次**只抓 `-11` 那族的例** ✓（便宜 ✓）；
* 对其中一个模块跑 CLI ✓ ⇒ 看是不是**必崩** ✗；
* **判因** ✓：把第 167 轮的深度修复**临时撤掉**跑同一个模块 ✓ ——
  若撤掉后不崩 ✓ ⇒ **是我的修复引入的** ✗ ⇒ 按纪律**撤回该修复**并在台账写明 ✓；
  若撤掉后**照样崩** ✗ ⇒ 与它无关 ✓ ⇒ 再查 `__class__` 那笔（第 144 轮 ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓ 均为**本轮实测** ✓；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读结果 ✓、树干净 ✓）。

#### 第 363 轮：🎯🎯🎯🎯 **病灶本体读到** —— `dispatch_raise` 按 `entry.depth` 恢复，而该深度**没算外层循环**

**① 展开器（`executor.rs:815-829` ✓）** ✓：
```rust
let Some(entry) = handler else { return Err(ExecError::Raised { exception }); };
while frame.depth() > entry.depth { release(instance, frame.pop()?); }   // 弹到 depth ✓
for value in frame.truncate_stack(entry.depth as usize) { … }            // 再截到 depth ✓
…
push(instance, frame, exception)?;
decoder.set_position(entry.target / 2);
```
⇒ `entry.depth` 的语义＝「**进入处理块时要保留的栈深**」✓
⇒ 它**少算**（只算 `handler_depth` ✗，没算外层 `for` 的迭代器＋当前值 ✓）
⇒ 展开时把**多出来的真实项**也当"多余"弹掉 ✗ ⇒ 处理块带着**过少**的栈开跑 ✓
⇒ 里面的 `POP_EXCEPT`／`COPY` 取空栈 ⇒ **`StackUnderflow`** ✓（正是 9 行复现的现象 ✓）。
（注释里还记着第 164 轮修过同一处的另一半 ✓：先前**漏了**这一步 ⇒ 也有 `StackUnderflow` ✓
⇒ 说明这一处历史上就易错 ✓。）
**② 现成的工具（本会话已读过 ✓）** ✓：`emitter.rs:2344` 有
```rust
let for_depth = self.loops.iter().filter(|frame| frame.is_for).count();
```
⇒ **正好**是"外层 `for` 循环数" ✓ ⇒ 修法可以**极小** ✓（用它 ×2 加进 `depth` ✓；
`FOR_ITER` 期间栈上是 `[迭代器, 当前值]` ✓ ⇒ 每层 2 项 ✓ —— 与第 557 行 `2 * (index + 1)` 的
既有约定一致 ✓）。
**③ 下一轮（就一件 ✓）**：按 (b) 落地 —— 在 **`Try` 臂**（`emitter.rs:1589/1605` ✓）把
`self.handler_depth` 换成 `self.handler_depth + 2 * for_depth` ✓（`for_depth` 用 2344 那行的算法 ✓；
若 1589 处拿不到它 ✓ 就就地算 ✓）⇒ 跑：
* `target/m3-repro-loop-try.py` ✓（9 行复现 ✓）
* `target/repro_forelse.py` ✓（enum 形状 ✓）
* **逐字节 4/4** ✓（编译器改动的硬闸门 ✓）、`cargo test --workspace` ✓、对拍两模式 ✓、
  `check.py` 12/12 ✓、夹具 490 ✓；**红了整套撤回并如实记** ✓；
* 通过后跑受管后台重测 ✓（预期 **118 族大幅前进、上限上升** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 361 轮：🎯🎯🎯 **病灶表达式找到** —— `record_exception(…, self.handler_depth, …)` 只算处理器层数 ✗

**① 全部调用点** ✓（本轮 grep ✓）：
```
1396/1397  内层 try（`except` 体里那层 ✓）：`handler_depth` ／ `handler_depth + 1`（finally ✓）
1589/1605  **`Try` 臂**（普通 try ✓）：`handler_depth` ／ `handler_depth + 1`（finally ✓）
1610       （另一处 finally 清理 ✓）
557        `handler_depth + 2 * (index + 1)`（**某种嵌套 try 的补偿** ✓ ⇒ 说明"补偿项"确有先例 ✓）
1460/1461  `handler_depth += 1` / `-= 1`（进/出处理器体 ✓）
```
⇒ 异常表条目的 **`depth` ＝ `handler_depth`（＋ finally 的 +1 ✓）** ✗
⇒ **完全没算外层循环在值栈上的项** ✗
**② 为什么这会崩** ✓（与第 359 轮复现、第 360 轮先例一致 ✓）：
`FOR_ITER` 迭代期间，栈上是 `[… 迭代器, 当前值]` ✓ ⇒ 进入循环体的 `try` 时，
**真实栈深 ≥ 2** ✗，而条目记的 `depth` 是 **0** ✗ ⇒ 异常展开时**少弹/多弹** ✓
⇒ 展开后栈对不上 ⇒ 后面的 `POP_EXCEPT` 取空栈 ⇒ **`StackUnderflow`** ✓（正是复现的现象 ✓）。
**③ 下一轮（就一件 ✓，改动很小 ✓）**：
* 先看 `record_exception` 的**签名与 `depth` 的用法** ✓（本轮已 `grep` 到函数头 ✓，见命令输出 ✓）
  ⇒ 弄清 `depth` 在**展开**时怎么用 ✓（`executor/call.rs` 的 handler 一族 ✓）；
* 再把深度算**对** ✓ —— 两条路，取稳的那条 ✓：
  * **(a) 精确**：在 emitter 里**维护一个值栈深度计数器** ✓（每次 `emit_at` 压栈／弹栈时增减 ✓）⇒
    发 `try` 时用"进入时的真实深度" ✓ —— **最正确**，但要动的地方多 ✗；
  * **(b) 够用**：把 `depth` 加上"**外层 `for` 循环数 × 2**"✓（`LOOP` 期间迭代器＋当前值 ✓）——
    本会话第 557 行已有"`2 * (index + 1)`"这类**乘 2 的补偿**先例 ✓ ⇒ 与之一致 ✓；
* **判据** ✓（两条路都必须过 ✓）：`target/m3-repro-loop-try.py` 通过 ✓、`target/repro_forelse.py` 通过 ✓、
  **逐字节 4/4** ✓（编译器改动的硬闸门 ✓）、`cargo test --workspace` ✓、对拍两模式 ✓、`check.py` 12/12 ✓、
  夹具 490 ✓；红了整套撤回 ✓；通过后跑受管后台重测 ✓（预期 **118 族大幅前进、上限上升** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 359 轮：🎉🎉🎉 **9 行最小复现达成** —— 「循环体里的 `try`」在本层必然 `StackUnderflow` ✗（**通用**形态）

**① 最小复现（`target/bis_F3.py` ✓，另存 `target/m3-repro-loop-try.py` ✓）** ✓：
```python
def g(xs):
    out = None
    for x in xs:
        try:
            out = x.nope
        except AttributeError:
            out = "caught"
    return ("ok", out)

print(str(g([1, 2])))
```
**② 两侧对照** ✓：
```
本层：pyawa: 未捕获（状态 1）：帧操作失败：StackUnderflow        ✗ **崩溃**
参照：('ok', 'caught')                                          ✓
```
**③ 为什么这条最值钱** ✓：
* 它是**通用**形态 ✓ —— 「循环体里包 `try`」在标准库里**遍地都是** ✓（不是某个模块的特殊写法 ✓）；
* 本会话压着的几族都与之相关 ✓：**118 个 enum 族**（`getattr(…, None)` 吞异常 ✓）、
  **77 个 `eval` 族** ✓、**28 个 annotationlib** ✓（目标里 (b)(d) 也有份 ✓）；
* 而且它**崩得干脆**（`StackUnderflow` ✗）⇒ 指向明确 ✓，不是"悄悄返回 None"那种难查的 ✓。
**④ 下一轮（就一件 ✓，进入 Rust 侧）**：读**异常展开**的实现 ✓：
```
grep -n "StackUnderflow" crates/pyawa-core/src --include=*.rs        （找到抛出点 ✓）
grep -n "handler\|handler_stack\|unwind\|exception_table" executor/call.rs executor.rs
```
⇒ 重点看：**从循环体内展开异常时，恢复值栈深度用的是哪个基准** ✗
（最可疑：把**循环的占位项／迭代器**也算进了"要恢复的深度" ✗ ⇒ 于是恢复后少一项 ⇒ `StackUnderflow` ✓）。
**⑤ 判据**（修好后）✓：`target/m3-repro-loop-try.py` 通过 ✓、`target/repro_forelse.py` 通过 ✓、
**逐字节 4/4** ✓、`cargo test --workspace` ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓；
红了整套撤回 ✓；通过后跑受管后台重测 ✓（预期 **118 族大幅前进、上限上升** ✓）。
**⑥ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（复现在 `target/` ✓、树干净 ✓、逐字节 4/4 ✓、`check.py` 12/12 ✓）。

#### 第 357 轮：🎯🎯🎯 **真触发点找到了** —— `try/except` 包住一次「属性缺失」⇒ 本层 **`StackUnderflow` 崩溃** ✗

**① 三个变体的结果（规范判定 ✓，避开表象差异 ✓）** ✓：
```
E（去掉外层 `if __new__ is None:`）       本层 OK  ＝ 参照 ✓   ⇒ 外层 if **必要** ✓
F（`getattr(x, n)` 不带默认值，包 try/except）本层 **帧操作失败：StackUnderflow** ✗ ≠ 参照 OK ✓
G（把两层 for 压成一层）                  本层 OK  ＝ 参照 ✓   ⇒ "两层"**不必要** ✓
```
**② 读法（本轮的决定性收获 ✓）**：
* `F` 是本会话第一次见到的 **`StackUnderflow` 崩溃** ✗ —— 而且是**`try/except` 包住一次属性缺失**✓；
* 原复现里用的是 `getattr(x, name, None)`（**三参** ✓）⇒ 那个"吞掉 `AttributeError`"的动作
  **发生在我们的 `getattr` 内部** ✓ ⇒ 它与其说"吞掉"，不如说**把异常展开的现场搞坏了** ✗
  ⇒ 于是控制流/栈上落下残渣 ✓ ⇒ 最终函数**没走到 return** ⇒ 返回 `None` ✓（与第 349 轮的实测吻合 ✓）；
* ⇒ **真正的病根是"异常展开"（异常表 / `try` 的栈恢复）** ✗ —— 这解释了本会话好几族
  （含目标里的 (b) `eval`／`exec` 重入 ✓ 那条也常与异常栈有关 ✓）。
**③ 下一轮（就一件 ✓，继续往最小化 ✓）**：用 `target/bis_tryattr.py` 那个**6 行**形态 ✓：
```python
class K: pass
try:
    v = K.nope
except AttributeError:
    v = "caught"
print("v =", str(v))
```
⇒ 若它**也崩** ✗ ⇒ 病灶就在"**属性缺失 ⇒ 异常展开**"这条最小路上 ✓ ⇒ 直接读异常展开的实现 ✓
（`executor/call.rs` 的 handler 栈 ✓、第 2 轮修 `break` 时见过的 `finally_stack`／异常表一族 ✓）；
⇒ 若它**不崩** ✓ ⇒ 再往 F 靠（加 `return` 元组 / 加嵌套 for ✓）。
**④ 判据**（修好后）✓：`target/repro_forelse.py` 通过 ✓、**逐字节 4/4** ✓（硬闸门 ✓）、
`cargo test --workspace` ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓；红了整套撤回 ✓；
通过后跑受管后台重测 ✓（预期 **118 族大幅前进、上限上升** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（变体都在 `target/` ✓、树干净 ✓）。

#### 第 355 轮：🎉🎉🎉 **最小复现成功** —— `for…else` ＋ 嵌套 `for` ＋ `break` 的形状在我们这里返回 `None`

**① 复现脚本（`target/repro_forelse.py` ✓，也另存 `target/m3-repro-forelse.py` ✓）** ✓：
```python
def g(member_type, first_enum, classdict):
    __new__ = classdict.get("__new__", None)
    save_new = first_enum is not None and __new__ is not None
    if __new__ is None:
        for method in ("__new_member__", "__new__"):
            for possible in (member_type, first_enum):
                target = getattr(possible, method, None)
                if target not in {None, object.__new__}:
                    __new__ = target
                    break
            if __new__ is not None:
                break
        else:
            __new__ = object.__new__
    if first_enum is None or __new__ in (object.__new__,):
        use_args = False
    else:
        use_args = True
    return __new__, save_new, use_args
```
**② 结果（两侧对照 ✓）** ✓：
```
本层：结果: None        类型: NoneType     ✗
参照：结果: (<built-in method __new__ …>, False, False)  类型: tuple   ✓
```
⇒ **那 118 个模块的墙，如今冻结在一个 ~25 行的脚本里** ✓✓
⇒ 从这一轮起，修这个 bug **不再需要 `enum.py`** ✓（迭代会快很多 ✓，也不会再被"改一句就换墙"干扰 ✓）。
**③ 下一轮（就一件 ✓，用这个复现做二分 ✓）**：把形状逐项简化 ✓，找出**最小触发集** ✓：
1. 去掉 `getattr`（改成直接读属性 ✓）；
2. 去掉集合字面量（`{None, object.__new__}` → 单个比较 ✓）；
3. 去掉外层 `if __new__ is None:` 包裹 ✓；
4. 去掉末尾 `in (object.__new__,)` ✓；
⇒ 每去一项跑一次 ✓ ⇒ 得到**最小**形态 ✓ ⇒ 再据此在编译器里定位并修 ✓
（**判据** ✓：这个复现脚本通过 ✓、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；
通过后跑受管后台重测 ✓，预期 **118 族大幅前进** ✓）。
**④ 顺带** ✓：这个复现值得**沉淀进仓库**（`tests/` 的夹具或 a-new 用例 ✓）——等修好后一并落地 ✓，
让它成为**回归守卫** ✓（本会话的"一处真相"与"完成度如实"都要求这样 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（复现在 `target/` ✓、树干净 ✓）。

#### 第 353 轮：`for` 臂**只压 `loops`、没登记 `break` 目标** ✓（这解释了 `break` 为何落到 `exhausted`）

**① 读到的（`emitter.rs:2694-2699` ✓）** ✓：
```rust
self.loops.push(LoopFrame {
    continue_target: loop_label,
    is_for: true,
    rest: rest.to_vec(),
});
self.in_loop_body = true;
self.emit_block(body, false)?;
```
⇒ `for` 臂**只压了 `loops`** ✓（`continue` 用 ✓），**没有** `block_end_labels.push(…)` ✗
⇒ 于是循环体里的 `break` 走 `block_end_labels.last()` ✓ ⇒ 落到了**外层**某个登记点 ✗
（在 `_find_new_` 这种"`for…else` ＋ 嵌套 `for`"里 ⇒ 正好落到 `exhausted` 那条路 ✓）
⇒ **与第 351 轮的推理完全吻合** ✓。
**② 下一轮（就一件 ✓）**：`grep -n "block_end_labels.push" emitter.rs` ✓（本轮已跑 ✓，见命令输出 ✓）
⇒ 看**哪些构造**会登记 break 目标 ✓（`while`／`try`／`with` … ✓）
⇒ 然后按第 352 轮的三个落点给 `for` 臂**补上登记**（压 `end_label` ✓ 而不是 `exhausted` ✓）。
**③ 判据** ✓：`target/ifmin1.py` 通过 ✓、**逐字节 4/4** ✓（编译器改动硬闸门 ✓）、
`cargo test --workspace` ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓；红了整套撤回 ✓；
通过后跑受管后台重测 ✓（预期 **118 族大幅前进、上限上升** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 351 轮：🎯🎯🎯🎯 **病根定位** —— `for…else` 缺 `end` 标签 ⇒ **`break` 会执行 `else` 体** ✗

**① 读到的（`compile/emitter.rs:2723-2738` ✓）** ✓：
```rust
self.mark_label(exhausted);
self.emit_at(…, "END_FOR", 0);
self.emit_at(…, "POP_ITER", 0);
// 实测：`for … else` 的 else 体紧接 `POP_ITER`（正常耗尽才走到这里）
if !else_body.is_empty() {
    self.emit_block(else_body, false)?;
}
```
**② 病灶** ✓：**没有"else 之后"的标签** ✗ ⇒ 循环里 `break` 的目标只能落在 `exhausted`
（＝"循环耗尽"那一点 ✓）⇒ 于是 **`break` 会顺着执行 `else` 体** ✗ ——
而参照的语义是：`break` **跳过 `else`** ✓（参照的骨架是
`… JUMP_BACKWARD ／ exhausted: END_FOR; POP_ITER; <else> ／ end:` ✓，**`break` 跳 `end`** ✓）。
⇒ 在 `enum.py:1016` 的
```python
for method in ('__new_member__', '__new__'):
    for possible in (member_type, first_enum):
        …
        if … : __new__ = target; break
    if __new__ is not None: break
else:
    __new__ = object.__new__
return __new__, save_new, use_args
```
里 ✓：**内层 `break` 会去执行外层 `for` 的 `else` 体** ✗（本不该 ✓）
⇒ 控制流被搅乱 ✓ ⇒ 最终走到函数尾的隐式 `return None` ✗ ⇒ `_find_new_` 返回 `None` ✓
⇒ 517 行的 3 元解包报"不可迭代" ✗ ⇒ **118 个模块**卡住 ✓ —— **全部对上** ✓✓。
**③ 这会是**第四个真 bug** ✓**（编译器 · `for…else` 的 `break` 目标 ✗），
且与本会话修过的**嵌套 `break` 截断**（第 2 轮 ✓）是**同一片代码**（`emit_rest_and_tail`／`block_end_labels` ✓）。
**④ 下一轮（就一件 ✓，然后跑全闸门 ✓）**：在 `for` 臂里**补一个 `end_label`** ✓：
* 在 `emit_block(else_body, …)` **之后** `self.mark_label(end_label)` ✓；
* 并把**循环体里 `break` 的目标**改成这个 `end_label` ✓
  （即 `block_end_labels` 在 for 臂里**先压 `end_label`** ✓、`exhausted` 不再兼作 break 目标 ✓）；
* **注意**：无 `else` 时 `end_label` 与 `exhausted` 相邻 ✓ ⇒ 行为不变 ✓（**逐字节 4/4** 会替我验证这一点 ✓）。
**⑤ 判据** ✓：`target/ifmin1.py` 通过 ✓、**逐字节 4/4** ✓（编译器改动的硬闸门 ✓）、
`cargo test --workspace` ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓；红了整套撤回 ✓；
随后跑受管后台重测 ✓（预期 **118 族大幅前进 ✓、上限上升 ✓**）。
**⑥ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 349 轮：🎯🎯🎯🎯 **钉到指令级** —— `_find_new_` 在 `END_FOR`／`POP_ITER` 处就"返回"了 ✗

**① `RETURN_VALUE` 探针（门控 ✓，只打 `_find_new_` ✓）** ✓：
```
[return] _find_new_ -> tuple    site=EnumType._find_new_@221   （4 次 ✓ 正常）
[return] _find_new_ -> NoneType site=EnumType._find_new_@128   ✗ ← 失败这次
```
**② unit 128 的 dis 对照** ✓（`enum.py` 的 `_find_new_` ✓）：
```
unit125 POP_TOP              行1028
unit126 JUMP_FORWARD  to L9  行1028
unit127 **END_FOR**          行1016      ← `for…else` 的收尾（1016 行＝`for method in ('__new_member__','__new__'):` ✓）
unit128 **POP_ITER**         行1016      <<< 返回时 ip 停在这条 ✗
unit129 LOAD_GLOBAL object   行1030
```
**③ 结论（本轮的决定性收获 ✓）**：
* `_find_new_` 走的正是 **`for…else` 的 else 路径** ✓（1016→1017→1028 ✓），
  而 `return __new__, save_new, use_args` 在**更后面**（unit 194 附近 ✓ ⇒ **没走到** ✗）；
* 执行 `RETURN_VALUE` 的那一刻，**ip 停在 `POP_ITER`（unit 128）** ✗
  ⇒ 强烈指向 **我们 VM 对 `END_FOR`／`POP_ITER` 的处理** ✗：
  很可能 `END_FOR`（或紧随的清理）**被当成了"帧结束"** ✗ ⇒ 直接走了**隐式 `return None`** ✗
  ⇒ `_find_new_` 返回 `None` ⇒ 517 行的 3 元解包报"不可迭代" ✗ ⇒ **118 个模块**卡住 ✓。
* 这解释了为什么**最小 `for…else` 探针是好的** ✓（简单形态 ✓），
  而这里多了 **嵌套 `for` ＋ `if` ＋ `break`**（与本会话修过的**嵌套 `break` 截断**同族 ✓）。
**④ 下一轮（就一件 ✓）**：读 **`END_FOR`／`POP_ITER`** 的实现 ✓
（`grep -n '"END_FOR"\\|"POP_ITER"' executor.rs` ✓）⇒ 看它们是否错误地**结束帧／跳到函数尾** ✗
⇒ 只在那一处修 ✓；**判据** ✓：`target/ifmin1.py` 通过 ✓、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；
再跑受管后台重测 ✓（预期 **118 族前进/减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了 `RETURN_VALUE` 探针**（门控 ✓，硬闸门：0 警告／4/4／`check.py` 12/12 ✓）。

#### 第 347 轮：🎯🎯🎯 **实测钉死** —— 失败的解包是 517 行（`_find_new_` 返回 `None`）

**① Rust 侧探针（不扰动 Python ✓，`UNPACK_SEQUENCE` 支 ✓）** ✓：
```
[unpack_site] opcode=119 raw_type=tuple    site=EnumType.__new__@328     ← 516 行（`_get_mixins_`，2 元组 ✓ 正常）
[unpack_site] opcode=119 raw_type=tuple    site=EnumType.__new__@350     ← 517 行（`_find_new_`）**有时正常** ✓
[unpack_site] opcode=119 raw_type=NoneType site=EnumType.__new__@350     ← ✗ **同一点这次拿到 `None`**
```
**② 结论（实测 ✓，不再是 dis 推断 ✓）**：
* 失败的是 **`unit 315` ⇒ `enum.py:517`** ✓：
  `__new__, save_new, use_args = metacls._find_new_(classdict, member_type, first_enum,)`
  ⇒ **`_find_new_` 返回了 `None`** ✗；
* 而**同一个点**（`@350` ✓）在别的时候拿到 **tuple** ✓ ⇒ 说明是**按输入不同**、
  **某条分支走到"没有 return"** ✗（或其内部的异常被吞 ✓）。
**③ 于是下一手（下一轮 ✓）**：
* 先看 `_find_new_` 的**分支结构** ✓（`enum.py:997-1038` ✓ 我已读过 ✓：
  `if __new__ is None:` → 双 `for` + `for…else` → 然后 `if first_enum is None or __new__ in (Enum.__new__, object.__new__):` ✓
  → 末尾 `return __new__, save_new, use_args` ✓）⇒ **末尾有 return** ✓ ⇒ 所以只可能是
  **"中途抛错被吞"** ✗ 或 **"某条 `return` 被跳过"** ✗；
* 用**Rust 侧**再打一发：在 **异常被吞** 的那条路（`PYAWA_ATTR_MISS_DEBUG` 已经覆盖属性缺失 ✓）
  加看"**被吞的异常类型/消息**"✗ ⇒ 若它是 `AttributeError` 且在 `_find_new_` 里 ✓ ⇒ 那就对上了
  （第 345 轮探针里 `__new_member__` 的查不到正是在 `_find_new_@61` ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族前进/减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了 UNPACK 现场探针**（门控 ✓，硬闸门见上 ✓）。

#### 第 345 轮：🎯🎯🎯 Rust 侧探针（**不扰动 Python** ✓）一次给出现场链

**① 输出** ✓：
```
[attr_miss] name=__bool__       type=type      site=EnumType._get_mixins_@82
[attr_miss] name=__len__        type=type      site=EnumType._get_mixins_@82
[attr_miss] name=__new_member__ type=type      site=EnumType._find_new_@61      ← ✗ 可疑
[attr_miss] name=__iter__       type=NoneType  site=EnumType.__new__@350        ← 那个 None ✗
pyawa: … TypeError: 'NoneType' object is not iterable
```
**② 读法** ✓：
* `__bool__`／`__len__` 在**类型对象**上查不到 ✓（`type` 本来就没有它们 ✓）⇒ 属**正常**的"查不到" ✓
  （被吞掉 ✓）——这也顺带证明这条探针**不会误报** ✓；
* **`__new_member__` 在 `_find_new_`（`@61`）里查不到** ✗ ⇒ 与 `enum.py:1016` 的
  `for method in ('__new_member__', '__new__'):` 一族有关 ✓ ⇒ **很可能那条路让它提前返回** ✗；
* 然后 `__iter__` 被用在 **`NoneType`** 上 ✓、现场 `EnumType.__new__@350` ✓ ⇒
  与第 321 轮的 dis 对照（`unit 315`＝**517 行** `__new__, save_new, use_args = metacls._find_new_(…)` ✓）**对上** ✓
  ⇒ 即 **`_find_new_` 返回了 `None`** ✗（3 元解包拿到 None ⇒ 报"不可迭代" ✓）。
**③ 下一轮（就一件 ✓，只读 ✓）**：读 `_find_new_` 的完整实现 ✓（本轮已 `grep` 出位置 ✓，见命令输出 ✓）
⇒ 找出它**在什么情况下会走到"没有 return"**（Python 里 ⇒ 返回 `None` ✓）✗
—— 最可能就是 `__new_member__` 那条查表走岔了 ✗（`enum.py:1014` 附近的注释正说"check all possibles ✓"）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族前进/减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了 Rust 侧探针**（门控 ✓，硬闸门见上 ✓）。

#### 第 343 轮：`base` **都有**该属性 ✓；而且**加一条打印就改变了报错** ✗（又见"布局敏感"的影子）

**① 探针（955 行前 ✓，按原文锚点 ✓）** ✓：
```
DBG repr base= <EnumType object at 0x602f4a0f9b40> has= True
DBG repr base= <EnumType object at 0x602f4a0f63a0> has= True
DBG repr base= <EnumType object at 0x602f4a0f63a0> has= True
pyawa: 未捕获（状态 1）：TypeError: 'NoneType' object is not iterable      ← **换墙** ✗
```
⇒ 两个观察（都重要 ✓）：
1. **955 行这条读不会缺属性** ✓（`has= True` ✓）⇒ `'type' object has no attribute '_value_repr_'` ✗
   来自**别的读取点** ✓（`enum.py` 里还有 `1261`／`1560` 的 `self.__class__._value_repr_` ✓
   与 `1764` 的 `etype._value_repr_` ✓）；
2. **只多插了一条 `print`，失败点就从 `_value_repr_` 变回了 `'NoneType' is not iterable`** ✗
   ⇒ 说明**我们的行为对"语句数/局部布局"敏感** ✗ —— 这与第 307／308 轮修掉的
   **融合加载槽号溢出** 是**同一族**（源：`enum.py` 这种函数局部很多 ✓、槽号 ≥16 ✓）
   ⇒ **强烈怀疑还有一处同类**（另一条 opcode 的槽号/半字节打包 ✗，或另一处 ≥16 槽的编码 ✓）。
**② 下一轮（就一件 ✓，换"不插 Python 语句"的探法 ✓）**：**从 Rust 侧**打现场 ✓ ——
在**属性缺失**的抛出点加一发门控打印 ✓（`PYAWA_ATTR_MISS_DEBUG=1` ✓：打印**属性名**与**对象类型** ✓、
以及 `current_site()` ✓ —— 第 332 轮刚把 `current_site` 放宽成 `pub(crate)` ✓ 正好可用 ✓）
⇒ 就能**不扰动 Python 布局**地看到：`_value_repr_` 是在**哪一句**、对**哪个对象**读失败 ✓
（这条探法的价值：它同时能验证"布局敏感"这个怀疑 ✓ —— 若同一个用例在**不加任何 Python 语句**时
也报 `_value_repr_` ✓，那就说明扰动确实来自"我们自己的布局处理" ✗）。
**③ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改副本并已还原 ✓、树干净 ✓）。

#### 第 342 轮：❌ 「建类丢属性」假设**也被否掉** ✓（`_value_repr_ in __dict__ == True`）⇒ 缺属性的是**另一个类**

**① 探针（按原文锚点 ✓，插在 550 之后 ✓）** ✓：
```
DBG vrepr in dict: True None
DBG vrepr in dict: True None
DBG vrepr in dict: True None
DBG vrepr in dict: True None        ← 4 次都 `True` ✓（值 `None` ✓ —— 空 bases 时 `_find_data_repr_` 就该给 None ✓）
```
⇒ **`_value_repr_` 确实进了类型字典** ✓ ⇒ **建类这一步是好的** ✗
⇒ 第 341 轮「`type.__new__` 没把 classdict 项带进去」的假设**不成立** ✓（**又是一次"先探针后结论"救回来** ✓）。
**② 收窄** ✓：报错的是 **`'type' object has no attribute '_value_repr_'`** ✗ ⇒
* 被读的对象**是类型对象** ✓（所以不是那个占位 `None` ✓ —— 那会报 `'NoneType' …` ✓）；
* 而且它**不是**刚才这 4 个（它们都有 ✓）⇒ 是**第 5 个类** ✓：某条**没走 537 行**的建类路径 ✗
  （`enum.py` 里还有 `_simple_enum`／`global_enum` 一类 ✓，以及 `EnumType.__new__` 的
  **`_simple`** 早退分支（487 行 ✓ `if _simple: return super().__new__(…)` ✓ —— **它跳过了 537 行那一整片写** ✗ ✓✓）。
**③ 下一轮（就一件 ✓，Python 层探针 ✓）**：在 **955 行**（`return base._value_repr_`）前打印 `str(base)` ✓
```python
                    print("DBG repr base=", str(base), "has=", '_value_repr_' in base.__dict__)
                    return base._value_repr_
```
⇒ 一次就看出**是哪个类**在缺 ✓ ⇒ 再看它是**怎么被创建的** ✓（是不是走了 `_simple` 那条 ✓）
⇒ 那就对上"**参照在那条路上本来就会建出带该属性的类**"✗ 还是"**我们把它建歪了**"✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改副本并已还原 ✓、树干净 ✓）。

#### 第 339 轮：`_value_repr_` 的**定义点与读取点**都找到了 ✓

**① grep 结果（`enum.py` ✓）** ✓：
```
537:  classdict['_value_repr_'] = metacls._find_data_repr_(cls, bases)     ← 写进**类命名空间** ✓
954-955:  # if we hit an Enum, use it's _value_repr_
          return base._value_repr_                                        ← 在某个 **base（类型对象）** 上读 ✓
1261/1560: v_repr = self.__class__._value_repr_ or repr                    ← 经 `__class__` 读 ✓
1764:  body['_value_repr_'] = etype._value_repr_                           ← 另一处读 ✓
```
**② 读法** ✓：错误是 **`'type' object has no attribute '_value_repr_'`** ✗
⇒ 说明**被读的那个对象是"类型对象"** ✓，但它身上**查不到** `_value_repr_` ✗
（而 537 行**已经**把它写进 `classdict` ✓ 了 ⇒ 那个 classdict 属于**另一个类** ✓，
或者写进去之后**没进到最终的类型字典** ✗ ⇒ 两种可能都要看 ✓）。
**③ 下一轮（就一件 ✓，Python 层探针 ✓）**：在 954-955 处打印 `str(base)` ✓：
```python
                    print("DBG repr base=", str(base))
                    return base._value_repr_
```
⇒ 看出**是哪个类**在缺这个属性 ✓ ⇒ 再对照 537 行那个 classdict 是**哪个类**的 ✓
⇒ 就能判断是"**属性写进 classdict 没生效**"✗ 还是"**读的类不对**"✓。
（顺带：536 行附近还有 `_member_names_`／`_member_map_` 一族同一批写 ✓ —— 若"写 classdict 没生效"✗，
那这一族会**成片**出问题 ✓，与本会话"一处真相"的修法一致 ✓。）
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族前进** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 336 轮：🎯 读到 `_get_mixins_` 真身 —— `(object, None)` 说明**全局名 `Enum` 解析成了 `None`** ✗

**① 原文（`enum.py:929` ✓）** ✓：
```python
    def _get_mixins_(mcls, class_name, bases):
        if not bases:
            return object, Enum          # ← 空 bases 的分支：第二个是**全局名 `Enum`** ✓
        first_enum = bases[-1]
        if not isinstance(first_enum, EnumType):
            raise TypeError(…)
        member_type = mcls._find_data_type_(class_name, bases) or object
        return member_type, first_enum
```
**② 推理（本轮的决定性收获 ✓）**：我们的探针给出 `(object, None)` ✗ ⇒
对照原文，**第一项 `object` 对 ✓** ⇒ 说明走的是 `if not bases:` 那条 ✓、
**而第二个位置上的 `Enum` 求值成了 `None`** ✗ ⇒ 也就是说：
**对"尚未绑定的全局名"（或刚被 `del` 掉的全局名 ✓）做了全局查找，本层给了 `None`** ✗
（参照在这个位置要么拿到类 ✓、要么抛 `NameError` ✗ —— **不会**静默给 `None` ✓）
⇒ 这与本会话早先那条"**未绑定局部报 `UnboundLocal`**"（第 291 轮读到 `LOAD_FAST` 的行为 ✓）形成对照 ✓：
**未绑定的全局**那条路似乎**静默给 `None`** ✗ ⇒ **第四个真 bug 的高度可疑点** ✓。
**③ 下一轮（就一件 ✓，Python 层探针即可 ✓ 不用重编译 ✓）**：在 `_get_mixins_` 开头插打印 ✓
（`print("DBG gmix", str(bases), str(Enum))` ✓）⇒ 看清 `bases` 与 `Enum` 的实际取值 ✓；
再用一个**最小脚本**验证"未绑定全局读取"的行为 ✓：
```python
try:
    print("undefined global:", str(UNDEFINED_NAME_GLOB))
except NameError as e:
    print("NameError:", str(e))
```
⇒ 若本层给 `None` ✗ ⇒ 立即修**全局查找**那条路 ✓（`LOAD_GLOBAL`／`LOAD_NAME` ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族前进** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 335 轮：❌ 负下标假设被**自己的探针否掉** ✓（本层 `IndexError` 正确）⇒ 改读 `_get_mixins_` 原文

**① 探针结果（`target/negidx.py` ✓）** ✓：
```
本层：t[-1] → IndexError: tuple index out of range ✓ ；l[-1] → 2 ✓ ；l[-3] → IndexError: list index out of range ✓
参照：完全相同 ✓
```
⇒ **本层的负下标越界行为是对的** ✗ ⇒ 第 334 轮"负下标返回 `None`"的假设**不成立** ✓（如实更正 ✓，
而且是**我自己的探针**推翻的 ✓ —— 这就是"先探针后下结论"的价值 ✓）。
**② 于是回到 `_get_mixins_` 原文** ✓（本轮已 `grep` 出位置 ✓，见命令输出 ✓）
⇒ 下一轮读它的**完整实现** ✓，看它在什么情况下会返回 `None` 作为第二项 ✗
（也可能是它**内部**调用的别的东西给了 `None` ✓ —— 例如 `mcls._find_data_type_(…)` ✓ 或某个 `bases[0]` 分支 ✓）。
**③ 顺便**：小例现在的墙是 **`'type' object has no attribute '_value_repr_'`** ✓（第 334 轮看到 ✓）
⇒ 那也是**独立的一条缺口** ✓（`Enum` 类体里 `_value_repr_ = …` 的读取 ✓），
两条可以分别推进 ✓（**先修便宜的那条** ✓ —— 若 `_value_repr_` 是"类型对象的属性读取"一类 ✓ 很可能便宜 ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 334 轮：🎯🎯🎯 **真凶=负下标越界** —— 空元组 `bases[-1]` 没报 `IndexError`，而是给了 `None` ✗

**① 探针（按 grep 到的**原文**签名做锚点 ✓ —— 第 333 轮教训生效 ✓）** ✓：
```
DBG bases: () cls: Enum                     ← 🎯 `bases` 是**空元组** ✓（不是"含 None" ✗）
pyawa: 未捕获（状态 1）：AttributeError: 'type' object has no attribute '_value_repr_'   ← 又换了一堵墙 ✓
```
**② 推理（本轮的决定性收获 ✓）**：`enum.py` 的 `_get_mixins_` 头一句就是
```python
first_enum = bases[-1]
```
⇒ 当 `bases == ()` ✓（`class Enum(metaclass=EnumType)` ✓ 本来就是空的 ✓）时，
参照会**抛 `IndexError`**（❗这是 `enum.py` 里有意为之的守卫 ✓ hmm ✗ —— 更准确地说：
参照里这条 `bases[-1]` 只在 `if not bases:` 分支**之后**才走 ✓；无论哪种，
**负下标越界必须报 `IndexError`** ✓ 是硬语义 ✓）
⇒ 而**本层给了 `None`** ✗ ⇒ 于是：
`first_enum = None` ✗ ⇒ `_get_mixins_` 返回 `(object, None)` ✗ ⇒ **第 331 轮那个 `(object, None)` 完全解释通** ✓✓
⇒ 即**第四个真 bug：负下标越界没有报错、返回了 `None`** ✗（`subscript.rs` 那条路 ✓）。
**③ 本轮顺手做的越界探针（`target/negidx.py` ✓，结果见命令输出 ✓）**：
```python
t = (); t[-1]                 # 参照：IndexError ✓
l = [1, 2]; l[-1]             # 参照：2 ✓
l[-3]                         # 参照：IndexError ✓
```
**④ 下一轮（就一件 ✓）**：按探针结果去 `executor/subscript.rs` ✓ 的**负下标归一化**处修 ✓
（本会话修过"负下标"一类 ✓ —— `normalize_index` 就在 `executor/ctrls.rs:52` ✓ 返回 `Option` ✓
⇒ 只要**越界一律报 `IndexError`** ✓、**不要**回落到 `None` ✗ 即可 ✓）；
**判据** ✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改副本并已还原 ✓、树干净 ✓）。

#### 第 332 轮：✅ 补上**类型对象的 `__class__`**（第八处能力缺口）

**① 改动（一处，纯插入 ✓，`executor/attribute.rs` 的 `__name__` 分支之前 ✓）** ✓：
```rust
if name == "__class__" && instance.is_type_object(object) {
    if let Some(ty) = instance.type_named("type") {
        return Ok(Attribute::Owned(ty.cast::<Header>()));
    }
}
```
（参照里**类型对象**的 `__class__` 就是 `type` ✓；`enum.py` 的 `_find_new_` 要读它 ✓ ⇒ 第 331 轮那族卡在这 ✗。）
**② 证据** ✓：
```
改前：AttributeError: 'EnumType' object has no attribute '__class__'          ✗
改后：TypeError: 'NoneType' object is not iterable                            ← **换墙** ✓（`__class__` 缺口关掉 ✓）
```
**③ 四条硬闸门** ✓：**0 警告** ✓、逐字节 **4/4** ✓、workspace ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓。
**④ 下一轮（就一件 ✓）**：回到第 331 轮的另一半 —— **`bases` 里那个 `None`** ✗
（`_get_mixins_` 给 `(object, None)` ✓）⇒ 在 `build_class_native`／`__build_class__` 一路打一发门控打印 ✓
看 `bases` 元组里到底是哪一项、以及**类体执行完的收尾**是怎么把基类填进去的 ✓
（这很可能是**第四个真 bug** ✓ —— 类创建路径 ✓、解释力强 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓。

#### 第 331 轮：🎯 拆分打印成功 —— `_get_mixins_` 给的是 `(object, None)`（`first_enum` 为 `None` ✗）＋ 类型对象缺 `__class__` ✗

**① 输出** ✓（副本插桩 ✓，跑完还原 ✓）：
```
DBG mixins: (<class 'object'>, None)
pyawa: 未捕获（状态 1）：AttributeError: 'EnumType' object has no attribute '__class__'
```
**② 两条结论（都很具体 ✓）**：
1. **`_get_mixins_` 返回 `(object, None)`** ✗ —— 参照里第二个应当是**枚举基类**（`IntFlag` 一类 ✓）✓。
   它的实现是 `first_enum = bases[-1]` ✓ ⇒ 所以**我们传进去的 `bases` 里有一个 `None`** ✗
   （即**类创建时把 `bases` 搞坏了** ✗ —— 这是**第四个真 bug 的候选** ✓，且解释力强 ✓）；
2. 紧接着报 **`'EnumType' object has no attribute '__class__'`** ✗ ⇒ **类型对象上取不到 `__class__`** ✗
   （参照里 `type.__class__` 是 `type` ✓）⇒ 这也是一条**独立的能力缺口** ✓（很可能一个注册就能补 ✓，
   与第 241／271 轮给 `object`／`NoneType` 补 `__str__` 同一套路 ✓）。
**③ 下一轮（就一件 ✓，两条都可判 ✓）**：
* 先补 `__class__`（便宜 ✓、无副作用 ✓）：在类型对象的属性读取处 ✓（`executor/attribute.rs` 的
  `is_type_object` 一族 ✓，第 244 轮加 `__mro__` 就在那里 ✓）加一条
  `if name == "__class__" && is_type_object(object) { return Ok(Attribute::Owned(<type 类型对象>)) }` ✓；
* 再看 `bases` 里那个 `None` ✗（在 `build_class_native`／`__build_class__` 一路打一发打印 ✓）。
**判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族再前进/减少** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改副本并已还原 ✓、树干净 ✓）。

#### 第 330 轮：带注释的锚点**命中** ✓，但第二个锚点写法不对（`count=0`）⇒ 脚本仍未写盘

**① 本轮结果（如实 ✓）**：
* **第一个锚点（带注释 `# data type of member and the controlling Enum class` ＋ 那一句 ✓）`count == 1` ✓ 命中** ✓
  ⇒ 「带上下文行做锚点」这条教训**有效** ✓；
* **第二个锚点**（`_find_new_` 那一句，我按三行原样写 ✓）`count == 0` ✗ ⇒ 实际源码的**换行/缩进与众不同** ✗
  ⇒ 断言在 `f.write_text` **之前**中止 ✓ ⇒ **副本未改动** ✓（已还原 ✓）。
**② 改法（下一轮 ✓，更简 ✓）**：**只插第一处** ✓ —— 只要知道 `_get_mixins_` 有没有给 `None` ✓
就足以判断"是哪一个"✓；`_find_new_` 那处等需要时再单独处理 ✓（先 `sed -n` 看清它的真实换行 ✓，
本会话已立此规矩 ✓）。
**③ 下一轮（就一件 ✓）**：
```python
        # data type of member and the controlling Enum class
        _mt = metacls._get_mixins_(cls, bases)
        print("DBG mixins:", str(_mt))
        member_type, first_enum = _mt
```
⇒ 若打印出的是**元组** ✓ ⇒ 病在 `_find_new_` ✓；若是 **None** ✗ ⇒ 进 `_get_mixins_` 找 ✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（脚本未写盘 ✓、树干净 ✓）。

#### 第 329 轮：拆分句插桩**没写进去**（锚点重复 2 次 ✗）⇒ 下一手用**带上下文的唯一锚点**

**① 发生了什么（如实 ✓）**：我想把 `enum.py` 516/517 两句拆开并打印 ✓，第一条锚点
```python
        member_type, first_enum = metacls._get_mixins_(cls, bases)
```
`count` 竟是 **2** ✗（同形的调用在 `enum.py` 里**不止一处** ✓ —— 早先 grep 也显示 473 行有同款 ✓）
⇒ 断言在 `f.write_text` **之前**中止 ✓ ⇒ **副本没改动** ✓（且它本来就是副本 ✓，跑完已还原 ✓）。
**② 教训（第 N 次同源 ✓）**：**短句锚点会撞车** ✗ ⇒ 以后拆分/插桩一律带**上下文行**做锚点 ✓
（比如把上一行的注释 `# data type of member and the controlling Enum class` 一起写进锚点 ✓。
本文件里恰好有这句注释 ✓ ⇒ 天然唯一 ✓）。
**③ 下一轮（就一件 ✓）**：用带注释的锚点重做那次拆分打印 ✓：
```python
        # data type of member and the controlling Enum class
        _mt = metacls._get_mixins_(cls, bases)
        print("DBG mixins:", str(_mt))
        member_type, first_enum = _mt
```
（`_find_new_` 那处同理 ✓，它带 `__new__, save_new, use_args =` 这个独特前缀 ✓ 已经唯一 ✓）
⇒ 一眼看出**哪个 classmethod 给了 `None`** ✗ ⇒ 再进那个函数体找 ✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（脚本未写盘 ✓、树干净 ✓）。

#### 第 328 轮：🎯🎯🎯 **两行都指到 classmethod** —— `_get_mixins_` / `_find_new_` 返回了 `None`

**① 精确映射（`dis.findlinestarts` ✓，不再用坏掉的 `starts_line` ✓）** ✓：
```
unit295 → 行 516：member_type, first_enum          = metacls._get_mixins_(cls, bases)
unit315 → 行 517：__new__, save_new, use_args      = metacls._find_new_(classdict, member_type, first_enum, …)
```
⇒ **两次解包的目标都是"某个 classmethod 的返回值"** ✓，而它们给了 `None` ✗
⇒ 即 **`_get_mixins_`／`_find_new_` 在本层返回 `None`** ✗（参照里它们都返回**元组** ✓）。
**② 于是本轮顺手做的探针（`target/cmprobe.py` ✓，结果见命令输出 ✓）**：
```python
class K:
    @classmethod
    def cm(cls):
        return (1, 2)
print("via class:", str(K.cm()))
print("via inst:", str(K().cm()))
```
⇒ 用来分清"**classmethod 一律坏**"✗ 还是"**只有它们两个**"✓（下一轮据此收窄 ✓）。
**③ 下一轮（就一件 ✓，仍是副本插桩 ✓）**：在 `enum.py` 里给这两句**前面**加打印 ✓
（`print("DBG mixins:", str(metacls._get_mixins_(cls, bases)))` ✗ 会重复调用 ✓
⇒ 更稳的是把它们**拆成两行**：先 `_mt = metacls._get_mixins_(cls, bases)` ✓、打印 `str(_mt)` ✓、
再解包 ✓）⇒ 一眼看出**哪个**给了 `None` ✗、以及**调用是否真的执行了**（有没有打印）✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族再前进** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 326 轮：✓ opcode 探针**一次就分出**（`opcode=119` ⇒ `UNPACK_SEQUENCE`）；并列出该函数里两处解包

**① 探针（门控 ✓，插在兜底里 ✓）** ✓：
```
[unpack] opcode=119 走迭代器兜底        ← 119 = UNPACK_SEQUENCE ✓
[iter] 不可迭代：type=NoneType site=EnumType.__new__@350
```
⇒ 走的**不是**下标赋值／import 那条 ✓，而是 **`UNPACK_SEQUENCE`**(119) ✓ ⇒ "解包的来源是 `None`" ✗。
**② 该函数里两处解包** ✓（dis ✓，**不用重编译** ✓）：
```
unit 295 UNPACK_SEQUENCE ／ unit 315 UNPACK_SEQUENCE
```
（本轮把它们的**源码行**也打了出来 ✓，见命令输出 ✓ —— 下一轮据此读那两句 ✓。）
**③ 下一轮（就一件 ✓）**：读那两句源码 ✓ ⇒ 看**哪个表达式的值**应当可迭代却成了 `None` ✗
⇒ 常见嫌疑：某个**方法在本层返回了 `None`** ✗（本会话已修过 `set.pop`／`NoneType.__str__`／
数据类型 `__new__` 一类 ✓）或某个**分支算错**（例如 `if` 的某条路没赋值 ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族再前进** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（探针保留 ✓、四条硬闸门 ✓）。

#### 第 324 轮：`dict.items()` **是好的** ✗（排除）⇒ 看"谁在调 `sequence_items`"

**① 探针结果（`target/dictitems.py` ✓）** ✓：
```
本层：items: [('a', 1)] ／ keys: ['a'] ／ values: [1]        ✓ **可迭代** ✓（表观与参照不同 ✗：参照是 `dict_items(…)` ✓）
参照：items: dict_items([('a', 1)]) ／ keys: dict_keys(['a']) ／ values: dict_values([1])
```
⇒ **`dict.items()` 本身没问题** ✗（第 323 轮的猜测被否 ✓），只是**表观差异** ✓（记下 ✓，不属本轮）。
**② 于是下一手（本轮的产出 ✓）**：`sequence_items` 的**调用方清单** ✓（本轮已 `grep` ✓，见命令输出 ✓）
⇒ 因为 `sequence_items` 是**解包专用** ✓，那个 `None` 只能来自**某次解包的目标** ✓；
把它所有调用点列出来 ✓ ⇒ 与 `EnumType.__new__@350` 附近**没有**可见的 `UNPACK_*` 这一点对上 ✓
（第 321 轮的字节码里 342-357 只有 `CALL`／`STORE_SUBSCR`／`JUMP_BACKWARD` ✓）
⇒ 说明**解包发生在 `CALL` 内部** ✓（即**调用机制自己**在解包：如 `*args`／关键字展开 ✓）
⇒ 方向：**我们的调用机制**（`call_callable`／类实例化）在某处**解包一个 `None`** ✗
（很可能是 `*args` 的实参元组是 `None` ✗，或 `CALL_FUNCTION_EX` 那条路 ✓）。
**③ 下一轮（就一件 ✓）**：按调用点清单逐个看 ✓（尤其 `call.rs` 里的那几处 ✓）⇒ 定位"哪一次解包"
⇒ 再问"它的目标为什么是 `None`"✓ ⇒ 修 ✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 323 轮：🎯🎯🎯 **回溯点出真凶链** —— 是"解包目标"本身为 `None`；`enum.py` 那处是 `classdict.items()` 一类

**① 回溯（门控 `PYAWA_ITER_DEBUG=1` ✓）** ✓：
```
[iter] 不可迭代：type=NoneType site=EnumType.__new__@350
   0: pyawa_core::executor::iter::iter_value
   2: pyawa_core::executor::ctrls::sequence_items        ← 🎯 **我新写的迭代兜底**（UNPACK_SEQUENCE）
   3: pyawa_core::executor::execute
   5: pyawa_core::executor::call::call_callable
   7: pyawa_core::classes::build_class_native
  11: pyawa_core::executor::import::load_module
```
**② 读法** ✓：`sequence_items` 是**解包**用的 ✓ ⇒ 说明"**被解包的那个值**是 `None`" ✗
⇒ 也就是说：`enum.py` 里那处 `for … in <表达式>` 的 `<表达式>` 给出了 `None` ✗
—— 从 342-357 的字节码看（`STORE_FAST value` / `_proto_member` / `STORE_SUBSCR` / `JUMP_BACKWARD` ✓）
那是一个 `for name, value in classdict.<something>()` ✓ ⇒ **最可能是 `classdict.items()` 返回了 `None`** ✗。
**③ 本轮顺手做的探针** ✓（`target/dictitems.py` ✓，结果见命令输出 ✓）：
```python
d = {"a": 1}
print("items:", str(d.items()))
print("keys:", str(d.keys()))
print("values:", str(d.values()))
```
⇒ 下一轮据此定：若 `items()` 给 `None` ✗ ⇒ 修 `dict.items` 一族 ✓（那是**能力缺口** ✓、且很可能一次解一大片 ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族再前进** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了带回溯的探针**（门控 ✓，四条硬闸门见上 ✓）。

#### 第 320 轮：🎯🎯🎯 **拿到精确现场** —— `TypeError` 来自 `EnumType.__new__@350`（`enum.py`）

**① 改动（两处，都是精确整行/整句替换 ✓、不用正则 ✓）** ✓：
```
crates/pyawa-core/src/instance.rs  ：`    fn current_site` → `    pub(crate) fn current_site`（只改这一行 ✓）
crates/pyawa-core/src/executor/iter.rs：`Ok(None) => { let name = …; }` 之后加一发门控打印 ✓
                                        （`PYAWA_ITER_DEBUG=1` ✓：打印类型名 ＋ `current_site()` ✓）
```
**② 实测（一次就中 ✓）** ✓：
```
[iter] 不可迭代：type=NoneType site=EnumType.__new__@350
pyawa: 未捕获（状态 1）：TypeError: 'NoneType' object is not iterable
```
⇒ **`None` 是在 `enum.py` 的 `EnumType.__new__` 里、unit 350 处被拿去迭代的** ✓
（函数码元的 `offset × 2` 换算**可靠** ✓ —— 第 115 轮验证过 ✓ ⇒ 即**字节 700** ✓）。
**③ 四条硬闸门（这次先读再提交 ✓，第 315 轮立的规矩 ✓）** ✓：见上（警告数／逐字节／`check.py`／对拍 ✓）。
**④ 下一轮（就一件 ✓）**：把 **unit 350** 对到源码行 ✓：
```python
python3 - <<'PY'
import dis, types
src = open("target/lib-full/enum.py").read()
code = compile(src, "enum.py", "exec")
def find(c, name):
    for k in c.co_consts:
        if isinstance(k, types.CodeType):
            if k.co_name == name: return k
            r = find(k, name)
            if r: return r
f = find(code, "__new__")
for i in dis.get_instructions(f):
    if 320 <= i.offset//2 <= 380:
        print(f"byte{i.offset} unit{i.offset//2} {i.opname} {i.argrepr[:30]} 行{i.starts_line}")
PY
```
⇒ 看到那一句之后 ✓，就知道是**哪个值**该给出来却给了 `None` ✗ ⇒ 再修 ✓（判据同前 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓。

#### 第 318 轮：插桩补丁**又写坏了**（插到模块层 ⇒ 12 个编译错）⇒ 整套撤回 ✓

**① 我干了什么（如实 ✓）**：想在 `executor/iter.rs` 里那条 `is not iterable` **之前**插一发门控打印 ✓
（打印类型名 ＋ `instance.current_site()` ✓）⇒ 我用"往前找最近的 `let message`／`Err(`"来定位插入点 ✓
⇒ 结果插入点落到了**函数之外**（模块层 ✓）⇒ `error: expected item, found keyword 'if'` ✗
＋ 连带 `executor/format.rs` 的导入解析全崩 ✗（共 **12 个编译错** ✗）。
**② 处置** ✓：`git checkout -- crates/` **整套撤回** ✓ ⇒ 复核 **0 警告** ✓、逐字节 **4/4** ✓、树**干净** ✓。
**③ 教训（第三次同类 ✓）**：**"往前找最近的某个片段"来定插入点**这个套路在本文件上**不可靠** ✗
（同样的手法已在第 294／307 轮翻过车 ✓）⇒ 以后插桩一律：
* **先 `sed -n` 把目标函数体打出来** ✓、**看清缩进与边界** ✓；
* 插入点用**紧邻的完整语句**（含其缩进 ✓）当锚点 ✓、**不再用"最近的 `Err(`"这种模糊定位** ✗；
* 改完**先 `cargo check` 一遍**再谈跑 ✓（本轮我是"改完直接 build 整个 bin"⇒ 报错一次性涌出 ✓，虽读了报错但已经浪费一轮 ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（补丁已撤 ✓）。

#### 第 317 轮：新墙仍**在 `enum` 执行里**（`types` 已通 ✓）⇒ 下一手：给 `is not iterable` 的抛出点打**当前现场**

**① 导入链（`PYAWA_TRACE_IMPORT=1` ✓）** ✓：
```
[载入] 试 …/_types.py … ／ target/_types/__init__.py ／ Lib/_types.py ／ Lib/_types/__init__.py
[载入] 模块 types 执行完：命名空间 37 个名字        ✓ **types 通了** ✓
[载入] 模块 enum 执行出错：Raised { exception: … }  ← 🎯 又是在 **enum** 里 ✗
[载入] 模块 enum 执行完：命名空间 37 个名字
pyawa: … TypeError: 'NoneType' object is not iterable
```
⇒ 与"同一批 118 个模块"吻合 ✓：它们都卡在 **`enum`** 这条链上 ✓（`enum` 一挂，`re`／`argparse`／`asyncio` 全挂 ✓）。
**② 下一手（就一件 ✓）**：那条 `TypeError: 'X' object is not iterable` 是**我们**在
`executor/iter.rs` 的 `iter_value` 里发的 ✓ ⇒ 在那里加一发**门控打印** ✓（`PYAWA_ITER_DEBUG=1` ✓）：
打印**对象的类型名** ✓ ＋ **当前 Python 现场**（`instance.current_site()` ✓ —— 这函数在 core 内部可用 ✓，
以前从 `executor/call.rs` 调时因为可见性报过错 ✓ ⇒ 在 `iter.rs` 里试 ✓）⇒
一次就能得到**`enum.py` 的那一行** ✓ ⇒ 再看那一行为什么拿到 `None` ✗。
**③ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族再前进/减少** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 316 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只跑了一条追踪 ✓、树干净 ✓）。

#### 第 316 轮：✅ 迭代器兜底生效（那 118 个**又往前挪一格**）—— 新头号＝`'NoneType' object is not iterable`（118）

**① 受管作业实测（03:26／03:28 ✓）** ✓：
```
上限诊断：能 import **162** 个（25.8%）（未动 ✓）
     118  TypeError: 'NoneType' object is not iterable          ← 🎯 **同一批 118 个模块** ✓（换了措辞 ✓）
      77  NameError: name 'eval' is not defined
      28  SyntaxError：annotationlib（第 327 行 列 18-20）
      19  ModuleNotFoundError: No module named '_struct'
       9  ModuleNotFoundError: No module named 'binascii'
       8  NameError: name 'complex' is not defined
       8  ImportError: cannot import name 'getDOMImplementation' from 'xml'
       7  AttributeError: 'module' object has no attribute 'warnoptions'
       7  ModuleNotFoundError: No module named '_codecs_jp'
       7  ModuleNotFoundError: No module named '_codecs_iso2022'
       6  TypeError: bad operand type for unary -: 'timedelta'      ← 新露出来的族 ✓
       6  ImportError: cannot import name 'open' from 'builtins'    ← 新露出来的族 ✓（内建缺 `open` ✗）
       6  SyntaxError：加载模块 'typing'：`class` 后面要冒号       ← 新露出来的族 ✓（**编译器**类 bug ✓）
判据①：**通过 155 ＋ 参照口径 17 ＝ 172 ÷ 628 ⇒ 27.4%** ✓（未动 ✓）／进度 **55.1%** ✓
```
**② 读法** ✓：
* `UNPACK_SEQUENCE` 那条 `Unsupported` **不再出现** ✓ ⇒ 第 314 轮的迭代器兜底**生效** ✓；
* 同一批 118 个模块**往前挪了一格** ✓：现在是 **`for x in None`** 那种
  `TypeError: 'NoneType' object is not iterable` ✗ ⇒ 说明**更上游**有表达式给出了 `None` ✓
  （参照语义下这条错误**本身是对的** ✓ ⇒ 要找的是"**谁给了 None**" ✗）。
**③ 下一轮（就一件 ✓，正好便宜 ✓）**：**小例就能复现它** ✓（`target/ifmin1.py` 现在报的就是这一条 ✓）
⇒ 用**副本插桩**或 `PYAWA_TRACE_IMPORT=1` ✓，定位"哪个表达式是 `None`"✓
（另一条更快的路：先看 `enum.py` 里哪些 `for` 会拿到可能为 `None` 的东西 ✓）。
**④ 顺带记下三条新族** ✓（下一批候选 ✓）：内建缺 **`open`**（6 ✓）、`timedelta` 的一元负号（6 ✓）、
`typing` 的 **`class` 后面要冒号**（6 ✓ —— 这条像**我们编译器/解析器**的问题 ✓，值得单列 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓ 均为**本轮实测** ✓；
**未声称任何阶段完成** ✓。

#### 第 314 轮：✅ 给 `UNPACK_SEQUENCE` 接上**迭代器协议**（兜底不再报 `Unsupported`）

**① 改动（一处，`sequence_items` 的末尾兜底 ✓；自包含一段、一次写盘 ✓）** ✓：
```rust
// 兜底：走迭代器协议（tuple／list／set／str 已在上面走快路）
let iterator = instance.iter_object(raw)?;                 // 新引用 ⇒ 我方持有 ✓
let mut collected: Vec<NonNull<Header>> = Vec::new();
let outcome = loop {
    match instance.advance_iterator(iterator) {
        Ok(Some(item)) => collected.push(item),            // 元素是新引用 ⇒ 直接收 ✓
        Ok(None) => break Ok(()),
        Err(error) => break Err(error),
    }
};
instance.release(iterator);                                // 迭代器那份要还 ✓
outcome?;
Ok(collected)
```
**② 证据** ✓：
```
改前：pyawa: 未捕获（状态 5）：指令 119 的这个形态尚未接线：解包只接线了 tuple／list／str（迭代器协议未接线）  ✗
改后：pyawa: 未捕获（状态 1）：TypeError: 'NoneType' object is not iterable                                  ← **换墙** ✓
逐字节 4/4 ✓ ；0 错 0 警告 ✓
```
⇒ `UNPACK_SEQUENCE` 的 `Unsupported` 缺口**关掉了** ✓；新错误是**参照语义下也会有的**那条 ✓
（`for x in None` 本来就要报 `TypeError` ✓）⇒ 说明**更上游**有某个表达式给出了 `None` ✗
（那属于下一堵墙 ✓，不是本次改动的错 ✓）。
**③ 闸门** ✓：见上（0 警告 ✓、workspace ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓）。
**④ 下一轮（就一件 ✓）**：沿用"**副本插桩**"这招 ✓（本会话最有效的一招 ✓）——
`grep -n "is not iterable"` 找到那句抛出点 ✓、再在 `target/lib-full` 里定位到底是**哪个表达式**给了 `None` ✓；
**或**先用"受管后台重测"看这堵墙换得值不值 ✓（这一步便宜 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 309 轮实测 ✓，**新数字待重测** ✓）；
**未声称任何阶段完成** ✓。

#### 第 313 轮：兜底补丁**没写进去** ✗（锚点断言失败）—— 下一手改成**自包含**写法

**① 本轮发生了什么（如实 ✓）**：我写的补丁分两步（① 替换末尾的 `Err(Unsupported{…})` ✓、
② 在函数开头插 `let mut items_owned = Vec::new();` ✓）；**第 ② 步的锚点没命中** ✗
（`let ty = unsafe { raw.as_ref() }.ty();` 这句断言 `count == 1` 失败 ✗）
⇒ 脚本在 `f.write_text` **之前**就抛了 ✓ ⇒ **树未改动** ✓（复核：0 错 ✓、逐字节 4/4 ✓、`git status` 干净 ✓）。
**② 教训与改法（下一轮照做 ✓）**：
* **不要分两步跨文件位置改** ✗ —— 把兜底写成**自包含的一段** ✓（在兜底处**就地声明**那个 `Vec` ✓、
  就地循环 ✓、就地 `release(iterator)` ✓）⇒ **一次替换** ✓、不碰函数开头 ✓。
* 具体替换目标（一个锚点 ✓）：末尾那段
  ```rust
      Err(ExecError::Unsupported { … "解包只接线了 tuple／list／str（迭代器协议未接线）" })
  ```
  整段换成：
  ```rust
      // 兜底：走迭代器协议 ✓（tuple／list／set／str 已在上面走快路 ✓）。
      let iterator = instance.iter_object(raw)?;                 // 新引用 ⇒ 我方持有 ✓
      let mut collected: Vec<NonNull<Header>> = Vec::new();
      let outcome = loop {
          match instance.advance_iterator(iterator) {
              Ok(Some(item)) => collected.push(item),            // 元素是新引用 ⇒ 直接收 ✓
              Ok(None) => break Ok(()),
              Err(error) => break Err(error),
          }
      };
      instance.release(iterator);                                // 迭代器那份要还 ✓
      outcome?;
      Ok(collected)
  ```
**③ 判据** ✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；
再跑受管后台重测 ✓（这次**有理由期待上限与判据同时上移** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 309 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（脚本未写盘 ✓）。

#### 第 312 轮：迭代器助手的**签名确认** ✓（改法可以写了；差一处"所有权契约"要核）

**① 签名** ✓（`instance/convert.rs:70／79` ✓）：
```rust
pub fn advance_iterator(&self, object: NonNull<Header>) -> Result<Option<NonNull<Header>>, ExecError>;  // None ⇒ 取尽
pub fn iter_object(&self, object: NonNull<Header>)      -> Result<NonNull<Header>, ExecError>;
```
两者都**转调执行器那一份** ✓（`executor::runtime::advance` ✓、`executor::iter::iter_value` ✓ —— **一处真相** ✓）。
**② 兜底要写的形状（下一轮照写 ✓）**：
```rust
// tuple／list／set／str 已在上面走快路 ✓ ⇒ 其余一律**走迭代器协议** ✓。
let iterator = instance.iter_object(raw)?;
let mut items = Vec::new();
loop {
    match instance.advance_iterator(iterator)? {
        Some(item) => items.push(item),
        None => break,
    }
}
Ok(items)
```
**③ 唯一要核的点（下一轮先读再写 ✓）**：**所有权** ✓ ——
* `iter_object` 交回的**迭代器**是我方持有 ✓（要释放 ✓）还是借用 ✗？
* `advance_iterator` 交回的**元素**是**持有** ✓（直接 `push` ✓）还是借用 ✗（要 `retain` ✓）？
⇒ 去看 `iter_value`／`advance` 的注释与调用点（`executor/iter.rs` ✓、`executor/runtime.rs` ✓）即可定 ✓
—— **不要凭猜写** ✗（本会话已有多次"凭猜 ⇒ 撤回"的教训 ✓）。
**④ 判据**（写好之后）✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓（关键 ✓）、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（这次**有理由期待上限与判据同时上移** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 309 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 311 轮：`sequence_items` 的结构读清了 —— 兜底就是末尾那一个 `Err(Unsupported)`

**① 结构** ✓（`executor/ctrls.rs:60` ✓）：
```
tuple  ⇒ 直接取 entries ✓          list ⇒ 直接取 entries ✓
set    ⇒ 直接取 entries ✓          str  ⇒ 逐字符造 StrObject ✓
（都没有）⇒ return Err(ExecError::Unsupported { … "解包只接线了 tuple／list／str（迭代器协议未接线）" })
```
⇒ **兜底就是末尾那一个 `Err(...)`** ✓ ⇒ 修法＝**把它换成迭代器循环** ✓（保留上面四条快路 ✓、**不动** `UNPACK_SEQUENCE` 的长度校验 ✓）。
**② 下一手（就一件 ✓）**：读 `Instance::iter_object` 与 `Instance::advance_iterator` 的**签名** ✓
（本轮已 `grep` 出位置 ✓，见命令输出 ✓）⇒ 然后在兜底处写：
```
在兜底处：iter = instance.iter_object(raw)?; 循环 advance_iterator(…) 直到 StopIteration ⇒ 收集 ✓
（"没有元素"⇒ 仍按上面的 ValueError 报 ✓，与参照一致 ✓）
```
**判据** ✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；
再跑受管后台重测 ✓（这次**有理由期待上限与判据同时上移** ✓ —— 挡的是**同一批 118 个模块** ✓）。
**③ 顺带记下** ✓：同一个迭代器协议缺口还在**另外两处**（`executor/iter.rs:209／290` ✓
的 `Unsupported`：「只接线了 tuple／list／dict／set／str／bytes 的内建迭代器（其余走 `__iter__` 协议）」✓）
⇒ 兜底一旦写好 ✓，那两处可以**复用它** ✓（一处真相 ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 309 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 310 轮：下一堵墙的**落点找到了** —— `sequence_items`（`UNPACK_SEQUENCE` 靠它取元素）

**① 读到的（`executor.rs:1523` ✓）** ✓：
```rust
"UNPACK_SEQUENCE" | "UNPACK_EX" => {
    let raw = frame.get().pop()?;
    let items = sequence_items(instance, raw, opcode_number);   // ← 限制在这里面 ✓
    …
```
⇒ 报错那句"**解包只接线了 tuple／list／str**"是 **`sequence_items`** 发的 ✓
⇒ 修法就是把它的**兜底分支**从"报 `Unsupported`"改成"**走迭代器协议**"✓。
**② 好消息（修法几乎现成 ✓）**：本层的 `Instance` 上**已经有**迭代器助手 ✓
（第 236 轮列的 `instance.rs` 方法清单里就有 **`iter_object`** 与 **`advance_iterator`** ✓）
⇒ 兜底只需：`iter_object(raw)` ✓ ⇒ 循环 `advance_iterator(…)` 直到 `StopIteration` ✓ ⇒ 收集元素 ✓
（参照语义 ✓：**长度不符**仍按上面的 `ValueError` 报 ✓、**迭代器没元素**时同样报 `ValueError` ✓）。
**③ 下一轮（就一件 ✓）**：读 `sequence_items` 的完整实现 ✓（本轮已 `grep` 出位置 ✓）⇒
在**兜底分支**里接上迭代器协议 ✓（保留 tuple／list／str 快路 ✓、**不动**上面的长度校验逻辑 ✓）；
**判据** ✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；
再跑受管后台重测 ✓（这次**有理由期待上限与判据同时上移** ✓ —— 因为它挡的是**同一批 118 个模块** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 309 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读定位 ✓、树干净 ✓）。

#### 第 309 轮：✅ 那一族（118）**消失了** ✓ —— 编译器修复真的解封了它；上限／判据暂未动

**① 受管作业实测（03:14／03:16 ✓）** ✓：
```
上限诊断：能 import **162** 个（25.8%）（与上次同 ✓）
（头号族的**计数行没被我的窄 grep 抓到** ✗，但它的 `例：` 列表是——
  例：_markupbase, _osx_support, _pylong, _pyrepl.pager, _pyrepl.unix_console, argparse, asyncio, asyncio.__main__ …
  ⇒ **正是原来卡在 `NoneType __str__` 的那一批模块** ✓）
      77  NameError: name 'eval' is not defined
      28  SyntaxError：annotationlib（第 327 行 列 18-20）
      19  ModuleNotFoundError: No module named '_struct'
       9  ModuleNotFoundError: No module named 'binascii'
       8  NameError: name 'complex' is not defined
       8  ImportError: cannot import name 'getDOMImplementation' from 'xml'
       7  AttributeError: 'module' object has no attribute 'warnoptions'
       7  ModuleNotFoundError: No module named '_codecs_jp'
判据①：**通过 155 ＋ 参照口径 17 ＝ 172 ÷ 628 ⇒ 27.4%** ✓（未动 ✓）
进度指标：283 个同步文件 ⇒ 156 个 ⇒ **55.1%** ✓
```
**② 两条读法** ✓：
* **`NoneType ... '__str__'` 这一族从表里消失了** ✓✓ ⇒ 第 308 轮的编译器修复**确实解封了它** ✓
  （这与"小例报错换成正常能力缺口"一致 ✓）；
* 但**上限与判据都没动** ✗ —— 因为那批模块**只是往前挪了一格** ✓（它们的 `例：` 列表一模一样 ✓）
  ⇒ 新头号族就是小例刚撞到的那堵墙 ✓：**`UNPACK_SEQUENCE`（指令 119）只接线了 tuple／list／str，
  没接迭代器协议** ✓（一条**定义明确**的能力缺口 ✓，不是玄学 ✓）。
**③ 下一轮（就一件 ✓）**：**给 `UNPACK_SEQUENCE` 接上迭代器协议** ✓ ——
先读它的实现（`executor.rs` 的 `"UNPACK_SEQUENCE"` 分支 ✓，报错消息就是它发的 ✓）⇒
按参照语义：先取 `iter(obj)` ✓，再逐个 `next()` ✓（长度不符报 `ValueError` ✓，
元素不足报 `ValueError` ✓）；把 **tuple／list／str 那条快路保留** ✓、**其余对象走迭代器** ✓。
**判据** ✓：`target/ifmin1.py` 通过 ✓（或再换一堵墙 ✓）、**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；
再跑受管后台重测 ✓（这次**有理由期待上限与判据同时上移** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓ 均为**本轮实测** ✓；
**未声称任何阶段完成** ✓。累计已修 **三个真 bug**（形参槽所有权 ✓、嵌套 `break` 截断 ✓、融合加载槽号溢出 ✓）
＋ **7 处能力缺口** ✓。

#### 第 308 轮：🎉🎉🎉 **修好第三个真 bug** —— 融合加载的**前提判断**（槽号必须装得进 4 位）

**① 改动（一处，纯插入一个带守卫的分支 ✓、不动任何括号 ✓）** ✓（`compile/emitter.rs:3032` 的 `fused_pair` ✓）：
```rust
(Some(first), Some(second)) if first <= 0x0F && second <= 0x0F => { Some((first, second)) }
(Some(_), Some(_)) => None,          // 装不下 ⇒ 交给非融合回退 ✓（发两条独立加载 ✓）
```
**② 证据（与"绕过实验"的预测逐字一致 ✓）** ✓：
```
改前：AttributeError: 'NoneType' object has no attribute '__str__' …                    ✗
改后：pyawa: 未捕获（状态 5）：指令 119 的这个形态尚未接线：解包只接线了 tuple／list／str
      （迭代器协议未接线）                                                                ✓ **正常能力缺口**
```
（第 288 轮那次"用 `setattr` 绕过"得到的正是**同一条**下一步错误 ✓ ⇒ 说明这次是**真修** ✓。）
**③ 全闸门** ✓：**0 警告** ✓ ；`cargo test --workspace` ✓ ；对拍 普通与 DANGLING 均 **`test result: ok`** ✓ ；
**逐字节 4/4** ✓（编译器改动的硬闸门 ✓）；`check.py` 12/12 ✓ ；夹具 **490** ✓ ；语料下限 **182** ✓ ；`selftest` 22 ✓。
**④ 根因链（完整 ✓）**：`EnumType.__new__` 局部很多（槽号 20+）⇒ 编译器把
`enum_class.__str__ = method` 编成 **融合加载** ✗ ⇒ 槽号 **22** 塞进 4 位半字节**溢出** ✗ ⇒ 读出**槽 7** ✓
⇒ `STORE_ATTR` 拿到别的对象 ⇒ 报"往 None 设置属性" ✗ ⇒ **118 个模块**卡住 ✓。
**⑤ 下一轮（就一件 ✓）**：用**受管后台作业**重跑上限＋判据 ✓（窄 grep ✓）⇒ 把这次修复**量化** ✓
（预期 **118 族大幅减少、上限上升、判据① 首次明显上移** ✓）。
**⑥ 数字与交代** ✓：判据① 台账仍按**上次实测 27.4%（172÷628）** ✓（**本轮未重测** ✓）；上限 **162** ✓；
**未声称任何阶段完成** ✓ —— 但**修复本身**已由"报错换墙"与全闸门证实 ✓。

#### 第 307 轮：多站点补丁**写坏结构** ⇒ 撤回 ✓（并写下正确改法）

**① 我干了什么（如实 ✓）**：用一个正则给**4 处融合发射点**各加"两个槽号 ≤15"的前提 ✓——
我试图把 `if let (Some(a), Some(b)) = (…) {` **换成**"同一个条件 + 内层 `if a<=15 && b<=15 {`" ✓
⇒ 结果：**只命中 3 处** ✗，而且**花括号没配平** ✗（多开了一层 `if`、少一个 `}` ✓）⇒ 代码结构被破坏 ✓。
**② 处置** ✓：`git checkout -- crates/` **整套撤回** ✓ ⇒ 复核 **0 警告** ✓、逐字节 **4/4** ✓、树**干净** ✓。
**③ 教训（写进台账 ✓）**：**不要用正则去改控制流的花括号结构** ✗ —— 这种改动要么
* 用**纯插入**（在 `if let` **之前**加一条 `let ok = a <= 0x0F && b <= 0x0F;` ✓，
  再把条件原地改成 `if let (…(…) if ok` ✗ 不行 ✓）……
* **正确做法** ✓：把守卫写成**条件里的一个附加项** ✓ —— 但 `if let` 的括号里塞不进任意表达式 ✗
  ⇒ 所以改成 **`if let (Some(a), Some(b)) = (…) .filter(|(a, b)| *a <= 0x0F && *b <= 0x0F)`** ✗ 也绕 ✓
  ⇒ 最干净的是**在解析阶段就拒绝**：**在做 `fused_pair`／`slot_of` 判定时**加一条
  "**两个槽号都 ≤15 才算命中**"✓（即**改判定点，而不是改发射点的括号** ✓）——
  那 4 处都先算出一个 `Option<(usize, usize)>`（`fused_pair`／`slot_of` 等 ✓）⇒
  只要在这些 `Option` **产出处**加 `.filter(|(a, b)| *a <= 0x0F && *b <= 0x0F)` ✓ 就行 ✓（**不碰大括号** ✓）。
**④ 下一轮（就一件 ✓，按上面"改判定点"的写法 ✓）**：定位这 4 处各自的 **`Option` 产出表达式** ✓
（如 3045 行的 `fused_pair` ✓、4670 行的 `slot_of` ✓ … ✓）⇒ 在产出处加 `.filter(… <= 0x0F …)` ✓
⇒ 编译 ✓ ＋ **小例** ✓ ＋ **逐字节 4/4** ✓ ＋ workspace／对拍／`check.py` ✓；**红了整套撤回** ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（补丁已撤 ✓）。

#### 第 306 轮：融合形式的**发射点找到了 4 处** —— 修法＝加"两个槽号都 ≤15"的前提

**① 本轮查到的（`compile/emitter.rs` ✓）** ✓：
```
3049: opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")      ← 某处（注释提到 `i, self` 先值后对象）
4048: opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")      ← 推导式的 `<键槽, 值最左槽>`（注释 4019 ✓）
4489: opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")      ← 第三处
4686: opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")      ← 第四处（注释 4670：
                                                          "高 4 位先压 | 低 4 位后压" ✓ **正是那条打包规则**）
```
**② 根因回顾** ✓（第 305 轮闭环 ✓）：槽号 **22** 装进 4 位半字节**装不下** ✗ ⇒ 读出来是 `7` ✗
⇒ 拿错对象 ⇒ `STORE_ATTR` 报"往 None 设置属性" ✗ ⇒ **118 个模块**卡住 ✓。
参照的编译器**只在两个下标都 <16 时**才融合 ✓ —— 这是该超指令的**前提** ✓。
**③ 下一轮（就一件 ✓，改法已定）**：加一个 helper ✓，例如
```rust
/// 融合加载的前提：**两个槽号都必须装得进 4 位** ✓（否则发两条独立加载 ✓）。
fn fused_slots_fit(a: usize, b: usize) -> bool { a <= 0x0F && b <= 0x0F }
```
⇒ 在**那 4 处**逐个加"前提不满足就走**非融合分支**"✓（每处都要有非融合回退 ✓——若某处原本没有
回退分支 ✓，就补成两条 `LOAD_FAST_BORROW` ✓）。
**判据** ✓（编译器改动必须过这几条 ✓）：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、
**逐字节 4/4** ✓、`cargo test --workspace` ✓、对拍两模式 ✓、`check.py` 12/12 ✓、夹具 490 ✓；
**红了整套撤回并如实记** ✓；随后跑受管后台重测 ✓（预期 **118 族大幅减少、上限上升** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读定位 ✓、树干净 ✓）。

#### 第 305 轮：🎯🎯🎯🎯 **闭环** —— 写入槽 **22**、读取槽 **7** ⇒ **槽号 >15 时不该融合成 4 位半字节**

**① 探针（只加不改 ✓）与输出** ✓：
```
[store_fast] slot=22 type=builtin_function_or_method        ← `method = member_type.__str__` 写**槽 22** ✓
[store_fast] slot=22 type=builtin_function_or_method        （重复一次 ✓）
[store_fast] slot=20 type=EnumType                          ← `enum_class` 一族在**槽 20** ✓
[fused_load] oparg=116 first=7 second=4 left=list right=NoneType   ← 读它时却是**槽 7 / 槽 4** ✗
```
⇒ **写入槽（22）≠ 读取槽（7）** ✓✓ ⇒ 而且 **22 > 15** ✗ ⇒
**融合加载把两个槽号塞进 4 位半字节（`oparg >> 4` / `oparg & 0x0F` ✓）**
⇒ **槽号 ≥16 时根本装不下** ✗ ⇒ **编译器不该在那种情况下发融合形式** ✓✓
（参照的编译器只在两个下标都 <16 时才融合 ✓ —— 这是它的**前提条件** ✓）。
**② 这就是根因** ✓（第三个真 bug ✓，而且解释力最强 ✓）：
* `EnumType.__new__` 有**很多局部** ✓（`enum_class`／`method`／`_tmp`… ✓ ⇒ 槽号到 20+ ✓）；
* 编译器给 `enum_class.__str__ = method` 发了 `LOAD_FAST_BORROW_LOAD_FAST_BORROW` ✗
  ⇒ 槽号被**截断/串了** ✓ ⇒ 读到**别的槽**的内容（`list`／`None` ✓）；
* 于是 `STORE_ATTR` 拿错对象 ⇒ 报"往 None 设置属性" ✗ ⇒ **118 个模块**全卡在这 ✓。
**③ 下一轮（就一件，且很可能是一次成 ✓）**：在**编译器发融合形式**的那处加**前提判断** ✓：
**只有两个槽号都 ≤ 15 才融合** ✓（否则发**两条独立的加载** ✓——参照也正是这样 ✓）。
落点：`compile/emitter.rs` 里发 `LOAD_FAST_LOAD_FAST`／`LOAD_FAST_BORROW_LOAD_FAST_BORROW` 的地方 ✓
（本轮先只找、不改 ✓：`grep -n "LOAD_FAST_BORROW_LOAD_FAST_BORROW" compile/emitter.rs` ✓）。
**判据** ✓：`target/ifmin1.py` 通过 ✓、**逐字节 4/4** ✓（编译器改动必须过 ✓）、workspace／对拍／`check.py` ✓；
再跑受管后台重测 ✓（预期 **118 族大幅减少 ✓、上限上升 ✓**）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了 `STORE_FAST` 探针**（门控 ✓、0 错 0 警告 ✓）。

#### 第 304 轮：🎯🎯🎯 **铁证** —— 融合加载 `oparg=116` ⇒ 槽 7／槽 4 ⇒ 取出 `list`／`NoneType`

**① 探针（只加不改 ✓，留在树里 ✓）与输出** ✓：
```
[fused_load] oparg=67  first=4 second=3 left=type right=EnumType                     ✓
[fused_load] oparg=35  first=2 second=3 left=type right=EnumType                     ✓（重复两次 ✓）
[fused_load] oparg=69  first=4 second=5 left=builtin_function_or_method right=bool   ✓
[store_attr_stack] depth=2 items=[builtin_function_or_method,EnumType]               ✓
[fused_load] oparg=116 first=7 second=4 left=list right=NoneType                     ✗ **就是这一句**
[store_attr_stack] depth=2 items=[list,NoneType]                                     ✗
pyawa: … AttributeError: 'NoneType' object has no attribute '__str__' …
```
⇒ **对上了** ✓：`enum_class.__str__ = method` 编译成**融合加载 `oparg=116`** ✓（拆成 **槽 7** 与 **槽 4** ✓）
⇒ 而那两槽里是 `list`／`None` ✗ ⇒ **不是**融合加载的实现错 ✗、**不是**拆法错 ✗，
而是**编译器把这两个名字编到了错误的槽** ✗ ⇒ **写入者与读取者的槽号不一致** ✓
⇒ **第三个真 bug：局部槽分配** ✓（本会话第 84／266 轮碰过 `SlotKind`／`locals`／`cells` ✓）。
**② 下一轮（就一件 ✓）**：给 **`STORE_FAST`** 也加门控打印 ✓（`oparg` ＝槽号 ✓、写入值的类型 ✓）
⇒ 看"**写 `method` 那次写进了哪个槽**"✓ ⇒ 与"读它时用的是槽 7/4 ✓"对照 ✓
⇒ 一旦**写入槽 ≠ 读取槽** ✓，就锁定到**编译器给该函数分配槽**的那段代码 ✓（那时就可以动刀 ✓）。
**③ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓（关键 ✓）、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了融合加载探针**（门控 ✓、0 错 0 警告 ✓）。

#### 第 303 轮：交换高低半字节**失败并撤回** ✓（原拆法是对的 ⇒ 病在上游：**槽号分配**）

**① 我试了什么** ✓：把融合加载 `LOAD_FAST_LOAD_FAST | LOAD_FAST_BORROW_LOAD_FAST_BORROW` 的拆法
从"**高 4 位先压**"（`first = oparg >> 4` ✓）换成**低 4 位先压** ✓。
**② 结果** ✗：
```
小例：AttributeError: 'NoneType' object has no attribute 'fset' …   ← ✗ **错误提前到了 property 那几句**
      （而那几句在**原拆法**下是**正确**的 ✓ ⇒ 说明原拆法**才是对的** ✗）
逐字节 4/4 ✓、0 警告 ✓（这条闸门对两种拆法都过，可惜当不了裁判 ✗）
```
⇒ **整套撤回** ✓ ⇒ 复核 **0 警告** ✓、逐字节 **4/4** ✓、树**干净** ✓。
**③ 于是"融合加载压错值"必须换解释** ✓：既然**拆法**是对的 ✓、且那些**槽里本来就是内容**（`list`／`None` ✗）
⇒ 那么问题就是**"给这一句分配的槽号本身不对"** ✗ —— 也就是**编译器**为
`enum_class.__str__ = method` 这两条加载**选错了槽** ✓（例如把 `enum_class` 记成了 `_last_values` 的槽 ✓、
把 `method` 记成了别的槽 ✓ ⇒ 才会读到 `list`／`None` ✓✓）。
**④ 下一轮（就一件 ✓）**：给**融合那一支**加门控打印 ✓（`oparg` ✓、拆出的**两个槽号** ✓、
两个槽的**值类型** ✓）⇒ 与"源码里那两个名字对应哪几个槽"对照 ✓
⇒ 若槽号就不对 ✗ ⇒ 去**编译器**（`compile/emitter.rs` 的局部槽分配 ✓ —— 本会话第 84／266 轮碰过的
`SlotKind`／`locals`／`cells` ✓）找 ✓：**这会是第三个真 bug** ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（补丁已撤 ✓）。

#### 第 302 轮：🎯🎯🎯 **最终定位** —— 错值来自**融合加载**（`LOAD_FAST_BORROW_LOAD_FAST_BORROW`），不是 `LOAD_FAST`

**① 弹之前的整栈转储（只加不改 ✓、探针这次留着 ✓）** ✓：
```
[store_attr_stack] depth=2 items=[builtin_function_or_method, EnumType]   ✓（`__format__` 那句，**正确**）
[store_attr] name=__format__ object_type=EnumType value_type=builtin_function_or_method depth_before=2 stack=[]
[store_attr_stack] depth=2 items=[list, NoneType]                         ✗（`__str__` 那句，**错**）
[store_attr] name=__str__ object_type=NoneType value_type=list depth_before=2 stack=[]
```
⇒ **在弹之前**，栈上就是 `[list, NoneType]` ✗（深度 2 ✓ 个数对 ✓）⇒ 即**这一句的两条加载压错了值** ✗。
**② 与 `LOAD_FAST` 探针的对照** ✓（第 301 轮数据 ✓）：尾部只到
`oparg=13 type=type`／`oparg=22 type=builtin_function_or_method` ✓ ——
**但那些属于 `__format__` 那一句** ✓（正好就是它弹出的 `[builtin_function_or_method, EnumType]` ✓）
⇒ **`__str__` 那两句加载根本没被 `LOAD_FAST` 探针记录** ✗ ⇒ 它们走的是**另一条 arm** ✓：
**融合的 `LOAD_FAST_BORROW_LOAD_FAST_BORROW`** ✓（本会话第 203 轮读过它 ✓，当时结论是"只读 + push、清白" ✗
—— 那条结论现在**要重查** ✓：问题很可能在它的 **`oparg` 高低位拆分**（`oparg >> 4` 与 `oparg & 0x0F` ✓）
⇒ 若两半**取反了** ✗，就会压出**两个别的槽**的值 ✓ ⇒ 恰好是 `list` / `None`（别的槽的内容 ✓）✓✓。
**③ 下一轮（就一件 ✓）**：给**融合那一支**也加门控打印 ✓
（`oparg` ✓、拆出的两个槽号 ✓、两个槽的**值类型** ✓）⇒ 与参照的语义（3.14 的 `dis` 里
`LOAD_FAST_BORROW_LOAD_FAST_BORROW 1 (a, b)` ⇒ `a`＝低位 ✓、`b`＝高位 ✓）对照 ✓
⇒ 只改`>>4` / `&0x0F` 那一处即可（**并跑全闸门**：编译器/执行器改动必须过**逐字节 4/4** ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了"只加不改"的转储探针**（门控 ✓、0 错 0 警告 ✓）。

#### 第 301 轮：`LOAD_FAST` 探针**有效** ✓；"把转储挪到弹之前"的补丁**写坏了打印** → 撤回 ✓

**① 本轮有效产出** ✓（在撤回之前拿到的数据 ✓）：给 `LOAD_FAST` 一族加门控打印 ✓（`PYAWA_LOAD_FAST_DEBUG=1` ✓）：
```
[load_fast] oparg=20 type=EnumType
[load_fast] oparg=3  type=dict
[load_fast] oparg=13 type=type
[load_fast] oparg=22 type=builtin_function_or_method
[load_fast] oparg=13 type=type
[store_attr] name=__str__ object_type=NoneType value_type=list depth_before=2 stack=[]
```
⇒ 加载侧**确实**在压入各种值 ✓（`EnumType`／`dict`／`type`／`builtin_function_or_method` ✓），
但 `__str__` 那次弹到的仍是不相干的 `NoneType`／`list` ✗ ⇒ 说明**关键的那两条加载没出现在尾部** ✗，
或者**弹的不是刚压的**✗ —— 而这正好需要**弹之前的整栈转储** ✓。
**② 我干了什么蠢事（如实 ✓）**：把转储**从"弹之后"挪到"弹之前"**时 ✗，
我把变量名改成了 `stack_dump` ✓ 却**没改** `eprintln!` 里的实参（仍用被删掉的 `dump` ✓）⇒
编译虽过（0 错 ✓，因为实参其实还在别处 ✓）但**那两条打印不再出现** ✗ ⇒ 说明我把打印**接到了别的分支** ✗。
⇒ 按纪律 **`git checkout -- crates/` 整套撤回** ✓ ⇒ 复核：**0 警告** ✓、逐字节 **4/4** ✓、树**干净** ✓
（代价：本轮那发 `LOAD_FAST` 探针也随之丢掉 ✗ —— 如实记 ✓）。
**③ 下一轮（就一件 ✓，写法要更保守 ✓）**：**先 `sed -n` 看清**要动的 12 行 ✓，
再**只加不改**（新加一个独立的 `if flag { … }` 块 ✓、**不碰**已有 `eprintln!` 的实参 ✓）⇒
拿到**弹之前的整栈转储** ✓ ⇒ 那时就能最终判定"**弹的是不是刚压的**"✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（补丁已撤 ✓）。

#### 第 300 轮：栈转储确认"弹得干净" ⇒ 病在**压进去的那两项**

**① 转储输出** ✓（探针已扩成打印整个帧值栈 ✓）：
```
[store_attr] name=__format__ object_type=EnumType value_type=builtin_function_or_method depth_before=2 stack=[]
[store_attr] name=__str__    object_type=NoneType value_type=list                        depth_before=2 stack=[]
```
⇒ 两次都是 `stack=[]` ✓（弹完之后为空 ✓）⇒ **弹得干净** ✓ ⇒ 排掉"没弹干净"✗（第 296／297 轮那条线也走到头 ✓）。
**② 于是唯一剩下的解释** ✓：**压进去的那两项本身就不对** ✗ ——
`enum_class.__str__ = method` 应当压入 **枚举类**（✓ 却得到 `NoneType` ✗）与 **绑定原生方法**（✓ 却得到 `list` ✗）；
换成 `_a`／`_b` 之后又变成 `list`／`dict` ✗ ⇒ **压栈侧系统性给出无关的值** ✓
⇒ 最可疑的就是那两条**加载**：`LOAD_FAST enum_class`（或 `LOAD_FAST_BORROW` ✓）与 `LOAD_FAST method` ✓。
**③ 下一轮（就一件 ✓）**：在 `LOAD_FAST`／`LOAD_FAST_BORROW` 那一支加一发**门控打印** ✓
（`PYAWA_LOAD_FAST_DEBUG=1` ✓：打印 `oparg`／槽号 ✓、取到的值的**类型名** ✓，并标明是"局部位"还是"cell 回落"✓）
⇒ 就能看出那两条加载**给了什么** ✓、以及**槽里的东西是不是本来就错** ✗。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了栈转储版探针**（门控 ✓、0 错 0 警告 ✓）。

#### 第 299 轮：🎯 临时局部对照**照样弹到垃圾**（`list`/`dict`，且同一对**出现两次**）⇒ 是**陈旧的残留值**

**① 对照结果（副本插桩 ✓，跑完还原 ✓）** ✓：把
`enum_class.__str__ = method` 换成 `_a = enum_class; _b = method; _a.__str__ = _b` ✓：
```
[store_attr] name=__str__ object_type=list value_type=dict depth_before=2      ✗
[store_attr] name=__str__ object_type=list value_type=dict depth_before=2      ✗（**同一对又来一次**）
[store_attr] name=__new_member__ object_type=NoneType value_type=function      ✗
pyawa: … AttributeError: 'NoneType' object has no attribute '__new_member__' …
```
**② 读法（本轮的决定性信息 ✓）**：
* 换成临时局部**也没救** ✓ ⇒ 第 298 轮"读那两个槽"的猜想也**被否** ✗（如实 ✓）；
* 弹到的**是垃圾对**（`list`／`dict` ✓）而不是本句要用的东西 ✓ ⇒ 那些**根本不是** `_a`／`_b` 压进去的 ✓
  ⇒ **压栈没生效** ✗ 或**栈上留着一层更早的东西** ✗（`list`／`dict` 恰好是 `_last_values`／`classdict` 的类型 ✓
  ⇒ 像是 **`EnumDict` 那一族**留下的 ✓）；
* **同一对连续出现两次** ✓ ⇒ 提示那段代码**跑了不止一遍** ✗（`enum.py` 的类创建里有循环/重试 ✓）。
**③ 下一轮（就一件 ✓，把探针再扩一格 ✓）**：在 `STORE_ATTR` 里**把整个帧值栈都打出来** ✓
（每一项的类型名 ✓ ＋ 总深 ✓）⇒ 一次就能看到"**栈上到底堆着什么**"✗、以及**本句应该压的两项有没有进去** ✓
⇒ 这比只看弹出来的两项更直接 ✓（本会话已证明"看被弹的东西"会被残渣误导 ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 298 轮：两次删除**都没改变**操作数 ✗ ⇒ 病灶在更前面；且那两项（`None`／`list`）像**更早的残渣**

**① 实验（副本插桩 ✓，两个变体一次跑完并还原 ✓）** ✓：
```
变体 1：删掉 `classdict['__format__'] = enum_class.__format__` ⇒ __str__ 仍是 NoneType/list ✗
变体 2：再把 `if '__str__' not in classdict:` 换成 `if True:` ⇒ 仍是 NoneType/list ✗
```
⇒ 这两句都**清白** ✗（第 297 轮的押注 `STORE_SUBSCR` 落空 ✓，如实记 ✓）。
**② 本轮的新线索（有价值 ✓）**：那两项的内容是 **`None`** 与 **`list`** ✓ ——
而更早那批 `STORE_ATTR` 里正好有 **`_last_values`（值是 `list` ✓）**、`_ignore`（`list` ✓）、
`_auto_called`（`bool` ✓）⇒ 这两项**像是好几条语句之前的残渣** ✗
⇒ 也就是说：**`STORE_ATTR` 弹掉了两项**（深度 ✓ 从 2 减到 0 ✓ 对得上 ✓），
但**压进去的那两项不是这一句该用的** ✗ ⇒ **压栈侧就错了** ✓ —— 而压栈来自
`LOAD_FAST enum_class` 与 `LOAD_FAST method` ✓ ⇒ 那两条**加载**给出了不相关的旧值 ✗。
**③ 下一轮（就一件 ✓）**：把**这一句自己**的两条加载**替换掉**做对照 ✓（副本插桩 ✓）：
```python
                _a = enum_class
                _b = method
                _a.__str__ = _b          # 原写法：enum_class.__str__ = method
```
⇒ 若这样**能过** ✓ ⇒ 病就在"**读 `enum_class`／`method` 这两个槽**"✗（与第 290 轮"临时局部能过"呼应 ✓）
⇒ 那就去查**这两个槽**的分配/写入 ✓（尤其 `method` 是**刚由 `STORE_FAST` 写入**的 ✓、`enum_class` 是参数/局部 ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 297 轮：深度数据出来了 —— **个数对（两次都 2）**，错的是**内容** ✗

**① 探针扩充后实测** ✓（`depth_before` 在 `STORE_ATTR` 弹之前记录 ✓）：
```
[store_attr] name=__format__ object_type=EnumType value_type=builtin_function_or_method depth_before=2   ✓ 正确
[store_attr] name=__str__    object_type=NoneType value_type=list                        depth_before=2   ✗ 内容错
```
**② 读法（本轮的关键 ✓）**：
* **深度两次都等于 2** ✓ ⇒ 不是"多留/少留一项"✗（第 296 轮的猜想否掉 ✓）；
* 而是**栈上那两项的内容不对** ✗ ⇒ 即"**压进去的值不对**"✓ 或"**栈上还留着上一次的东西**"✗
  （`__str__` 那一次拿到了 `None` 与 `list` ✓ —— 这两个都不是那一句要用的东西 ✓）。
**③ 下一轮（就一件 ✓，仍是副本插画 ✓）**：在 `enum.py` 里**逐句删**，看内容何时变对 ✓：
1. 先删 `classdict['__format__'] = enum_class.__format__` ✓（`STORE_SUBSCR` ✓ —— 它正好在两句之间 ✓，
   最可能"没把操作数弹干净 ✗"⇒ 后面压栈就压在了残渣上 ✗）；
2. 再删 `if '__str__' not in classdict:` ✓（成员测试 + 跳转 ✓）。
⇒ 哪一句删掉之后 `__str__` 的 `object_type` 变回 `EnumType` ✓，**那一条 opcode** 就是病灶 ✓
（那时就可以去读它、并且**只改那一处** ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了探针的扩版**（门控 ✓、0 错 0 警告 ✓）。

#### 第 296 轮：否掉 `if` 块（删掉后操作数照样错 ✗）⇒ 下一手＝**打印栈深度**

**① 实验（副本插桩 ✓，跑完还原 ✓）** ✓：把 `enum.py` 里
```python
                method = member_type.__str__
                if method is object.__str__:
                    method = member_type.__repr__
```
**删成**只留 `method = member_type.__str__` ✓ ⇒ 重跑 ✓：
```
[store_attr] name=__str__ object_type=NoneType value_type=list      ✗ **一样错**
pyawa: … AttributeError: 'NoneType' object has no attribute '__str__' …
```
⇒ **那个 `if` 是清白的** ✗（第 295 轮的最可疑项被否 ✓）。
**② 于是范围再收** ✓：在 `__format__`（操作数**正确** ✓）与 `__str__`（**两个都错** ✗）之间，
剩下的只有：
* `__format__` 那一句**自身留下的栈记账** ✗（它**取对了**两个操作数 ✓，但
  **可能多留/少取了一项** ✗ —— 我的打印在**弹完之后** ✓ ⇒ 看不见深度变化 ✓）；
* `classdict['__format__'] = enum_class.__format__` ✓（`STORE_SUBSCR` ✓）；
* `if '__str__' not in classdict:` 与 `method = member_type.__str__` ✓。
**③ 下一轮（就一件 ✓，把**已提交的那发探针**扩成"带栈深度"的 ✓）**：在 `STORE_ATTR` 分支里
**同时打印调用前的值栈深度** ✓（`frame.get().stack_len()` 一类 ✓；若没有就打印 `frame.get().stack.borrow().len()` ✓）
⇒ 一句话就能看出：**哪条语句之后栈多了一项/少了一项** ✗ —— 这就是最后一步定位 ✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无净代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 295 轮：🎯🎯🎯 **定位到相邻两句之间** —— `__format__` 正确、紧接着的 `__str__` 两个操作数都错

**① 按顺序打印（探针已在树里 ✓，不必重编译 ✓）** ✓：
```
[store_attr] name=_member_names object_type=EnumDict value_type=dict          ✓
[store_attr] name=_last_values object_type=EnumDict value_type=list           ✓
[store_attr] name=_ignore       object_type=EnumDict value_type=list          ✓
[store_attr] name=_auto_called  object_type=EnumDict value_type=bool          ✓
[store_attr] name=_cls_name     object_type=EnumDict value_type=str           ✓
[store_attr] name=__format__    object_type=EnumType value_type=builtin_function_or_method   ✓ **正确**
[store_attr] name=__str__       object_type=NoneType value_type=list          ✗ **两个都错**
pyawa: 未捕获（状态 1）：AttributeError: 'NoneType' object has no attribute '__str__' …
```
**② 结论** ✓：栈在 **`__format__` 那次之后、`__str__` 那次之前**歪掉 ✓。
这两句之间的 `enum.py` 代码是 ✓：
```python
            if '__str__' not in classdict:
                method = member_type.__str__              # LOAD_ATTR + STORE_FAST ✓
                if method is object.__str__:              # **`is` 比较 + 分支** ✗（最可疑）
                    method = member_type.__repr__
                enum_class.__str__ = method
                classdict['__str__'] = enum_class.__str__
```
⇒ 中间只有：`LOAD_ATTR`／`STORE_FAST`／**`is` 比较**／**一个 `if` 分支** ✓
⇒ 最可疑的是**带 `is` 的 `if`**（比较 + 跳转 ✓）—— 若分支的**跳转目标**少算/多算一项栈 ✓ 就正好这样 ✗。
**③ 下一轮（就一件 ✓，副本插桩 ✓）**：把那两行 `if` 块**删掉**（直接 `method = member_type.__str__` ✓ 不判断 ✓）
⇒ 若 `__str__` 那次的操作数**变正确** ✓ ⇒ 病就在**那个 `if`（含 `is` 比较）**的编译/跳转上 ✓
⇒ 再缩小到最小 Python 片段（`x = a.b; if x is object.c: x = a.d; o.e = x` ✓）。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（用已提交的探针 ✓、树干净 ✓）。

#### 第 294 轮：🎯🎯🎯 **精确钉住** —— `name=__str__` 那次弹出的两个操作数都错（`object=NoneType` ✗、`value=list` ✗）

**① 探针留在树里 ✓（门控 `PYAWA_STORE_ATTR_DEBUG=1` ✓，零开销 ✓），实测** ✓：
```
[store_attr] name=__str__ object_type=NoneType value_type=list
pyawa: 未捕获（状态 1）：AttributeError: 'NoneType' object has no attribute '__str__' …
```
（顺带：`property` 一族那次的操作数是**对的** ✓ ⇒ 说明**不是**"这个 opcode 一律取错" ✗，
而是**某条路径之后栈就歪了** ✓。）
**② 读法** ✓：`enum_class.__str__ = method` 应当弹到
* object ＝ **枚举类** ✓（却得到 `NoneType` ✗）
* value ＝ **绑定的原生方法** ✓（却得到 `list` ✗）
⇒ **两个都错** ✓ ⇒ 栈在**这一句之前**就已经错位 ✓（不是这一句本身的问题 ✓）。
**③ 于是下一轮（就一件 ✓，探针已在树里 ✓ 不必再重编译 ✓）**：看**紧邻它前面**的那段
（`enum.py` 里原样是）：
```python
            if '__format__' not in classdict:
                enum_class.__format__ = member_type.__format__
                classdict['__format__'] = enum_class.__format__
```
⇒ 用探针把 `name=__format__` 那一行也抓出来 ✓（`grep name=__format__` ✓）：
* 若**它的**操作数也错 ✗ ⇒ 再往前找 ✓；
* 若它**正确** ✓ ⇒ 那歪掉就发生在这两行**之间** ✓ ⇒ 大概率是 **`classdict['__format__'] = …`
  （`STORE_SUBSCR` ✓）多留/少取了一项** ✗ —— 这就把范围压到**另一条 opcode** ✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**提交了探针**（门控诊断 ✓，0 错 0 警告 ✓）。

#### 第 292 轮：🎯🎯🎯 **`STORE_ATTR` 取错操作数** —— 接收者随栈布局变化（`None` ↔ **`str`**）

**① 二分结果（副本插桩 ✓，两次跑完还原 ✓）** ✓：
```
A 版：`_noop = 0` ＋ 原写法 `enum_class.__str__ = method`
      ⇒ AttributeError: **'NoneType'** object has no attribute '__str__' …            ✗
B 版：`_tmp = enum_class; _tmp2 = _tmp; _tmp2.__str__ = method`
      ⇒ AttributeError: **'str'** object has no attribute '__str__' …                 ✗
```
**② 读法** ✓（本轮的决定性信息 ✓）：
* **同一个 opcode** ✓，接收者却随**周围语句**变 ⇒ `None` ✗／`str` ✗ ⇒ 说明
  **`STORE_ATTR` 从栈上取的不是"那个对象"** ✓ —— **操作数取错** ✗（**不是**"某个槽坏了" ✗，
  也**不是**"借用加载给 None" ✗ —— 前几轮的两个假设都被本轮数据否掉 ✓）；
* B 版里那个 `str` ✓ 极可能就是**属性名字符串**（`"__str__"` ✓）⇒ 即**取到了"名字"而不是"对象"** ✗
  ⇒ 与"栈序/操作数错位"一致 ✓（第 288 轮那条猜测**重新抬头** ✓ —— 那轮我按**文档**把它削弱了 ✗，
  但文档说的是参照的**语义** ✓，**不能**证明本层**编译器**压栈顺序与**执行器**取值顺序一致 ✓ —— 如实修正这一点 ✓）。
**③ 下一轮（就一件 ✓，直接测执行器到底取了什么）**：在 `STORE_ATTR` 分支里加一发**门控打印** ✓
（`PYAWA_STORE_ATTR_DEBUG=1` ✓）：打印 `name`、弹出的 `object` 与 `value` 的**类型名** ✓
⇒ 一次就能看出"对象"位上拿到的是 `str`（名字 ✓）还是 `str`（值 ✓）✓ ⇒ 据此**只改一处**（编译器或执行器 ✓）。
**判据** ✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓（关键 ✓）、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 290 轮：🎯🎯🎯 **分流成功** —— 病在"**读那个局部**"（不是 `STORE_ATTR`）

**① 实验（副本插桩 ✓，跑完还原 ✓）** ✓：把那句改成**先用临时局部接住接收者** ✓：
```python
                _tmp = enum_class
                _tmp.__str__ = method            # 原写法：enum_class.__str__ = method
```
**② 结果** ✓：
```
pyawa: … AttributeError: 'NoneType' object has no attribute '__new_member__' …
```
⇒ `__str__` 那一句**过了** ✓（错误**移到下一处** `__new_member__` ✗ —— 正是第 288 轮里"下一个同类写法" ✓）。
**③ 结论** ✓（本轮的关键 ✓）：
* 病**不在 `STORE_ATTR`** ✗（第 287／288 轮那个方向**被否** ✓，如实记 ✓）；
* 病在「**读那个局部 `enum_class`**」这条路上 ✓ —— 即**该槽在被读的那一刻给出了 `None`** ✗；
* 而**换个槽**（`_tmp` ✓）读就正常 ✓ ⇒ 说明差别在**这个槽本身**或**读它的那条指令** ✓。
**④ 高度可疑的具体点** ✓（下一轮直查 ✓）：本层 3.14 的
**`LOAD_FAST_BORROW`**（**借用的**本地加载 ✓）——
本会话第 108／203 轮接触过它：当时结论是"单值版与 `LOAD_FAST` 同路" ✗、
而"融合版"（`LOAD_FAST_BORROW_LOAD_FAST_BORROW` ✓）**只读 + push** ✓ 是清白的 ✓。
⇒ 若编译器对 `enum_class` 发的是 **`LOAD_FAST_BORROW`** ✓、而对新局部 `_tmp` 发的是 **`LOAD_FAST`** ✓
⇒ 那么"**借用加载"这一支在这个槽上给出了 `None`** ✗ 就能解释一切 ✓（也解释了为什么其它地方都正常 ✓：
它们大多是**复制**出来用的 ✓）。
**⑤ 下一轮（就一件 ✓）**：读 `LOAD_FAST_BORROW` 的实现 ✓（`executor.rs` ✓；`grep -n '"LOAD_FAST_BORROW"'` ✓）
⇒ 核对它与 `LOAD_FAST` 的**取值路径**是否一致 ✓（尤其：槽里是 `Option<…>` ✓ 时是否会给出 `None` ✗）。
**判据** ✓：`target/ifmin1.py` 通过 ✓（或再遇下一堵正常缺口 ✓）、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑥ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 288 轮：🎉 两处 `setattr` 绕过之后 —— **enum 机制全通了** ✓；下一堵墙是**正常能力缺口**（`UNPACK_SEQUENCE`）

**① 实验（副本插桩 ✓，跑完还原 ✓）** ✓：把 `enum.py` 里**两处**属性赋值换成 `setattr` ✓：
```
                enum_class.__str__ = method            →  setattr(enum_class, "__str__", method)      ✓
                enum_class.__new_member__ = __new__    →  setattr(enum_class, "__new_member__", __new__) ✓
                （第二处的真身是 **613 行** ✓ —— 我先前猜的那行不存在 ✓，grep 出来才知道 ✓）
```
**② 结果** ✓✓：
```
pyawa: 未捕获（状态 5）：指令 119 的这个形态尚未接线：解包只接线了 tuple／list／str（迭代器协议未接线）
```
⇒ **不再是** `NoneType ... __str__`／`__new_member__` ✗，而是**一条正常的能力缺口** ✓
（`UNPACK_SEQUENCE`（119 ✓）只接线了 tuple／list／str ✓、**没接迭代器协议** ✗）
⇒ 结论**很硬** ✓：**`STORE_ATTR`（`X.attr = v`）这条 opcode 就是这 118 个模块的拦路石** ✓
—— 把这两处绕开，整条 enum 机制（`EnumType.__new__`／`_EnumDict`／`IntFlag` ✓）就走通了 ✓。
**③ 下一轮（就一件 ✓，也是本会话最值钱的一处）**：**修 `STORE_ATTR`** ✓ —— 已知：
* `setattr(类, 名, 值)` ✓ 走 `builtins_module` 的 `setattr_native` ⇒ `set_attribute_value` ⇒ **成功** ✓；
* `类.名 = 值`（`STORE_ATTR` ✓）⇒ **失败** ✗（报"往 None 设置"✓）。
⇒ 两者都最终调用 `instance_attribute_set` ✓（`executor.rs:3636` 那段我读过 ✓、`protocol.rs` 也读过 ✓）
⇒ 差别**只可能在栈序/取值**上 ✓：`STORE_ATTR` 的注释写"栈是 `[值, 对象]`（**对象在 TOS**）"✗
⇒ 而参照 CPython 3.14 的 `STORE_ATTR` 是 **TOS＝值、TOS1＝对象** ✓ ⇒ **很可能就是这里反了** ✓
（那会解释一切：把 `值` 当对象 ⇒ 对象常常是 `None` ✓ ⇒ 报"往 None 设置属性" ✗ ✓）。
**④ 判据** ✓：`target/ifmin1.py` 通过 ✓（或再遇到下一堵正常缺口 ✓）、**逐字节 4/4** ✓（关键 ✓）、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期 **118 族开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 287 轮：🎯🎯🎯 **属性赋值语法 vs `setattr` 分家** —— 前者坏、后者好（都把范围压到 `STORE_ATTR`）

**① 决定性对照（语料副本插桩 ✓，跑完还原 ✓）** ✓：把 `enum.py` 那句
`enum_class.__str__ = method` **换成** `setattr(enum_class, "__str__", method)` ✓：
```
D5 obj none: False val none: False
D6 after setattr            ← **成功了** ✓（两次 ✓）
然后错误**移到下一处同类写法**：'NoneType' object has no attribute '__new_member__' and no __dict__ …
```
⇒ **同一个接收者、同一个值** ✓：
* 用 **`setattr(…)`** ⇒ **成功** ✓；
* 用 **`X.attr = v`**（即 `STORE_ATTR` ✓）⇒ **失败** ✗ ⇒ 病在 **`STORE_ATTR` 这条 opcode** ✓
（本会话第四次把"一族 118 个模块"缩到**一条 opcode** ✓）。
**② 最小化尝试（`target/storemin4.py` ✓）** ✓：元类 `__new__` 里 `r.__str__ = lambda …` ⇒ 本层
**通过** ✓（`assigned ok` ✓）⇒ 触发还需要 enum 的上下文 ✓（如实 ✓：**不是**"元类里赋值就坏" ✓）。
**③ 顺带发现一处**表观差异**（如实记 ✓，不在本轮修）** ✓：同一个小例里
```
本层：C ok <M object at 0x…>        ✗（拿通用 object repr ✓）
参照：C ok <class '__main__.C'>     ✓
```
⇒ 本层 `str(<类对象>)` 走的是通用对象 repr ✗ ⇒ **另一条**（表观层）缺陷 ✓，记下 ✓。
**④ 下一轮（就一件 ✓，仍是"副本插桩"这招 ✓）**：把 `enum.py` 里**两处**失败写法
（`__str__` ✓、`__new_member__` ✓）**都换成 `setattr`** ✓ ⇒ 看 `import enum`／`class X(enum.IntFlag)` 能走多远 ✓：
* 若**通关** ✓ ⇒ 证明这条 `STORE_ATTR` 缺陷就是这 118 个模块的**唯一**拦路石 ✓ ⇒ 修它收益最大 ✓；
* 若还卡在别处 ✓ ⇒ 一并记下 ✓（每修一处都在缩小未知 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 286 轮：类型写入路径**清白** ✓ ⇒ 嫌疑转到 **`STORE_ATTR`**（`enum_class.__str__ = method` 用的就是它）

**① 读到的（`protocol.rs:205-233` ✓）** ✓：类型对象的属性写入是
```rust
let Some(namespace) = instance.type_namespace(object) else { … "类型对象没有命名空间" … };
let dict = …cast::<DictObject>()…;
match position { Some(slot) => { if let Some(old) = dict.replace_value(slot, value) { release(old) } }
                 None      => { let key = …new StrObject…; dict.insert_raw(key, value); } }
return Ok(());
```
⇒ 这条路径**没有**会报"往 `None` 设置属性"的地方 ✓ ⇒ 若它被走到，就应当成功 ✓。
**② 于是嫌疑转到 opcode 那一层** ✓：`enum_class.__str__ = method` 编译成
`LOAD_FAST enum_class; LOAD_FAST method; **STORE_ATTR __str__**` ✓ ⇒ 执行的是 `STORE_ATTR` ✓
⇒ 它的实现（`executor.rs` 或 `executor/*.rs` ✓）**可能**：
* 直接调 `instance_attribute_set` ✓（那就回到上面这条 ✓ ⇒ 应当成功 ✗ 与观察矛盾 ✓），或
* 走**自己的一条**路径 ✗（例如"先取 `type(obj).__setattr__`"✓ ⇒ 而本层 `type.__setattr__` 可能把
  接收者传丢 ✓ ⇒ 报 `NoneType` ✗ ✓ —— 与"本层 `type.__str__` 是 `object` 的原生实现"那条**同类**问题 ✓：**描述符/原生方法的接收者**）。
**③ 下一轮（就一件 ✓）**：读 `STORE_ATTR` 的实现 ✓（本轮已 `grep` 出位置 ✓，见命令输出 ✓）⇒
看它**怎么定位接收者** ✓、以及是否经过"类型的 `__setattr__`"✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 285 轮：`__set__` 探针**没命中**（更正第 284 轮的猜测 ✗）⇒ 目标转向**类型命名空间写入**那一段

**① 探针与结果** ✓（`target/setprobe2.py` ✓）：
```
本层：d = <built-in function __str__> ／ type(d) = builtin_function_or_method ／ type(d).__set__ = **无** ✓
参照：d = <slot wrapper '__str__' of 'object' objects> ／ type(d) = wrapper_descriptor ／ __set__ = **无** ✓
```
⇒ `__set__` 两侧都**没有** ✓ ⇒ 第 284 轮"数据描述符探针误命中"的猜测**不成立** ✗（如实更正 ✓）。
（附带一条**真实差异** ✓：本层 `type.__str__` 是 `<built-in function __str__>` ✗，参照是
`<slot wrapper '__str__' of 'object' objects>` ✓ —— 是"**拿 `object.__str__` 的原生实现当 `type.__str__`**"的
后果 ✓；**记下** ✓，但这本身不是本轮的死因 ✓。）
**② 于是死因在**下一段** ✓**：`instance_attribute_set` 里描述符那一支（155-180 ✓）**跳过**之后 ✓，
代码走到
```rust
if instance.is_type_object(object) {
    …
    let Some(namespace) = instance.type_namespace(object) else { … }   // ← 205 行附近，我还没读它后面 ✓
```
⇒ **类型对象的属性写入**那一段 ✓（`dict_set` 到命名空间 ✓）才是真正执行到的地方 ✓
⇒ 病根要么在 `type_namespace(object)` 交回 `None` ✗、要么在它的 `else` 分支报错 ✗、
要么在 `dict_set` 那一步把接收者搞错 ✗。
**③ 下一轮（就一件 ✓）**：读 `protocol.rs` **205-250 行** ✓（类型对象写属性的完整路径 ✓）⇒
找到"往 `None` 设置属性"那条消息是**从哪一步**发出的 ✓ ⇒ 那就是修的地方 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 284 轮：🎯🎯🎯 **病根那段代码找到了** —— `instance_attribute_set` 的"数据描述符"探针误命中

**① 代码（`crates/pyawa-core/src/executor/protocol.rs:155` 起）** ✓：
```rust
if name != "__dict__" {
    let object_type = unsafe { object.as_ref() }.ty();                    // enum_class 的类型 = 元类 EnumType ✓
    if let Some(found) = instance.type_lookup(object_type, name) {         // 在元类上找 "__str__" ⇒ 命中 type.__str__ ✓
        let found_ty = unsafe { found.as_ref() }.ty();                     // 那个描述符的类型 ✓
        if let Some(setter) = instance.type_lookup(found_ty, "__set__") {   // 🎯 **找 __set__**
            let this = instance.retain(object);
            instance.retain(value);
            let returned = call_callable(instance, setter, Some(found), vec![this, value], …)?;
```
**② 机制** ✓：`enum_class.__str__ = method` 里
* `found` ＝ **`type.__str__`**（一个**普通描述符** ✓，参照里**不是**数据描述符 ✓ ⇒ 参照会**跳过**这一支 ✓，
  把 `__str__` 写进**类的命名空间** ✓）；
* 而本层**取到了 `__set__`** ✗ ⇒ 于是拿**描述符自己**当接收者去调 `__set__` ✓ ⇒
  接收者在里面丢了／被当成 `None` ✗ ⇒ 报 `'NoneType' object has no attribute '__str__' …` ✓。
⇒ 这与第 283 轮的插桩**完全吻合** ✓（接收者与值都正常 ✓、却在这一句炸 ✓）。
**③ 下一轮（就一件 ✓）**：**核实"`__set__` 探针为何命中"** ✓ —— 最小探针：
```python
d = type.__str__
print(str(d))
print(str(getattr(type(d), "__set__", "无")))
print(str(getattr(d, "__set__", "无")))
```
⇒ 参照里前两者应当**没有** `__set__` ✓（`wrapper_descriptor` 不是数据描述符 ✓）；
若本层"有" ✗ ⇒ 就是**描述符类型上多挂了 `__set__`** ✓ ⇒ 病在**哪个类型多挂了** ✓
（很可能是我们给 `type` 挂 `__str__` 时（第 241／271 轮同款做法 ✓）连带把 `object` 的 `__set__` 一族
带进了它的查找链 ✗）⇒ 找到后**只改那一处** ✓。
**判据** ✓：`target/ifmin1.py` 通过 ✓、`target/imp_markup.py` 打 `ok` ✓、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 283 轮：🎯🎯🎯 **精确钉住失败语句** —— `enum_class.__str__ = method`（接收者与值都正常 ✗）

**① 逐句插桩（副本 `enum.py` ✓，跑完还原 ✓）** ✓：
```
D1 method none: False type: builtin_function_or_method      ← 值正常（绑定的原生方法 ✓）
D2 before set: enum_class none: False                        ← 接收者是正常类对象 ✓
pyawa: … AttributeError: 'NoneType' object has no attribute '__str__' and no __dict__ for setting new attributes
（没有 D3 ⇒ **死在这一句** ✓）
```
⇒ 失败语句＝**`enum_class.__str__ = method`** ✓，而**接收者**（`enum_class` ✓）与**值**（原生方法 ✓）
**都正常** ✗ ⇒ 报错里的 `NoneType` **只能来自写属性**这条实现的**内部** ✓
（接收者在实现里被丢/替换成了 `None` ✗）。**这是本会话第三次把"一族 118 个模块"缩到"一条语句"** ✓。
**② 于是下一手（就一件 ✓）**：读 `instance_attribute_set` 给**类型对象**的那条分支 ✓
（`crates/pyawa-core/src/executor/protocol.rs:149` 起 ✓，我早先读过开头 ✓）：
```rust
if name != "__dict__" {
    if let Some(found) = instance.type_lookup(object_type, name) {      // 在**元类型**上找
        if let Some(setter) = instance.type_lookup(found_ty, "__set__") {   // 找数据描述符
            … 调用 setter …
```
⇒ 关键看**调用 `setter` 时传的接收者是谁** ✓（若传了 `None` ✗ 或传错对象 ✓ ⇒ 就是它 ✓）。
**③ 与观察的吻合点** ✓：`__str__` 在 **`type`（元类）** 上确实是个**数据描述符**（`type.__str__` ✓）⇒
所以走 `__set__` 那条路 ✓ ⇒ 而那条路在本层对**类对象**做 `setattr` 时**把接收者搞成了 `None`** ✗ ✓。
**④ 判据**（修好后）✓：`target/ifmin1.py` 通过 ✓、`target/imp_markup.py` 打 `ok` ✓、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期这 **118** 族终于开始减少 ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只改语料副本并已还原 ✓、树干净 ✓）。

#### 第 282 轮：两条更小的变体**也全过** ✗ ⇒ 触发点是 **enum 专属**的；下一手＝**逐句插桩**（在副本里）

**① 本轮两次探针（`target/` ✓）** ✓：
```
setattrcls.py ：`C.foo = 1` ✓ ／ `C.__str__ = f`（Python 函数 ✓）✓ —— 参照相同 ✓
setnat.py     ：`C.__str__ = int.__str__`（**绑定的原生方法** ✓）✓ ／ `setattr(D, "__str__", int.__str__)` ✓
```
⇒ 逐条排除 ✓：**运行期给类 setattr** 不是触发点 ✗、**值是原生方法**也不是 ✗。
⇒ 结合第 281 轮的决定性数据（**在失败点之前**打印 ✓：`enum_class` 正常 ✓、`member_type` = `int` ✓、
`classdict` 普通 `dict` ✓）⇒ 触发点**收缩到 enum 那条路径本身** ✓，且**紧邻那一行** ✓。
**② 下一手（就一件 ✓，用"副本插桩"这招 ✓ —— 第 281 轮它就是最有产出的一招 ✓）**：
在 `target/lib-full/enum.py` 的**这三句**前后各插一条打印 ✓：
```python
                method = member_type.__str__            # ① 读
                if method is object.__str__: …          # ② 判
                enum_class.__str__ = method             # ③ 写
                classdict['__str__'] = enum_class.__str__  # ④ 写回字典
```
⇒ 打印 `"①ok"`／`"②ok"`／`"③ok"`／`"④ok"` ✓（外加 `method is None` 与 `type(method).__name__` ✓）
⇒ 一次就能看出**到底死在哪一句** ✗（我上一轮只插在 ③ 之前 ⇒ 只能知道"③ 及其后" ✗）。
**③ 为什么这招靠谱** ✓：语料副本可随意改 ✓、不动 VM ✓、跑完还原 ✓；
第 281 轮正是靠它把"`enum_class` 是 None"这个错误假设**当场推翻** ✓（否则我还会继续追元类 `__new__` ✗）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 281 轮：🎯🎯🎯 **决定性数据** —— `enum_class` 不是 None；失败的是「**给类对象设置属性**」这条运行期路径

**① 做法** ✓：在**语料副本**里插两行打印 ✓（`target/lib-full/enum.py` ✓，**不动 VM** ✓，跑完即还原 ✓）：
```python
print("DBG enum_class none:", str(enum_class is None), "member_type:", str(member_type),
      "classdict:", str(type(classdict).__name__))
enum_class.__str__ = method        # ← 原来的失败点
```
**② 输出** ✓：
```
DBG enum_class none: False   member_type: <class 'int'>   classdict: dict
pyawa: 未捕获（状态 1）：AttributeError: 'NoneType' object has no attribute '__str__' and no __dict__ for setting new attributes
```
⇒ **`enum_class` 是正常类对象** ✓、`member_type` 是 `int` ✓、`classdict` 是普通 `dict` ✓
⇒ 而**紧接着那一行仍然失败** ✗ ⇒ 所以报错里的 `NoneType` **不是 `enum_class`** ✗
⇒ 而是「**给类对象做 `setattr`**」这条**运行期路径**里的**接收者**被传成了 `None` ✗ ✓。
**③ 与前面几轮的对照（解释了为什么我一直找错）** ✓：
* `class Y: __str__ = None` ✓ 能过 ⇒ 那是**类体内的 `STORE_NAME`** ✓（写类命名空间 ✓，另一条路 ✓）；
* `enum_class.__str__ = method` ✗ 失败 ⇒ 那是**运行期对"类对象"的 `setattr`** ✓（本层多半走
  `type.__setattr__` 一族 ✓）⇒ **这条路是坏的** ✗。
**④ 下一轮（就一件 ✓，先最小化再读实现 ✓）**：
```python
class C:
    pass


def f(self):
    return "s"


C.foo = 1
C.__str__ = f
print("C ok")
```
⇒ 若这也失败 ⇒ **真 bug 抓到** ✓（"给类设置属性"整体坏 ✗ —— 那影响面极大 ✓，能解释 `re`／`enum` 一族 ✓）；
⇒ 若只对某些名字失败 ⇒ 收缩到那个名字/那条分支 ✓。随后读 `type.__setattr__`／`instance_attribute_set`
给**类型对象**的那条分支 ✓（`executor/protocol.rs` ✓）找接收者为何成 `None` ✗。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只改过语料副本并已还原 ✓、树干净 ✓）。

#### 第 280 轮：🎯 失败点定位到 `enum_class.__str__ = method` ⇒ **`enum_class` 是 `None`**（元类 `__new__` 返回 None 的嫌疑）

**① 实测一（内建类型的 `__str__`）** ✓（`target/intstr.py` ✓）：
```
本层：int／str／float／tuple 的 `__str__` 都**不是 None** ✓        参照**相同** ✓
```
⇒ 排除"`member_type.__str__` 取不到"这条 ✗（第 279 轮的猜测被否 ✓）。
**② 实测二（读 `enum.py` 570-590 ✓）** ✓：
```python
            if '__format__' not in classdict:
                enum_class.__format__ = member_type.__format__
                classdict['__format__'] = enum_class.__format__
            if '__str__' not in classdict:
                method = member_type.__str__
                if method is object.__str__:
                    method = member_type.__repr__
                enum_class.__str__ = method                    # ← 🎯 **失败点在这里**
                classdict['__str__'] = enum_class.__str__
        for name in ('__repr__', '__str__', '__format__', '__reduce_ex__'):
            …
```
⇒ 报错是"**往一个 `None` 设置属性 `__str__`**"✗，而这一行里的接收者是 **`enum_class`** ✓
⇒ 结论：**`enum_class` 在那时刻是 `None`** ✗ ✓ —— 也就是**元类 `__new__` 交回了 `None`** ✓。
**③ 于是下一轮（就一件 ✓）**：用最小探针**直接验"元类 `__new__` 的返回值"** ✓：
```python
class M(type):
    def __new__(mcls, name, bases, ns, **kw):
        r = super().__new__(mcls, name, bases, ns)
        print("new returns None:", str(r is None))
        return r


class C(metaclass=M):
    pass


print("C is None:", str(C is None))
```
⇒ 若 `new returns None: True` ⇒ **真 bug 抓到** ✓（元类 `__new__` 的返回被吞 ✗ —— 那会连带解释
`IntFlag`／`ReprEnum`／`EnumType` 一族以及 `re` 的失败 ✓）；若 `False` ⇒ 病在 `enum.py` 的更早一步 ✓
（例如 `enum_class = super().__new__(…)` 那行在**本层**被求成 None ✓ ⇒ 再往前读 ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 279 轮：🎯🎯🎯 **重大更正** —— 真最小例是 **`class X(enum.IntFlag): A = 1`**（5 行）；`__str__ = object.__str__` 那行是红鲱鱼

**① 两轮探针的结果** ✓：
```
第 278 轮的"dict 子类 + __prepare__"三变体：**全过** ✗（`__setitem__` 有无、值 None ✓ 都一样过 ✓）
本轮（target/ifmin0.py / ifmin1.py）：
  ifmin0（不带语料路径）：ModuleNotFoundError: No module named 'enum' ✗（预期 ✓，需 sys.path ✓）
  ifmin1（带路径，**只有** `class X(enum.IntFlag): A = 1`）：
        本层 = AttributeError: 'NoneType' object has no attribute '__str__' and no __dict__ for setting new attributes ✗
        参照 = X ok 1 ✓
```
⇒ **真最小例只有 5 行** ✓，而且**根本不需要**那行 `__str__ = object.__str__` ✗ ⇒ 我第 274–278 轮追的那行
是**红鲱鱼** ✓（如实更正 ✓：它只是 `A = 1` 之后**紧接着要执行的下一条语句** ✓，所以失败时报的就是它 ✓）。
**② 于是得到真正该查的地方** ✓：`enum.py` 里处理 **`member_type`** 的那段 ✓ ——
`grep` 早先给出过：
```
target/lib-full/enum.py:578:                method = member_type.__str__
target/lib-full/enum.py:583:                enum_class.__str__ = method
```
⇒ 若 `member_type`（`IntFlag` 情形下是 **`int`** ✓）**取不到 `__str__`** ✗ ⇒ `method` 成了 `None` ✗
⇒ 而**下一步却去** `enum_class.__str__ = None` ✓ ⇒ 报错信息里"往 None 设置 `__str__`"✗
—— 这正与"**`enum_class` 在那一刻是 `None`** ✗"或"**`member_type.__str__` 的读取把目标搞成了 `None`**"吻合 ✓。
**③ 下一轮（就一件 ✓）**：读 `target/lib-full/enum.py` 的 **570-590 行** ✓（那段 `member_type.__str__`／
`__format__` 的处理 ✓）＋ 用最小探针核 **`int.__str__` 在本层是什么** ✓（期望：`int` 的 `__str__` 取得到 ✓）：
```python
print(str(int.__str__ is None))
```
⇒ 若为 `True`（取不到 ⇒ 成了 None ✓）⇒ 就去补 **内建数据类型（至少 `int`）的 `__str__`** ✓
—— 与本会话第 241／271 轮"给 `object`／`NoneType` 补 `__str__`"**同一套路** ✓，而且这次是**内建 `int`** ✓
（参照里 `int.__str__` 当然存在 ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 278 轮：🎯 触发点收窄到**"dict 子类命名空间"**（普通类全过 ✓；只有 `enum.IntFlag` 那版失败 ✗）

**① 本轮两次探针（全过 ✗ ⇒ 逐条排除）** ✓：
```
第 277 轮（target/storemin2.py）：A `x = object.__str__` ✓ ／ B `__str__ = lambda` ✓ ／ C `y = object.__repr__` ✓
第 278 轮（target/storemin3.py）：P `__str__ = object.__str__` ✓ ／ Q 再加 `A = 1` ✓   参照均相同 ✓
```
⇒ 逐条排除 ✓：**名字**不是触发点 ✗、**值 `None`/`5`** ✗、**存原生** ✗、**`A = 1` 在前** ✗。
**② 剩下的唯一差异** ✓：我的 6 行最小例**导入了 `enum` 且以 `enum.IntFlag` 为基类** ✓
⇒ 于是**类命名空间不是普通 `dict`** ✓，而是元类 `__prepare__` 返回的 **`_EnumDict`（dict 子类）** ✓
⇒ 病根落在「**往 dict 子类里存项**」这条路上 ✗（`_EnumDict.__setitem__` 是本层要走的 dict 子类路径 ✓）。
这**与第 191／192 轮那次"dict 子类命名空间 + 元类"完全同族** ✓（当时是内存双放 ✓，已修 ✓；
**这次是控制/写入路径** ✗ —— 同一片代码的另一个问题 ✓）。
**③ 下一轮（就一件 ✓）**：用**最小 dict 子类 + `__prepare__`** 复刻 ✓（照第 191 轮 `target/nsmin.py` 的形状 ✓）：
```python
class D(dict):
    def __setitem__(self, k, v):
        super().__setitem__(k, v)


class M(type):
    @classmethod
    def __prepare__(mcls, name, bases, **kwds):
        return D()


class C(metaclass=M):
    __str__ = object.__str__


print("C ok")
```
⇒ 若复现 ⇒ 病在**双下划线名字 + dict 子类 `__setitem__`** 的交互 ✓（或 `object.__str__` 求值时的绑定 ✓）
⇒ 再二分：把 `__setitem__` 去掉 ✓、把 `super().__setitem__` 换成别的 ✓、把值换成 `None` ✓。
**④ 判据**（修好后）✓：`target/intflagmin.py` 通过 ✓、`target/imp_markup.py` 打 `ok` ✓、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓（预期这 118 族**终于开始减少** ✓）。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 277 轮：🎯 触发点＝"把**原生函数对象**存进类体"（读没问题 ✓、换名字也一样 ✓ —— 见本轮探针输出）

**① 已知（第 276 轮 ✓）**：`v = object.__str__` 单独读**正常** ✓（`v is None` 为 `False` ✓，与参照一致 ✓）；
⇒ 触发需要"**读出来并存进类体**"这个组合 ✓。
**② 本轮分流探针（`target/storemin2.py` ✓）** ✓：
```python
class A:
    x = object.__str__          # 换了名字（不是 __str__）✓
class B:
    __str__ = (lambda self: "s")  # 换成 Python 函数 ✓
class C:
    y = object.__repr__          # 换另一个原生 ✓
```
结论见本命令输出 ✓（下一轮据此改 ✓）。
**③ 若猜测成立**（"存**原生**值 ⇒ 触发"✓）⇒ 病根在**类体存原生函数对象**这条路上 ✓
⇒ 很可能就是 **第 241／271 轮我往类型命名空间塞原生方法**留下的**副作用**相关同一片代码 ✓
（`instance.rs` 的 dunder 注册表 ⇒ 它是"把原生包装后 `dict_set`"✓ ⇒ 而**类体存储**走的是另一条路 ✓
⇒ 两条路在**同一处**（`dict_set`／属性写入）汇合 ✓ ⇒ 值得对比 ✓）。
**④ 下一轮（就一件 ✓）**：按本轮输出定位到**存储路径** ✓ ⇒ 读 `STORE_NAME` 的实现 ✓
（`executor.rs` ✓；类体帧的 `STORE_NAME` 写命名空间 ✓）＋ 对比"存 Python 函数（B ✓）"与"存原生（A／C ✗）"
在**值类型判断**上的差别 ✓ ⇒ 找到后修 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 276 轮：🎯 触发点锁定**值本身** —— `object.__str__`（三个变体全过 ✓）

**① 三变体探针（`target/storemin.py` ✓）** ✓：
```
本层 Y ok（`__str__ = None` ✓）／Z ok（`foo = None` ✓）／W ok（`__str__ = 5` ✓）—— 参照**相同** ✓
```
⇒ 触发点**不是名字 `__str__`** ✗、**也不是值 `None`** ✗ ⇒ 只剩**那个值 `object.__str__` 本身** ✓。
**② 于是当场验"读 `object.__str__`"** ✓（`target/readstr.py` ✓，结果见本命令输出 ✓）⇒ 结论下一轮补记 ✓。
**③ 下一轮（就一件 ✓）**：按本轮结果分流 ✓：
* 若**读**就失败 ⇒ 病在 **`type` 的属性读取**（`object.__str__` 那条路 ✓）⇒ 去读
  `executor/attribute.rs` 里"类型对象取 dunder"的分支 ✓（第 244 轮加 `__mro__` 就在那儿 ✓）；
* 若**读**正常、而"读+存"才失败 ⇒ 病在**类体里存这个值**的路径 ✓（那就是 `STORE_NAME` 与
  某类**描述符/绑定**的交互 ✗ ⇒ 看第 241／271 轮往类型命名空间塞 `__str__` 时有没有留下副作用 ✓）。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 275 轮：✅ **6 行最小复现**（`__str__ = object.__str__` 在类体里）；语料那个类是**无基类**的普通类

**① 最小例** ✓（`target/intflagmin.py` ✓）：
```python
import enum


class X(enum.IntFlag):
    A = 1
    __str__ = object.__str__


print("X ok", str(X.A))
```
```
本层：AttributeError: 'NoneType' object has no attribute '__str__' and no __dict__ for setting new attributes ✗
参照：X ok <X.A: 1> ✓
```
**② 语料里的真身** ✓：`re/__init__.py` 144 行的类是 **`class RegexFlag:`**（**没有基类** ✓，
不是枚举/元类路径 ✓）⇒ 所以我上一轮"元类/`__prepare__`"的猜测**要收窄** ✗（如实 ✓）；
但我的最小例用了 `enum.IntFlag` 也同样复现 ✓ ⇒ 说明**触发点就在那句赋值本身** ✓（与基类无关 ✓）。
**③ 于是下一轮（就一件 ✓）**：再缩两行 ✓：
```python
class Y:
    __str__ = None
print("Y ok", str(Y))
```
⇒ 若也报同一条 ✓ ⇒ 病在 **`STORE_NAME` 把 `None` 存进类命名空间**这条路上 ✗
（消息点名 `__str__` ✓、目标却是 `None` ✗ ⇒ 很像"把**待存的值**当成了**目标对象**" ✗ —— 一句话：
**值／目标**弄反 ✓）；若不报 ⇒ 病在"读 `object.__str__` 得到 `None`"那半步 ✓。
**④ 判据**（下一轮）✓：最小例通过 ✓、`target/imp_markup.py` 打 `ok` ✓、**逐字节 4/4** ✓、
workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 274 轮：🎯🎯🎯 **罪魁行**：`re/__init__.py:155` 的 `__str__ = object.__str__`（**类体里的名字赋值**）

**① grep 结果（第 273 轮尾随）** ✓：
```
target/lib-full/re/__init__.py:155:    __str__ = object.__str__
target/lib-full/enum.py:583:                enum_class.__str__ = method
target/lib-full/enum.py:1720:        cls.__str__ = global_str
```
⇒ 第 3 项是 `setattr(cls, …)` 同形 ✓；第 1 项**最可疑** ✓：它是**类体**里的**名字赋值** ✓，
而报错恰恰来自 `re` ✓（第 273 轮的导入链 ✓）。
**② 由此推出的机制** ✓（下一轮先验 ✓）：类体里的 `__str__ = object.__str__` 走的是 **`STORE_NAME`** ✓
⇒ 在类体帧里，`STORE_NAME` 应当写进**类命名空间** ✓ ⇒ 报「**往一个 `None` 设置 `__str__`**」✗
⇒ 说明那条路径上**目标对象（命名空间）是 `None`** ✗ —— 即 **类体帧的命名空间没建立/是 `None`** ✓
⇒ 这很可能与 `re/__init__.py` 里那个类的**元类/`__prepare__`** 路径有关 ✓（`re` 依赖 `enum` ✓，
而 `enum` 的 `__prepare__` 返回 `_EnumDict` ✓ —— 我们前几轮在 `IntFlag(int, ReprEnum, …)` 一族上刚打过交道 ✓）。
**③ 下一轮（就一件 ✓）**：读 `re/__init__.py` 那个类的**类头**（145-158 行 ✓，本轮已打印 ✓）⇒
用**最小例复刻它** ✓（例如同形的 `class X(enum.IntFlag): __str__ = object.__str__` ✓）⇒
看是不是"元类/`__prepare__` 路径下类体命名空间为 `None`"✗ ⇒ 是则修 `__prepare__`／类体帧的绑定 ✓。
**判据** ✓：最小例通过 ✓、`target/imp_markup.py`（`import _markupbase` ✓）能打印 `ok` ✓、
**逐字节 4/4** ✓、workspace／对拍／`check.py` ✓；再跑受管后台重测 ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 ✓、树干净 ✓）。

#### 第 273 轮：🎯 真身找到方向 —— 某处**给一个 `None` 设置属性 `__str__`**；错误**出在 `re` 里**（`enum` 已通 ✓）

**① 探针与参照对照** ✓（`target/noneattr.py` ✓）：
```
本层 A: 'NoneType' object has no attribute 'foo' and no __dict__ for setting new attributes
参照 A: **完全相同** ✓
本层 B: None ✓    参照 B: None ✓
```
⇒ 第 272 轮"往 None 写属性"的读法**本身是对的** ✓，但**属性名不是随便一个** ✗ ——
那条族的消息里点名的是 **`__str__`** ✓ ⇒ 真正失败的操作是
**`setattr(<某个 None>, "__str__", …)`** ✗（或 `None.x.__str__ = …` 同形 ✓）。
**② 导入链（`PYAWA_TRACE_IMPORT=1` ✓）** ✓：
```
[载入] 模块 enum 执行完：命名空间 32 个名字     ← **enum 通了** ✓（编译器 break 修复的效果 ✓）
[载入] 模块 re 执行出错：Raised { exception: … }  ← 🎯 **错误出在 re 里** ✗
[载入] 模块 _markupbase 执行出错：同一条 ✓（只是传播 ✓）
pyawa: … AttributeError: 'NoneType' object has no attribute '__str__' and no __dict__ …
```
**③ 于是下一轮（就一件 ✓）**：在语料里找"**给 `__str__` 赋值/设置**"的地方 ✓：
`grep -rn "__str__ *=" target/lib-full/re/*.py target/lib-full/enum.py | head` ✓
（`enum.py` 里那句 `setattr(obj, '__reduce_ex__', _break_on_call_reduce)` ✓ 是**同款写法** ✓ ——
很可能还有 **`__str__` 的同款** ✓，而 `obj` 在某些路径上是 `None` ✗）
⇒ 找到后就能判定：是**本层把某个表达式求成了 `None`** ✗，还是**某条路径下 obj 本就是 None** ✓。
**④ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓（第 272 轮实测 ✓）；
**未声称任何阶段完成** ✓；本轮**无仓库内代码改动** ✓（只读 + `target/` 探针 ✓、树干净 ✓）。

#### 第 272 轮：⚠️ **`NoneType.__str__` 没清掉那一族**（消息一字不变 ✗）；但读出了**更准的形状**

**① 受管作业实测（02:44／02:46 ✓）** ✓：
```
上限诊断：能 import **162** 个（25.8%）（仍同 ✓）
     118  AttributeError: 'NoneType' object has no attribute '__str__' and no __dict__ for setting new attributes
      77  NameError: name 'eval' is not defined
      28  SyntaxError：annotationlib（第 327 行 列 18-20）
      19  _struct ／ 9 binascii ／ 8 complex ／ 8 xml.dom ／ 7 module.warnoptions
判据①：172 ÷ 628 ⇒ **27.4%** ✓（不变 ✓）／进度指标 156÷283 ⇒ **55.1%** ✓
```
⇒ **118 一个没少** ✓、消息**逐字不变** ✗ ⇒ 第 271 轮那处接线**没有解除**这条 ✗（如实 ✓）。
**② 但消息的**后半句**给出了更准的形状** ✓：
```
… and no __dict__ for setting new attributes
```
⇒ 这是本层**"往对象上写属性"失败**时那条消息 ✓（不是"读不到 `__str__`"那么简单 ✗）⇒
**真正失败的操作是"给某个 `None` 写属性"** ✗ —— 也就是某处代码走了 `x.attr = value` 而 `x` 是 `None` ✓。
**③ 于是有两种可能（下一轮分辨 ✓）**：
1. **本层把某个表达式求成了 `None`** ✗（本该是对象 ✓）⇒ 那要找出是哪个表达式 ✓；
2. 或者纯属**消息里 `__str__` 是"报错时要把属性名格式化"的副产物** ✓（属性名拼装那一步用了 `__str__` ✓）
   ⇒ 那就说明真正缺的还是 `__str__` ✓ —— 但我已经补了 ✗ ⇒ 说明**补的地方不是 `None` 实际用的那个类型** ✗
   （例如 `None` 的 `ty` 不是 `type_named("NoneType")` ✗，或者该查找走的是**另一条**路 ✓）。
**④ 下一轮（就一件 ✓）**：**先验"补的地方对不对"** ✓ —— 写一个最小探针，打印
`type(None).__name__`、`type(None) is NoneType`（若能取到 ✓）、以及"在一个 `None` 上写属性"时**具体怎么失败** ✓：
```python
print(str(type(None).__name__))
x = None
try:
    x.foo = 1
except AttributeError as e:
    print(str(e))
```
⇒ 如果 `x.foo = 1` 报的正是那条**一字不变**的消息 ✓ ⇒ 说明这条族的真身是"**语料里某处 `x.attr = …`**" ✗
⇒ 那就用 `PYAWA_TRACE_IMPORT=1` 跑一个受害模块（如 `_markupbase` ✓）把**导入链**打出来 ✓，
定位是哪个表达式给了 `None` ✓；若**不报**⇒ 说明另有出处 ✓。
**⑤ 数字与交代** ✓：判据① **27.4%（172÷628）** ✓、上限 **162** ✓、进度 **55.1%** ✓ 均为**本轮实测** ✓；
**未声称任何阶段完成** ✓；本轮**无代码改动** ✓（只读 + 记录 ✓、树干净 ✓）。

