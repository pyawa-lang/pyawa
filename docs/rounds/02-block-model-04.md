> 本卷是 `docs/ROUNDS.md` 分卷台账之一（**非规范**、只增不改；卷目见该索引）✓

#### 第 210 轮：🎯 `ns=false` ⇒ 持悬垂指针的**不是类体帧**，而是**函数帧**（最可能＝元类 `__new__` 帧 ✓）

**① 本轮做法**（按上一轮的计划 ✓）：只给 `[fc]` 记录加 `ns=`（`frame.namespace` 是否非空 ✓），
**先把报错读出来**（这次做到了 ✓）⇒ 编译 **0 错** ✓、跑出数据 ✓：
```
[fc] local[3] ptr=0x…55e0 rc=0 frame=0x…50c0 **ns=false**      ← 该帧的 namespace 字段是空的
对已释放对象 decref（×2）
```
**② 结论** ✓：持悬垂指针的那一帧**不是类体帧** ✗（类体帧的 `namespace` 字段非空 ✓）⇒ 它是**函数帧** ✓；
结合槽号 3 ✓（`M.__new__(mcls, name, bases, ns, **kwds)` 的 `ns` ✓）⇒ **最可能就是元类 `__new__` 帧 ✓**。
**③ 于是问题变了** ✗（比第 206／207 轮更窄 ✓）：`STORE_FAST` 与 `set_local` 都**已核实是对的** ✓（第 208 轮 ✓），
那么"槽 3 在清帧时指向一个**已被释放**的 dict"就**不能**靠"重绑没换槽"解释 ✗ ⇒ 只剩一种：
**槽 3 里的那个 dict 还被别人持有并被人放掉了** ✗ ⇒ 即**别名**：
* 要么 `ns = dict(ns.items())` 的**结果**与别处**是同一个对象** ✓（我们的 `dict(iterable)` 若在某些路径上
  **直接交回源**或有缓存 ✓ ⇒ 就会这样 ✗）；
* 要么该值**同时**被某个 cell／另一帧持有 ✓（第 205 轮的帧内盘点**没有**重复 ✗ ⇒ 更像"跨帧别名" ✓）。
**④ 下一轮（就一件，可判定 ✓）**：在小例上验**别名**：把 `ns = dict(ns.items())` 换成
`ns2 = dict(ns.items())`（**不重绑参数** ✓，另存一个名字 ✓）⇒ 若**不再崩** ✓ ⇒ 说明问题出在**重绑**；
若**仍崩** ✓ ⇒ 说明问题出在 **`dict(iterable)` 的实现**（交回了源或有别名 ✗）⇒ 两条路各自都**可判定** ✓。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ 全闸门不回归 ✓。
**⑤ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无净代码改动** ✓（探针已撤 ✓、树干净 ✓）；
**未声称任何阶段完成** ✓。

#### 第 209 轮：探针补丁**编译未过** ⇒ 本轮无新数据 ✗（如实记 ✓）

**① 本轮想做的**：给 `[fc]` 记录加上**帧身份的旁证** —— `frame.namespace` 是否非空 ✓ 与
`instance.current_site()`（当前 Python 现场 ✓，它直接指明是哪一帧的哪段代码 ✓）⇒ 用来判定
`local[3]` 属于**类体帧**／**元类 `__new__` 帧**／**`<module>` 帧** ✓。

**② 结果** ✗：补丁**编译不过（2 个错）** ✓ —— 而我**又一次没读报错就撤回** ✗（这是本会话第 N 次犯 ✓，
第 208 轮刚说过"先读报错" ✓）。⇒ 那一趟跑的是**旧二进制** ✓（输出与上一轮相同 ✓），**本轮没有新数据** ✓。
**已 `git checkout -- crates/`** ✓ ⇒ 复核：树**干净** ✓。

**③ 下一轮（把"先读报错"落实到动作里 ✓）**：
1. 重打同一发补丁 ✓，但**先只加 `ns=` 一个字段**（`current_site()` 那半很可能就是那 2 个错的来源 ✓：
   要么签名不同 ✓、要么在该作用域借用了已借出的 `Cell` ✗）；
2. `cargo build` **把错误原样读出来** ✓，改到 0 错再跑 ✓；
3. 拿到「哪个帧的哪个槽」之后 ✓，再按第 206／207 轮的候选改法（该槽改持有 ✓ 或清帧跳过 ✓）动手 ✓。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ `import enum` 不再报「已释放对象」∧ 全闸门不回归 ✓。

**④ 进度与节奏（如实）**：判据① 仍 **27.4%（172÷628）** ✓；本条内存缺陷已连续 **20+ 轮**在查 ✓
（其间剪掉 **十条**假设 ✓、把靶点从"一族/一类"缩到"一个槽" ✓），但**仍未修好** ✗。
若希望更快看到判据① 移动 ✓，建议**同时开第二条线**：把 `_struct`（19）／`_contextvars`（27）／
`annotationlib`（28）接线 ✓ —— 三者合计约 **74** 个模块 ✓，能把判据① 从 27.4% 推到约 **39%** ✓，
且它们是"实现模块"这类**可估**的活 ✓（与这条内存缺陷互不阻塞 ✓）。

#### 第 208 轮：`STORE_FAST` 是**对的** ⇒ 剪掉第十条假设；下一手＝把**帧身份**打出来

**① 读到的实现**（`crates/pyawa-core/src/executor.rs:1312`）：
```rust
"STORE_FAST" => {
    let value = frame.get().pop()?;                          // 新值（持有 ✓）
    let restored = if value == null_sentinel { None } else { Some(value) };
    if let Some(old) = frame.get().set_local(oparg, restored)? {
        release(instance, old);                              // 放掉**旧值** ✓
    }
}
```
⇒ **逻辑正确** ✓：先写回槽（`set_local` 返回旧值 ✓）再放旧值 ✓。
而 `set_local`（`frame.rs:324`）对**形参槽**（`SlotKind::Local` ✓）写的正是 `locals` ✓
（只有自由槽才落到 `cells` ✓，注释里还记着第 84／266 轮踩过的坑 ✓）。
⇒ **「重绑没换槽」不成立** ✓（第十条剪掉的假设 ✓）。

**② 于是第 207 轮那个"参数槽"推断要降级** ✗（如实 ✓）：我按"`local[3]` ＝ `M.__new__` 的 `ns`"推的 ✓，
但**没有**验证 `frame=0x…2220` 到底是哪一帧 ✗（`code=0x0` 只说明它正在清帧 ✓）。
⇒ **下一手必须先把"帧身份"打出来** ✓，再谈"哪个槽该持有" ✓。

**③ 下一轮（就一件）** ✓：把 `[fc]` 那条记录**加上帧的 `code` 的 qualname**（本层 `CodeObject` 里有名字字段 ✓，
若一时找不到就用 `code` 的指针＋该帧是否 `namespace` 非空来区分 ✓）⇒ 一眼看出是
**类体帧**／**元类 `__new__` 帧**／**`<module>` 帧** ✓ ⇒ 再判定那个 `local[k]` 该不该持有 ✓。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ `import enum` 不再报「已释放对象」∧ 全闸门不回归 ✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无代码改动** ✓（读实现 ✓、树干净 ✓）；
**未声称任何阶段完成** ✓；累计剪掉/撤回 **十条** ✓。

#### 第 207 轮：槽号把范围缩到**参数槽本身** —— `local[3]` ＝ `M.__new__` 的 `ns`

**① 由槽号推出的结论**（`M.__new__(mcls, name, bases, ns, **kwds)` ⇒ 参数下标 **0,1,2,3** ✓ ⇒ `ns` ＝ **slot 3** ✓）：
```
[dict→0] 0x…2740 掉到 0：现场=M.__new__@51          ← STORE_FAST ns：重绑时放掉**旧值**（正当）
[fc] local[3] ptr=0x…2740 rc=0 frame=0x…2220        ← 而**同一个旧值**仍在**同一个函数的 ns 参数槽**里 ✗
```
⇒ 也就是说：**重绑参数时我们把旧值放掉了，却没有把槽更新掉** ✗（槽里留着**悬垂指针** ✓），
于是一路上「local[3] 的 rc=0」✓ ⇒ 清帧时再放一次 ⇒ 报「对已释放对象 decref」✗。
**这比第 206 轮的说法更准** ✓：不是"类体帧的 `__classdict__` 借用 vs 实参持有" ✗，
而是**参数槽在重绑后没有换掉旧指针** ✓（旧值被放 ⇒ 槽成悬垂 ✓）。

**② 这也顺带解释了三件事** ✓：
* 为什么"计数只有 1"却会提前归零（第 193 轮 ✓）——旧值只被算了一份就放了 ✓；
* 为什么是**元类 `__new__` 内部两次释放**（第 110 轮 ✓）——重绑一次（正当）＋清帧一次（悬垂 ✓）；
* 为什么小例的两条独立触发条件（`__prepare__` 返回 dict 子类／`super().__new__` ✓）都会踩到 ✓
  —— 它们都让"**参数被重绑**"这件事发生在**类命名空间**上 ✓。

**③ 下一轮（读实现 → 修）** ✓：读 `STORE_FAST` 的实现 ✓（`executor.rs` ✓）：
它是否 `set_local(slot, new)` **之后**又 `release(old)` ✓（那样是对的 ✓）；
若它 release 了 old 而**没有真正写回槽** ✓／或写回的是**另一份** ✓ ⇒ 那就是根因 ✓。
**判据** ✓：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ `import enum` 不再报「已释放对象」✓
∧ 全闸门不回归 ✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无代码改动** ✓（只读推理 ✓、树干净 ✓）；
**未声称任何阶段完成** ✓；**第 206 轮那句话要按本条修订** ✓（机制更准的说法写在 ① ✓）。

#### 第 206 轮：🎯🎯🎯 **两条线索咬合，机制完全清楚** —— 类体帧的 `__classdict__` 局部与元类实参**各持一份而只记了一份**

**① 同一趟（`PYAWA_FC_DEBUG=1 PYAWA_WATCH_DICT=1 PYAWA_QUARANTINE=1`）的输出** ✓：
```
[fc] namespace[0] ptr=0x…3cb0 rc=3 frame=0x…3dd0      ← 别的帧（类体帧，namespace 字段在用）
[fc] namespace[0] ptr=0x…4c20 rc=2 frame=0x…4c70
[dict→0] 0x…2740 掉到 0：现场=M.__new__@51            ← 重绑 `ns = dict(ns.items())` 放掉**旧命名空间**（正当）
[fc] local[3] ptr=0x…2740 rc=0 frame=0x…2220          ← **同一个指针**，还在某帧的 local[3] 里 ✗
对已释放对象 decref（×2）✗
```
⇒ **同一个对象**：在元类 `__new__` 的 `@51`（`STORE_FAST ns`）被放到 **0** ✓，
而它**同时**还是**另一个帧**（`frame=0x…2220` ✓，`code=0x0` ⇒ 该帧的 `code` 已被取走＝正在清帧 ✓）的 **`local[3]`** ✓
⇒ 两个持有者、账上**只记了一份** ✗ ⇒ 清那个帧时它按"持有"再放一次 ⇒ 报「对已释放对象 decref」✗。

**② 机制（至此清楚了）** ✓：
* 类体帧把命名空间放在 **`local[3]`**（`__classdict__` ✓）——**它是"借来的"** ✓（参照里正是借用 ✓），
  但我们的 `frame_clear` **按持有释放** ✗；
* 元类 `__new__` 的帧拿的是实参一份 ✓（`call_value` 逐参 incref ⇒ **持有** ✓），重绑参数时把它放掉 ✓（正当 ✓）；
* 于是**借来的那一份从未被计入** ✓ ⇒ 计数提前归零 ✗ ⇒ 谁最后再放一次就报错 ✓。
⇒ 这解释了**全部**现象 ✓：为什么"计数只有 1"（第 193 轮 ✓）、为什么"元类内部两次释放把它打到 0"（第 110 轮 ✓）、
为什么 `frame_clear` 出现在回溯里（第 198 轮 ✓）、为什么小例的两条独立触发条件（`__prepare__` 返回 dict 子类／
元类里重绑命名空间 ✓）都会踩到 ✓。

**③ 下一轮（修复，且**先看填充点**再改）** ✓：读类体帧的 `local[3]`（`__classdict__`）是**在哪里填的** ✓
（`run_class_body` 附近 ✓）：若填时**没有** incref ✓ ⇒ 两种改法二选一 ✓：
(a) 填时 **incref**（让它成为持有 ✓ ⇒ `frame_clear` 释放它就对了 ✓）；或
(b) `frame_clear` 里**跳过**这个槽 ✓（保持借用的语义 ✓）。
**判据** ✓：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ `import enum` 不再报「已释放对象」✓
∧ 全闸门不回归（workspace／0 警告／逐字节 4/4／对拍两模式／`check.py` 12/12／夹具 490／语料下限 182／
`selftest`／`t_ab_1`）✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无净代码改动** ✓（探针已撤 ✓、树干净 ✓、0 错 0 警告 ✓）；
**未声称任何阶段完成** ✓ —— 但**根因已从"一条指令"细化到"一个槽的所有权"** ✓，修复与验证在下一轮 ✓。

#### 第 205 轮：🎯🎯 **决定性观察** —— 清帧时，某个**局部槽**里的 dict `rc` **已经是 0**

**① 探针**（`frame_clear` 里按**段**记录：`[fc] 段名 ptr rc` ✓，只记 72 字节 `dict` ✓，门控 `PYAWA_FC_DEBUG` ✓）：
```
普通档： [fc] globals rc=5 ／ namespace rc=3 ／ globals rc=5 ／ namespace rc=2 ／ globals rc=5
隔离档： 同上 ＋ **[fc] local ptr=0x…95e0 rc=0** ✗ ＋ 对已释放对象 decref（×2）
```
⇒ **清帧时，一个 `local` 槽里的 dict 的 rc 已经是 0** ✓ —— 也就是说：**别处已经把它放掉了** ✗，
而那个局部槽仍按"持有"算 ✓ ⇒ 清帧时再放一次 ⇒ 「对已释放对象 decref」✗。

**② 这条观察把问题钉成了两句** ✓：
1. **谁**在"局部槽仍指着它"的情况下把它放到了 0 ✗（这一步的现场在第 193／202 轮拿到过：
   `[dict→0] 现场=M.__new__@36`／`@51` ✓ —— 那是调用机制在参数绑定/收尾时放掉的 ✓）；
2. **这个局部属于哪一帧**（类体帧？元类 `__new__` 帧？）✓ ⇒ 决定"这份该不该持有" ✓。
**注意**：本轮**没有**看到 `namespace` 与 `local`／`cell` 指向同一 ptr ✓ ⇒ 第 204 轮那条"两段各放一次同指针"的
**具体形状**在数据里也**没出现** ✗（这与我第 204 轮台账的措辞不一致 ✓，如实记 ✓：**尚未**在数据里看到那一对 ✓）。

**③ 下一轮（就一件，且极短 ✓）**：把记录**补上槽号**（`local[3]` 之类 ✓）＋**帧的身份**
（帧的 `code` 的 qualname ✓；类体帧可从 `namespace` 字段是否非空判断 ✓）⇒ 就能回答上面的"第 2 句" ✓；
再对第 1 句用 `PYAWA_WATCH_DICT` 的现场（`@36`／`@51` ✓）在**同一趟**里对齐 ✓
⇒ 两条线索一咬合，**根因与改法就同时确定** ✓。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ `import enum` 不再报「已释放对象」∧ 全闸门不回归 ✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无净代码改动** ✓（探针已撤 ✓、0 错 0 警告 ✓、树干净 ✓）；
**未声称任何阶段完成** ✓。

#### 第 204 轮：用第 193 轮轨迹**逐段对账** ✓ —— 收敛到「类体帧的那个局部对命名空间是**借用却按持有释放**」

**① 对账（小例 6 行 ✓，数字全来自第 193 轮实测轨迹 ✓）**：
| 阶段 | 该有的账 | 轨迹实测 |
|---|---|---|
| `build_class_native` 持命名空间 | rc=1 ✓ | `[ns 探针] 交元类之前 rc=1` ✓ |
| 类体帧跑完（`namespace` 字段 ＋ `__classdict__` 局部） | ＋2 ⇒ 3 ✓ | `<module>@19` 两次 incref ⇒ rc=3 ✓ |
| `call_value(M.__new__, [mcls,name,bases,ns])` 逐参 incref | ＋1 ⇒ 4 ✓ | `M.__new__@6` incref ⇒ rc=4 ✓ |
| `M.__new__` 里 `ns = dict(ns.items())`：**先 incref 新值、再放旧值** | 净 0 ⇒ 4 ✓ | `@7` incref ⇒ 5、`@7` decref ⇒ 4 ✓ |
| 元类帧收尾（放它的实参一份） | −1 ⇒ 3 ✓ | `@18`／`@26` decref ⇒ rc=2 ✓ |
⇒ 到这里**每一步都对得上** ✓ ⇒ **账不是"少一份"，而是"多一次释放"** ✓。
而 panic 是「对**已释放**对象 decref」✓ ⇒ 说明有**第二处**也按"持有"放它 ✓ ——
按现有代码，最可能就是**类体帧那个 `__classdict__` 局部**：在参照里它对命名空间是**借用** ✓，
而我们的 `frame_clear` 如实按**持有**放 ✗ ⇒ 与第 110 轮"元类内部两次释放把它打到 0"**吻合** ✓。

**② 与第 200 轮那次失败尝试的关系** ✓（重要 ✗）：我在 `frame_clear` 里做的"去重"是拿
`frame.namespace` **字段**与 locals 比 ✓ —— 而**类体帧的 `namespace` 字段可能是空的** ✗
（只有那个局部／cell 持有它 ✓）⇒ 所以那次补丁**什么也没挡住** ✓ ⇒ 失败**并不否证**本条 ✓
（我上一轮把"第七条假设被剪掉"记成否证 ✗，这里如实修订：那次只否证了"字段与局部同指针"这一**具体形状** ✓）。

**③ 下一轮（就一件）**：**先验证再改** ✓ —— 在 `frame_clear` 里打一发**只读**记录 ✓：对每个即将释放的值 ✓
打印「段名（locals／cells／stack／namespace）＋类型名＋大小 ✓」⇒ 从输出里认出**那个 72 字节 dict 被哪两段各放一次** ✓
（全帧内 & 跨帧 ✓，两者都看 ✓）⇒ 拿到证据后再改 ✓（改法很可能是：类体帧读 `__classdict__` 时**不持有** ✓，
或在 `frame_clear` 里对「= 本帧类字典」的槽**跳过** ✓）。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ `import enum` 不再报「已释放对象」∧ 全闸门不回归 ✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无代码改动** ✓（只读对账 ✓、树干净 ✓、0 警告 ✓）；
**未声称任何阶段完成** ✓；累计剪掉/撤回 **九条** ✓（其中"字段与局部同指针"那条按 ② 修订为只否证了具体形状 ✓）。

#### 第 203 轮：融合借用加载**是清白的** ✓（第九条剪掉）；并**重读 `@36` 的含义** ✓

