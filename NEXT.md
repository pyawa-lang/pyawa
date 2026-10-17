# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 06（Lead＋队员团队模式）已收 **round 4–5**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 5」节）。

## 现在

- **HEAD**：`e542d35`（`dev`，**已推送** origin ✓）—— 本笔＝round 5：M4-A 批第 2 轮（TS-47 运行期发射 ＋ union 标签）；提交后 HEAD 见 `git log -1`。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：@@RATIO@@
- **M4 判据进展**：
  - **TS-46 ✓ 语法面**（round 4；round 5 又收口了变量标注里的嵌套 `?`）。
  - **TS-47 运行期半边 ✓**（round 5：扩展模式下已标注变量的赋值发 `CHECK_ASSIGNMENT`；`T?` 的
    "`None` 或成员"在**边界**与**赋值**两条路同时生效）。
  - **TS-47 编译期一半（静态检查器）／`TS-48`／`TS-50`＝未开始**（round 6）。
  - **M4 判据①（`T-BC-8`／`T-BC-9` 可观测）**：可执行证据已存在（`tests/boundary.rs` 的发射条件与
    四要素消息、`tests/assignment_check.rs` 的 `CHECK_ASSIGNMENT` 元数据与消息），但**未做显式归档**
    → round 6/7 归一张「判据→测试」表并补缺口（`T-BC-8` 的"两侧皆标注处禁止发射"`BC-25`① 跨模块
    静态信息缺口**仍未做**，如实记）。
  - **M4 判据②（渐进保证）**：`T-TS-1` 的可执行验收 round 6 建（`TS-9` 要求两条必达性质各有验收）。
- **round 5 落地**（详见台账本卷「round 5」节）：
  - **TS-47 运行期发射**：`AnnotationSource::{Expression,Type}`（纯模式产物逐字节不变）；扩展模式
    标注位先试类型语法（嵌套 `?` 收口）；emitter 每作用域 `assignment_labels`，`Assign`/`AugAssign`
    名字目标在 `STORE_*` 前发 `CHECK_ASSIGNMENT`；`AnnUnion` ⇒ union 标签元组。验收
    `tests/assignment_emission.rs` 7 用例。
  - **union 标签执行器语义**：`boundary_accepts` 接受 `("Union", 成员…)`（任一成员相容即接受）、
    `render_label` 渲染 `Union[…]`、`BC-24` 补形态定义。验收 `tests/union_labels.rs` 6 用例。
  - **如实边界**：属性／下标目标、链式／解包／`for` 目标不发检查；形参注解不登记（形参体内重赋值
    不报）；模块级标注＋函数内 `global x` 赋值不检查；类体标注会登记（略宽于 TS-47，扩展模式特有）。
- **未修 bug（6 条；本轮进展）**：
  - **`warnings.catch_warnings(record=True)` StackUnderflow —— 已定位（未修）**：根因在 `compile/`，
    `with` 体发 body 时没把栈上 2 格／项计入 `handler_depth` ⇒ body 内嵌套 `try` 的异常表 depth 少 2。
    修法＝`Statement::With` 发 body 前后 `handler_depth += 2*items.len()` save/restore；定位材料
    `target/recon/warnings_recon.md`（不进 git）。**round 6 由 `compile/` 写手落地（含组合回归）**。
  - 其余 5 条仍挂账：`global`＋函数内 `import` ⇒ UnboundLocalError（`Counter.most_common`）·
    `ChainMap.keys()` 静默空 · `KeyError` 消息少 repr · `functools.wraps` 不搬 `__name__` ·
    PEP 604 union 注解 dataclass 的 `__builtins__` 面。

## 下一轮开工顺序（round 6，M4-A 批第 3 轮）

1. **`compile/` 写手**：① 先落挂账 bug 的 3~5 行修法＋回归（`emitter.rs` 的 `with` depth；含
   with＋`return`／`break`／`for`＋嵌套 `try` 组合）；② 再做 **TS-47 编译期静态检查器**（新
   `crates/pyawa-core/src/compile/typecheck.rs`：能静态判定的不符在编译期报出；平台类型（未标注）与
   一切相容、禁止推断（`TS-48`））＋ **`TS-50` 收束**（`if x is None: return…` 之后／`if x is not None:`
   分支内；只对局部名与形参；`assert` 不作依据）；③ `CompileError` 新变体＋`pyawa-abi`／
   `executor/import.rs` 的映射（导入含静态类型错的 `.pyawa` ⇒ `TypeAssignmentError`）。
2. **验收测试写手（只写新测试文件）**：`T-TS-1` **渐进保证**（`TS-7`／`TS-9`）＋`T-TS-3`／`T-TS-7`，
   成对"注解版／去注解版"语料端到端比较——M4 判据② 的直接证据。静态检查器同轮落地 ⇒ 测试要允许
   "注解版编译期 `TypeAssignment` 失败"＝类型错误（不算行为变化）。
3. **并行线**：socket（P3-29）**等用户对侦察 A1–A4 拍板**；未拍板不启。

## M4 收口判据（唯一出处：`PLAN-milestones.md` §6）

① `T-BC-8`／`T-BC-9` 可观测（检查点可见、归责信息完整）；② **渐进保证**（对同一语料增删标注，
除类型错误外行为不变）。
落点＝扩展特性 A 批（条款在 `SPEC-type-system.md`）：`TS-46` → `TS-47`＋`TS-48` → `TS-50`。
硬约束：扩展特性**只在 `.pyawa` 生效**；`.py` 下必须合法且惰性（`BC-15`／`TS-5`）。
**收口前必须问用户**：是否照先例 `--no-ff` 收录 `main` ＋ 打 tag `m4`。

## 本战役的关键口径（新会话必读）

- **分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再
  `rm -rf Lib/__pyawa__`，然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；
  并发测量无效。
- **指令集**：`INSTRUCTION_SET_VERSION` 现为 **2**（`BC-40`：指令增删必须递增）；`CHECK_ASSIGNMENT`
  编号 **231**（**`BC-65`**）。`crates/pyawa-core/src/opcode_metadata.rs` 由
  `tools/gen_opcode_tables.py` 生成，**禁止手改**。
- **标签与异常**：`TypeBoundaryError`／`TypeAssignmentError` 均为 `TypeError` 子类；标签形态唯一
  定义在 **`BC-24`**（类型对象／`"Any"`／`(外, 内)`／**union `("Union", 成员…)`＝任一成员相容**）；
  `CHECK_ASSIGNMENT` 的 oparg＝常量表下标、常量必须**单标签元组**。
- **扩展特性边界**：`.py` 下 `?` 是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）
  ⇒ 验收靠"两种写法互相对拍＋端到端行为"；`.py` 产物**逐字节不变**（`TS-5`）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`（不夹带别人的暂存）；代码＋台账＋本页 **同笔**提交；
  提交前核对本页 HEAD／工作区两行；闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8；`pyawa-core` 同时至多**两名**写手且**不同子目录**；非 Rust 面／测试-only
  写手可并行；侦察先行（把缺口钉到最小复现再派）。
- **编号**：新增 `P*-` 前先重算当前最大号（墓碑算定义，`CX-19`；现最大 **P3-31**（已解除留墓碑））。
  现最大文档号：`BC-65`／`TS-50`／`T-BC-22`／`CP-37`／`CX-22`／`P3-31`。

## 纪律提醒

- 红灯不提交 ✓；判据① **5 轮不动**即报停滞 ✓；同一处 bug **3 轮上限**停手升级 ✓；
- 长复盘进台账（`docs/rounds/`），本页只留「HEAD／工作区／下一条命令／未修 bug＋判据」。
