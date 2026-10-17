# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 06（Lead＋队员团队模式）已收 **round 4**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 4」节）。

## 现在

- **HEAD**：`7db1b4e`（`dev`，**已推送** origin ✓）—— 本笔＝round 4：M4-A 批第 1 轮（TS-46 ＋ TS-47 运行期地基）；提交后 HEAD 见 `git log -1`。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 434 ＋ 参照口径 17 ＝ 451 ÷ 628 ⇒ 71.8%**（阈值 421/67% ✓；连测两次同值 ⇒ 区间 450–452；仪器退出码 0 ✓）。
  本轮不涉及 import 面 ⇒ 与 M3 收尾持平；`Lib/` 523 文件 ⇒ 435 能 import（83.2%）。
- **M4 判据进展**：
  - 判据①（`T-BC-8`／`T-BC-9` 可观测）＝**未开始**：本轮只到 TS-47 的**运行期地基**
    （`CHECK_ASSIGNMENT`＋`TypeAssignmentError` 已进执行器；**编译器尚未发射**）。
  - 判据②（渐进保证）＝**未开始**：随 TS-47 静态检查器一起做。
  - A 批顺序：**TS-46 语法面 ✓（round 4）** → **TS-47 运行期发射（下一轮）**
    → TS-47 静态检查＋TS-48 → TS-50 收束。
- **round 4 落地**（详见台账本卷「round 4」节）：
  - **TS-46 `T?`**：`?` 只在扩展模式成词素（`.py` 仍词法期 `Syntax`）；形参／返回值／变量／类属性四个
    标注位都收；`T?` 与手写 `T | None` 产物等价（9 用例）。**留缺**：变量标注里嵌套 `?`（如 `x: list[int?]`）仍报 Syntax。
  - **TS-47 运行期地基**：新异常 `TypeAssignmentError`（`TypeError` 子类）＋新指令 `CHECK_ASSIGNMENT`
    （**`BC-65`**、编号 **231**、`INSTRUCTION_SET_VERSION` **1→2**）、执行器语义＋7 用例；编译器未发射。
  - **socket 侦察**（P3-29）：`net` 域**已在规格/ABI**，缺实现三件套；首片建议 `socketpair`；
    **待用户拍板**（A1 `socketpair` 新槽要领 CP 编号／A2 `getaddrinfo`／A3 地址线格式／A4 fd 语义）。
    P3-29 **不抬判据①**。

## 下一轮开工顺序（round 5，M4-A 批第 2 轮）

1. **TS-47 运行期发射（`compile/` 写手）**：变量／类属性标注的**编译期类型标签**（扩展模式；`T?` 与
   手写联合统一成同一份标签）＋ emitter 在扩展模式对已标注变量的赋值发 `CHECK_ASSIGNMENT`
   （初始标注赋值＋后续赋值；浅层裸标签／深层带内层 `TS-31`）。验收：扩展模式端到端「赋值不符 ⇒ 抛
   `TypeAssignmentError`」＋ `.py` 下同源码**合法且不产生任何检查**（`TS-5`／`BC-25`②）。
2. **union 标签（`executor/` 写手）**：`T?` 的运行期检查语义（值 `None` 通过、否则按 `T`，`TS-46`）
   —— 标签形态扩展（如 `("Union", …)`）＋回归测试；顺带挑一条挂账 bug（executor/ 子目录内）。
3. **并行线**：socket 等用户拍板（A1–A4）；未拍板则先做 `sys.audit` no-op（`http.client` 建连硬前置，
   侦察已钉）或挂账 bug。
4. **挂账 bug 队列（6 条，复现见台账本卷 task-5B 节）**：`global`＋函数内 `import` ⇒ UnboundLocalError
   （`Counter.most_common`）· `warnings.catch_warnings(record=True)` StackUnderflow · `ChainMap.keys()`
   静默空 · `KeyError` 消息少 repr · `functools.wraps` 不搬 `__name__` · PEP 604 union 注解 dataclass 的
   `__builtins__` 面。

## M4 收口判据（唯一出处：`PLAN-milestones.md` §6）

① `T-BC-8`／`T-BC-9` 可观测；② **渐进保证**（对同一语料增删标注，除类型错误外行为不变）。
落点＝扩展特性 A 批（条款在 `SPEC-type-system.md`）：`TS-46` → `TS-47`＋`TS-48` → `TS-50`。
硬约束：扩展特性**只在 `.pyawa` 生效**；`.py` 下必须合法且惰性（`BC-15`／`TS-5`）。
**收口前必须问用户**：是否照先例 `--no-ff` 收录 `main` ＋ 打 tag `m4`。

## 本战役的关键口径（新会话必读）

- **分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再
  `rm -rf Lib/__pyawa__`，然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单之前）；
  单次 ±1 ⇒ 报区间。**并发测量无效**（`Lib/` 被同步进程增删时读数不作数）。
- **指令集**：`INSTRUCTION_SET_VERSION` 现为 **2**（`BC-40`：指令增删必须递增）；`CHECK_ASSIGNMENT`
  编号 **231**（**`BC-65`**）。`crates/pyawa-core/src/opcode_metadata.rs` 由
  `tools/gen_opcode_tables.py` 生成，**禁止手改**。
- **专有异常/标签**：`TypeBoundaryError`／`TypeAssignmentError` 均为 `TypeError` 子类；`CHECK_ASSIGNMENT`
  的 oparg＝常量表下标、常量必须**单标签元组**（标签形态同 `BC-24`：类型对象／`"Any"`／`(外, 内)`）。
- **扩展特性边界**：`.py` 下 `?` 是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）
  ⇒ 验收靠"两种写法互相对拍＋端到端行为"。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`（不夹带别人的暂存）；代码＋台账＋本页 **同笔**提交；
  提交前核对本页 HEAD／工作区两行；闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8；`pyawa-core` 同时至多**两名**写手且**不同子目录**；非 Rust 面（语料／工具）可并行；
  侦察先行（每轮开工先钉最小复现）。
- **编号**：新增 `P*-` 前先重算当前最大号（墓碑算定义，`CX-19`；现最大 **P3-31**（已解除留墓碑））。
  现最大文档号：`BC-65`／`TS-50`／`T-BC-22`／`CP-37`／`CX-22`／`P3-31`。

## 纪律提醒

- 红灯不提交 ✓；判据① **5 轮不动**即报停滞 ✓；同一处 bug **3 轮上限**停手升级 ✓；
- 长复盘进台账（`docs/rounds/`），本页只留「HEAD／工作区／下一条命令／未修 bug＋判据」。
