# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 06（Lead＋队员团队模式）已收 **round 4–7**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 7」节）。

## 现在

- **HEAD**：`c111302`（`dev`，**已推送** origin ✓）—— 本笔＝round 7：M4 收口准备（判据归档＋状态同步）；提交后 HEAD 见 `git log -1`。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：**通过 434 ＋ 参照口径 17 ＝ 451 ÷ 628 ⇒ 71.8%**（阈值 421/67% ✓；连测两次同值 ⇒ 区间 450–452；本轮无 import 面变化 ⇒ 与 round 6 持平）
- **M4 判据（全绿 ✓，只差阶段收录）**：
  - **判据①（`T-BC-8`／`T-BC-9` 可观测）✓**：可执行归档 `crates/pyawa-core/tests/m4_acceptance.rs`（3 例：
    检查点在 `dis` 可见／端到端归责四要素／三条检查指令元数据）；`BC-25`①（"两侧皆标注处禁止发射"，
    需跨模块静态信息）**仍未做**、已如实注记、不阻塞"可观测"。
  - **判据②（渐进保证 `T-TS-1`）✓**：`crates/pyawa-core/tests/gradual_guarantee.rs`（20 例，17 对成对语料
    ＋`T-TS-3`／`T-TS-7`）。
  - **A 批**：`TS-46` ✓／`TS-47` ✓（编译期＋运行期两层）／`TS-48` ✓／`TS-50` ✓。
- **round 7 落地**（详见台账本卷「round 7」节）：判据①② 归档测试＋「判据→测试」对照表进台账；
  `README.md`／`PLAN-milestones.md`（§6 新增「M4 的要求」、§9.2 `P3-13`、§9.4 第 9 项）／
  `DESIGN.md` §13 的 `12.` 行状态同步；顺带修正 `m4_acceptance.rs` 里一个**未定义测试编号**
  （`T-BC-27` ⇒ `BC-27`，`CX-1` 当场抓到）。
- **未修 bug（5 条，照旧挂账）**：`global`＋函数内 `import` ⇒ UnboundLocalError（`Counter.most_common`）·
  `ChainMap.keys()` 静默空 · `KeyError` 消息少 repr · `functools.wraps` 不搬 `__name__` ·
  PEP 604 union 注解 dataclass 的 `__builtins__` 面。另：`sys._getframe(depth>0)` 帧链缺口（既有台账项）
  挡住 `warnings.warn` 端到端（`with` 嵌套 `try` 的 StackUnderflow 本体已于 round 6 修）。

## 下一轮开工顺序（round 8 ＝ M4 **阶段收录**，等用户批准）

1. **必须先问**：`dev` 是否照先例以 `--no-ff` 收录 `main` ＋ 打 **tag `m4`** ＋ 推送？（`branch-rule.md`：
   合并提交＝阶段边界；**未批准不合并**。）
2. 批准后同笔：`PLAN-milestones.md` 的**兼容边界声明**追加**修订行**（"阶段边界必须更新本块"）；
   `README.md` 当前状态改「**M0–M4 均已成立**」；`NEXT.md` 终态刷新；合并＋打标＋推送。
3. 未批准时：转并行线（socket 等用户 A1–A4 拍板）或其余 5 条挂账 bug；不擅自合并。

## M4 收口判据（唯一出处：`PLAN-milestones.md` §6）

① `T-BC-8`／`T-BC-9` 可观测（检查点可见、归责信息完整）✓；② **渐进保证**（对同一语料增删标注，
除类型错误外行为不变）✓。落点＝扩展特性 A 批（条款在 `SPEC-type-system.md`）：`TS-46` ✓ → `TS-47` ✓
＋`TS-48` ✓ → `TS-50` ✓。硬约束：扩展特性**只在 `.pyawa` 生效**；`.py` 下合法且惰性（`BC-15`／`TS-5`）。

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
  `.py` 产物**逐字节不变**（`TS-5`，`lib_compile` 全量守）。**B 批**（`BC-62`／`BC-63`＋`TS-49`）未做。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手；
  非 Rust 面／测试-only 写手可并行。
- **编号**：新增 `P*-`／`T-*` 编号前先查是否**已定义**（`CX-1` 会抓"无定义处的引用"；墓碑算定义，`CX-19`）。
  现最大文档号：`BC-65`／`TS-50`／`T-BC-22`／`CP-37`／`CX-22`／`P3-31`。

## 纪律提醒

- 红灯不提交 ✓；判据① **5 轮不动**即报停滞 ✓；同一处 bug **3 轮上限**停手升级 ✓；
- 长复盘进台账（`docs/rounds/`），本页只留「HEAD／工作区／下一条命令／未修 bug＋判据」。
