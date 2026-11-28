# Spike A-4a：替代「每 opcode 一个续体」的幂等重启可行吗？（2026-11-28）

- **问题**（`PLAN §12.22` 的 A 本体后续轮）：wasm 上「原生→脚本」每层嵌套一次 `execute` 就吃一层
  宿主引擎 wasm 调用栈，约 500–600 层即 `RangeError` trap（`DIV-11` ③；round 291 实测）。A-4 的
  通用解是"每个 opcode 一个续体"，代价大；A-4a 备选是 **opcode 挂起时中止、把用户调用交给驱动
  单独跑，回来后重放该 opcode**，用 **operand 对象内 `pending` 状态**保证重放幂等。
- **方法**：只读分析（`executor.rs` 分派／`execute_stack`／`call_callable`／`Frame::ResumePoint`）
  ＋ 用 main `1456939e` 的 release CLI 实测各族"深链每层吃几层 `execute`"（`PYAWA_NATIVE_DEPTH`
  阈值法，脚本见 §7）。未改生产代码；本分支只含本报告。
- **结论一句话**：**A-4a NO-GO**（作为通用机制）；**A-4 走逐族显式状态机（A-1／A-2a 式）GO**。

## 1. 结论（先行）

1. **重放不可幂等的面太大，不是边角**。多个 native helper 在调用用户代码**之前**已有不可回退的
   原生副作用：内建迭代器链先推进内层再调谓词/映射函数（`advance_map`：`iter_advance.rs:910`
   推进 → `:919` 调用户函数）；`sorted` 先把可迭代源消费成 `items` 再算 key（`builtins_module.rs:3290`
   → `:3300`）；`list.sort` 的 Timsort 整个跑在 Rust `sort_by` 闭包里（`list.rs:238`），无法从
   比较回调处挂起；类实例化先分配实例/跑 `__new__` 再调 `__init__`（`call.rs:560` → `:596`/`:614`），
   重放 = 建第二个实例、`__new__` 副作用重复。
2. **"省掉逐 opcode 续体"的省力前提不成立**。重放要求：(a) 操作数不能先 pop——现 handler 正是
   先 pop 再调（`step_operators.rs:48-49`／`:108-109`）；(b) 值栈要能回到指令前（需快照/回卷 +
   incref 记账）；(c) 用户调用结果要按**调用序号**做 memo（多调用族如 rich compare 左/右/`__bool__`
   最多 3 次，`protocol.rs:1770-1810`）。这三样加起来≈逐族状态机的工作量，且比显式状态机多一层
   "重放副作用重复"的隐藏语义风险。
3. **重放代价 O(k²)**（k＝该条指令内的用户调用数）。大容器 `repr`（A-1 worklist 每步可能一次
   `__repr__`）与排序类会把 native 工作重算到不可接受；`map` 链是 round 291 的实测探针路径
   （`docs/rounds/04-post-m6-01.md:14080`），恰好被 A-4a 覆盖不了。
4. **实测**：深链每层恰 1 层嵌套 `execute`（`PYAWA_NATIVE_DEPTH=64` 时最大成功深度：类实例化
   62、用户 `__eq__`／`__repr__`／`map` 回调 63、dict 探测 62；再深即可捕获 `RecursionError`）。
   所以只要把这几族改成显式帧/状态机，wasm 深度就从引擎栈 500–600 抬到语义额度。
5. **A-4a 唯一可用的子集**是"单次用户调用 + 调用前无对象突变 + 操作数可保留"的 opcode
   （COMPARE_OP／BINARY_OP／FOR_ITER-用户迭代器），但这正是显式状态机最便宜的地方——直接做成
   逐族状态机即可，**不必再引入 A-4a 这个名目**。

## 2. 现有机制与最小钩子（Q1）

- **分派**：`run_frame`（`executor.rs:2214`）每条指令查静态表（`:2279`），把 handler 结果分三路：
  `Step::Return/Yield`（`:2306-2319`）、`Raised → dispatch_raise`（`:2320`）、**其它 `Err(other)` 原样
  上抛**（`:2334`，`return` 在 `:2369`）⇒ **新的 `Deferred*` 变体天然从 `run_frame` 穿出，不必改分派**
  （先例：`DeferredScriptCall` 定义 `executor.rs:98-103`、消费 `:2473`）。
