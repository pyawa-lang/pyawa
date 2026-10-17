# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 06（Lead＋队员团队模式）已收 **round 4–6**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 6」节）。

## 现在

- **HEAD**：`97b04ae`（`dev`，**已推送** origin ✓）—— 本笔＝round 6：M4-A 批第 3 轮（TS-47 静态检查器＋TS-48＋TS-50＋渐进保证验收＋挂账 bug＋`sys.audit`）；提交后 HEAD 见 `git log -1`。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：**通过 434 ＋ 参照口径 17 ＝ 451 ÷ 628 ⇒ 71.8%**（阈值 421/67% ✓；连测两次同值 ⇒ 区间 450–452；本轮无 import 面变化 ⇒ 与 round 5 持平）
- **M4 判据进展（A 批已全部落地，只差收口归档）**：
  - **TS-46 ✓／TS-47 ✓（编译期＋运行期两层）／TS-48 ✓／TS-50 ✓**（round 4–6）。
  - **判据②（渐进保证）有了可执行验收** ✓：`tests/gradual_guarantee.rs` 的 `T-TS-1` 20 例（17 对成对语料）＋`T-TS-3`／`T-TS-7`。
  - **判据①（`T-BC-8`／`T-BC-9` 可观测）**：可执行证据已在（`tests/boundary.rs`、`tests/assignment_check.rs`、`tests/type_check.rs`）——**round 7 做「判据 → 测试」归档 + 收口陈述**。
  - **`BC-25`①（"两侧皆标注处禁止发射"需跨模块静态信息）仍未做**——round 7 归档时如实写清（不阻塞"可观测"）。
- **round 6 落地**（详见台账本卷「round 6」节）：
  - **TS-47 编译期一半**：新 `crates/pyawa-core/src/compile/typecheck.rs`（872 行，纯静态视图）；能静态判定的不符报新变体
    `CompileError::TypeAssignment(String)`（消息含变量名／期望／实际／位置＋`TS-47` 引用）；`pyawa-abi` ⇒ `PA_ERR_SYNTAX`
    （`AB-20`）；`executor/import.rs` ⇒ 导入含类型错模块抛 `TypeAssignmentError`（当前加载器只编纯模式 ⇒ 该臂暂不可达，已注记）。
  - **TS-48**：未标注／判不出＝平台类型 `Any`（与一切相容、禁止推断）；**TS-50**：两种收窄形态（`x is None` 终止支后／
    `x is not None` 分支内与 else；只对名字、`assert` 不作依据）。
  - **验收**：`tests/type_check.rs` 7 例；`tests/gradual_guarantee.rs` 20 例（T-TS-1／3／7）。
  - **挂账 bug `with` 内嵌 `try` StackUnderflow 根因修** ✓：新增 `with_depth` 记异常表 depth（未并进 `handler_depth`，
    避免 `for: with: break` 多发 `POP_EXCEPT`）；`tests/warnings_catch.rs` 3 例。**原 `warnings.warn` 脚本另卡既有
    `sys._getframe(depth>0)` 缺口**（台账卷 04，非本 bug），第 3 例改走 `warn_explicit`。
  - **并行线 `sys.audit` no-op** ✓：`pyawa-stdlib/sys_module.rs`＋`SPEC-c-modules.md` §5.2.3 合约＋`tests/sys_audit_shapes.rs`
    2 例；`http.client` 建连从卡 `audit` 推进到 `_socket.getaddrinfo` 未接线（P3-29 进度证据）。真审计机制仍未实现（另立项）。
- **如实边界（M4 A 批）**：类体不查；嵌套函数读外层局部不建模；函数返回值/形参**边界**检查不属静态检查器；
  TS-50 未覆盖 `None is x` 反序／`== None`／`and`/`or` 复合守卫／`not (x is None)`／`isinstance`／`match`；
  形参注解不登记（形参体内重赋值不报）；属性/下标/链式/解包/`for` 目标不发检查；`global x` 静态查但运行期不发射
  （静态略严于运行期，`TS-7` 下"类型错误"允许）；CLI 消息文件名显示 `<string>`（chunkname 为 NULL，`pyawa-runtime/bin` 未接）。