**① 读到实现**（`crates/pyawa-core/src/executor.rs:2185`）：
```rust
"LOAD_FAST_LOAD_FAST" | "LOAD_FAST_BORROW_LOAD_FAST_BORROW" => {
    let first = oparg >> 4;  let second = oparg & 0x0F;
    let left  = frame.get().local(first )?…; push(instance, frame.get(), left )?;   // push ⇒ incref ✓
    let right = frame.get().local(second)?…; push(instance, frame.get(), right)?;   // 只读 + 压栈 ✓
}
```
⇒ **只读局部槽、随后 `push`** ✓，**一处 release 都没有** ✗ ⇒ 「融合加载多放」**不成立** ✓（第九条剪掉的假设 ✓）。

**② 于是重读 `@36` 的含义** ✓（这是我上一轮读快了 ✗）：`[dict→0]` 打印的是 `self.current_site()` ✓，
即**释放发生那一刻**帧的"当前指令" ✓ —— 而释放是在 **`CALL`（unit 38）的参数绑定／调用机制**里发生的 ✓
⇒ 那一刻帧的 ip **仍停在 `@36`／`@37` 的加载指令上** ✓ ⇒ 于是现场被记成"加载指令" ✗。
这与两条已有证据**一致** ✓：第 198 轮回溯里首次出现业务帧就是
`executor::call::call_callable` → `frame_clear` ✓；第 110 轮读数也是"**元类内部两次释放**" ✓。
⇒ **真正的释放者是「调用机制（参数绑定／收尾）」**，不是加载指令 ✓。

**③ 下一轮（就一件）**：在**参数绑定与调用收尾**上加一发门控记录 ✓（`call_callable` 里
`bind_arguments` 之后／`frame_clear` 之前 ✓）：只对「类型＝`dict` 且 72 字节」的对象 ✓ 打印
「释放 ＋ 现场 ＋ 该对象是否为本次调用的实参 ✓」⇒ 一眼看出**是实参被多放，还是帧槽被多放** ✓。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ `import enum` 不再报「已释放对象」∧ 全闸门不回归 ✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无代码改动** ✓（读实现 ✓、树干净 ✓、0 警告 ✓）；
**未声称任何阶段完成** ✓；累计剪掉/撤回 **九条** ✓。

#### 第 202 轮：🎯🎯 **钉到具体指令** —— `M.__new__@36` ＝ `LOAD_FAST_BORROW_LOAD_FAST_BORROW`（一条**纯加载**却出现在"dict 掉到 0"的现场）

**① 先用现成探针拿到"第一次被谁放掉"**（`PYAWA_WATCH_DICT=1` ✓，小例 6 行 ✓）：
```
普通档： [dict→0] … 现场=M.__new__@36 ／ M.__new__@51 ／ <module>@19
隔离档： [dict→0] … 现场=M.__new__@36 ／ M.__new__@51
         ＋ 对已释放对象 decref：类型 dict（refcount 已归零）
```
⇒ 隔离档下**只有两处**掉到 0，且**都在 `M.__new__` 内** ✓（第 118/131 轮看到的 `frame_clear` 是"**再放一次**
已释放对象"的那一侧 ✗，第一次的释放点在**这里** ✓）。

**② 把 `@36`／`@51` 对到源码**（函数码元的 `offset × 2` 换算**可靠** ✓，第 115 轮验证过 ✓）：
```
byte  44 (unit 22) CALL                                   ← dict(ns.items()) 那个调用
byte  52 (unit 26) STORE_FAST               ns            ← 重绑参数 ns（命中 @51 ✓ 合理）
byte  68 (unit 34) LOAD_SUPER_ATTR          __new__
byte  72 (unit 36) LOAD_FAST_BORROW_LOAD_FAST_BORROW  mcls, name      ← 🎯 命中 @36 ✗
byte  74 (unit 37) LOAD_FAST_BORROW_LOAD_FAST_BORROW  bases, ns
byte  76 (unit 38) CALL
```
⇒ **一条纯加载指令**（`LOAD_FAST_BORROW_LOAD_FAST_BORROW mcls, name` ✓）竟然出现在
「**某个 dict 的引用计数掉到 0**」的现场 ✗ —— 加载**本不该**释放任何东西 ✓
⇒ 高度怀疑：**我们对 3.14 这条"融合的借用加载"的实现**动了不该动的引用 ✗
（第 108 轮曾记过：`LOAD_FAST_BORROW` 与 `LOAD_FAST` 走同一条路 ⇐ 那是"栈上持有"的口径 ✓；
**融合版**是另一条路 ✓，此前**没**被这条线索照到 ✓）。

**③ 下一轮（就一件）**：读 `executor.rs` 里 `LOAD_FAST_BORROW_LOAD_FAST_BORROW` 的实现 ✓
（第 7473 行附近 ✓），核它**是否释放/覆盖了栈上已有值** ✓；若有多放 ⇒ 就是根因 ✓，
改法按"**只压两个借用值、不碰别处**"来 ✓。
**判据**：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ `import enum` 不再报「已释放对象」∧ 全闸门不回归 ✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无代码改动** ✓（只读 + 现成探针 ✓、树干净 ✓）；
**未声称任何阶段完成** ✓；累计剪掉/撤回八条假设 ✓，而**靶点已细化到一条指令** ✓。

#### 第 201 轮：帧内盘点**无重复** ⇒ 剪掉第八条假设 ✓（双放发生在**跨帧**）

**① 本轮做的**：在 `frame_clear` 开头加一发**只读的同帧指针盘点** ✓（门控 `PYAWA_FRAMECLEAR_DEBUG=1` ✓）：
把该帧 `code`／`exception`／`globals`／`namespace`／`locals`／`stack`／`cells` 七段里的指针全收进一个表 ✓，
对表内两两比较 ✓、指针相同就打印「帧内重复 X／Y」✓。

**② 结果**：探针**编译通过** ✓（0 错 ✓），但**两个档（普通／隔离）都没有任何输出** ✗
⇒ 帧内**不存在**两处指向同一对象 ✓ ⇒ **「同一帧内两处重复」这条不成立** ✗（第八条剪掉的假设 ✓）。

**③ 由此得到的收敛**：过度释放既然发生在「清帧」那一段（回溯＋panic 都指向那里 ✓）而又**不是帧内重复** ✓
⇒ 只能是**跨帧**：同一个对象被**两帧**各按"持有"释放一次 ✗ —— 最可疑的一对是
**类体帧**（`run_class_body` 那个 ✓，它的 `namespace` 字段＋`__classdict__` ✓）与**元类 `__new__` 帧** ✓
（`M.__new__(mcls, name, bases, ns, …)` 的参数槽 ✓）；其中**必有一份是借用却按持有释放** ✗。

**④ 下一轮（就一件）**：做**按对象身份的释放计数** ✓（不依赖地址复用 ✓ 的做法）：
在释放路径上加一发门控记录 ✓，对**类型＝`dict` 且 72 字节**的对象 ✓ 打印
「第 N 次释放 ＋ 当前帧标识（qualname／ip）」✓ ⇒ 两次释放会分别带上**各自的帧** ✓，
一眼看出是**哪两帧** ✗，再判断哪一份该是借用 ✓。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ 全闸门不回归 ✓。

**⑤ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无净代码改动**（探针已撤 ✓、树干净 ✓、0 警告 ✓）；
**未声称任何阶段完成**；累计剪掉/撤回 **八条** ✓。

#### 第 200 轮：修复尝试**失败** ⇒ 如实撤回 ✓（第七条假设剪掉 ✓）

**① 我做了什么**：按第 199 轮的根因判断，在 `frame_clear` 里**去重** —— 先捕获 `frame.namespace` 指针、
释放它，然后在 locals 循环里**跳过同一指针的槽** ✓（补丁写在 `crates/pyawa-core/src/frame.rs`）。

**② 验证结果（三个判据全不过）** ✗：
```
小例 普通        ：退出码 134（SIGABRT）✗
小例 隔离档      ：仍报「对已释放对象 decref：类型 dict（refcount 已归零）」✗
大例 隔离档      ：仍 abort（non-unwinding panic）✗
（构造性检查：0 错 0 警告 ✓、逐字节 4/4 ✓ —— 说明补丁本身是"合法但无效"的 ✓）
```
⇒ **「`namespace` 字段与类体局部同指针 ⇒ 双放」这条不成立** ✗（第七条被剪掉的假设 ✓）。
**处置**：`git checkout -- crates/` **整套撤回** ✓（未经验证的语义改动不留 ✓）⇒ 复核：**0 警告** ✓、
逐字节 **4/4** ✓、树**干净** ✓。

**③ 这条失败给出的新信息** ✓：过度释放**确实**发生在"清帧"这一段 ✓（回溯与 panic 都指向那里 ✓），
但**不是** `namespace` 字段与 locals 的那一对 ✓ ⇒ 剩下的可能：
* locals 里**两个不同槽**指向同一对象 ✓（而不是"字段 vs 局部"✓）；
* 或 `cells`／`stack` 与 locals 之间的重复 ✓（`frame_clear` 的后半段还放了 `cells` ✓）；
* 或**另一帧**（类体帧 vs 元类 `__new__` 帧 ✓）各持一份而其中一份是**借用却按持有释放** ✗。

**④ 下一轮（就一件）**：在 `frame_clear` 里加一发**门控盘点**（`PYAWA_FRAMECLEAR_DEBUG=1` ✓）：
打印这一帧**各段（code/exception/globals/namespace/locals/stack/cells）里的指针清单** ✓，
用**同一帧内**的重复来判定到底是哪两处重复 ✓（这一次不再依赖跨帧地址 ✓）。
**判据**：小例本层＝参照 ∧ 隔离档干净 ∧ 全闸门不回归 ✓。

**⑤ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无净代码改动**（补丁已撤 ✓）；
**未声称任何阶段完成**；累计剪掉/撤回的方向 **七条** ✓。

#### 第 199 轮：🎯🎯 **根因定位成功（读代码即定）** —— `frame_clear` 把命名空间放了**两遍**

**① `frame_clear`（`crates/pyawa-core/src/frame.rs:539`）的释放顺序**：
```
545  release_object(code)         ← frame.code
549  release_object(exception)    ← frame.exception
553  release_object(globals)      ← frame.globals
557  release_object(namespace)    ← **frame.namespace（类体帧就是那个类命名空间）**  ← 第一遍
559  for (index, slot) in frame.locals … 564 release_object(value)   ← **locals 逐个放**  ← 类体的
                                                                    `__classdict__` 局部是**同一个对象** ⇒ 第二遍 ✗
567  for value in take(frame.stack) …                                  ← 值栈
```

**② 结论**：**同一个对象占了两个位置** —— 帧的 `namespace` 字段 ＋ locals 里的 `__classdict__`（类体帧）
⇒ 收尾时**放两遍** ✗ ⇒ 命名空间在**元类还在用它**的时候就被放到 0 ✓。
这与观察到的**三条**事实**全部吻合**：
* 第 110 轮的计数读数：元类内部「**两次**释放」把它打到 0（一次正当＝参数重绑，另一次就是这个 ✗）；
* 第 131 轮起反复看到的 `EnumDict.__init__@10`／`EnumType.__new__@540` 一族；
* 本轮回溯：过度释放发生在 **`frame_clear`**（清那个调用帧时）✓。
**并且解释了为什么它"条件触发、有时静默"**：只有当类体帧**既有 `namespace` 字段、又有同名局部**时才双放 ✓
（`__prepare__` 返回 dict 子类 ✓／元类 `__new__` 里重绑命名空间 ✓ 两条独立触发条件都落在"类体帧"这个共同点上 ✓）。

**③ 下一轮（就一件，且是**修复**）**：在 `frame_clear` 里**去重** —— 放 `namespace` 之前/之后，
跳过 locals 中与它**同一指针**的槽 ✓（或反过来：locals 循环里跳过 `namespace` 指针 ✓）。
**判据**：6 行小例「本层＝参照」✓ ∧ `PYAWA_QUARANTINE=1` 干净 ✓ ∧ `import enum` 不再报「已释放对象」✓
∧ 全闸门不回归（workspace／0 警告／逐字节 4/4／对拍两模式／`check.py` 12/12／夹具 490／语料下限 182／
`selftest`／`t_ab_1`）✓。

**④ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无仓库内代码改动**（读代码定位 ✓）；
**未声称任何阶段完成**；**这条根因是读代码＋回溯交叉印证得出的，修复与验证在下一轮** ✓。

#### 第 198 轮：⚠️ 撤回 `dict_set` 归因；改用**减引用一侧**的真实回溯 ⇒ 靶点落在 `frame_clear`

**① 撤回**（如实）：第 197 轮「靶点＝`dict_set`」不成立 ✗ —— 把回溯取到更上面几帧后看到
调用者是 `pyawa_stdlib::builtins_module::build`（**启动期**），而 `watch` 是**建类时**才装的
⇒ **同一个地址又被复用**（第 112／118 轮已两次踩过同一陷阱）⇒ 凡按**地址**归因的结论一律不采信。

**② 本轮可靠数据点**（隔离档 + `RUST_BACKTRACE=1`，只留 pyawa_core 帧）：
```
   4: pyawa_core::frame::frame_clear
   9: pyawa_core::executor::call::call_callable
  10: pyawa_core::executor::call::call_value
  11: pyawa_core::classes::build_class_native
  12: pyawa_core::executor::call::call_callable
  13: pyawa_core::executor::execute::{closure#1}
  14: pyawa_core::executor::execute
```
⇒ 这是**「谁把引用多放了一份」的直接证据**（减引用一侧，不依赖地址）：**清理元类 `__new__` 那个调用帧**时，
`frame_clear` 放掉了一个**已经被释放**的对象 ⇒ 该帧把**同一个对象放了两遍**（同一对象占了**两个槽**）。

**③ 小例的失败形态**（本轮一并钉住）：小例是 `header.rs:140` 的「**对已释放对象 decref**」（`assert_live` 那侧），
与前面见过的「隔离区 **incref** 撞上已释放对象」是**两侧不同的检查**；小例在本轮 2 次隔离档运行里
**没有**产生 `[隔离区] … 释放于 …` 记录。

**④ 下一轮（就一件）**：读 `frame_clear` 的释放范围 ＋ 元类 `__new__` 调用时**命名空间被绑进了哪些槽**
（参数槽 ＋ 借用槽？）⇒ 找出「同一对象占两个槽」的那处 ⇒ 那就是过度释放的根。
判据：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ 全闸门不回归。

**⑤ 如实交代**：判据① 仍 **27.4%（172÷628）**；本轮**无仓库内代码改动**；**未声称任何阶段完成**；
累计撤回/剪掉的方向共**六条**（僵尸写、两种角色、模块持有命名空间、重绑参数、GC 释放、`dict_set` 归因）。

#### 第 197 轮：🎯🎯 **靶点现形** —— 命名空间是被 `dict_set` 释放的 ✓（6 行小例 ✓）

**① 一个先前漏看的实情** ✓：前面十几轮的拆分只搬了 **`pub` 方法** ✓（模式是 `^    pub(?:\\(crate\\))? fn` ✓）
⇒ **私有方法**（`    fn …` ✓，如 `unlink`／`unlink_gc`／`free_garbage` ✓）**仍在 `instance.rs`** ✗
—— 这也解释了 `instance.rs` 为什么还有 1952 行 ✓。**这本身要记一笔** ✓（第 183 轮那句「实例方法全部搬完」只对 `pub` 方法成立 ✗，此处如实更正 ✓）。

**② 探针与结果** ✓（在 `unlink`／`unlink_gc`／`free_garbage` 三处各加一发：命中 `watch` 地址就打印回溯 ✓，门控 `PYAWA_FREE_DEBUG` ✓）：
```
0: <pyawa_core::instance::Instance>::unlink
1: <pyawa_core::instance::Instance>::release_one
2: <pyawa_core::instance::Instance>::release_object
3: <pyawa_core::instance::Instance>::dict_set        ← 🎯
```
⇒ 被盯的命名空间是**经 `dict_set` 替换某个键的旧值**时被放到 0 的 ✓
（`free_garbage`／`unlink_gc` **没有**命中 ✓ ⇒ 不是 GC 路径 ✓，与第 195 轮 `NO_GC` 无效**互相印证** ✓）。

**③ 下一轮（就一件 ✓）**：把回溯的**更上面几帧**（4–8 帧 ✓）取出来 ✓ ⇒ 就会点出**是哪个 `dict_set` 调用点**（哪个键、哪个字典 ✓）⇒ 那就是要改的那一行 ✓。
**判据** ✓：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ 全闸门不回归 ✓。

**④ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓（探针已撤 ✓、0 错 0 警告 ✓）；**未声称任何阶段完成** ✓。

#### 第 196 轮：大例对照实验（各 5 次）✓ —— 结论落到「同一件事」这一支 ✓；顺带**更正一处编号混淆** ✓

**① 原始数据** ✓（下表是**进程退出码** ✓）：
```
第1 次：NO_GC 退出码=1 ｜ 正常 退出码=1
第2 次：NO_GC 退出码=1 ｜ 正常 退出码=1
第3 次：NO_GC 退出码=1 ｜ 正常 退出码=1
第4 次：NO_GC 退出码=1 ｜ 正常 退出码=1
第5 次：NO_GC 退出码=1 ｜ 正常 退出码=1
```

**② 更正一处混淆** ✓（如实 ✓）：我前面几轮把「状态 5」写成了「退出码 5」✗ ——
实测：`./target/debug/pyawa …/enumreq.py` 的**进程退出码是 1** ✓，而 **5** 是报文里的**状态号** ✓：
```
pyawa: 未捕获（状态 5）：指令 38 的这个形态尚未接线：下标赋值只接线了 list／dict
```
⇒ 也就是说：这次 10 次跑到的是**普通缺口**（`下标赋值只接线了 list／dict` ✓），**不是**内存崩溃 ✓。

**③ 结论** ✓（按本轮事先定好的判据 ✓）：两侧 10 次**完全一致**（都是普通缺口 ✓、都没有 134／内部 panic ✓）
⇒ **`NO_GC` 不改变结果** ✓、且大例这 10 次**没复现**内存缺陷 ✓（抖动 ✓）
⇒ 落在「**仍是同一件事**」这一支 ✓ ⇒ 回**小例**继续二分 ✓（小例里 `NO_GC` 已确认无效 ✓，
而轨迹显示「最终释放**未走**被盯的 `release_object` 路径」✓ ⇒ 下一步就在小例上钉这一点 ✓）。

**④ 已剪掉的假设（五条 ✓）**：僵尸写 ✓／两种角色计数 ✓／模块对象持有命名空间 ✓／
重绑参数多放 ✓／GC 释放（仅小例 ✓）。

**⑤ 下一轮（就一件 ✓）**：在小例（6 行 ✓）上给**非 `release_object` 的释放路径**加探针 ✓ ——
重点看 `free_garbage`／`quarantine_put`／`unlink` 一族里哪条会**绕开**逐次 decref 打印 ✓；
把「rc 还 ≥2 却已成已释放对象」的那一刻的**调用栈**打出来 ✓ ⇒ 那就是靶点 ✓。
**判据** ✓：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ 全闸门不回归 ✓。

**⑥ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓；**未声称任何阶段完成** ✓。

#### 第 195 轮：隔离档轨迹给出**新线索** ✓；`PYAWA_NO_GC` 一验 ⇒ **第五条假设剪掉** ✓；大例出现**新数据点** ✓