- **驱动**：`execute` 只做 D1 记账（`executor.rs:2376-2392`；`enter_native_execute` `instance/misc.rs:472-490`）；
  `execute_stack`（`:2397`）已具备压帧/弹帧（`:2465-2480`）、返回值入父栈、异常在父帧 `call_offset`
  上回退派发（`:2409-2448`，含 `attach_traceback` `:2425`）、Deferred 消费（`:2473`）。
- **可直接复用的三件**：
  - **S2 `DeferredScriptCall`**：字节码 CALL 把准备完毕的帧交回驱动（`call_from_bytecode`
    `call.rs:257-268`；目标匹配 `call.rs:965-975`）——"帧交给驱动、不新开原生 `execute`"已有先例。
  - **A-2a（并行 lane，未合入：`feat/a-yield-from@32feb2c7`）**：`DeferredGeneratorSend`＋
    `DelegateSlot`＋`execute_stack` 的 Return/Yield/Raise 三条臂（该 commit `executor.rs:111`／
    `:2174`／`:2590`），SEND 命中生成器时压**委托帧**（`step_iterators.rs:706`）——这正是"逐族显式
    帧"的样板，A-4 应照此逐族加，而不是做通用重放。
  - **`Frame::ResumePoint`**（`frame.rs:44-51`：ip／exception_cursor／**持有引用的值栈镜像**；
    `suspend`／`resume` `:560-624`；`frame_traverse` `:669-673`；`frame_clear` `:739`）——挂起状态
    入帧并被 GC 遍历/清理的现成先例。**A-4a 所需的"值栈检查点"实际上要新增**：`ResumePoint` 存的是
    **挂起那一刻**的栈（生成器让出后），不是指令前镜像。
- **必须补的钩子**（A-4 逐族也用得到）：
  1. `ExecError` 新变体（建议**逐族**：如 `DeferredInitCall`／`DeferredProtocolCall{ frame }`；
     A-2a 已示逐族变体可行）。
  2. `execute_stack` 两条新语义：子帧 Return ⇒ 结果**喂回父帧 resume 槽**并把 decoder/ip 回卷到
     挂起指令（`decoder.set_position` 先例 `executor.rs:1519`；`set_instruction_pointer` `:2267`）；
     子帧 Raised ⇒ 清 resume 槽后按父帧挂起指令偏移派发（复用 `:2409-2448`）。
  3. resume 槽放 **`Frame`**（新字段）+ `frame_traverse`/`frame_clear` 增补（先例 `:669`/`:739`）；
     **不要**放 `execute_stack` 的 Rust 局部——GC 根只沿 `current_frame`/`f_back` 链扫
     （`instance/gc.rs:442-454`），Rust 局部的对象不在根集合里。
  4. **挂起授权**：现有 `deferred_call_target` 是**实例级单 Cell**（`instance.rs:349`，置/清
     `call.rs:265-267`），表达不了"哪一帧哪条指令可挂起"；嵌套 helper 会错挂。必须改成按帧
     （`Frame` 字段）或至少 `(frame, 指令偏移)` 配对。
  5. **操作数保留**：除 FOR_ITER 用 `peek`（`step_iterators.rs:30`）外，其余 handler 先 pop；
     逐族改写成"peek → 挂起 → 结果回填后再 pop/push"，或加值栈检查点（代价见 §4）。
- **覆盖不到的地方**（A-4 也要另想办法）：trace 回调本身是原生→脚本，且在指令分派**之前**
  （`executor.rs:1914`），没有"当前指令"可回卷；类体/`exec`/`eval`/`import` 的 `execute`
  （`executor.rs:572`、`builtin/compile.rs:229`、`import.rs:221,/586`）不在 opcode 挂起语义内。

## 3. 逐族可行性（Q2）