- **未修 bug**：`with`-StackUnderflow **已修**（上面）；其余 5 条仍挂账：`global`＋函数内 `import` ⇒ UnboundLocalError
  （`Counter.most_common`）· `ChainMap.keys()` 静默空 · `KeyError` 消息少 repr · `functools.wraps` 不搬 `__name__` ·
  PEP 604 union 注解 dataclass 的 `__builtins__` 面。另：`sys._getframe(depth>0)` 帧链缺口（既有台账项）挡住
  `warnings.warn` 端到端。

## 下一轮开工顺序（round 7 ＝ M4 **收口**，非新特性）

1. **判据归档**：把 M4 判据①（`T-BC-8`／`T-BC-9`）与判据②（`T-TS-1`）各写成「判据 → 测试名／命令 → 实测结果」对照，
   进台账 + `PLAN-milestones.md` §9.2 的 M4 收口注；明确 `BC-25`① 跨模块缺口未做、不阻塞"可观测"。
2. **状态同步**：`README.md` 当前状态改「M0–M4 均已成立」；`DESIGN.md` §13 的 `12.` 行"已落地三条"改成与实际一致
   （A 批已落：`TS-46`／`TS-47`／`TS-48`／`TS-50`；`BC-62`／`BC-63`／`TS-49` 未做）；`docs/ROUNDS.md` 计数同步。
3. **报告用户批准**：M4 收口是否照先例 `dev` `--no-ff` 收录 `main` ＋ 打 **tag `m4`** ＋ 推送（**收口前必须问**）。
4. 并行线：socket 等用户 A1–A4 拍板；未拍板不动。
5. 跑终审闸门（判据①＋slowcheck 十项）后才算收口。

## M4 收口判据（唯一出处：`PLAN-milestones.md` §6）

① `T-BC-8`／`T-BC-9` 可观测（检查点可见、归责信息完整）；② **渐进保证**（对同一语料增删标注，除类型错误外行为不变）。
落点＝扩展特性 A 批（条款在 `SPEC-type-system.md`）：`TS-46` ✓ → `TS-47` ✓ ＋`TS-48` ✓ → `TS-50` ✓。
硬约束：扩展特性**只在 `.pyawa` 生效**；`.py` 下必须合法且惰性（`BC-15`／`TS-5`）。

## 本战役的关键口径（新会话必读）

- **分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再
  `rm -rf Lib/__pyawa__`，然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；并发测量无效。
- **指令集**：`INSTRUCTION_SET_VERSION`＝**2**；`CHECK_ASSIGNMENT` 编号 **231**（**`BC-65`**）；
  `opcode_metadata.rs` 由 `tools/gen_opcode_tables.py` 生成、**禁止手改**。
- **标签与异常**：`TypeBoundaryError`／`TypeAssignmentError` 均为 `TypeError` 子类；标签形态唯一出处 **`BC-24`**
  （类型对象／`"Any"`／`(外, 内)`／**union `("Union", 成员…)`＝任一成员相容**）；`CHECK_ASSIGNMENT` oparg＝常量表下标、
  常量必须**单标签元组**。
- **两层检查**：静态层 `compile/typecheck.rs`（扩展模式、`CompileError::TypeAssignment`，映射 `PA_ERR_SYNTAX`／
  导入时 `TypeAssignmentError`）；运行期层 `CHECK_ASSIGNMENT`（`BC-65`）＋ `TypeAssignmentError`。
- **扩展特性边界**：`.py` 下 `?` 是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`lib_compile` 全量守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8（当前队员已满 ⇒ 下一轮起**只唤醒旧成员**（复用 `ds-*`），不再新 spawn）；
  `pyawa-core` 同时至多**两名**写手且**不同子目录**；非 Rust 面／测试-only 写手可并行。
- **编号**：新增 `P*-` 前先重算当前最大号（墓碑算定义，`CX-19`；现最大 **P3-31**）。现最大文档号：
  `BC-65`／`TS-50`／`T-BC-22`／`CP-37`／`CX-22`／`P3-31`。

## 纪律提醒

- 红灯不提交 ✓；判据① **5 轮不动**即报停滞 ✓；同一处 bug **3 轮上限**停手升级 ✓；
- 长复盘进台账（`docs/rounds/`），本页只留「HEAD／工作区／下一条命令／未修 bug＋判据」。