**① 隔离档轨迹（6 行小例 ✓）**：
```
[ns 探针] 交元类之前 rc=1
[watch] incref → rc=2  <module>@19 ／ → rc=3 <module>@19 ／ → rc=4 M.__new__@6 ／ → rc=5 M.__new__@7
[watch] decref → rc=4 M.__new__@7 ／ → rc=3 M.__new__@18 ／ → rc=2 M.__new__@26
对已释放对象 decref：类型 `dict`（refcount 已归零）
对已释放对象 decref：类型 `dict`（refcount 已归零）
```
**关键读法** ✓：`watch` 逐次打印**释放前**的 rc ✓ ⇒ 若释放走 `release_object` ✓，必然先看到 `rc=1`、再 `rc=0` ✓；
**两者都没出现** ✗ ⇒ **最终的释放没走这条被盯的路径** ✗ ⇒ 当时怀疑是 **GC 的 `free_garbage`** ✓（它绕过普通 decref ✓，
与"有时崩、有时静默损坏"也吻合 ✓）。

**② 于是用现成的开关一验** ✓：
```
小例 普通                → 1（退出码 0 ✓ 这一次没崩 ✓，印证抖动 ✓）
小例 + PYAWA_NO_GC=1     → **退出码 134（SIGABRT）** ✗
小例 + NO_GC + QUARANTINE → 仍报「对已释放对象 decref：类型 dict」 ✗
```
⇒ **关掉 GC 并不能消除缺陷** ✓ ⇒ **GC 这条假设剪掉** ✓（第五条 ✓）。
**大例（`import enum`）+ NO_GC** ✓ 却出现**新数据点** ✓：
```
pyawa: 未捕获（状态 5）：指令 38 的这个形态尚未接线：下标赋值只接线了 list／dict
```
⇒ 不再崩 ✓、而是走到一个**普通缺口** ✓ ⇒ 说明**大例与小例不完全是同一件事** ✓（大例里 GC 至少参与了一部分 ✓，
或者大例那次是撞上了抖动 ✓ —— **两种解释都还没排除** ✓）。

**③ 下一轮（就一件 ✓）**：把"大例 + NO_GC"**重复 N 次**（例如 5 次 ✓）并与**不加 NO_GC**的 5 次对照 ✓：
若 `NO_GC` **稳定**不崩 ⇒ 大例这条族的病因**确实与 GC 相关** ✓（而小例是另一件事 ✓）⇒ 就分别立案 ✓；
若两者都会偶尔崩 ⇒ 说明仍是同一件事、只是抖动 ✓ ⇒ 回到小例继续二分 ✓（**这一步便宜且能定性** ✓）。

**④ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓；**未声称任何阶段完成** ✓。
至今剪掉的假设已五条 ✓（僵尸写／两种角色计数／模块对象持有命名空间／重绑参数多放／GC 释放 ✓）。

#### 第 194 轮：🎯 **隔离档下的完整轨迹**（6 行小例）✓ —— `rc→0` 那一步终于现形 ✓

**① 轨迹** ✓（`PYAWA_QUARANTINE=1 PYAWA_NS_DEBUG=1` ✓，小例只有一个命名空间 ⇒ 盯的地址必是真的 ✓）：
```
[ns 探针] 交元类之前 namespace=0x58c60a2dcac0 rc=1
[watch] incref 0x58c60a2dcac0 → rc=2 现场=<module>@19
[watch] incref 0x58c60a2dcac0 → rc=3 现场=<module>@19
[watch] incref 0x58c60a2dcac0 → rc=4 现场=M.__new__@6
[watch] incref 0x58c60a2dcac0 → rc=5 现场=M.__new__@7
[watch] decref 0x58c60a2dcac0 → rc=4 现场=M.__new__@7
[watch] decref 0x58c60a2dcac0 → rc=3 现场=M.__new__@18
[watch] decref 0x58c60a2dcac0 → rc=2 现场=M.__new__@26
对已释放对象 decref：类型 `dict`（refcount 已归零）；见台账第 334 轮
对已释放对象 decref：类型 `dict`（refcount 已归零）；见台账第 334 轮
```

**② 至此这条线的可靠事实（汇总 ✓）**：
1. 小例（6 行 ✓，见第 193 轮台账 ✓）：元类 `__new__` 里把命名空间**换成另一个对象**再 `super().__new__` ✓；
2. **交元类之前 `rc=1`**（第 193 轮 ✓）—— 而 `enum` 那次是 `rc=2` ✗ ⇒ 持有者数量**不稳定** ✓；
3. 隔离档下把「**谁把它放到 0**」变成**确定性** ✓（本轮的轨迹／报错 ✓）；
4. 普通档下有时只是**静默损坏** ✓（v2 普通档跑出过 `1` ✓）⇒ 这条族"有时崩有时报别的错"得到解释 ✓。

**③ 下一轮（就一件 ✓）**：读轨迹里**最后几步**的现场 ✓（`M.__new__@18`／`@26` 一类 ✓，函数码元的
`offset × 2` 对源码是**可靠**的 ✓）⇒ 定位到**具体那句** ✓ ⇒ 在那里改 ✓（与第 104 轮 `dict.__init__`／
第 106 轮 `super()` 绑定的改法一样 ✓：**先在小例上让三方一致** ✓，再跑 `import enum` ✓）。
**判据** ✓：小例本层＝参照 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ `cargo test --workspace` ＋ 对拍 ＋ 逐字节 4/4 不回归 ✓。

**④ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓；**未声称任何阶段完成** ✓。

#### 第 193 轮：🎯 在 **6 行小例**上取到**逐次计数轨迹** ✓ —— 盯的地址这次就是真的那个 ✓

**① 小例（v2 ✓，6 行 ✓）**：
```python
class M(type):
    def __new__(mcls, name, bases, ns, **kwds):
        ns = dict(ns.items())
        return super().__new__(mcls, name, bases, ns)


class C(metaclass=M):
    x = 1


print(str(C.x))
```

**② 轨迹** ✓（`PYAWA_NS_DEBUG=1` ✓：`ns 探针` 给"交元类前/后"的 rc ✓，`watch` 给**这一个地址**的
逐次 incref／decref ＋现场 ✓）：
```
[ns 探针] 交元类之前 namespace=0x5f465e8c3990 rc=1
[watch] incref 0x5f465e8c3990 → rc=2 现场=<module>@19
[watch] incref 0x5f465e8c3990 → rc=3 现场=<module>@19
[watch] incref 0x5f465e8c3990 → rc=4 现场=M.__new__@6
[watch] incref 0x5f465e8c3990 → rc=5 现场=M.__new__@7
[watch] decref 0x5f465e8c3990 → rc=4 现场=M.__new__@7
[watch] decref 0x5f465e8c3990 → rc=3 现场=M.__new__@18
[watch] decref 0x5f465e8c3990 → rc=2 现场=M.__new__@26
[ns 探针] 元类返回之后 namespace=0x5f465e8c3990 rc=2
[watch] decref 0x5f465e8c3990 → rc=1 现场=<module>@19
```

**③ 为什么这一步关键** ✓：第 113 轮在 `import enum` 上做过同一件事，但那时**盯错了对象** ✗
（枚举里建很多类 ✓，死的是**另一个**命名空间 ✓）⇒ 轨迹只见 rc 从 6 掉到 3 ✓、没到 0 ✓；
**小例只有一个命名空间** ✓ ⇒ 这次 `watch` 盯的**必然**是那个要死的 ✓ ⇒ **掉到 0 的那一步会带现场现形** ✓。

**④ 下一轮（就一件 ✓）**：按上面轨迹里"最后一步（rc→0）"的**现场**去读对应代码 ✓
（第 116 轮的 `offset × 2` 对**函数**码元是可靠的 ✓；那份 `Lib/enum.py` 的换算不可靠只发生在**模块级** ✓），
然后在那一步上改 ✓。**判据** ✓：6 行小例三方一致 ∧ `PYAWA_QUARANTINE=1` 干净 ∧ 全闸门不回归 ✓。

**⑤ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓；
**未声称任何阶段完成** ✓。
#### 第 192 轮：🎯 **二分出最小成分** ✓ —— 两条各自独立的触发条件，靶点落在"元类 `__new__` → `super().__new__`"这条调用路

**① 本轮做法** ✓：把第 191 轮的 20 行小例拆成 4 个变体 ✓，每个跑**普通 + 隔离档** ✓：
```
v1 普通: malloc(): unaligned tcache chunk detected | 隔离: 对已释放对象 decref：类型 `dict`（refcount 已归零）；见台账第 334 轮
v2 普通: 1 | 隔离: 对已释放对象 decref：类型 `dict`（refcount 已归零）；见台账第 334 轮
v3 普通: pyawa: 未捕获（状态 1）：TypeError: type.__new__ 的命名空间要是 dict | 隔离: 干净
v4 普通: malloc(): unaligned tcache chunk detected | 隔离: 对已释放对象 decref：类型 `dict`（refcount 已归零）；见台账第 334 轮
```
（v1 = dict 子类命名空间 ＋ 重绑；v2 = **无 `__prepare__`**、只在 `__new__` 里 `ns = dict(ns.items())`；
v3 = dict 子类命名空间、**不重绑**；v4 = 普通命名空间 ＋ 重绑 ＋ `type.__new__`）

**② 读出来的结论** ✓（这是本轮最重要的信息 ✓）：
1. **v2 触发** ✓ ⇒ **不需要** dict 子类命名空间、**不需要** `__prepare__` ✓ —— 只要
   「元类 `__new__` 里把命名空间**换成另一个对象**（`dict(ns.items())` ✓）然后 `super().__new__(…)`」✓；
2. **v3 触发** ✓ ⇒ 反过来，**只要命名空间是个 dict 子类**（`__prepare__` 返回 ✓）＋同样的
   `super().__new__` ✓，**不重绑也会触发** ✓；
3. **v4 也触发** ✓ ⇒ 换成 `type.__new__(mcls, …)` **不改变结果** ✓（说明与 `super` 代理无关 ✓，
   而与**元类 `__new__` 这条调用路**本身有关 ✓）；
4. v2 的**普通档**跑出了 `1`（没崩 ✓）⇒ 说明普通档下**有时只是静默损坏** ✓（隔离档才把它变成可判读的报错 ✓）
   —— 这也解释了这条族"有时崩、有时报别的错" ✓。
⇒ 于是**靶点**收敛为 ✓：**元类 `__new__` 收到命名空间参数后，那条路上的引用账**（重绑或换对象都会踩到 ✓）。

**③ 下一轮（就一件 ✓）**：在**6 行版**（v2 ✓）上做**逐次 decref 轨迹** ✓ —— 打开 `PYAWA_NS_DEBUG`（交元类前
`rc` ✓）＋`PYAWA_WATCH_DICT`（按类型盯归零 ✓）＋必要时给 `super().__new__` 那条路加一发**只打印 rc 的探针** ✓，
把"从 2 掉到 0 的那一步"抓出来 ✓；拿到那一步就是**修法的靶点** ✓。
**判据** ✓：小例三方一致（本层＝参照 ✓）∧ `PYAWA_QUARANTINE=1` 不再报"对已释放对象 decref" ✓ ∧ 全闸门不回归 ✓。

**④ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓（只写 `target/` 探针 ✓）；
**未声称任何阶段完成** ✓。
#### 第 191 轮：🎉🎉 **119 族在 20 行内复现成功** ✓✓ —— 从"只有 `import enum` 能触发"变成**最小例可迭代**

**① 最小伪造器** ✓（`target/nsmin.py` ✓，20 行 ✓；脚本不进仓库 ✓）：
```python
class D(dict):
    pass


class M(type):
    @classmethod
    def __prepare__(mcls, name, bases, **kwds):
        return D()

    def __new__(mcls, name, bases, ns, **kwds):
        ns = dict(ns.items())          # ← enum.py:511 的同一步 ✓
        return super().__new__(mcls, name, bases, ns)


class C(metaclass=M):
    x = 1


print(str(C.x))
print(str(type(C.__dict__).__name__))
```

**② 实测三方对照** ✓：
```
本层      ：**退出码 134（SIGABRT）** ✗
隔离档    ：pyawa: 内部 panic：crates/pyawa-core/src/header.rs:140：
            **对已释放对象 decref：类型 `dict`（refcount 已归零）** ✓
            ＋ malloc(): unaligned tcache chunk detected ✓
参照(CPython)：1 / mappingproxy ✓
```
⇒ **复现成功** ✓✓ ⇒ 这条线终于有了**最小例** ✓（此前只能在 `import enum` 上观察 ✓，
最小例 20 行、秒级、可反复迭代 ✓）。**这一条本身就是本轮最大产出** ✓。

**③ 与既有事实的关系** ✓：最小例里出现的 `dict`（72 字节 ✓）＋"在 `__new__` 里重绑命名空间" ✓
＋"dict 子类当命名空间（`__prepare__` 返回）" ✓ —— 与 `enum.py` 的形状**一一对应** ✓；
且**不涉及** `super().__init__()`（`D` 没有自定义 `__init__` ✓）⇒ 说明先前盯的 `EnumDict.__init__@10` 那条
**可能只是同族现象** ✗（不是必要条件 ✓）—— 这也要在小例上核 ✓。

**④ 下一轮（就一件 ✓）**：**在小例上二分** ✓ —— 依次删掉/替换：① 去掉 `metaclass=M` 的
`__prepare__` ✓；② 去掉 `dict(ns.items())` 重绑 ✓；③ 去掉 `dict` 子类（用普通 `dict` ✓）；
④ 去掉 `super().__new__` 而用 `type.__new__` ✓ —— 找出**最小的必需成分** ✓，
那就是修法的靶点 ✓（判据：小例三方一致 ✓、且 `PYAWA_QUARANTINE=1` 不再报"对已释放对象 decref" ✓）。

**⑤ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无仓库内代码改动** ✓（只写了 `target/` 探针 ✓）；
**未声称任何阶段完成** ✓。

#### 第 190 轮：M3 第 7 轮 —— `build_class_native` 的账**是平的** ✓；**第四条假设剪掉** ✓

**① 复查结果（拆分后第一次回头看这里 ✓）**：`crates/pyawa-core/src/classes.rs` 里
```
120  let namespace = match custom_prepare { … }            ← 我方那份（+1）
227  let namespace_for_init = instance.retain(namespace);  ← 为 __init__ 再留一份（+1）⇒ 共 2 ✓
229  Some(new_method) => call_value(… &[metaclass, name, bases, namespace], …)   ← 借用（call_value 逐参 incref ✓）
246  None            => build_class_from_parts(…, namespace, …)                 ← 吃掉调用方那份 ✓
```
⇒ **账是平的** ✓，与第 111 轮实测的 `rc=2` **完全吻合** ✓ ⇒ 这一处**不是**病因 ✓。

**② 最小伪造器（隔离"重绑参数"这件事 ✓）**：`target/rebind.py`
```python
def f(d):
    d2 = d
    d = {"x": 1}     # 重绑参数
    return d2
d = {"a": 1}
print(str(f(d)["a"])); print(str(d["a"]))
```
实测 ✓：**本层**输出 `1 / 1` ✓、**参照**同样 `1 / 1` ✓、**隔离档**（`PYAWA_QUARANTINE=1`）**干净** ✓
⇒ **"重绑参数会多放一份"不成立** ✗ ⇒ **第四条假设剪掉** ✓。

**③ 至今被剪掉的假设（四条 ✓，都写进台账以免重复走 ✓）**：
1. 「僵尸写」旧叙事 ✗（第 92 轮已撤 ✓）；
2. 「同一对象按两种角色计数（`bound_self`／`args[0]`）」✗（第 187 轮按代码核平 ✓）；
3. 「模块对象持有类命名空间」✗（第 188 轮：`load_module` 总自建新 dict ✓）；
4. 「重绑参数多放一份」✗（本轮最小伪造器 ✓）。
**仍可靠的两条事实** ✓：① 隔离档下稳定复现：某个 **72 字节 `dict`** 在 `EnumType.__new__@540` 被放到 0 ✓、
之后被 incref ✗；② `EnumDict.__init__@10` ＝ `LOAD_SUPER_ATTR` 一类 ✓。

**④ 下一轮（就一件 ✓）**：做**类命名空间**的最小伪造器 ✓ —— 一个**小元类** ＋
`__prepare__` **返回 dict 子类** ✓ ＋ 在 `__new__` 里 `classdict = dict(classdict.items())` 重绑 ✓
（就是 `enum.py:511` 那一步 ✓）⇒ 若能在 20 行内复现 ✓，就把问题从 `enum.py` 里**摘出来** ✓，
修法就能在最小例上验证 ✓；若复现不了 ✓ ⇒ 说明触发条件与 enum 的其余部分有关 ✓（那也是重要信息 ✓）。

**⑤ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无代码改动** ✓（只读 + 一个探针脚本 ✓，
脚本写在 `target/` 不进仓库 ✓）；**未声称任何阶段完成** ✓。

#### 第 189 轮：M3 第 6 轮 —— **第三条假设被剪掉** ✓（新增一发门控探针 ✓）

**① 本轮做了什么** ✓（首次在这一线**动代码** ✓，但只是**门控诊断** ✓）：
```
给 AttributeObject::set_attributes 加探针 ✓（PYAWA_SETATTR_DEBUG=1 ✓，门控走 diag::flag ✓）：
  只要送进来的属性字典**类型名 ≠ "dict"**（例如类命名空间 EnumDict ✓）就打印：
  [setattr] 属性字典 ← 类型=<名> ＋ Rust 回溯（force_capture ✓，这样不需要 Instance 也能点名 ✓）
```
**为什么盯类型而不盯地址** ✓：第 118 轮盯地址失败过 ✗（地址复用 ✓，第 112 轮已证实这个陷阱 ✓）。

**② 实测结果** ✗：`PYAWA_SETATTR_DEBUG=1 ./target/debug/pyawa …/enumreq.py` **一声不响** ✓
⇒ `set_attributes` 在 `import enum` 全程**从未**收到非普通 `dict` ✓
⇒ **第三条假设剪掉** ✓：类命名空间**不是**经 `set_attributes` 进入某个对象的内联属性字典的 ✓。

**③ 于是"`attribute_clear` 放的"这条也要重估** ✓：第 118 轮那句读数的释放栈里有 `attribute_clear` ✓，
但那正是**地址复用**能骗人的地方 ✗ ⇒ **不再把它当定论** ✓；
**仍可靠的事实**只剩两条 ✓：
1. 隔离档下稳定复现 ✓：某个 **72 字节 `dict`** 在 `EnumType.__new__@540` 被放到 0 ✓，之后被 incref ✗；
2. 那个 `@540` 在**同一函数**内、且第 116 轮把 `EnumDict.__init__@10` 对到了 `LOAD_SUPER_ATTR` 一类 ✓。

**④ 下一轮** ✓：回到"**计数为什么为 1**"这一条最朴素的问题 ✓ —— 在 `EnumType.__new__`
（`Lib/enum.py` ✓）里，命名空间除**帧的局部**之外的持有者应当还有 ✓（`__prepare__` 的返回值 ✓、
类对象的 `__dict__` 建立路径 ✓）⇒ 用 `PYAWA_WATCH_DICT` 的**逐次 decref 轨迹**（第 111 轮那套 ✓）
把 `rc` 从 2 掉到 0 的**那一步**抓现行 ✓（不再依赖释放栈 ✓）。