| 族 | 入口（main `1456939e`） | 挂起前已发生的不可回退副作用 | 幂等重放？ |
|---|---|---|---|
| COMPARE_OP（用户 `__eq__`/`__lt__`） | `step_operators.rs:47`（pop `:48-49`；`rich_compare` `protocol.rs:1770`；`equality_compare_result` `:1086`；最多 2 dunder＋1 `__bool__` `:1806`） | 无对象突变；操作数已在局部 | **可**（保留操作数＋调用 memo；多调用需序号） |
| BINARY_OP（用户 dunder） | `step_operators.rs:107`（pop `:108-109`；`arithmetic.rs:397`/`:505`） | 无对象突变（in-place 语义在用户代码里） | **可**（同上） |
| CONTAINS_OP／TO_BOOL | `step_operators.rs:9`/`:24` | pop | **可** |
| FOR_ITER 用户迭代器 | `step_iterators.rs:29`（`peek`）；`advance_iterator`→`iter_advance.rs:157` `call_value(__next__)` | 无（推进由子调用做）；`StopIteration` 在 native 吞掉 | **可**（memo 命中即不再推进） |
| FOR_ITER 生成器 | `step_iterators.rs:33-72`（`resume` `:51` → `execute` `:55`） | **恢复点已被 consume、generator 置 running** | **不可**（二次 `resume` 是错误）⇒ 需 A-2a 式委托帧 |
| 生成器 `SEND`／`send`／`throw`／`close` | `step_iterators.rs:416`/`:555`/`:624`；`executor.rs:1533`/`:1703` → `execute` `:1575`/`:1729`；`builtin/generator.rs:227/262` | 生成器帧状态被推进 | **不可** ⇒ 委托帧（A-2a 已覆盖 SEND 精确 generator；coroutine/async_generator 未覆盖） |
| CALL 族：实例化／`__init__`／`__call__`／类创建钩子 | `call.rs:560`（构造器）→ `:596`/`:614`（`__init__`）；`__call__` `:318`/`:338`；类创建 `:81`；通用 dunder／方法 `:113`/`:192` | **实例已分配、`__new__` 已跑、`__init__` 可能已改 self** | **不可**（重放=双实例/双副作用）⇒ 显式 `{created, initializer, step}` 状态 |
| `sorted`／`list.sort`（key/比较回调） | `builtins_module.rs:3290-3306`；`core/builtin/list.rs:216-240` | 源迭代器已被消费；Timsort 状态在 Rust 闭包内，**无法穿过挂起** | **不可**（`sorted(generator)` 重放会读到已消费的源）⇒ 只能把排序改造成可恢复状态机 |
| `map`/`filter`/`zip`/`chain`… 内建迭代器链 | `iter_advance.rs:882-923`（先推进内层 `:910`，后调用户函数 `:919`） | 内层迭代器已前进 | **不可**（重放丢元素/错位；round 291 探针正是此族）⇒ A-1 式 worklist |
| 容器 `repr`（A-1 后） | `repr_worklist.rs`（worklist 在 Rust 局部）；用户 `__repr__` 经 `protocol.rs:1976-1993` 回调 | worklist 中间态/输出缓冲在 Rust 栈；`dict` 条目快照可被用户改表影响 | **部分可**（重算＋memo，O(k²)；改表口径会漂）⇒ 建议 worklist 状态入帧（A-1 扩） |
| `hash()`／dict 操作 | `values.rs:485`（`key_hash_checked`）；dict 探测 `builtin/dict.rs:96/125/138`、`instance/containers.rs:314` | 查找无突变；插入/`update` 是多步已提交写 | 查找**可**；多步写**不可**（重放后 dict 已变 ⇒ 探测序列/memo 错位） |
| `yield from`（A-2a 在做） | A-2a：`SEND` 精确 generator → `DeferredGeneratorSend`（`@32feb2c7` `executor.rs:111`/`:2590`、`step_iterators.rs:706`） | — | **已按显式委托帧解决**；边界：FOR_ITER 生成器、coroutine/async_generator/asend 仍走 `resume→execute` |
| class body／`exec`/`eval`／`import` | `executor.rs:572`；`builtin/compile.rs:229`；`import.rs:221/586` | 命名空间/`sys.modules` 已登记 | **不可**重放（深链少见，可留边界） |

## 4. `pending` 状态放哪（Q3）

