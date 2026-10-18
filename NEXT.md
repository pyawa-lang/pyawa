# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 06（Lead＋队员团队模式）已收 **round 4–9**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 9」节）。

## 现在

- **HEAD**：`59dfbb3`（`dev`，本笔前，已推送 origin ✓）—— 本笔＝round 9：**M4 阶段收录**；
  阶段边界合并提交在 `main`（见 `git log --merges -1`），**tag `m4`** 已打并推送 ✓。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 434 ＋ 参照口径 17 ＝ 451 ÷ 628 ⇒ 71.8%**（阈值 421/67% ✓；区间 450–452；本轮只改文档 ⇒ 与 round 8 持平）。
- **M4（类型系统）已成立并收口 ✓**：判据①（`T-BC-8`／`T-BC-9` 可观测）✓ 归档
  `crates/pyawa-core/tests/m4_acceptance.rs`；判据②（渐进保证 `T-TS-1`）✓ 归档
  `crates/pyawa-core/tests/gradual_guarantee.rs`；A 批 `TS-46`／`TS-47`（编译期＋运行期两层）／`TS-48`／`TS-50` 全部落地；
  `README` 状态＝**M0–M4 均已成立**；`PLAN` 兼容边界声明已追加 M4 阶段边界修订行。
- **M4 如实缺口（不阻塞判据，日后按队列取）**：`BC-25`①"两侧皆标注处禁止发射"（需跨模块静态信息）未做；
  `TS-50` 部分形态未覆盖（`None is x` 反序／`== None`／`and`/`or` 复合守卫／`not (is None)`／`isinstance`／`match`）；
  类体不查、嵌套函数读外层局部不建模、形参注解不登记。
- **未修 bug／缺口**：`functools.wraps.__name__`（挂账第 5 条；根因＝`setattr` 写实例字典被类型 `getattr` 槽遮蔽，
  正路＝`FunctionObject` 名字覆盖字段＋可写 getset 数据描述符 ⇒ 类型级描述符专项）· 函数对象 `__builtins__` 未接 ·
  union 对象 `==`／`repr` 与参照有差（pre-existing）· `sys._getframe(depth>0)` 帧链（挡 `warnings.warn` 端到端）。
- **round 8 顺带清账**：6 条挂账 bug **收口 5 条**（`global`+函数内 `import` 的 UnboundLocalError ✓／PEP 604 `X | None`
  运行期 ✓／`ChainMap` 迭代 ✓／未捕获 `KeyError` 消息 repr ✓／`with` 嵌套 `try` 的 StackUnderflow 已在 round 6 修）；
  仅 `functools.wraps.__name__` 转专项未修。

## 下一阶段（未开工；M5 起）

1. **M5 能力层正式化**（`PLAN-milestones.md` §6）：`SPEC-capabilities.md` §11 的 `T-CP-*` 全绿；同一程序在真实实现与
   受限实现下写法不变；启动延迟/常驻内存升为阻塞项。**`P3-29` socket 真 I/O** 是用户已拍板的并行线，但
   **A1–A4 四个拍板点仍等用户口径**（侦察结论：`net` 域已在规格/ABI、ABI 零改动；首片建议 `socketpair`＋领 CP 编号、
   地址线格式 IPv4 4B＋端口 2B 大端、fd 沿用 fs 先例）。**未拍板不启**。
2. **扩展特性 B 批**（`DESIGN.md` §2.1 的扩展特性索引）：`BC-62`（多语句 `lambda`）· `BC-63`＋`TS-49`（空安全操作符）——
   规格已定、实现未做，都要动词法与发射器。
3. **类型级描述符专项**：`functools.wraps.__name__` 正路（`FunctionObject` 名字覆盖字段＋可写 getset 数据描述符）＋
   `P3-17` 同族（`property.__set__`／`staticmethod.__get__` 等）。
4. 其余挂账按 `PLAN-milestones.md` §9.2 队列取；`unittest.mock`／`multiprocessing.heap(mmap)`／CJK `_codecs_*`／`xmlrpc` 等。

## 本战役的关键口径（新会话必读）

- **分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再
  `rm -rf Lib/__pyawa__`，然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；并发测量无效。
- **指令集**：`INSTRUCTION_SET_VERSION`＝**2**；`CHECK_ASSIGNMENT` 编号 **231**（**`BC-65`**）；
  `opcode_metadata.rs` 由 `tools/gen_opcode_tables.py` 生成、**禁止手改**。
- **标签与异常**：`TypeBoundaryError`／`TypeAssignmentError` 均为 `TypeError` 子类；标签形态唯一出处 **`BC-24`**
  （类型对象／`"Any"`／`(外, 内)`／**union `("Union", 成员…)`＝任一成员相容**）；`CHECK_ASSIGNMENT` oparg＝常量表下标、
  常量必须**单标签元组**。
- **两层检查（M4 成果）**：静态层 `compile/typecheck.rs`（扩展模式、`CompileError::TypeAssignment`，映射
  `PA_ERR_SYNTAX`／导入时 `TypeAssignmentError`）；运行期层 `CHECK_ASSIGNMENT`（`BC-65`）＋ `TypeAssignmentError`。
- **扩展特性边界**：`.py` 下 `?` 是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`lib_compile` 全量守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **分支**：`main` 只在阶段完成时以 `--no-ff` 收录；**不 reset／不重写历史／不 force-push**（`agents-rules/branch-rule.md`）。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
- **编号**：新增 `P*-`／`T-*` 编号前先查是否**已定义**（`CX-1` 抓"无定义处的引用"；墓碑算定义，`CX-19`）。
  现最大文档号：`BC-65`／`TS-50`／`T-BC-22`／`CP-37`／`CX-22`／`P3-31`。

## 纪律提醒

- 红灯不提交 ✓；判据① **5 轮不动**即报停滞 ✓；同一处 bug **3 轮上限**停手升级 ✓；
- 长复盘进台账（`docs/rounds/`），本页只留「HEAD／工作区／下一条命令／未修 bug＋判据」。