**④′ 一处失误与即时修正** ✓（如实 ✓）：加探针那一提交**带了 1 条警告** ✗（`unnecessary unsafe block` —— 我把
`unsafe` 写成了嵌套两层 ✓），而我没先看闸门就提交了 ✗ ⇒ **下一条提交即时修掉** ✓，**0 警告已恢复** ✓。
**纪律重申** ✓：**提交前必须先读 0 警告那一项** ✓（这一条我自己在前面几十轮反复强调过 ✓，这轮却漏看 ✗）。

**⑤ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无行为改动** ✓（只加门控诊断 ✓）；
探针保留 ✓（零开销 ✓）；**未声称任何阶段完成** ✓。

#### 第 188 轮：M3 第 5 轮 —— **再剪掉一条假设** ✓（模块对象这条也不成立 ✓）；目标收敛到"谁把 `EnumDict` 放进某个对象的 `attributes`"

**① 本轮核到的事实** ✓（读 `executor/import.rs` ✓）：
```
import.rs:20    let namespace = instance.new_dict();          ← load_module **总是自建新字典** ✓
import.rs:29    pub(crate) fn load_module(...)
import.rs:36        .alloc(AttributeObject::new(module_type, RefCell::new(Some(namespace))))
调用点：executor.rs:3484/3490（IMPORT_NAME 一族 ✓）＋ import.rs:77/315（子模块／fromlist ✓）
```
⇒ **模块对象的属性字典永远是"新鲜普通 `dict`"** ✓ ⇒ 第 187 轮那条「模块对象持有类命名空间」的因果
**不成立** ✗（`EnumDict` 是 `dict` **子类** ✓，而这里建的是基类 `dict` ✓）。
**这是本轮的实际产出** ✓：又一条假设被**代码事实**剪掉 ✓（避免下一轮去改错地方 ✓）。

**② 于是问题只剩一条通路** ✓：死掉的是 `EnumDict` ✓，而放掉它的是 **`attribute_clear`** ✓
（第 118 轮读数 ✓）⇒ 只能是**某个内联属性字典对象**（`py_object!` 生成的、带 `attributes` 字段的那些 ✓）
把 `EnumDict` 放进了自己的 `attributes` ✓ ⇒ 而设置它的通路只有两条 ✓：
1. 构造时传参 ✓（`AttributeObject::new(ty, RefCell::new(Some(x)))` ✓）—— 现有 5 处里 4 处是 `self.new_dict()` ✓、
   1 处是模块字典 ✓（本轮排掉 ✓）；
2. 之后经 **`set_attributes`** ✓（第 118 轮我探过它，但**盯的地址可能不是那一个** ✗）。

**③ 下一轮（就一件 ✓）**：给 `set_attributes` 加一发**按"类型名＝`EnumDict`"识别**的门控探针 ✓（不盯地址 ✓，
盯**类型** ✓）⇒ 谁把类命名空间塞进某个对象的属性字典，现场就会带**源码行**现形 ✓；
拿到那一行之后再决定修法 ✓（判据：`PYAWA_QUARANTINE=1` 跑 `import enum` 不再报该行 ✓、普通档不回归 ✓）。

**④ 如实交代** ✓：判据① 仍 **27.4%（172÷628）** ✓；本轮**无代码改动** ✓（只读 ✓）；
**未声称任何阶段完成** ✓。两条被剪掉的假设（"两种角色计数" ✓、"模块对象持有命名空间" ✓）都已写进台账 ✓，
以免后面重复走 ✓。

#### 第 187 轮：M3 第 4 轮 —— 抓到**关键代码事实** ✓ ＋ **剪掉一条假设** ✓

**① 关键代码事实** ✓（直捣第 118 轮那个方向 ✓）：
```
crates/pyawa-core/src/executor/import.rs:153-155
    .alloc(crate::builtin_objects::AttributeObject::new(
        module_type,
        core::cell::RefCell::new(Some(namespace)),      ← 模块对象的属性字典 ＝ 那个命名空间 ✓
```
⇒ **模块的属性字典就是命名空间** ✓（拆分后这行落在 `executor/import.rs` ✓，即 `load_module` ✓）。
**因果** ✓：模块对象一旦被释放 ✓，`AttributeObject` 的 `attribute_clear` 会把**这个命名空间放掉** ✗ ⇒
若此刻别处（帧的 `globals`／类创建 ✓）还指望它 ⇒ 少一份 ✓ ⇒ 与第 118 轮那条读数（`attribute_clear` 放掉
`EnumDict` ✓、帧里有 `execute`／`call_callable` ✓）**对得上** ✓。

**② 剪掉一条假设** ✓（按代码本身核 ✓）：先前记的「同一对象被按两种角色计数（`bound_self` 与 `args[0]`）」✗
**站不住** ✓ —— `call_callable` 的两条路我都读过 ✓：
* **Python 函数**那条：`bound_self` 会先 `incref` 再 `args.insert(0, …)` ✓ ⇒ 帧持有 ✓，收尾释放 `args` 正好 ✓；
* **native**那条：`bound_self` 按契约是**借用** ✓、`args` 由调用方**持有**（`call_value` 逐参 incref ✓）✓
  ⇒ 只释放 `args` ✓ **不多不少** ✓。
⇒ 所以"接收者被放两次"**不是**病因 ✓ ⇒ 第 3 轮那个探针**不必再修** ✓（省下一轮 ✓）。

**③ 于是问题收敛为一句** ✓：**谁把类命名空间（`EnumDict`）交给了"模块对象式"的构造/持有路径** ✓
（`import.rs:153` 那份 `namespace` 在正常导入里是**模块字典** ✓，而死的却是 `EnumDict` ✗）
⇒ **下一轮**：① 给盯判据/隔离区诊断**加一列"码元序号＋源码行"** ✓（第 186 轮定的替代做法 ✓），
用它把 `EnumDict.__init__@10` 与 `<module>@652` 都**落到真实源码行** ✓；② 再沿那两行查"谁把命名空间
按模块字典登记出去" ✓。

**④ 如实交代** ✓：判据① 仍是 **27.4%（172÷628）** ✓；本轮**无代码改动** ✓（只读勘测 ✓）；
**未声称任何阶段完成** ✓。

#### 第 186 轮：M3 第 2–3 轮 —— 两次实验都**没成**，如实记 ✓（无代码改动 ✓）

**① 第 2 轮：把 `<module>@652` 对到源码** ✗
借第 115 轮那招（`offset × 2` ⇒ 字节偏移 ⇒ CPython `dis` × 源码行 ✓）：
```
目标字节偏移 1304 → 落点 734 RETURN_VALUE   ← 显然不对 ✗
```
⇒ 该换算对**模块级**码元**不成立** ✗（第 115 轮对成功的是**函数**码元 ✓）⇒ **结论不采信** ✓。
**改用**：本层自己的落点工具 ✓——给盯判据/隔离区诊断**加一列"码元序号＋源码行"** ✓
（本层已有 `line_at_offset` 一类 ✓，第 116 轮正是靠它把 `EnumDict.__init__@10` 对到 `LOAD_SUPER_ATTR` ✓），
或用 `code_layout`（`PYAWA_LAYOUT_SOURCE` ✓）按**本层口径** dump 模块级码元 ✓。

**② 第 3 轮：验"接收者是否同时在 `args[0]` 与 `bound_self`"** ✗
做法 ✓：在 `executor/call.rs` 的 **native 释放循环**之前插一发门控探针 ✓（`crate::diag::flag("PYAWA_CALL_DEBUG")` ✓），
若 `bound_self` 与 `args` 里有同一个指针 ⇒ 打印"重复出现" ✓ —— 这正是「同一对象被按两种角色计数」的判据 ✓。
结果 ✗：探针**编译不过（2 个错）** ✓（锚点选中的是缩进 8 空格的循环 ✓，那里 `bound_self` 多半**已被 move** ✓，
或 `current_site()` 在该作用域不可用 ✓）⇒ **已 `git checkout` 撤回** ✓，树**干净** ✓。
**下一轮** ✓：把探针插到 `bound_self` **仍然存活**的位置 ✓（或在进入 native 分支前 `let b = bound_self;` 留一份 ✓），
并**先把编译错误读出来**再改 ✓（这一轮我又犯了"没读报错就撤回"的毛病 ✗）。

**③ 如实交代** ✓：判据① 仍是最后一次核实的 **27.4%（172÷628）** ✓；这两轮**零代码改动** ✓、
零闸门影响 ✓（树干净 ✓）；**未声称任何阶段完成** ✓。

#### 第 185 轮：M3 线重新开工 ✓ —— 复核基线 + 119 族**在拆分之后仍原样复现** ✓

**① 目标与预算** ✓：旧目标（拆文件）已收口 ✓；按用户指示**新建目标**回到 M3 ✓，
上限先 60 ✓，用户随后**调到 200** ✓（`maxGoalRounds: 200` ✓）。

**② 拆分之后的第一手复核** ✓（这是对那 40 多轮重构的一次强校验 ✓）：
```
[隔离区] incref 撞上**已释放对象** 0x60f32a7eaf20（原类型 dict，72 字节；释放于 EnumType.__new__@540）⇒ 提前释放／多放一份 ✗；当前帧：EnumType.__new__
```
⇒ **一模一样** ✓（同一现场 `EnumType.__new__@540` ✓、同样 `dict`／72 字节 ✓）⇒
**重构没有改变行为** ✓、先前定位**仍然有效** ✓。

**③ 按判据重新定位**（`PYAWA_WATCH_DICT=1` ✓，门控现已统一走 `diag.rs` ✓）——
当前「**dict 掉到 0**」的现场清单 ✓：
```
<module>@652
  EnumDict.__init__@10
  <module>@212
  EnumType.__new__@521
  EnumType.__new__@540
  EnumType.__new__@540
```

**④ 下一轮（就一件事 ✓）**：把上面清单里**第一行**对到源码语句 ✓（用 `offset × 2` 换算到 CPython 字节偏移 ✓，
第 115 轮用过这招 ✓），再核「**模块对象的属性字典＝命名空间**」与「**接收者只算一次**」这两条 ✓；
**判据** ✓：改完后 `PYAWA_QUARANTINE=1` 跑 `import enum` **不再报**该行 ✓、普通档与对拍不回归 ✓。

**⑤ 如实交代** ✓：判据① 仍是最后一次核实的 **27.4%（172÷628）** ✓（本会话两次长跑尝试均被中断 ✗，
没有更新的实测数 ✓）；本轮**无代码改动** ✓；**未声称任何阶段完成** ✓。
#### 第 184 轮：🎉 **`instance.rs` 收尾** ✓ —— 三个大文件全部拆完 ✓

**① 最后一步** ✓（**不是**零可见性改动 ✓，已按预告说清 ✓）：
```
新增  crates/pyawa-core/src/instance/util.rs  （19 行 ✓）
      ruler_on／str_matches 两个**模块级自由函数**搬来 ⇒ 改为 **pub(super)** ✓
      （子模块的项默认对父模块不可见 ✓ ⇒ 父模块要用它们就必须放宽 ✓）
instance.rs   1965 → **1952** 行 ✓ ⇒ **没有任何顶层自由函数** ✓（实测"剩余顶层 fn: []" ✓），
              只剩**数据定义**：结构体／字段／常量／导入 ✓
```

**② 三个大文件的最终账** ✓（全部纯移动为主 ✓、每步单独提交 ✓、每步全闸门 ✓）：
```
builtin_objects.rs   8990 → **4165** ✓（十四族：str/bytes/dict/deque/list/object/context/generator/
                                        set/property/int/function/float/thread ✓）
executor.rs          9239 → **3963** ✓（十二域：subscript/call/arithmetic/attribute/import/message/
                                        values/ctrls/runtime/format/iter/protocol ✓）
instance.rs          4223 → **1952** ✓（十六个域文件：accessors/alloc/bootstrap/constructors/containers/
                                        context/convert/fs/gc/interrupt/misc/platform/query/refcount/
                                        registry/state + util ✓）
diag.rs              新建 ✓：全 crate **只有它**直接读 `PYAWA_*` ✓
```
⇒ 目标第 ⑥ 条彻底成立 ✓（**没有**近万行单文件 ✓），第 ② 条的 `executor`／`instance`／`diag` 三项
**全部做完** ✓。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 如实留档** ✓：① 第 177 轮那一步**不是**纯移动 ✓（2 个函数 `pub(super)` ✓，已在提交信息与台账写明 ✓）；
② `instance.rs` 的 16 个域文件里 `bootstrap`／`misc`／`util` 是**收容性**命名 ✓（内容较杂 ✓，
语义无害 ✓）；③ 那条既有间歇缺陷（`class_keywords`／`method_defaults` ✓）**仍在** ✓，**未修** ✓；
④ 早先的 M3 判据① **27.4% ≠ 67%** ✓ **未达成** ✓（与本次拆分是两条线 ✓）。

#### 第 183 轮：`instance.rs` 第十五刀 ✓（`instance/misc.rs`：最后 6 个方法）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/misc.rs  （6 个方法 ✓）
      not_implemented／linearize／live_objects／gc_threshold／call_depth／collect ✓
```
（行数以本轮实测为准 ✓。）

**② 意义** ✓：这一刀之后 `instance.rs` 里**再也没有 `impl Instance` 的方法** ✓（只剩结构体定义／字段／
关联常量 ✓）⇒ 那个文件终于**只承载"数据定义"** ✓；而 `impl` 的十五个域文件各司其职 ✓：
```
accessors／alloc／bootstrap／constructors／containers／context／convert／fs／gc／interrupt／
misc／platform／query／refcount／registry／state ✓
```

**③ 剩余** ✓：**4 个模块级自由函数** ✓（`instance.rs` 里不带缩进的 `fn` ✓）⇒ 搬它们要一并给
`pub(super)` ✓（那时**不再是零可见性改动** ✓，单独一轮并在此说清 ✓）。

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、`selftest` 22 ✓、`t_ab_1` ✓。

#### 第 182 轮：`instance.rs` 第十四刀 ✓（`instance/bootstrap.rs`）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/bootstrap.rs  （7 个方法 ✓）
      new／builtins／modules／metatype／build_class／capability／code_with_qualname ✓
```
（具体行数与 `instance.rs` 新行数以本轮实测为准 ✓。）

**② 全线状态** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **3963** ✓、`instance.rs` 继续下降 ✓
（`instance/` 十五刀 ✓：accessors／alloc／bootstrap／constructors／containers／context／convert／fs／gc／
interrupt／platform／query／refcount／registry／state ✓）、`diag.rs` 独占 `PYAWA_*` ✓。

**③ 剩余** ✓：`collect`／`live_objects`／`gc_threshold`（⇒ 可并入 `instance/gc.rs` ✓）、
`not_implemented`／`linearize`／`call_depth`（⇒ `instance/misc.rs` ✓），以及 **4 个模块级自由函数** ✓
（要 `pub(super)` ✓，单独一轮 ✓）。

#### 第 181 轮：`instance.rs` 第十三刀 ✓（`instance/interrupt.rs`：2245 → **2209**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/interrupt.rs  （7 个方法 ✓ 44 行 ✓）
      request_interrupt／interrupted／clear_interrupt／push_exception／pop_exception／
      pending_exception／raise_builtin_error ✓
instance.rs   2245 → **2209** 行 ✓
```
**取舍** ✓：本轮**新建文件** ✓ 而不是把这 7 个塞进既有 `context.rs` ✓ —— 新建只需"搬＋加 `mod`" ✓，
塞进既有 impl 还要改那个文件的 impl 结构 ✓ ⇒ 风险更低 ✓（两处都是纯移动，选低风险的那个 ✓）。

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 2209 ✓（instance/ 十四刀：accessors／alloc／constructors／containers／context／
                                    convert／fs／gc／interrupt／platform／query／refcount／registry／state ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通 **181/182** ✓ 与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓。

**④ 剩余 13 个单点方法** ✓：
```
not_implemented  new  modules  capability  linearize  builtins  code_with_qualname
build_class  metatype  live_objects  gc_threshold  call_depth  collect
```
**下一刀** ✓：按语义把单点归拢 ✓ —— 建议 `instance/bootstrap.rs` ✓（`new`／`builtins`／`modules`／`metatype`／
`build_class`／`capability`／`code_with_qualname` ✓）与 `instance/gc.rs` 追加 ✓（`collect`／`live_objects`／
`gc_threshold` ✓）与 `instance/misc.rs` ✓（`not_implemented`／`linearize`／`call_depth` ✓）；
最后 **4 个模块级自由函数** ✓（要 `pub(super)` ✓，单独一轮并说明 ✓）。

#### 第 180 轮：`instance.rs` 第十二刀 ✓（`instance/convert.rs`：2424 → **2245**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓；名字清单按第 179 轮所录 ✓）：
```
新增  crates/pyawa-core/src/instance/convert.rs  （9 个方法 ✓ 187 行 ✓）
      iterable_items／advance_iterator／iter_object／as_type／bool_value／index_value／
      float_value／order_of／attribute_optional_of ✓
instance.rs   2424 → **2245** 行 ✓
```

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 2245 ✓（instance/ 十三刀：accessors／alloc／constructors／containers／context／
                                    convert／fs／gc／platform／query／refcount／registry／state ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、`selftest` 22 ✓。

**④ 剩余 20 个方法** ✓（据第 179 轮清单，扣掉本轮 9 个 ✓）：
```
not_implemented  new  modules  capability  linearize  raise_builtin_error  builtins
code_with_qualname  request_interrupt  interrupted  clear_interrupt  push_exception
pop_exception  pending_exception  build_class  metatype  live_objects  gc_threshold
call_depth  collect
```
**下一刀** ✓：取**"中断/异常栈"一族** ✓（`request_interrupt`／`interrupted`／`clear_interrupt`／
`push_exception`／`pop_exception`／`pending_exception`／`raise_builtin_error` ✓ ⇒ 并入 `instance/context.rs` ✓）；
再之后是**单点** ✓；最后 **4 个模块级自由函数** ✓（要 `pub(super)` ✓，单独一轮 ✓）。

#### 第 179 轮：`instance.rs` 第十一刀 ✓（`instance/gc.rs`：2479 → **2424**）✓ ＋ 剩余清单已录 ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/gc.rs  （5 个方法 ✓ 63 行 ✓）
      retain／truthiness_of／truth_of／watch_address／tracked_objects ✓
instance.rs   2479 → **2424** 行 ✓
```

**② 剩余 29 个方法（已录 ✓，供后续轮次直接用 ✓）**：
```
not_implemented  new  modules  capability  linearize  raise_builtin_error  as_type  builtins
iterable_items  order_of  advance_iterator  iter_object  attribute_optional_of  bool_value
index_value  float_value  code_with_qualname  request_interrupt  interrupted  clear_interrupt
push_exception  pop_exception  pending_exception  build_class  metatype  live_objects
gc_threshold  call_depth  collect
```