| 方案 | 结论 | 依据 |
|---|---|---|
| **operand 对象内**（A-4a 原设想） | **不可** | ① 别名：同一迭代器可被嵌套 `for` 共用（`it=iter(x); for a in it: for b in it:` 合法）⇒ 对象级 pending 无法区分两个 FOR_ITER 站点；② 布局/ABI：对象头与宿主对象载荷是 ABI 面（`AB-14` 不透明句柄；宿主对象布局由 `host.rs` 固定）⇒ 每类加字段=动所有类型 traverse/clear 与线格式；③ GC/引用：pending 内引用必须进 traverse，否则成环误回收（`frame_traverse` 先例 `frame.rs:635-674`）；④ 用户可见（`__slots__`/`sys.getsizeof`/weakref）。 |
| **执行栈槽** | **不可** | 值栈上界 `co_stacksize` 与异常表 `depth` 都是编译期定死（`frame.rs:67-68`；`emitter/control.rs:712`；`decode.rs:129-137`）；多一个"幽灵槽"会让 `BC-43` 越界、`dispatch_raise` 的 `truncate_stack`（`executor.rs:1509-1516`）口径失配，并改 `f_lasti`/traceback 观察面。 |
| **opcode 参数** | **不可** | 参数在 code object 内共享且不可变（同一码元多帧执行）。 |
| **每帧一个 `resume` 槽**（推荐） | **可行** | 每次只有一条指令处于挂起态；状态随帧走：GC 沿 `current_frame`/`f_back` 链把帧当根（`instance/gc.rs:442-454`），traverse/clear 加两处（先例 `frame.rs:669`/`:739`）；异常/中断/帧清理时随 `frame_clear` 释放；不改变值栈/ABI。`ResumePoint` 已证明"帧持有挂起状态＋被 GC 遍历"可行。 |

配套语义：
- **trace**：行事件按 `trace_last_line` 去重（`executor.rs:1877-1880`）⇒ 回卷/重放同一条指令不会重发
  `'line'`；若走逐族状态机（不重放）则无重复问题。trace 回调自身（`:1914`）仍是原生→脚本，留 D1 边界。
- **traceback**：`run_frame` 在分派前已把 ip 设为当前指令（`:2267`），子帧异常经 `execute_stack`
  在父帧 `call_offset` 上 `dispatch_raise`（`:2409-2448`）⇒ 与今日递归路径的父帧行号/`lasti` 一致。
- **异常/中断清理**：`Interrupted` 与子帧 Raised 都要**先清 resume 槽**（释放持有引用）再走既有
  上抛/派发；不能让它随 Rust 局部 Drop 而漏引用（先例：`FrameSlot` 的 `pending_raise`/A-2a
  `finish_delegate` 都是显式清理）。

## 5. 与 wasm 的关联（Q5）

- **直连 `execute` 的嵌套点（7 个）**：`call.rs:989`（脚本函数体；所有 `call_callable` 的汇合点）、
  `step_iterators.rs:55`（FOR_ITER 生成器）、`executor.rs:572`（类体）、`:1575`（`resume_generator`）、
  `:1729`（`resume_generator_with_raise`）、`import.rs:221/586`、`builtin/compile.rs:229`（`exec`/`eval`）。
- **触发面**：core 里 `call_callable(...)` 调用点 **64 处**、`stdlib` 里 `call_value`/`call_callable`
  **61 处**（`grep` 计数，2026-11-28 @`1456939e`）⇒ CALL／COMPARE_OP／BINARY_OP／CONTAINS／TO_BOOL／
  FOR_ITER／SEND／LOAD_ATTR／STORE_ATTR／NB_SUBSCR／STORE_SUBSCR／repr／str／format／hash／dict 探测／
  map/filter/zip／sorted/list.sort 回调都可能每层吃一层 `execute`。
- **A-4a 覆盖**：只有 §3 中"可"的几族（且需 memo/序号，实际就是状态机）；**不覆盖** wasm 实测的
  `map` 回调链（round 291 探针）、类实例化链、`sorted`/内建迭代器链、生成器 resume、import/exec。
  **CALL 族本质不可幂等重启**：实例已建、`__new__`/`__init__` 已跑（§3 行），重放无法回到"调用前"。
- 故**不能**期望 A-4a 把 wasm 上限从 500–600 抬到 1000；逐族状态机才覆盖实测 trap 路径。

## 6. 结论与代价（Q4）

**NO-GO（A-4a 通用化）／GO（A-4 逐族显式状态机）**。建议分段（每段＝一个 round 级改动，均不新增 ABI 面）：

| 阶段 | 内容 | 估计 |
|---|---|---|
| P0 | 挂起内核：Deferred 变体＋`Frame.resume` 槽＋驱动回卷＋异常/中断/trace 清理＋授权位（§2 的五条钩子，可被每族复用） | 1–1.5 轮 |
| P1 | 比较/相等（COMPARE_OP 全阶梯、`values_equal` 的 dunder 路径；顺带 `CONTAINS`/`TO_BOOL`） | 1–1.5 轮 |
| P2 | 实例化/`__init__`/`__call__`/描述符（`{created, initializer, step}` 状态；收益最大——类链 1 execute/层） | 1.5–2 轮 |
| P3 | 迭代：用户 `__next__`/`__iter__`＋FOR_ITER 生成器委托帧（A-2a 收口）＋内建迭代器链 worklist（`advance_map` 族） | 1.5–2 轮 |
| P4 | repr/str/format：A-1 worklist 扩成"可挂起"（状态入帧，而不是重算） | 1 轮 |
| P5 | 有状态内建回调（`sorted`/`list.sort`/`print(file=)`/dict 多步写）——最难，可整体延后，残留如实登记 `DIV-11`（wasm 这些路径仍受 500–600 限制） | 2–4 轮 |

合计 **6–10 轮**，**风险高**（执行热路径、引用计数、异常/trace 语义、conformance 407 语料 0 diff、
wasm E5 冒烟）。新增种类：每族 1 个 `ExecError::Deferred*` 变体（或 1 个通用变体＋族枚举）＋
`Frame.resume` 的一个族状态枚举＋挂起授权位；**不新增 ABI**。

若仍要做 A-4a：它最多只能作为"单次调用族的第一步"，且一旦要正确就得把 memo/序号/操作数检查点
显式写出来——即退化为 P1/P2 的状态机。**建议弃用 A-4a 名目，按 P0–P5 推进 A-4。**

## 7. 实测证据（可复现）

用 main `1456939e` 的 `target/release/pyawa`（Linux x86_64）跑探针脚本，`PYAWA_NATIVE_DEPTH=64`
（CLI 默认在 `bin/pyawa.rs:288-290` 声明 1000，可被外部覆盖；有效值＝`min(声明, recursion_limit)`
`instance/context.rs:155-156`），n 递增到可捕获 `RecursionError`：

| 族 | 脚本 | 64 下最大成功 n |
|---|---|---|
| 类实例化链（`Node(n-1)` in `__init__`） | `class Node: def __init__(self,k): self.child = Node(k-1) if k>0 else None` | **62**（n=63 起 caught） |
| 用户 `__eq__` 链（同形 Node `==`） | `a=chain(63); b=chain(63); a==b` | **63**（n=64 caught） |
| 用户 `__repr__` 链 | `repr(chain(63))` | **63** |
| `map` 回调链 | `def cb(k): return 0 if k==0 else list(map(cb,[k-1]))[0]+1` | **63** |
| dict 探测 + 用户 `__eq__` 链 | `d={K(n):1}; d[K(n)]`（`__hash__` 固定、`__eq__` 递归） | **62** |

⇒ 每层恰 1 层嵌套 `execute`（底帧 1 ＋ n ≤ 64 成立，n+1 = 64 起触发）。对照组：纯脚本递归
`def f(n): return 0 if n==0 else f(n-1)` 在预算 64 下 n=300 正常返回（S2 显式帧栈）。

## 8. 不做 / 边界

- 不改生产代码、不动 A-2a 正在改的文件（`executor.rs`／`step_iterators.rs` 只读）；本分支只含本报告。
- 不实现 A-4a 原型（结论已足够：其可用子集＝逐族状态机）；不构 wasm（复用 `DIV-11` ③ 的既有实测）。
- 不承诺 P5 的时间；`sorted`/Timsort 的可挂起化若代价过高，按 `DIV-11` 如实保留 wasm 限制。