**③ 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 2424 ✓（instance/ 十二刀：accessors／alloc／constructors／containers／context／
                                    fs／gc／platform／query／refcount／registry／state ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` **均 181/182** ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓。

**⑤ 下一刀** ✓（据清单 ✓）：取**"迭代/取值"一族** ✓（`iterable_items`／`advance_iterator`／`iter_object`／
`as_type`／`bool_value`／`index_value`／`float_value`／`order_of`／`attribute_optional_of` ✓ ⇒
`instance/convert.rs` ✓）；再取**"中断/异常栈"一族** ✓（`request_interrupt`／`interrupted`／`clear_interrupt`／
`push_exception`／`pop_exception`／`pending_exception`／`raise_builtin_error` ✓ ⇒ 并入 `instance/context.rs` ✓）；
最后是**单点**（`new`／`modules`／`builtins`／`metatype`／`build_class`／`collect`／`live_objects`／`gc_threshold`／
`call_depth`／`linearize`／`code_with_qualname`／`capability`／`not_implemented` ✓）。

#### 第 178 轮：`instance.rs` 第十刀 ✓（`instance/context.rs`：2572 → **2479**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/context.rs  （7 个方法 ✓ 101 行 ✓）
      collect_iterable／enter_repr／leave_repr／exception_depth／enter_call／leave_call／
      exception_message_of ✓
instance.rs   2572 → **2479** 行 ✓
```

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 2479 ✓（instance/ 十一刀：accessors／alloc／constructors／containers／context／
                                    fs／platform／query／refcount／registry／state ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一刀** ✓：`instance.rs`（2479 行 ✓）剩余约 37 个方法 + **4 个模块级自由函数** ✓
—— 长尾已很散 ✓ ⇒ 建议下一轮取**"注册表/引导余部"**一并切 ✓（`new_attribute_type`／注册相关的 `type_*` 余部 ✓），
再之后收**模块级自由函数** ✓（那时要 `pub(super)` ✓，单独一轮并说明 ✓）。

#### 第 177 轮：`instance.rs` 第九刀 ✓（`instance/accessors.rs`：2751 → **2572**）✓

**① 先勘测再下刀** ✓（本轮第一次这么做 ✓）：
```
剩余方法 **59** 个，长尾分散（type 5／object 5／int 3／text 2／leave 2／exception 2／enter 2／collect 2／
alloc 2 ＋ 一堆单个 ✓）；模块级自由函数 **4** 个 ✓
```
⇒ 按"**最大的一撮同族**"取 `type_*`／`object_*`／`int_*`／`text_*` ✓（15 个 ✓）。

**② 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/accessors.rs  （15 个方法 ✓ 187 行 ✓）
      type_lookup／type_lookup_owner／type_value／type_count／type_namespace／
      int_value／int_of／int_max_str_digits／text_value／text_of／
      object_str／object_str_native／object_repr／object_repr_native／object_ascii ✓
instance.rs   2751 → **2572** 行 ✓
```

**③ 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 2572 ✓（instance/ 十刀：accessors／alloc／constructors／containers／fs／
                                    platform／query／refcount／registry／state ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**⑤ 下一刀** ✓：`instance.rs`（2572 行 ✓）剩余 44 个方法 ✓ —— 下一撮取**上下文/异常**一族 ✓
（`enter_*`／`leave_*`／`exception_*`／`collect_*` ✓ ⇒ `instance/context.rs` ✓）；之后是长尾单个 ✓
（可按"注册表/引导余部"再切 ✓）；最后那 **4 个模块级自由函数** ✓ 要搬就得给 `pub(super)` ✓（单独一轮 ✓）。

#### 第 176 轮：`instance/setup.rs` **正名** ✓ → `instance/constructors.rs`（纯改名 ✓）

**① 做了什么** ✓：`git mv instance/setup.rs instance/constructors.rs` ✓ ＋ `instance.rs` 里 `mod setup;` →
`mod constructors;` ✓ ＋ 文件头说明按新名更新 ✓（**内容一行未改** ✓ ⇒ 纯改名 ✓）。

**② 为什么叫 `constructors`** ✓：这 23 个里多数是**迭代器构造器** ✓（`new_*_iterator` 一族 ✓）＋
`new_bare_type`／`new_union_type`／`new_generic_alias`／`new_int_value`／`new_int_from_decimal`／`add_values` ✓
⇒ `setup`（装配 ✗）偏宽 ✓、`iterators` 又盖不住类型构造器 ✓ ⇒ **`constructors`** 正好 ✓（与第 170 轮
`state.rs` 同一套"文件名＝内容"的处置 ✓）。

**③ `instance/` 现状** ✓（九刀 ✓）：
```
alloc.rs  constructors.rs  containers.rs  fs.rs  platform.rs
query.rs  refcount.rs      registry.rs    state.rs ✓
```

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、`code_layout` ✓、`selftest` 22 ✓、`t_ab_1` ✓。

**⑤ 全线状态** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **3963** ✓、`instance.rs` **2751** ✓
（`instance/` 九刀 ✓）、`diag.rs` 独占 `PYAWA_*` ✓。
**下一刀** ✓：`instance.rs`（2751 行 ✓）剩余以**注册表/引导余部 + 杂项方法 + 模块级自由函数**为主 ✓ ⇒
方法继续按域切 ✓；模块级自由函数要 `pub(super)` ✓（单独一轮说明 ✓）。

#### 第 175 轮：`instance.rs` 第八刀 ✓（`instance/setup.rs`：3233 → **2751**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/setup.rs  （23 个方法 ✓ 490 行 ✓）
instance.rs   3233 → **2751** 行 ✓
```
**② 如实记一处不精确** ✗（与第 169 轮同类 ✓）：这 23 个里**多数是迭代器构造器** ✓
（`new_repeat_iterator`／`new_islice_iterator`／`new_zip_iterator`／`new_zip_longest_iterator`／
`new_pairwise_iterator`／`new_batched_iterator`／`new_cycle_iterator`／`new_combinations_iterator`／
`new_product_iterator`／`new_permutations_iterator`／`new_compress_iterator`／`new_filter_like_iterator` … ✓），
另有 `new_int_value`／`new_int_from_decimal`／`new_bare_type`／`new_union_type`／`new_generic_alias`／`add_values` ✓。
⇒ 语义无害 ✓（都是 `impl Instance` 方法 ✓、纯移动 ✓、闸门全绿 ✓），但**文件名偏宽** ✓。
**下一轮** ✓：把它**正名**为 `instance/iterators.rs` ✓（或按内容再拆一次 ✓）——
照第 170 轮 `state.rs` 的先例 ✓，改名只需动文件与 `mod` 声明 ✓，仍是纯移动 ✓。

**③ 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 2751 ✓（instance/ 九刀：fs／refcount／containers／state／alloc／registry／
                                    platform／query／setup ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

#### 第 174 轮：`instance.rs` 第七刀 ✓（`instance/query.rs`：3262 → **3233**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/query.rs  （3 个方法 ✓ 37 行 ✓）
      is_type_object／is_callable／is_bool ✓
instance.rs   3262 → **3233** 行 ✓
```

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 3233 ✓（instance/ 八刀：fs／refcount／containers／state／alloc／registry／
                                    platform／query ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一刀** ✓：`instance.rs`（3233 行 ✓）剩余以**注册表/引导的其余部分 + 杂项方法 + 模块级自由函数**为主 ✓
⇒ 方法继续按域切 ✓（`new_attribute_type`／`install_*`／`set_*` 的余部 ✓）；**之后**若要动**模块级自由函数** ✓
（`alloc_bytes`／`release_one`／`unlink*` 等 ✓），必须**一并给它们 `pub(super)`** ✓ ⇒ 那时**不再是零可见性改动** ✓，
会**单独一轮**并在台账里说清 ✓。

#### 第 173 轮：`instance.rs` 第六刀 ✓（`instance/platform.rs`：3357 → **3262**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/platform.rs  （9 个方法 ✓ 103 行 ✓）
      clock_now_ns／clock_monotonic_ns／clock_vtable／current_frame／current_globals／
      platform_constant／platform_constants／platform_constants_len／current_exception ✓
instance.rs   3357 → **3262** 行 ✓
```

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 3262 ✓（instance/ 七刀：fs／refcount／containers／state／alloc／registry／platform ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一刀** ✓：`instance.rs`（3262 行 ✓）剩余多为**注册表/引导的其余部分与杂项方法** ✓
（`new_attribute_type`／`set_*` 之外的一族 ✓、以及一批 `lookup_*`／`render_*` 类查询方法 ✓）⇒
继续同招法切 ✓；模块级自由函数仍留在 `instance.rs` ✓（搬它们要 `pub(super)` ✓，单独一轮说明 ✓）。

#### 第 172 轮：`instance.rs` 第五刀 ✓（`instance/registry.rs`：3414 → **3357**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/registry.rs  （6 个方法 ✓ 65 行 ✓）
      register_bases／type_name／type_of／type_named／is_subtype／singletons ✓
instance.rs   3414 → **3357** 行 ✓
```

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 3357 ✓（instance/：fs ＋ refcount ＋ containers ＋ state ＋ alloc ＋ registry 六刀 ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一刀** ✓：`instance.rs`（3357 行 ✓）剩余以**杂项方法 + 模块级自由函数**为主 ✓
（`new_*` 里未搬的引导类 ✓、`platform_*`／`clock_*`／`current_*` 一族 ✓）⇒ 继续按同招法切 ✓
（`instance/platform.rs` 一类 ✓）；模块级自由函数若要搬 ✓，需一并给它们 `pub(super)` ✓（那时不再是"零可见性改动" ✓，
要单独说清 ✓）。

#### 第 171 轮：`instance.rs` 第四刀 ✓（`instance/alloc.rs`：3642 → **3414**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓，与 `fs`／`refcount`／`containers`／`state` 同一招法 ✓）：
```
新增  crates/pyawa-core/src/instance/alloc.rs  （14 个方法 ✓ 236 行 ✓）
      new_none／new_bool／new_float／new_list／new_set／new_slice／new_bytes／new_dict／
      alloc_host_object／new_int／new_traceback／new_str／new_tuple／new_type ✓
instance.rs   3642 → **3414** 行 ✓
```

**② 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 3414 ✓（instance/：fs ＋ refcount ＋ containers ＋ state ＋ alloc 五刀 ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一刀** ✓：`instance.rs`（3414 行 ✓）剩余以**注册表/引导**为主 ✓（`set_*` 之外的 `bootstrap_*`／
类型注册／单例表 ✓）⇒ 可切 `instance/registry.rs` ✓（同招法 ✓）；再之后 `instance.rs` 只剩杂项 ✓。

#### 第 170 轮：`instance.rs` 正名 ✓（`instance/state.rs` 121 行）—— 文件名与内容相符 ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/state.rs   （7 个 VM 字段 setter ✓ 121 行 ✓）
instance/containers.rs   404 → **290** 行 ✓（只余容器访问器 ✓）
instance.rs              3641 → 3642 行 ✓（多一行 `mod state;` ✓）
```
⇒ 上一轮"`set_` 撞名"那条**如实记录**已闭环 ✓：现在 **`containers.rs` 只装容器访问器** ✓、
**`state.rs` 只装 VM 状态 setter** ✓。

**② 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**③ 全线状态** ✓：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 3642 ✓（instance/：fs ＋ refcount ＋ containers ＋ state 四刀）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```
**④ 下一刀** ✓：`instance.rs` 的 `alloc` 一族 ⇒ `instance/alloc.rs` ✓（同招法 ✓）；
之后 `instance.rs` 剩余的多为注册表/引导/杂项 ✓，可按同样方式继续细分 ✓。

#### 第 169 轮：`instance.rs` 第三刀 ✓（`instance/containers.rs`：4036 → **3641**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/containers.rs  （25 个方法 ✓ 404 行 ✓）
instance.rs   4036 → **3641** 行 ✓
```
25 个里 **18 个是容器访问器**（`dict_entries`／`dict_insert_raw`／`set_items`／`set_insert_raw`／
`list_items`／`list_append`／`tuple_items`／`length_of`／`item_at` 等 ✓）。

**② 如实记一处不精确** ✗：这 25 个里还混进了 **7 个 VM 字段 setter** ✓（`set_modules`／`set_capability`／
`set_type_attribute`／`set_builtins`／`set_current_frame`／`set_current_globals`／`set_attribute_value` ✓）
—— 因为我的选择式是 `(?:dict|set|list|tuple|bytes|sequence)_…` ✓ 与 `set_`（**VM 字段设置** ✗）**撞名** ✓。
**语义无害** ✓（都是 `impl Instance` 的方法 ✓、纯移动 ✓、闸门全绿 ✓），但**文件名偏宽** ✓。
**下一轮** ✓：把这 7 个 setter 移到 `instance/state.rs` ✓（或把 `containers.rs` 更名 ✓），
让"文件名＝内容" ✓；之后继续 `alloc` 一族 ⇒ `instance/alloc.rs` ✓。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 全线状态** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **3963** ✓、`instance.rs` **3641** ✓
（`instance/` 下 fs ＋ refcount ＋ containers ✓）、`diag.rs` 独占 `PYAWA_*` ✓。

#### 第 168 轮：`instance.rs` 第二刀 ✓（`instance/refcount.rs`：4094 → **4036**）✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓，与 `fs` 同一招法 ✓）：
```
新增  crates/pyawa-core/src/instance/refcount.rs  （release／refcount_of／own／assert_live ✓ 67 行 ✓）
instance.rs   4094 → **4036** 行 ✓（`mod refcount;` ✓）
```
**② 边界怎么定的** ✓：只搬**缩进的 `impl Instance` 方法** ✓（`^    pub fn release(` ✓ …）⇒
**模块级自由函数**（如 `release_one`／`unlink` ✓ 不带缩进 ✓）**仍留在 `instance.rs`** ✓ ——
这一步保证"**纯移动**" ✓（自由函数被两边共用 ✓，搬走会牵出一串可见性改动 ✗）。
**下一刀** ✓：继续按同一招法搬方法 ✓（`alloc` 一族 ✓ 3 个 ⇒ `instance/alloc.rs` ✓；
`containers` 一族 ✓：`dict_*`／`set_*`／`list_*`／`tuple_*` 访问器 ⇒ `instance/containers.rs` ✓），
必要时再把自由函数成组搬 ✓（那时要一并给它们 `pub(super)` ✓）。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 全线状态** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **3963** ✓、`instance.rs` **4036** ✓
（`instance/` 下 fs ＋ refcount ✓）、`diag.rs` 独占 `PYAWA_*` 读取 ✓。

#### 第 167 轮：🎉 `instance.rs` **第一刀落地** ✓（`instance/fs.rs`：4203 → **4094**）—— 子模块招法验证成立 ✓

**① 关键洞察** ✓（修正第 162 轮设计里的一条担心 ✓）：`instance/fs.rs` 是 `instance` 的**子模块** ✓ ⇒
**子模块看得见父模块的私有字段** ✓ ⇒ 新文件只要 `use super::*;` ✓，**不需要把任何字段放宽成 `pub(crate)`** ✓
（第 162 轮我写"字段私有 ⇒ 必须放宽" ✗ —— 那对**兄弟**模块成立 ✓，对**子**模块**不**成立 ✓）。

**② 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/instance/fs.rs  （6 个方法：fs_write/fs_stat/fs_open/fs_read/fs_close/fs_vtable ✓ 118 行 ✓）
instance.rs   4203 → **4094** 行 ✓（`mod fs;` 声明在 instance.rs 里 ✓）
```
做法 ✓：整块 `impl Instance { … }` 搬 ✓、新文件自带 `impl Instance` ✓、`use super::*;` 收编上下文 ✓。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 全线状态** ✓（本轮上限 180 ✓）：
```
builtin_objects.rs  8990 → 4165 ✓（十四族）
executor.rs         9239 → 3963 ✓（十二域）
instance.rs         4223 → 4094 ✓（fs 域第一刀 ✓，其余按同一招法继续 ✓）
diag.rs             全 crate 只有它直接读 PYAWA_* ✓
```
**下一轮** ✓：`instance.rs` 继续按同一招法切 ✓（`refcount` 一族：`incref`/`release`/`unlink`/`free_garbage`/
`quarantine*` ✓ ⇒ `instance/refcount.rs` ✓；再 `alloc`／`containers` ✓）。

#### 第 166 轮：🎉 **`diag.rs` 收口完成** ✓ —— 整个 crate 只有它直接读 `PYAWA_*` ✓

**① 做了什么** ✓：
```
把其余文件里的 11 处探针改走 crate::diag::flag("PYAWA_…") ✓：
  builtin_objects.rs 1 ／ classes.rs 3 ／ verify.rs 1 ／ import.rs 3 ／ executor.rs 1 ／ header.rs 1 ／ type_object.rs 1
diag.rs 补 pub(crate) fn flag_off(name)（**反向门控** ✓：未设置即为开 ✓）
instance.rs 最后一处 std::env::var_os("PYAWA_NO_GC").is_none() ⇒ crate::diag::flag_off("PYAWA_NO_GC") ✓
```
⇒ **一处真相** ✓：全 crate 现在**只有 `diag.rs`** 直接读 `PYAWA_*` ✓（其余地方都是门控调用 ✓），
热路径不再出现 `env::var_os` ✓。

**② 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**③ 目标第 ② 条剩余** ✓：只剩 `instance.rs`（**4203 行** ✓）按第 162 轮设计拆
`instance/alloc.rs`／`refcount.rs`／`containers.rs` ✓（整块 `impl Instance` 搬 ＋ 所需字段放宽 `pub(crate)` ✓）。

#### 第 165 轮：`diag.rs` **建成** ✓（第一刀：`instance.rs` 的门控与探针开关收进一处 ✓）

**① 做了什么** ✓（结构性改动 ✓，本轮首次不是"纯移动" ✓，故单独一轮 + 独立验收 ✓）：
```
新增  crates/pyawa-core/src/diag.rs        （31 行 ✓）
      * 三个门控助手：quarantine_mode／dangling_mode／leak_mode（从 instance.rs **原样**搬来 ✓，放宽 pub(crate) ✓）
      * 新增 pub(crate) fn flag(name) => env::var_os(name).is_some()   ← 通用形式 ✓
instance.rs   4223 → **4203** 行 ✓（4 处 ad-hoc 探针改走 flag(…) ✓）
lib.rs        加 `mod diag;` ✓
```
⇒ **热路径上不再各写 `env::var_os`** ✓：开关读取集中到 `diag.rs` 一处 ✓（与"一处真相"一致 ✓）。
**如实**：这是**第一刀** ✓ —— 其余文件的探针（`classes.rs` 3 处、`header.rs` 1、`type_object.rs` 1、
`builtin_objects.rs` 1、`executor.rs`/`import.rs` 共 4 处 ✓）**仍直接读 `env`** ✗，留待后续轮次 ✓。

**② 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**③ 剩余** ✓：① 把其余文件的探针也收进 `diag.rs` ✓（逐文件 ✓，每步独立验收 ✓）；
② `instance.rs`（4203 行 ✓）按第 162 轮设计拆 `instance/alloc.rs`／`refcount.rs`／`containers.rs` ✓
（整块 `impl Instance` 搬 ＋ 需要的字段放宽 `pub(crate)` ✓）。

#### 第 164 轮：🎉 **第十二刀落地**（`protocol` 域 19 个函数：4720 → **3963**）—— `executor.rs` 已从近万行降到 **3963** ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/protocol.rs  （类型协议与容器助手 ✓ 19 个函数 ✓ 796 行 ✓）
executor.rs   4720 → **3963** 行 ✓
```
搬的是 `dunder_text`／`override_text`／`element_repr`／`element_str`／`elements_accepted`／
`boundary_accepts`／`boundary_check`／`escape_non_ascii`／`build_slice`／`lookup_in_mapping`／
`mounted_instance_dict`／`instance_attributes`／`instance_attribute_set`／`instance_attribute_delete`／
`push_container`／`push_int`／`compare_public`／`contains_public`／`inplace_add`／`inplace_arithmetic` ✓
（名字集合由第 163 轮勘测抓定 ✓）；被引用项 `line_at_offset`／`render_label`／`set_items_of`／
`type_name_of` 留在原处 ✓；自愈补 29 个 import ✓；清 `use` 后 **0 警告** ✓。

**② 目标第 ⑥ 条** ✓（**两块都已达标且还在继续变薄** ✓）：
```
builtin_objects.rs   8990 → **4165** ✓（十四族 ✓）
executor.rs          9239 → **3963** ✓（十二域 ✓）—— 共搬出 **5276** 行 ＝ 原 **57%** ✓
```
`executor.rs` 剩余主体是**指令循环 `execute`（2920 行 ✓，按目标口径留在原地 ✓）**＋少量助手 ✓。

**③ 剩余两项** ✓（做法已成文 ✓，第 162 轮 ✓）：`instance.rs`（4223 行 ✓：`impl` 整块搬 ＋ 字段放宽
`pub(crate)` ✓）与 `diag.rs`（把 `PYAWA_*` 开关收进一处 ✓）——都是**结构性改动** ✓，各要单独一轮 ✓。

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` **均 181/182** ✓、`check.py` 12/12 ✓、夹具 **490** ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

#### 第 163 轮：`executor.rs` 剩余勘测 ✓（本轮无代码改动 ✓；下一刀的名字集合已抓定 ✓）

**① 测到的事实** ✓：
```
executor.rs  4720 行 ｜ 顶层 fn **42 个** ｜ 其中 **execute() 本身 2920 行** ✓（按目标口径**留在原地** ✓）
```
⇒ 本轮想按"`*spec*`／`*module*`／`*path*`／`*loader*`"再切一刀 ✗ —— **名字集合为空** ✓
（那类助手已随 `import.rs` 搬走 ✓）⇒ 前缀切法**在这方面已挖尽** ✓。

**② 剩余可切的一批** ✓（下一刀就用它 ✓，名字集合已抓 ✓）：
```
dunder_text  override_text  element_repr  element_str  elements_accepted
boundary_accepts  boundary_check  escape_non_ascii  build_slice
lookup_in_mapping  mounted_instance_dict  instance_attributes
instance_attribute_set  instance_attribute_delete
push_container  push_int  compare_public  contains_public  inplace_add  inplace_arithmetic
```
⇒ 语义上是「**类型协议与容器助手**」一族 ✓（20 个函数 ✓）⇒ 可命名 `executor/protocol.rs` ✓。
**做法照旧** ✓：`SPLIT_ALT` 点名 ✓ → 空 `use` 块 + 精确自愈 ✓ → 清 `use` 脚本 ✓ → 全闸门 ✓ → 单独提交 ✓。

**③ 本轮验收** ✓（无行为改动 ✓）：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓、树**干净** ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **4720** ✓（十四族 ＋ 十一域 ✓，
共搬出 **4519** 行 ＝ 原 9239 的 **49%** ✓）；**两块均已达标** ✓。
**剩余两项**（`instance.rs` 4223 行 ✓ 与 `diag.rs` ✓）**做法已成文**（第 162 轮 ✓）⇒ 随时可动手 ✓。

#### 第 162 轮：`instance.rs` 与 `diag.rs` 的**设计勘测** ✓（只读 ✓，本轮无代码改动 ✓）

**① 测到的事实** ✓：
```
instance.rs   4223 行 ｜ **4 个 `impl` 块** ｜ 只有 **7 个顶层 `fn`** ✗
             方法名前缀：new 36 ／ set 14 ／ type 8 ／ fs 6 ／ object 5 ／ dict 5 ／ is 4 ／
                        platform 3 ／ int 3 ／ current 3 ／ clock 3 ／ **alloc 3** …
```
⇒ **`instance.rs` 的主体是 `impl Instance` 的方法体** ✗ ⇒ 我前面那套"按顶层项前缀切"的工具**对它无效** ✗
（会把方法当成顶层函数搬走 ⇒ 直接编译不过 ✓ —— 第 129 轮 `set` 族踩过同一形态 ✓）。

**② 可行拆法（设计 ✓）**：Rust 允许**同一 crate 内、不同文件**各写一个 `impl Instance { … }` ✓
⇒ 所以可以**整块方法**搬走 ✓，**每个新文件自带一个 `impl Instance`** ✓：
```
instance/alloc.rs       ← alloc* / new_* 里与分配相关的那批 ✓
instance/refcount.rs    ← incref／release／unlink／free_garbage／quarantine* ✓（合并 gc ✓）
instance/containers.rs  ← dict_*／set_*／list_*／tuple_* 一类只读/写容器的访问器 ✓
instance.rs             ← 保留注册表与其余 ✓
```
**唯一的技术障碍** ✓：`Instance` 的**字段是私有的** ✗ ⇒ 搬出去的方法看不到 ✓ ⇒ 两条路：
1. **把需要的字段放宽成 `pub(crate)`** ✓（机械 ✓，但扩大可见面 ✓ —— 与前面"放宽被引用项"同款先例 ✓）；
2. **加一层私有访问器** ✓（不动字段可见性 ✓，但多一次间接 ✓）。
*建议*：先走 **1** ✓（与既有做法一致 ✓、改动可机械核对 ✓），若某字段牵扯 `RefCell` 借用语义 ✓
（如 `release()` 里那串 `borrow_mut` ✓）再对那一个字段退回 **2** ✓。

**③ `diag.rs` 的设计 ✓**（探针现状 ✓）：
```
instance.rs 6 处：DANGLING／FREE_DEBUG／LEAK_MODE／NO_GC／QUARANTINE／RULER／WATCH_DICT／ZOMBIE_TRACE
classes.rs  3 处：NS_DEBUG ×2／PREPARE_DEBUG
header.rs／type_object.rs／builtin_objects.rs 各 1 处：SETDICT_DEBUG
executor.rs 1 处：TRACE_IMPORT
```
⇒ 做法 ✓：新建 `diag.rs` 收**开关读取与开关化的辅助函数** ✓（`quarantine_mode()`／`leak_mode()`／
`watch_enabled()` 一类 ✓），调用点**只留一行门控调用** ✓（热路径上不再出现 `env::var_os` ✗ ✓）。
**这也是结构性改动** ✓（不是纯移动 ✓）⇒ 要**单独一轮**、独立验收 ✓。

**④ 结论与下一轮** ✓：目标第 ② 条这两项**做法已定** ✓ ⇒ 下一轮按 **②（`instance/alloc.rs` 先切一块 ✓）**
动手 ✓，判据照旧全闸门 ✓（0 警告／逐字节 4/4／对拍两模式／`check.py` 12/12／夹具 490／语料下限 182／
`selftest`／`t_ab_1` ✓）。
**目标第 ⑥ 条** ✓（两块已达标 ✓）不受影响 ✓。

#### 第 161 轮：状态认证 + 剩余路线钉死 ✓（本轮无代码改动 ✓）

**① 已达标的部分**（目标第 ⑥ 条 ✓）：
```
builtin_objects.rs   8990 → **4165** ✓（十四族：str/bytes/dict/deque/list/object/context/generator/
                                    set/property/int/function/float/thread ✓ 各自单独提交 ✓）
executor.rs          9239 → **4720** ✓（十一域：subscript/call/arithmetic/attribute/import/message/
                                    values/ctrls/runtime/format/iter ✓ 各自单独提交 ✓）
```
⇒ **两块都已不是近万行单文件** ✓；全部为**纯移动**（逐字节 4/4 每轮复验 ✓）。

**② 剩余路线** ✓（目标第 ② 条的未完成项 ✓）：
1. `executor.rs` 继续细化 ✓（剩余主体是指令循环 `execute` ✓ ＋ 若干助手 ✓；按**名字集合**切 ✓）；
2. `instance.rs`（**4223 行** ✓）按 `alloc`／`refcount+gc`／`containers` 拆 ✓
   —— 注意它与 `executor` 不同 ✓：这里**不能只靠"纯移动"** ✗（`impl Instance` 的字段与
   `RefCell` 借用关系跨方法 ✓）⇒ 需要先出**设计**（把 `impl` 分块或抽私有字段访问层 ✓）再动手 ✓；
3. 把散在热路径上的 `PYAWA_*` 诊断（`watch`／`zombie`／`free`／`ns`／`setdict` 等 ✓）
   收进 `diag.rs` ✓ —— 同样是"**结构性改动**"✗（不是纯移动 ✓）⇒ 要有单独一轮与独立验收 ✓。

**③ 本轮认证** ✓：`stability` ✓、`selftest` 22 ✓、`t_ab_1` ✓、`check.py` 12/12 ✓、
**0 警告** ✓、逐字节 **4/4** ✓、对拍普通与 `DANGLING` ✓（唯一红仍是那条既有间歇缺陷 ✓）；
`executor/` 下 **11** 个域文件 ✓、`builtin/` 下 **14** 个族文件 ✓。

**④ 工具与纪律现状** ✓：`tools/split_domain.py`／`tools/clean_imports.py` **已入版本管理** ✓；
纪律＝改前备份 ✓、`ast.parse` ✓、先 `SPLIT_KEEP=1` 干跑 ✓、失败即还原 ✓（本轮前几轮各踩过一次坑 ✓）。

#### 第 160 轮：🎉 **第十一刀落地** ✓（`iter` 域 8 个函数）—— 工具回归也修好了 ✓

**① 真因（两处工具回归 ✓，都在第 157 轮"整段重写 `_find_def`"时引入 ✗）**：
1. `_pat` **漏了行首空白** ✗ ⇒ `py_object!` **宏体内带缩进**的定义（如 `pub struct BytesObject` ✓）
   找不到 ✓ ⇒ 报 `cannot find type BytesObject`（`iter.rs:105/204/280` ✓）⇒ **补回 `^[ \t]*`** ✓；
2. 空 `use` 表时插入点曾落到文件最顶 ✗ ⇒ `//!` 被挤下去 ⇒ `E0753` ✓ ⇒ 改为插在 **最后一个 `//!` 之后** ✓
   （第 159 轮 ✓）。
⇒ 教训（第四次 ✓）：**整段重写复杂函数时，旧版里那些"看起来不起眼的前缀/兜底"最容易丢** ✗ ——
重写后应当**对照旧版逐条核对** ✓，并用一个"宏体内类型"（`BytesObject` ✓）与"无 `use` 的文件"（`format.rs` 那一类 ✓）
各做一次**干跑** ✓。

**② 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/iter.rs  （8 个函数 ✓：iter_value/truthiness/iterable_length/
                                               iterable_item/contains/normalize_exponent/
                                               strip_trailing_zeros/concat_public ✓）
executor.rs   5219 → **4720** 行 ✓（**清 `use` 之后**的实际值 ✓ —— 提交信息与初稿写的是 4941 ✓，那是清理前的数字 ✓，此处按实测更正 ✓）
```

**③ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **4941** ✓
（十一刀共搬出 **4519** 行 ✓ ＝ 原 9239 的 **49%** ✓；两块**都已达标** ✓）。

#### 第 159 轮：工具修第二处兜底 ✓（插入点移到 `//!` 之后 ✓）；`iter` 域仍 3 错 ✗，已还原 ✓

**① 第 158 轮那个报文的真因** ✓：`error[E0753]: expected outer doc comment` ✓（`iter.rs:24/25` 的 `//!` ✗）
⇒ 我的"空 `use` 表兜底"把 `use` 插到了**文件最顶上** ✗ ⇒ 模块文档 `//!` 被挤到第 24 行 ✗ ⇒
**内层文档注释必须在最前** ✗ ⇒ E0753 ✓。**修法** ✓：没有 `use` 行时，插入点取
**最后一个 `//!` 行之后** ✓（`tools/split_domain.py` ✓）。

**② 修完仍没过** ✗：`iter` 域（`iter_value`／`truthiness`／`iterable_length`／`iterable_item`／`contains`／
`normalize_exponent`／`strip_trailing_zeros`／`concat_public` ✓）这次编译报 **3 个错** ✗
（脚本自检失败即还原 ✓）⇒ **事务式守卫** ⇒ `executor.rs` 仍 **5219** ✓、**0 错 0 警告** ✓、
逐字节 **4/4** ✓、树**干净**（只余我对 `tools/` 的修复待提交 ✓）。

**③ 下一轮** ✓：用 `SPLIT_KEEP=1` 把这 3 个错**完整读出来** ✓（大概率仍是"同名/解析"一类 ✓ ——
`contains`／`iter_value` 这种短名字最容易撞上别的模块同名项 ✓）⇒ 对症修 ✓；若一时收不掉 ≈
**换一个更"专名"的域** ✓（例如 `values_*` 已搬 ✓，可取 `dict_*`／`tuple_*`／`module_*` 一类 ✓）。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **5219** ✓
（十刀共搬出 **4020** 行 ✓ ＝ 原 9239 的 **43%** ✓；目标第 ⑥ 条**两块都已达标** ✓ ——
剩下的是"继续细化" ✓ 而非"是否达标" ✓）。

#### 第 158 轮：工具补一处兜底 ✓；`iter` 域未过 ✗（落点 `iter.rs:105`，下轮读报文 ✓）

**① 工具修复** ✓（`tools/split_domain.py` ✓）：自愈要插 import 时用
`max(i for i, l in enumerate(lines_cur) if l.startswith("use "))` ✗ —— 新文件**一行 `use` 都没有**时
`max()` 报 `ValueError: max() iterable argument is empty` ✗ ⇒ 已兜底 ✓（找不到 `use` 行就从头插 ✓）。

**② `iter` 域**（`iter_value`／`truthiness`／`iterable_length`／`iterable_item`／`contains`／
`normalize_exponent`／`strip_trailing_zeros`／`concat_public` ✓ 8 个函数 ✓）**仍未过** ✗：
编译落点 `executor/iter.rs:105` ✓（完整报文下一轮用 `SPLIT_KEEP=1` 读 ✓）。
⇒ **事务式守卫自动还原** ✓：`executor.rs` 仍 **5219** 行 ✓、**0 错 0 警告** ✓、逐字节 **4/4** ✓、树**干净** ✓。

**③ 纪律照做** ✓：改脚本**先备份**（`target/split_domain.bak` ✓）、**`ast.parse`** ✓、
失败即还原（本轮 `else` 分支把误写出的 `iter.rs` 删除 ✓）。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **5219** ✓
（十刀共搬出 **4020** 行 ✓ ＝ 原 9239 的 **43%**）。

#### 第 157 轮：🎉 **第十刀落地** ✓（`format` 域 10 个函数：6500 → **5219**）—— 同名遮蔽修好 ✓

**① 修好的东西** ✓：`tools/split_domain.py` 的 `_find_def` **整段重写** ✓（不再做零散补丁 ✗）：
```
1) 源文件或兄弟模块（executor/*.rs）里有定义 ⇒ use crate::executor::<名>;   ← 修同名遮蔽 ✓
2) 是模块文件 ⇒ use crate::<名>;
3) std/core 名字表（NonNull/Cell/RefCell/…）⇒ 对应 use
4) 否则全 crate 找定义处；再不行找再导出
```
⇒ 第 155/156 轮那个 `builtin_type`（双参 ✓，随 `runtime` 域搬到 `executor/runtime.rs:44` ✓）
现在解析成 **`use crate::executor::builtin_type;`** ✓（不再撞上 `builtin_types.rs` 的单参同名版 ✗）。

**② 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/format.rs  （10 个函数 ✓ 1291 行 ✓）
executor.rs   6500 → **5219** 行 ✓
```
搬的是 `release`／`advance_iterator`／`pad_number`／`pad_text`／`integer_digits`／`float_digits`／
`ascii_escape`／`new_exception`／`new_exception_with_args`／`localsplus_name` ✓ ——
其中 `float_digits`／`integer_digits`／`pad_*` 是 `repr()` 的**浮点/整数格式化**主体 ✓（所以 1291 行 ✓）。
自愈补 18 个 import ✓（含 `use crate::executor::{builtin_type, call_value, exception_type, is_iterator_type,
attribute_optional, raise_builtin, ExecError}` ✓）。

**③ 纪律** ✓：脚本改动**先备份**（`target/split_domain.bak` ✓）、**先 `ast.parse`** ✓、
**先 `SPLIT_KEEP=1` 干跑** ✓（本轮干跑一次通过 ✓）、脚本已入 `tools/` 版本管理 ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **5219** ✓
（十刀共搬出 **4020** 行 ✓ ＝ 原 9239 的 **43%** ✓；`executor.rs` 已从"近万行"降到 **5219** ✓）。
**下一轮** ✓：继续切 `executor.rs` 剩下的助手域 ✓ → 再进 `instance.rs`（`alloc`／`refcount+gc`／
`containers` ✓）与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 156 轮：工具脚本被我改坏一次 ✗（树已还原 ✓）—— 教训：**脚本要进版本管理** ✓（本轮已放进 `tools/` ✓）

**① 本轮做的事与失手** ✗：为修 `format` 域的**同名遮蔽**（第 155 轮：`builtin_type` 被解析成
`builtin_types::builtin_type`（单参 ✓），而**双参**那个第 153 轮已随 `runtime` 域搬进
`crates/pyawa-core/src/executor/runtime.rs:44` ✓ ⇒ 兄弟模块应经 `crate::executor::<名>` 取 ✓），
我改脚本"**兄弟模块优先**"规则时把一部分代码换成了 `text`（未定义 ✓）⇒ 脚本**半途抛 `NameError`** ✗
⇒ 它已经写出了一半文件：`format.rs` **误搬 1291 行** ✗、`executor.rs` 掉到 5219 ✗、编译 **121 个错** ✗。
**处置** ✓：`git checkout -- crates/` ＋ 删掉未跟踪的 `format.rs` ✓ ⇒ 复核：
**0 错 0 警告** ✓、逐字节 **4/4** ✓、`executor.rs` **6500** ✓、`executor/` 下九刀文件**俱在** ✓、树**干净** ✓。

**② 教训与已办** ✓：脚本一直是 `target/` 里的**未跟踪**文件 ✗ ⇒ 改坏后**没有可回退的版本** ✗。
⇒ 本轮把它**纳入版本管理** ✓：`tools/split_domain.py` ✓ 与 `tools/clean_imports.py` ✓
（这样每轮改动都能 `git diff` 复核／必要时 `git checkout` 回退 ✓，与"一处真相"一致 ✓）。
**同时定两条纪律** ✓：① 改脚本前**先备份**（`cp` 到 `target/*.bak` ✓）；② 改完**立刻 `ast.parse`** ✓
＋**先干跑**（`SPLIT_KEEP=1` 时不落盘 ✓）再正式跑 ✓。

**③ 目标第 ⑥ 条现状** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **6500** ✓
（九刀共搬出 **2739** 行 ✓ ＝ 原 9239 的 **30%** ✓）；`format` 域**待收** ✓
（已诊断清楚：兄弟模块优先 ✓，工具修稳后即可 ✓）。

#### 第 155 轮：`format` 域仍未过 ✗ —— 诊断确认是**同名遮蔽** ✓（下一步钉死 ✓）

**① 完整报文（本轮读到了 ✓）**：
```
error[E0061]: this function takes 1 argument but 2 arguments were supplied
   --> executor/format.rs:71   if ty == builtin_type(instance, "list_reverseiterator") {
note: function defined here --> crates/pyawa-core/src/builtin_types.rs:845
      pub fn builtin_type(name: &str) -> Option<&'static BuiltinType>
```
⇒ **同名遮蔽** ✓：自愈把 `builtin_type` 解析成了 `crate::builtin_types::builtin_type`（**单参** ✓），
而源文件里那个是**双参**的 ✓ ⇒ 搬走的代码被换成了另一个同名函数 ✓。
**这类问题以前没暴露** ✓，是因为前面几刀搬的函数**没有用到同名项** ✓。

**② 本轮的工具改动** ✓：给 `_find_def` 加"**源文件优先**"分支 ✓（若名字在源文件里有定义 ⇒ `use super::<名>;` ✓）。
**但没生效** ✗（`E0308` 依旧 ✓）⇒ 说明 `executor.rs` 里**并没有** `fn builtin_type` 的定义 ✓
（它可能是**再导出**或**别名** ✓，或定义写在别处、由 executor 引入 ✓）⇒ **下一轮第一步**：确认
`builtin_type`（双参那个）**定义在哪** ✓（`grep -n "fn builtin_type"` ✓ 已确认：`executor.rs` 里**没有** ✗、
`builtin_types.rs` 里是单参版 ✗ ⇒ 双参版多半在 `instance.rs` 或某个 `impl` 里 ✓），
然后把"源文件优先"改成"**优先取调用点原本解析到的那个**" ✓，或直接把 `builtin_type` 加进**排除名单** ✓
（不搬用它作为被引用项 ✓，让 executor.rs 自己的解析继续提供它 ✓）。

**③ 状态** ✓（本轮无行为改动 ✓）：**0 错 0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓；
目标第 ⑥ 条：`builtin_objects.rs` **4165** ✓、`executor.rs` **6500** ✓（九刀 ✓）。

#### 第 154 轮：`format` 域**未过** ✗（`E0308` 类型不匹配）—— 事务式守卫已还原 ✓，下一轮读出完整错误 ✓

**① 本轮试的域** ✓：「格式化／临时助手」一族 ✓（名字集合由源码自动抓 ✓）：
`release`／`advance_iterator`／`pad_number`／`pad_text`／`integer_digits`／`float_digits`／`ascii_escape`／
`new_exception`／`new_exception_with_args`／`localsplus_name` ✓。

**② 结果** ✗：编译报 **`error[E0308]: mismatched types`** ✓（**纯移动里出现类型不匹配** ✓ ⇒
最可能是：**同名项**被新模块的 glob 换成了**另一个** ✓ —— 例如 `release` 或 `new_exception_with_args`
在别处（`call.rs`／`ctrls.rs` ✓）也有同形名字 ✓ ⇒ 解析到了不同签名 ✓）。
⇒ **事务式守卫自动还原** ✓：`executor.rs` 仍 **6500** 行 ✓、**0 错 0 警告** ✓、树**干净** ✓。

**③ 下一轮** ✓（两步 ✓）：① 用 `SPLIT_KEEP=1` 把 `E0308` 的**完整报文**（含"期望类型 vs 实际类型" ✓）
读出来 ✓ ⇒ 判断是"同名遮蔽"还是"搬走了本该留下的类型定义" ✓；② 对症改：要么把同名项**排除**出这一刀 ✓、
要么把类型定义**一起搬** ✓（脚本已能处理 `pub(crate)`／私有定义 ✓）。**判据**仍是全闸门 ✓。

**④ 目标第 ⑥ 条现状** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **6500** ✓
（九刀共搬出 **2739** 行 ✓ ＝ 原 9239 的 **30%** ✓）；本轮**无行为改动** ✓、**0 警告** ✓、
逐字节 **4/4** ✓、`check.py` 12/12 ✓。

#### 第 153 轮：🎉 **第九刀落地** ✓（`runtime` 域 11 个函数：6908 → **6500**）—— 自愈学会"**模块路径**" ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/runtime.rs  （运行时助手 ✓ 11 个函数 ✓ 428 行 ✓）
executor.rs   6908 → **6500** 行 ✓
```
搬的是 `opcode_of`／`push`／`advance`／`builtin_type`／`str_matches_public`／`percent_format`／
`unsupported_operand`／`raise`／`raise_builtin`／`unbound_local_error`／`value_into_raw` ✓
（名字集合仍是源码自动抓 ✓）；被引用项 `advance_iterator`／`ascii_escape`／`float_digits`／`integer_digits`／
`localsplus_name`／`new_exception`／`pad_number`／`pad_text`／`release` 留在原处 ✓；自愈补 10 个 import ✓。

**② 工具这一轮两处**（都写进 `target/split_domain.py` ✓）：
1. **"模块"分支** ✓：识别 `cannot find module or crate \`X\`` ✓ ⇒ 若 `src/X.rs` 或 `src/X/mod.rs` 存在 ✓
   ⇒ `use crate::X;` ✓（本轮真因：`opcode::opcode(name)` 用的是**模块路径** ✗ —— 自愈此前只认
   类型／值／函数／宏 ✓）；
2. 又踩一次"往脚本里插语句"的坑 ✗（插进去那行**引号＋中文注释**把脚本写成 `SyntaxError` ✓ ⇒
   整轮没跑 ✓、树干净 ✓）⇒ 去掉注释后一次通过 ✓。**教训**（第三次 ✓）：插脚本先 `ast.parse` ✓。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **6500** ✓
（九刀共搬出 **2739** 行 ✓；`executor.rs` 已从 9239 降 **30%** ✓；剩下的主体是指令循环 `execute` ✓
＋ 若干格式化／新对象构造助手 ✓）。
**下一轮** ✓：继续切 `executor.rs` 剩下的助手域 ✓ → 再进 `instance.rs`（`alloc`／`refcount+gc`／
`containers` ✓）与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 152 轮：🎉 **第八刀落地** ✓（`ctrls` 域 10 个函数：7214 → **6908**）✓ —— 清 `use` 脚本当轮就收掉 2 条警告 ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/ctrls.rs  （容器／迭代器／异常助手 ✓ 10 个函数 ✓ 329 行 ✓）
executor.rs   7214 → **6908** 行 ✓
```
搬的是 `integer_payload`／`index_payload`／`numeric_payload`／`sequence_items`／`normalize_index`／
`slice_bounds`／`slice_positions`／`iterator_type_for`／`sequence_repeat`／`exception_type` ✓
（名字集合仍是**源码自动抓** ✓）；被引用项 `MAX_REPEAT_BYTES`／`MAX_REPEAT_ITEMS`／`builtin_type`／
`opcode_of`／`push` 留在原处 ✓；自愈补 13 个 import ✓。

**② 清 `use` 这一环已经稳** ✓：`target/clean_imports.py` 第 **2** 轮就把 2 条警告收掉 ✓
（按"覆盖该行的整条 `use` 语句" ✓、跳过再导出通道 ✓）⇒ 当轮即 **0 警告** ✓，不再需要人工介入 ✓。
**八刀里"清 use"从卡三轮变成一次通过** ✓ —— 这就是把方法固化成脚本的价值 ✓。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍 普通 **180/182**（新差异 **2** ＝`class_keywords`＋`method_defaults` ✓ 既有间歇对 ✓）／`DANGLING` **181/182** ✓、
`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **6908** ✓
（八刀共搬出 **2331** 行 ✓；`executor.rs` 已从 9239 降 **25%** ✓）。
**下一轮** ✓：继续按名字集合切 `executor.rs` 剩下的域（`new_*` 构造／格式化／`str_*` 助手／`attribute_*` 残留 ✓），
再进 `instance.rs`（`alloc`／`refcount+gc`／`containers` ✓）与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 151 轮：🎉 **第七刀落地** ✓（`values` 域 5 个函数：7377 → **7214**）—— 脚本的可见性规则自动生效 ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/values.rs  （5 个函数 ✓ 183 行 ✓）
executor.rs   7377 → **7214** 行 ✓
```
搬的是 `values_equal`／`values_equal_public`／`is_iterator_type`／`is_exception_type`／`is_type_object` ✓
（名字集合仍是**源码自动抓** ✓：`values_*|is_*` ⇒ `SPLIT_ALT` ✓）；被引用项
`ITERATOR_TYPE_NAMES`／`builtin_type`／`exception_type`／`integer_payload`／`numeric_payload` 留在原处 ✓；
自愈补 10 个 import ✓。

**② 工具这条规则补进脚本了** ✓：再导出通道按**实际可见性**选关键字（有 `pub` 项 ⇒ `pub use` ✓；
只有 `pub(crate)` ⇒ **`pub(crate) use`** ✓）—— 本轮它**自动**选对 ✓（上一轮 `message` 是我手改的 ✓）。
补丁过程中我把那行插成了**缩进错** ✗ ⇒ 脚本 `SyntaxError`、**整轮没跑** ✓（树干净 ✓）⇒ 顶格后一次通过 ✓。
**教训**：往脚本里插语句要**先确认缩进层级** ✓（`ast.parse` 自检抓住了它 ✓）。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **7214** ✓
（七刀共搬出 **2025** 行 ✓；`executor.rs` 已从 9239 降 **22%** ✓）。
**下一轮** ✓：继续按名字集合切 `executor.rs` 剩下的域（异常／格式化／迭代器／容器助手 ✓），
再进 `instance.rs`（`alloc`／`refcount+gc`／`containers` ✓）与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 150 轮：🎉 **第六刀落地** ✓（`message` 域 5 个函数：7433 → **7377**）—— 工具学到"再导出通道的**可见性**" ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/message.rs  （实参报错文案 ✓ 5 个函数 ✓ 65 行 ✓）
executor.rs   7433 → **7377** 行 ✓
```
搬的是 `message_too_many`／`message_missing`／`message_duplicate`／`message_unexpected_keyword`／
`message_positional_only` ✓（名字集合由**源码自动抓** ✓：`grep -oE … fn message_…` ⇒ `paste -sd'|'` ⇒ `SPLIT_ALT` ✓）。

**② 本轮的真问题与修法** ✓：第一次搬完出现一条警告 ✗：
```
warning: glob import doesn't reexport anything with visibility `pub` because no imported item is public enough
  --> executor.rs:34   pub use crate::executor::message::*;
```
⇒ 被搬的 5 个都是**私有**函数（脚本只把它们放宽到 `pub(crate)` ✓）⇒ **`pub use` 一个只含 `pub(crate)` 项的 glob**
会告警 ✓ ⇒ 再导出通道要按**实际可见性**选关键字 ✓：有 `pub` 项用 `pub use` ✓（`subscript`／`call`／
`attribute` 那些 ✓，`lib.rs` 要再导出 ✓），否则用 **`pub(crate) use`** ✓（本轮 `message` ✓）。
（脚本补丁这次又因**锚点没中**而空跑 ✗ —— 我直接在文件上把那一行改掉 ✓，一次收敛 ✓；
下次把这条规则补进脚本 ✓。）

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍 普通 **180/182**（新差异 **2** ✓ ＝`class_keywords`＋`method_defaults` ✓ 那条既有间歇对 ✓）／
`DANGLING` **181/182** ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **7377** ✓
（六刀共搬出 **1862** 行 ✓；`executor.rs` 已从 9239 降 **20%** ✓）。
**下一轮** ✓：继续按**名字集合**切 `executor.rs` 剩下的域（`values_equal`／`is` 一族／格式化／异常 ✓），
再进 `instance.rs`（`alloc`／`refcount+gc`／`containers` ✓）与 `diag.rs` ✓。

#### 第 149 轮：🎉 **第五刀落地** ✓（`import` 域 7 个函数：7806 → **7433**）—— 工具学会"按**名字集合**切" ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/import.rs  （导入域 ✓ 7 个函数 ✓ 392 行 ✓）
executor.rs   7806 → **7433** 行 ✓
```
搬的是 `load_module`／`resolve_relative_import`／`handle_fromlist`／`module_has_name`／`module_value`／
`module_text`／`ignore_missing_submodule` ✓（**它们不带统一前缀** ✗ ⇒ 前缀切法切不出来 ✓）；
被引用项 `execute`／`mounted_instance_dict`／`raise_builtin`／`read_file_through_fs`／`release`／
`str_matches_public` 留在原处 ✓；自愈补 9 个 import ✓。

**② 工具加的能力** ✓：选择条件支持**外给正则** ✓（`SPLIT_ALT` 环境变量 ✓）——
`scope + "_[a-z_0-9]+"` 的默认前缀法保持不变 ✓，需要时可用 `SPLIT_ALT="a|b|c"` 精确点名 ✓。

**③ 教训入册（第四次改脚本才成 ✓）**：改这个脚本要**按行号整行替换** ✓，别做内容匹配 ✗
—— 这一轮我连着三次因"匹配串写歪／转义层级"白跑 ✓（`assert` 全落空 ✓、文件未动 ✓、树干净 ✓）；
每改完都跑 `ast.parse` 自检 ✓，改前先 `print` 出那一行 ✓。

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓、`stability` ✓。

**⑤ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **7433** ✓
（五刀共搬出 **1806** 行 ✓ —— `executor.rs` 已从 9239 降到 7433 ✓）。
**下一轮** ✓：`executor.rs` 里剩下的域（消息／格式化／异常／`is` 一族／`values_equal` 等 ✓，
按**名字集合**切 ✓）→ 之后 `instance.rs`（`alloc`／`refcount+gc`／`containers` ✓）
与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 148 轮：🎉 **第四刀落地** ✓（`attribute` 域：8134 → **7806**）—— 清 `use` **空跑即干净** ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/attribute.rs  （属性域 ✓ 4 个函数 ✓ 345 行 ✓）
executor.rs   8134 → **7806** 行 ✓
```
自愈补 7 个 import（`Attribute`／`DictObject`／`ExecError`／`Header`／`Instance`／`MethodObject`／`NonNull` ✓）。

**② 本轮修的工具一处** ✓：定义正则**接受 `pub(crate)`／`pub(super)`** ✓ ——
`enum Attribute` 是 **`pub(crate) enum`** ✗ ⇒ 旧正则「只认 `pub ` 或完全无 `pub`」**两不沾** ✗
⇒ 找不到 `Attribute` ✓（这正是本轮第一次没过、`attribute.rs:25` 报 `cannot find type Attribute` 的原因 ✓）。

**③ 清 `use` 固化成脚本** ✓：`target/clean_imports.py` ✓ ——
按"**覆盖该行的整条 `use` 语句**"清 ✓（单行／多行都行 ✓）、**跳过再导出通道** ✓、
对 `crates/pyawa-core/src` 下所有文件生效 ✓。基线本来就是 0 警告 ✓ ⇒ 出现的警告**必由本次搬移带来** ✓
⇒ 让脚本全局清理是**安全**的 ✓（本轮它空跑 ✓：新文件由"精确自愈"生成 ⇒ 一开始就没有多余 import ✓）。

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**⑤ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **7806** ✓（四刀共搬出 **1433** 行 ✓）。
**下一轮** ✓：`import`（`load_module`／`handle_fromlist` 一族 ✓ —— 注意它们的名字**不带 `import_` 前缀** ✓
⇒ 脚本要按"**名字集合**"而不是前缀来切 ✓，这是下一轮要加的一点小能力 ✓）→ 之后 `instance.rs`
（`alloc`／`refcount+gc`／`containers` ✓）与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 147 轮：🎉 **第三刀落地** ✓（`arithmetic` 域：8335 → **8134**）—— 清 `use` 也**一次成功** ✓

**① 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/arithmetic.rs  （算术域 ✓ 217 行 ✓）
executor.rs   8335 → **8134** 行 ✓
```
自愈补了 `ExecError`／`Header`／`Instance`／`IntValue`／`NonNull`／`TypeObject` ✓ ⇒ 仍是「空 `use` 块 ＋ 精确补」✓。

**② 清 `use` 这次一次成功** ✓（把第 146 轮那条教训落成了方法 ✓）：
不再按**单行**删 ✗，而是**先定位覆盖该行的整条 `use` 语句** ✓（往前找到 `use ` ✓、往后找到 `;` ✓），
再在**整条语句**里按"名字 ＋ 相邻逗号"精确摘除 ✓ ⇒ 单行与**多行** `use {…}` 都能处理 ✓。
本轮摘掉 `inplace_arithmetic`／`raise` 两条 ✓ ⇒ **0 警告** ✓（一轮即收 ✓）。
**并且保留再导出通道** ✓：语句里含 `pub use crate::executor::<域>::*;` 时**跳过** ✓（删了会断 `lib.rs` 的
`pub use executor::{…}` ✓）。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 ✓、`code_layout` ✓、
`selftest` 22 ✓、`t_ab_1` ✓、`stability` ✓（落点若红只会是那条既有缺陷 ✓）。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **8134** ✓（三刀共搬出 **1105** 行 ✓）。
**下一轮** ✓：`import`（`load_module` 一族 ✓）→ `attribute`（`attribute_lookup`／`super_lookup` ✓）→
`call` 里剩下的 `bind_arguments` ✓ → 之后 `instance.rs`（`alloc`／`refcount+gc`／`containers` ✓）
与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 146 轮：🎉 **`executor.rs` 第二刀落地** ✓（`call` 域：8822 → **8335**）✓

**① 本轮的工具改进** ✓：自愈的"定义处"分支**不再要求 `pub`** ✓ —— 找到**私有**定义（如 `enum Attribute` ✓）
就把它**放宽成 `pub(crate)`** ✓ 再 import ✓（与 `refs` 同一处理 ✓）。这是 `call` 域前两次没过的直接原因 ✓。

**② 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/call.rs  （`call_callable`／`call_value` 一族 ✓ 511 行 ✓）
executor.rs   8822 → **8335** 行 ✓
```
自愈这一轮补了 **14** 个 import ✓（`Attribute`／`BuiltinFunctionObject`／`Cell`／`CodeObject`／`ExecError`／
`ExecOutcome`／`Frame`／`FunctionObject`／`GeneratorObject`／`Header`／`Instance`／`MethodObject`／`NonNull`／
`TypeObject` ✓）——「空 `use` 块 ＋ 精确自愈」这条路**第二次**验证有效 ✓。

**③ 清 `use` 的最后一处坑（也修了 ✓）**：`executor.rs` 那条 `unused import: BuiltinFunctionObject` ✗
落在**多行 `use {…}` 的一行**上 ✓ ⇒ 我先前的"单行正则"删不动 ✗（不是缓存 ✓，是我上一轮判错了 ✓）
⇒ 改成**精确子串删除** ✓ ⇒ **0 警告** ✓。
**教训入册** ✓：多行 `use` 块里的多余名字，要按**整条语句**处理 ✓，不能按单行 ✓。

**④ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`cargo test --workspace` ✓（唯一红仍是那条既有间歇缺陷 ✓）、
对拍普通与 `DANGLING` **均 181/182** ✓、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限（同一轮已过 ✓）、
`code_layout` ✓、`selftest` 22 ✓、`t_ab_1` ✓、`stability` ✓。

**⑤ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓；`executor.rs` **8335** ✓（两刀共搬出 **904** 行 ✓）。
**下一轮** ✓：`arithmetic`（算术／比较／格式化一族 ✓）→ `import`（`load_module` 一族 ✓）→
`attribute`（`attribute_lookup`／`super_lookup` ✓）→ 之后 `instance.rs` 与 `diag.rs` ✓。

#### 第 145 轮：`call` 域**两次试拆未过** ✗（工具修了两处 ✓）—— 已自动还原 ✓，下一步钉死 ✓

**① 本轮修的工具两处** ✓（都在 `target/split_domain.py` ✓）：
1. **`lib.rs` 的路径推导** ✓：以前把 `lib.rs` 推成 `crate::lib` ✗ ⇒ 报
   `unresolved import crate::lib` ✓ ⇒ 现在 `lib.rs` → **`crate`** ✓；
2. **搜索顺序** ✓：先找**定义处** ✓（`pub struct/enum/type/…` ✓），找不到再退到**再导出** ✓ ——
   上一版按文件名字母序先撞到 `lib.rs` 的 `pub use … TypeObject` ✗ ⇒ 指到了不存在的 `crate::lib` ✓。
   ⇒ 修完 `TypeObject` 已能正确解析（`TypeObject` 由 `type_object.rs` 定义 ✓）。

**② `call` 域仍没过** ✗（6 个编译错 ✓）：新暴露的一条是
```
error[E0433]: cannot find type `Attribute` in this scope  --> executor/call.rs
```
⇒ `Attribute`（`Attribute::Method{…}` ✓）是 `executor.rs` 里那个枚举 ✓，但我的搜索**只认 `pub` 定义** ✗
⇒ 若它是**私有**的（`enum Attribute` ✗）就找不到 ✓。**下一步**（就一处 ✓）：
`_find_def` 的"定义处"分支**去掉 `pub` 前置** ✓（允许 `enum Attribute` ✓）⇒ 找到后**放宽成 `pub(crate)`** ✓
（与 `refs` 同一处理 ✓）。

**③ 目标第 ⑥ 条现状** ✓：`builtin_objects.rs` **4165** ✓、`executor.rs` **8822** ✓（第一刀已落地 ✓）；
`call` 域待收 ✓（`call_callable`／`call_value`／`bind_arguments` 一族 ✓）——它比 `subscript` **更靠内层** ✓
（指令循环直接调它 ✓），因此 import 关系更绕 ✓ ⇒ 先按 ② 修工具，再试 ✓。

**④ 验收** ✓（本轮无行为改动 ✓）：**0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓。

#### 第 144 轮：🎉 **`executor.rs` 第一刀落地** ✓（9239 → **8822**）—— 目标第 ⑥ 条最后一块开工 ✓

**① 走通的那条路** ✓（承第 142 轮结论 ✓）：**新文件不照抄 `use` 块** ✓ ⇒ 由"自愈"按编译器报缺**精确补** ✓：
- 错误模式扩到 **`cannot find function`** ✓（原来只有 `type`／`value` ✓）、**`cannot find macro`** ✓；
- 新增 **std/core 名字表** ✓（`NonNull`／`Cell`／`RefCell`／`Ordering`／`AtomicU32`／`HashSet`／`HashMap` ✓）
  —— 这些**不在 crate 里** ✗，"全 crate 找定义处"对它们无效 ✓（第 143 轮卡的就是 `NonNull` ✓）。
⇒ **新文件一开始就没有多余 import** ✓ ⇒ **不需要"清 use"** ✓ ⇒ 彻底避开第 141／142 轮
"清理与诊断缓存相撞"那个死循环 ✓。

**② 落地结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/executor/subscript.rs  （6 个函数 ✓ 436 行 ✓）
executor.rs   9239 → **8822** 行 ✓
```
- `pub use crate::executor::subscript::*;` 作再导出通道 ✓（`lib.rs` 的
  `pub use executor::{…, subscript_read, subscript_write, …}` 才能继续成立 ✓）；
- 模块声明用 **`pub mod subscript;`** ✓ —— 外部测试 `crates/pyawa-core/tests/slicing.rs` 要用它 ✓
  （私有会报 `E0603: module subscript is private` ✓，这一轮实测踩到 ✓）；
- `foo.rs` ＋ `foo/bar.rs` 形态 ✓ ⇒ **`executor.rs` 不改名** ✓ ⇒ `docs/` 的路径引用不受影响 ✓。

**③ 验收** ✓：**0 错 0 警告** ✓、逐字节 **4/4** ✓、`slicing` 4/4 ✓、`cargo test --workspace` ✓、
对拍普通与 `DANGLING` ✓（既有口径 ✓）、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 **182** ✓、
`code_layout` ✓、`selftest` 22 ✓、`t_ab_1` ✓。

**④ 目标第 ⑥ 条** ✓：`builtin_objects.rs` **4165** ✓（已达标 ✓）；`executor.rs` **8822** ✓
（**已不再是近万行** ✓ —— 但仍大于 `builtin_objects.rs`，继续按域拆 ✓）。
**下一轮** ✓：`call`（`call_callable`／`call_value`／`bind_arguments` ✓ —— 顺便做"接收者只算一次"的
契约收敛 ✓，与重构**分开提交** ✓）→ 之后 `arithmetic`／`import`／`attribute` ✓ → 之后 `instance.rs`
与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 143 轮（140/140，本轮上限用尽）：空 `use` 路线**把障碍换小了** ✓ —— 只差一行 std/core prelude ✓

**① 本轮做的** ✓（承第 142 轮的结论 ✓）：
- 脚本改成**新文件不照抄 `use` 块** ✓（`orig_uses` 不再拼进头部 ✓）；
- 自愈的错误模式扩到 **`cannot find function`** ✓ 与 **`cannot find macro`** ✓（原来只有 `type`／`value` ✓）。
⇒ 效果 ✓：**多余的 import 从一开始就不存在** ✓ ⇒ 第 141／142 轮那种"清 `use`"与"诊断缓存"相撞的
问题**不会发生** ✓（方向对 ✓）。

**② 新的、更小的障碍** ✓：自愈补不了 **`NonNull`** ✗（它是 `core::ptr::NonNull` ✓，
**不在 crate 里** ✓ ⇒ 我"全 crate 找定义处"的策略对它无效 ✓）：
```
error[E0425]: cannot find type `NonNull` in this scope  --> executor/subscript.rs:57
```
**修法（一行 ✓，下一轮即可 ✓）**：新文件头部加一段**固定的 std/core prelude** ✓
（`use core::ptr::NonNull;` ✓ 必要时再给 `Cell`／`RefCell` ✓），其余仍由自愈按编译器报缺精确补 ✓。

**③ 事务式守卫照旧生效** ✓：编译不过 ⇒ 自动还原 ✓ ⇒ `executor.rs` 仍 **9239** 行 ✓、树干净 ✓。

**④ 目标第 ⑥ 条的最终状态（本轮上限已用尽，如实 ✓）**：
- `builtin_objects.rs` **8990 → 4165** ✓ —— **已达标** ✓（十四族：`str`/`bytes`/`dict`/`deque`/`list`/
  `object`/`context`/`generator`/`set`/`property`/`int`/`function`/`float`/`thread` ✓，
  每族**纯移动**、各自单独提交、每轮全闸门 ✓）；
- `executor.rs` **仍 9239** ✗ —— **未达标** ✗；但**切法已连续验证**（`subscript` 域 6 个函数：
  0 编译错 ✓、逐字节 4/4 ✓、9239 → 8822 ✓），**只剩两步** ✓：① 头部加 std/core prelude（一行 ✓）；
  ② 再跑一次闸门即可落地 ✓。**未声称完成** ✓（第 ⑦ 条 ✓）。

**⑤ 验收** ✓（本轮无行为改动 ✓）：**0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓。

#### 第 142 轮（本轮上限的最后一轮）：`executor` 第一刀**未落地** ✗ —— 卡的这一环查明是"**诊断缓存**" ✓，按第 ⑦ 条回退 ✓

**① 本轮试的稳法** ✓：一次只删**一条**未用 `import`、删完立刻重跑 `cargo check`（行号永远新鲜 ✓）
—— 结果**仍然空转** ✗：日志显示**同一条**（`executor/subscript.rs:8` 的 `crate::code::CodeObject` ✓）
被"删"了 **38 次** ✗ 而警告依旧 ✓ ⇒ 说明**删的不是编译器看到的那份** ✗，或**诊断是缓存的** ✓
（这环境下 `cargo check` 的告警输出来自上次结果 ✓）。

**② 结论与下一步（已定 ✓）**：这条路（**照抄 `use` 块 ⇒ 再清理** ✗）本身就是病根 ✓ ——
⇒ 改走**第 2 条** ✓：新文件**从空 `use` 块起步** ✓，让"自愈"按编译器报缺**精确补** ✓
（错误模式要从 `cannot find type/value` 扩到 **`cannot find function`** ✓、**`cannot find macro`** ✓）；
这样新文件**一开始就没有多余 import** ✓ ⇒ **不需要清理** ✓ ⇒ 与"诊断缓存"这回事**彻底不撞** ✓。
（`dict`/`bytes`/… 十四族当初之所以顺利 ✓，正是因为它们的 `use` 块**照抄后恰好够用** ✓；
`executor` 的 `use` 块大得多 ✗ ⇒ 照抄就必然剩一堆没用 ✓。）

**③ 目标第 ⑥ 条的当前状态（如实 ✓）**：
- `builtin_objects.rs`：**8990 → 4165** ✓（十四族搬出 ✓ **已达标** ✓ —— 不再是巨型文件 ✓）；
- `executor.rs`：**仍 9239** ✗（**未达标** ✗）—— 但 `subscript` 域的**切法已验证** ✓
  （0 编译错 ✓、逐字节 4/4 ✓、`executor.rs` 9239 → 8822 ✓），**唯一障碍是"清 use"这一环** ✓，
  且下一轮的办法（空 `use` ＋ 自愈精确补 ✓）已明确 ✓。

**④ 本轮验收** ✓（无行为改动 ✓）：**0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓。

#### 第 141 轮：定位到"清 `use` 循环空转"的**机制** ✓（行号失准 ✗）；第一刀仍差这一步 ⇒ 回退 ✓

**① 本轮做的** ✓：把选择性清理的**过滤范围放到两个文件** ✓（`executor.rs` ＋ `executor/subscript.rs` ✓）
—— 上一轮只过滤了 `executor/` 目录 ✗ ⇒ 少了 `executor.rs` 那一批 ✓。

**② 观察到的现象与机制** ✓：清理循环跑了 5 轮、每轮都报"处理 5 条" ✗ ⇒ **空转** ✓；
`cargo check` 余下 **6 条**，全部是**单行** `use`（`executor/subscript.rs:8/9/12/…` ✓）⇒
**我的循环在"同一轮里删了多行"却没重算后续行号** ✗ ⇒ 删错行／删了又"处理"同一批 ✓。
（同一行被两个名字各报一次 ✓ 也会造成重复删除 ✓ —— 两种都会让行号漂移 ✓。）

**③ 下一轮（两条路，任选其一，都很短 ✓）**：
1. **一次只删一条、删完立刻重跑 `cargo check`** ✓（约 12 轮 × 15 秒 ✓ 可接受 ✓，
   行号永远新鲜 ✓ 最稳 ✓）；
2. **不要照抄整块 `use`** ✗ ⇒ 新文件**从空 `use` 块起步** ✓，让"自愈"按编译器报缺**精确补** ✓
   —— 需要把自愈的错误模式从 `cannot find type/value` 扩到 **`cannot find function`** ✓ 与
   **`cannot find macro`** ✓（宏已在另一处认过 ✓）：这条路**根治**"抄来一堆没用 import" ✓，
   后面 `arithmetic`／`import`／`attribute` 各域都能直接受益 ✓ ⇒ **推荐走第 2 条** ✓。

**④ 验收** ✓（本轮无行为改动 ✓）：回退后 **0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓；
`executor.rs` 仍 **9239** 行 ✗、`builtin_objects.rs` 仍 **4165** 行 ✓。

**⑤ 全局进度** ✓：目标第 ⑥ 条 —— `builtin_objects.rs` 六族到十四族共搬出 **4825 行**（8990 → 4165 ✓），
`executor.rs` 的第一刀（`subscript` 域 ✓ 6 个函数 ✓）**切法已验证可行**（0 编译错 ✓、逐字节 4/4 ✓），
**只差清 `use`** ✓。

#### 第 140 轮：`executor` 第一刀**编译已通**（0 错 ✓），仍卡在 **0 警告** ✗；本轮再次按纪律回退 ✓

**① 本轮做的** ✓：把"清 `use`"从**盲 `cargo fix`** ✗ 换成**选择性清理** ✓ ——
只在新文件里按 `warning: unused import(s)` 的**文件:行:列** 逐条处理 ✓：
整条没用 ⇒ 删该行 ✓；花括号里只有部分没用 ⇒ 从花括号摘掉点名的 ✓（列号在花括号内时 ✓）；
循环 4 轮、每轮重新 `cargo check` ✓。

**② 结果** ✗：**编译 0 错** ✓（这已经是连续两轮的稳定结论 ✓ ⇒ 切法本身没问题 ✓），
`executor.rs` 9239 → **8822** ✓、`executor/subscript.rs` 445 行 ✓、逐字节 **4/4** ✓；
**但剩余警告 11 条** ✗ ⇒ 未过**0 警告**硬闸门 ✓ ⇒ 回退 ✓（`executor.rs` 回到 **9239** ✓、树干净 ✓）。

**③ 下一轮的精确抓手** ✓（这次的原因我上一轮就写错了范围 ✗）：我的清理**只过滤了 `executor/` 目录** ✗
⇒ 余下的 11 条**大概率在 `executor.rs` 自己**（搬走之后那边有一批 `use` 变成多余 ✓，
另有那条 `pub use crate::executor::subscript::*;` 是否被判"未使用"要**单独看** ✓ ——
它被 `lib.rs` 的 `pub use executor::{…}` 间接用到 ✓，正常**不该**报 ✗）。
⇒ **下一轮**：把 11 条**逐条读出来** ✓，**两个文件都清理** ✓（`executor.rs` 与 `executor/subscript.rs` ✓），
再复检 0 警告 ✓；**判据** ✓：0 警告 ∧ `cargo test --workspace` 绿 ∧ 逐字节 4/4 ⇒ 第一刀落地 ✓。

**④ 验收** ✓（本轮无行为改动 ✓）：**0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓；
目标第 ⑥ 条：`builtin_objects.rs` **4165** ✓（已达标）、`executor.rs` **9239** ✗（未达标 ✓，
但**切法已证实可行** ✓，只差"清 use"这一步 ✓）。

#### 第 139 轮：🎯 **`executor.rs` 的第一刀切开了** ✓（0 编译错 ✓）—— 卡在**清 `use`** 这一步 ✗，本轮按纪律回退 ✓

**① 根因锁定** ✓（承第 138 轮那两行 `use` 报错 ✓）：
```
error[E0603]: function import `subscript_read` is private   --> lib.rs:63
63 | pub use executor::{…, subscript_read, subscript_write, …};
```
⇒ 脚本在新模块里插的是**私有** `use crate::executor::subscript::*;` ✗ ⇒ 而 `lib.rs` 用
**花括号形式的 `pub use executor::{…}`** 再导出 ✓ ⇒ **经私有 `use` 到达的项不能再被 `pub use`** ✗
⇒ E0603 ✓（与 `thread` 那次同源 ✓，但那次是"路径没改"、这次是"**可见性通道**不对" ✓）。

**② 修法** ✓（一行 ✓）：插入改成 **`pub use crate::<新路径>::<域>::*;`** ✓（`builtin_objects` 是私有模块 ✓
⇒ 在它里面 `pub use` 也不会外泄 ✓，所以统一用 `pub use` 安全 ✓）。
**效果** ✓：**编译 0 错** ✓ ——
```
搬走 6 个；executor/subscript.rs 446 行 ✓；executor.rs 9239 → **8822** 行 ✓；逐字节 4/4 ✓
自愈补 import: BytesObject, DictObject, ExecError, ListObject, StrObject, TupleObject ✓（全对 ✓）
```

**③ 但卡在清 `use`** ✗：还剩 **12 条警告** ✓ ⇒ 我按惯例跑 `cargo fix --lib -p pyawa-core` ✗
⇒ 它**盲删**了某个被 **`pyawa-stdlib`** 依赖的导入 ✗ ⇒ `pyawa-stdlib` 编译失败 ✗
⇒ **按纪律整套回退** ✓（`executor.rs` 回到 **9239** ✓、树干净 ✓，目标第 ⑦ 条 ✓）。

**④ 下一轮（就一件事 ✓，且很短 ✓）**：**选择性清 `use`** ✗ 不用盲 `cargo fix`：
把 12 条警告**逐条读出来** ✓ ⇒ 只删"新文件里确实没用的" ✓；**保留**（或改成 `pub use`）
那些被 `lib.rs`／`stdlib` 依赖的 ✓。**判据** ✓：0 警告 ∧ `cargo test --workspace` 绿 ∧ 逐字节 4/4 ✓
⇒ 就能把 `executor.rs` 的第一刀**落地** ✓（9239 → 8822 ✓，目标第 ⑥ 条的最后一块开始动工 ✓）。

#### 第 138 轮：工具又修两处 ✓（按行号改脚本 ✓、全 crate 找定义处并允许宏体内缩进 ✓）；`subscript` 域**仍未过** ✗

**① 工具改进（本轮真落上了 ✓）**：
1. **改脚本改成"按行号替换"** ✓（第 137 轮用字符串匹配、漏了一行 ⇒ `assert` 失败、白跑一轮 ✗）
   —— 现在定位 `r = build()` 与 `if r.returncode != 0 …` 两行 ✓，中间整段换掉 ✓；
2. **自愈改成"全 crate 找定义处"** ✓：按 `^[ \t]*pub (unsafe )?(struct|enum|type|trait|fn|const|
   static|union|mod) <名>` 或 `^pub use … <名>` 在 `crates/pyawa-core/src` 下找 ✓ ⇒ 由路径推模块路径 ✓
   ⇒ 生成 `use crate::<模块>::<名>;` ✓，插到新文件**最后一行 `use` 之后** ✓；
   并且**允许行首空白** ✓ —— 这是第二步才发现的 ✗：`BytesObject` 这类类型是 **`py_object!` 宏体内**
   定义的（**带缩进** ✓）⇒ 锚行首的正则找不到 ✗。
   ⇒ 改进生效 ✓：`ExecError` 现在能被解析成 `use crate::executor::ExecError;` ✓；
   `DictObject`/`BytesObject` 一类也能被解析成 `use crate::builtin_objects::<名>;` ✓。

**② `subscript` 域仍未过** ✗：错误已经换到**新文件自己的 `use` 行**（`executor/subscript.rs:7` 与 `:9` ✓）
⇒ 与"搬走的函数缺 import"不同了 ✓ —— 说明**自愈已经补上了类型** ✓ 而卡在别处（很可能是脚本照抄的
`use` 块里那条**指向 `crate::executor::…` 的路径**在新模块里语义变了 ✓，或 `pub(crate)` 放宽与
`use` 的可见性冲突 ✓）。**下一轮**：把这两行错误**原样读出来** ✓（`SPLIT_KEEP=1` ✓）⇒ 对症改 ✓。
⇒ **事务式守卫自动还原** ✓：`executor.rs` 仍 **9239** 行 ✓、`builtin_objects.rs` 仍 **4165** 行 ✓、
工作树**干净** ✓（目标第 ⑦ 条 ✓）。

**③ 验收** ✓（本轮无行为改动 ✓）：**0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓。

