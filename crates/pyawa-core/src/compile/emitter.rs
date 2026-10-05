//! `compile` 的子模块（拆分自单文件时期，见 `AGENTS.md`）。

use super::*;

/// `with` 清理块的发射计划。
///
/// 干吗要单独一个结构体：参照把清理块（**冷块**）放在**作用域正常路径之后**（实测嵌套 `with`
/// 的顺序是「退出内层; 退出外层; 收尾; 内层清理; 外层清理; …」）⇒ 发射时机与"攒计划"的时机
/// 不是同一处。先纯重构抽出这半（行为不变），下一步再把发射时机改对。
#[derive(Clone)]
pub(super) struct WithCleanupPlan {
    /// 每一项的上下文跨度（清理块按**逆序**发）。
    pub(super) context_spans: Vec<Span>,
    /// 每一项受保护区间的起点（异常表要）。
    pub(super) region_starts: Vec<usize>,
    /// 受保护区间的终点。
    pub(super) region_end: usize,
    /// 每一层退出调用的标签（`index > 0` 时清理块跳过三连、`JUMP_BACKWARD_NO_INTERRUPT` 跳回它）。
    pub(super) exit_labels: Vec<usize>,
    /// 清理块出口要**重放**的余部（`emit_rest_and_tail`）。
    pub(super) rest: Vec<Statement>,
    /// 整条 `with` 语句的跨度。
    pub(super) span: Span,
}

pub(super) struct Emitter {
    pub(super) unit: CompiledUnit,
    pub(super) kind: ScopeKind,
    /// 编译输入（`BC-14`／`TS-31`）：检查指令**只**在扩展模式 ＋ 深层档位下发射（`BC-25`②）。
    pub(super) mode: Mode,
    pub(super) tier: CheckTier,
    /// 当前作用域的 `co_qualname`（`BC-4`）：嵌套 `def` 要用它算下一层的名字。
    pub(super) qualname: String,
    /// 返回值的边界检查标签下标（`BC-23` 的 `CHECK_BOUNDARY_OUT`；`None` ⇒ 不发）。
    pub(super) boundary_out: Option<usize>,
    /// **延迟入池**的常量：`(LOAD_CONST 的实参字节偏移, 常量)`。
    ///
    /// 用途：字面量默认值折叠出来的元组，在参照实现里**排在常量表最后**（在模块收尾的 `None`
    /// 之后）——所以要等收尾时再入池，再把下标回填到那条 `LOAD_CONST` 的实参字节上。
    pub(super) deferred: Vec<(usize, Constant)>,
    /// 最后一条真指令的位置（隐式 return 用它）。
    pub(super) last_span: Span,
    /// 模块收尾两条指令的位置。实测：`+` 形态跟**右值**走，比较／字面量／名字跟**目标**走
    /// （与 `STORE_NAME` 的形态规则只差比较那一格）。
    pub(super) epilogue_span: Span,
    /// **`global` 声明的名字**（第 114 轮）：本作用域里它们不进 `varnames` ✓，
    /// 存储走 `STORE_GLOBAL` ✓（实测模块层与函数层都是 ✓）。
    pub(super) global_names: Vec<String>,
    /// **折叠出来的常量**：登记时机在收尾之后，先记下"要回填的 `LOAD_CONST` 实参位置"。
    pub(super) pending: Vec<(usize, Constant)>,
    /// 跳转回填：`(要回填的实参字节位置, 标签号, 该指令占用的码元数)`。
    /// `BC-55`：目标码元 = 当前码元 + 指令占用码元数 + 有符号 oparg ⇒ 回填时反过来算。
    pub(super) jumps: Vec<(usize, usize, usize)>,
    /// 标签 ⇒ 码元位置。
    pub(super) labels: Vec<Option<usize>>,
    /// 模块收尾还需不需要补 `LOAD_CONST None; RETURN_VALUE`。
    /// 实测：末尾的 `if/else` 两个分支都 `return` ⇒ **没有**可落到末尾的路径 ⇒ 参照不再补。
    pub(super) epilogue_needed: bool,
    /// 瞬时标志：正在编译**条件**（`if`／`while` 的）⇒ 比较要带 `bool(...)` 位
    /// （实测：`while a < b` 的 `COMPARE_OP` oparg 是 18 ＝ 2 | 16，而赋值里的比较是 2）。
    pub(super) in_condition: bool,
    /// 瞬时标志：当前这条语句是**作用域最后一条 `if`** ⇒ 它的每个分支末尾要补一条
    /// `LOAD_CONST None; RETURN_VALUE`（实测；只有模块末尾的 `if` 会这样）。
    /// `elif` 链的嵌套层：为真时**不**补自己的"末尾隐式 return"（由最外层补一次）。
    pub(super) suppress_chain_tail: bool,
    /// 当前嵌套的循环（`break`／`continue` 的落点 ＋ `break` 路径要重放的**余部**）。
    pub(super) loops: Vec<LoopFrame>,
    /// 各层语句块的"块尾"标签（退出路径重放余部后不终止时跳到它）。
    pub(super) block_end_labels: Vec<usize>,
    /// **待加宽的跳转**（第 121 轮）：`(opcode 所在码元, 完整实参)` —— 实参 > 255 时
    /// 收尾要在它**前面插入一个 `EXTENDED_ARG` 词**（CPython 的做法 ✓）。
    pub(super) wide_jumps: Vec<(usize, u16)>,
    /// **`BC-54`** 的异常表条目（字节偏移；收尾时按 6-bit varint 编码进 `exceptiontable`）。
    pub(super) exception_entries: Vec<(usize, usize, usize, usize, bool)>,
    /// **正在发射的推导式**的目标名（只在推导式内部当局部；模块级同名变量照旧走全局：
    /// 实测参照里 `for v in …`／`v = 99` 是 `STORE_NAME`／`LOAD_NAME`，而推导式内部是快速槽）。
    pub(super) comprehension_locals: Vec<String>,
    /// 待**外提**的推导式清理块（实测：清理块排在所在**语句块末尾**、连收尾之后）。
    pub(super) pending_cleanups: Vec<PendingCleanup>,
    /// 最近一条 `STORE_FAST_LOAD_FAST` 已经把哪个槽的值压回了栈顶（`None` 表示没有）：
    /// **紧接着的那一次**对该槽的读取不再单独发 `LOAD_FAST_BORROW`（实测的融合选择）。
    pub(super) pending_fused_load: Option<usize>,
    /// 处理块段的字节区间 ＋ 有没有 `as 名字`（目标＝清理块／名字清理，收尾时补）。
    pub(super) handler_segments: Vec<(usize, usize, bool)>,
    /// 最近一条 `if`／`elif` 子句的**条件尾**位点（`elif` 链的尾巴用它，实测参照如此）。
    pub(super) clause_condition_tail: Span,
    /// 最近一条 `if`／`elif` 子句**有没有 `else` 体**（链尾覆盖只在"最末子句无 `else`"时生效）。
    pub(super) clause_had_else: bool,
    /// **`and`／`or` 骨架指令**（`COPY`／`TO_BOOL`／跳转／`NOT_TAKEN`／`POP_TOP`）用的跨度：
    /// 参照给**整个布尔表达式**的跨度（实测 `return a and b` 的骨架是 `(2,2,11,18)`），
    /// 而操作数自己的 `LOAD` 仍取各自的跨度。
    pub(super) boolop_scaffold_span: Option<Span>,
    pub(super) if_implicit_return: bool,
    /// 紧随其后的那一次 `emit_block` 是不是**循环体**（只吃一次）。
    pub(super) in_loop_body: bool,
    /// 当前这条语句是不是**循环体的最后一条 `if`**（无 `else`）——窥孔用。
    pub(super) loop_last_if: bool,
    /// 当前正处在**需要收尾机制的块体**里（`with` 体、带非空 `finally` 的 `try` 体）——
    /// 实测：这种体里的 `return <字面量>`，其常量被**延迟**到常量表最后（小整数因此**不入池**）。
    pub(super) in_epilogue_body: bool,
    /// **一次性**标志：下一次字面量（`return` 的值）要不要走"延迟"那一支。
    pub(super) defer_return_literal: bool,
    /// **`with` 体内**的 `RETURN_VALUE` 取哪段跨度（实测：最外层 `with` 的**第一项上下文**；
    /// 嵌套时外层不被内层覆盖——退出调用是**逆序**发的，最后发的是第一项）。
    pub(super) with_return_span: Option<Span>,
    /// 当前正处于其**体**内的各层 `with`（每层记各 item 的上下文跨度）。`return` 要**逐层**跑退出调用
    /// （内层先、每层内再按 item 逆序）——实测 `with cm as y: with y: return 1` 的退出次序就是如此。
    pub(super) with_exit_stack: Vec<Vec<Span>>,
    /// **`with` 体内 `return` 的退出调用"起点"**（第 166 轮）：参照把该 `with` 的**受保护区**止于
    /// **正常流末尾**（＝退出调用之前 ✓），本层先前把整段 body（含退出调用尾声）都圈进去 ✗。
    pub(super) with_body_end: Option<usize>,
    /// **`finally` 栈**（第 270 轮）：`try/finally` 体内每个**出口**（`return`）都要先把 finally
    /// 跑一遍。参照的实测形状（`def f(x):\n    try:\n        return 1\n    finally:\n        y = 2\n`）：
    /// `NOP; NOP; <finally 体>; LOAD_SMALL_INT 1; RETURN_VALUE` ⇒ **finally 在 return 之前**，
    /// 本层此前发在之后（⇒ 正常 `return` 根本跑不到 finally，是**语义 bug**）。
    ///
    /// 只覆盖 `return`（`break`/`continue` 的出口、以及处理块里的出口留待后续）；
    /// 与 `with` 同时存在时统一按"先 `with` 退出、再 finally"发（`finally` 在外的嵌套是对的，
    /// `finally` 在内的嵌套顺序还不对）。
    pub(super) finally_stack: Vec<Vec<Statement>>,
    /// **处理器栈**（第 203 轮）：每进一层 `except` 处理块压一项（**该处理器的名字** ✓，可为 `None` ✓）。
    ///
    /// **为什么必需** ✗：处理器里 `return` 时，异常对象还在栈上 ✓ —— 参照的序列是
    /// `[值] → SWAP 2 → POP_EXCEPT → 名字清理 → RETURN_VALUE` ✓（值是**字面量**时反过来：
    /// `POP_EXCEPT → 名字清理 → 值 → RETURN_VALUE` ✓）。我们先前**什么都不发** ✗ ⇒
    /// `RETURN_VALUE` 抓错栈槽 ⇒ **取回错值** ✗（实测 `except … as exc: return str(exc)` ✓）。
    pub(super) handler_stack: Vec<Option<String>>,
    /// **处理器嵌套层数** ✓（第 217 轮真 bug 修复 ✗）：每进一层 `except` 处理器**体**就 +1 ✓。
    ///
    /// 为什么必须有它 ✗：处理块入口靠 `PUSH_EXC_INFO` 在栈上**多留一格**（"上一个异常" ✓）⇒
    /// 处理器体内发起的 `try`，其异常表条目的 `depth` **不是 0** ✗ 而是这一层数 ✓
    /// （实测：嵌套那层实际栈深 **1** ✗、而先前记的 `depth` 是 **0** ✗ ⇒ 展开时多弹一格 ⇒
    /// 后面 `POP_EXCEPT` 取空栈 ⇒ `StackUnderflow` ✓）。
    /// **块深度** ✓（第 202 轮真 bug 修复 ✗）：`compile_scope` 也是经 `emit_block` 发体的 ✓
    /// ⇒ **深度 1 就是"作用域自己的语句体"** ✓、≥ 2 才是**嵌套块** ✓。**嵌套块**里
    /// `try`／`with` 的**正常路径**不该发"**作用域收尾**" ✗（那会让外层以为已经收尾 ⇒ 后续语句
    /// **整段消失** ✗：`if 1:` 里一个 `try/except` ⇒ 后面的 `print` 没了 ✓、`Lib/os.py` 只剩半截 ✓）。
    /// **退出重放**（`break`／`continue`／异常路径 ✓）**照旧**要用收尾 ✓ ⇒ 所以**只**在
    /// `Try`／`With` 的正常路径上读这一位 ✓（`emit_scope_tail` 本身**不动** ✓）。
    pub(super) block_depth: usize,
    /// **当前块是不是"尾块"**：从这个块**落下去**是不是就走到作用域末尾。
    ///
    /// 它是"末尾那条 `if` 要补隐式 `return`"的**前提**（第 279 轮真 bug 修 ✗）：
    /// 先前只看"块内最后一条 `if`"✗ ⇒ 嵌套 `if` 也会补 ✗ —— 而它后面还有代码时，
    /// 那份 `LOAD_CONST None; RETURN_VALUE` 会**提前返回** ✗
    /// （实测：`Lib/importlib/_bootstrap.py` 的 `_spec_from_module` 因此返回 `None` ✗，
    ///  挡住 M3 的 import 链 ✓）。作用域自己的体 ＝ 尾块 ✓；`if` 的分支仅当
    /// "本条 `if` 是该块最后一条"时才是尾块 ✓（参照实测：`def f(x):\n    if x:\n        if x:\n            r = 1\n`
    /// 三层都补 ✓，而后面还有 `return` 时**一层都不补** ✓）。
    pub(super) block_tail: bool,
    pub(super) handler_depth: usize,
    /// **条件假出口的落点**（`if` 条件发射时收集，`if` 臂消费）。
    pub(super) condition_landings: Vec<usize>,
    /// 要不要给每个条件出口建**独立落点**：只有"块内最后一条 `if`"才要（带尾随代码时共享块尾 ✓）。
    pub(super) collect_condition_exits: bool,
    /// **待发的条件出口副本**：`(落点标签, 余部, 语句跨度)`；在作用域收尾之后冲刷（三个分支各一次）。
    pub(super) pending_condition_copies: Vec<(usize, Vec<Statement>, Span)>,
    /// **链式比较失败路径的"续部副本"**：`(落点标签, 目标名, 目标跨度, 余部, 语句跨度)`。
    /// 参照把失败块**外提**到语句之后，块里是 `SWAP 2; POP_TOP` ＋ **一份续部**（存入 ＋ 余部 ＋ 收尾）。
    pub(super) pending_chain_copies: Vec<(usize, String, Span, Vec<Statement>, Span)>,
    /// 赋值臂"接管"链式失败落点的凭据：`(标签, 余部, 目标名, 目标跨度)`；链式发射器取走后按它落点。
    pub(super) chain_takeover: Option<(usize, Vec<Statement>, String, Span)>,
    /// **抑制"链式比较接管"**：两种场合必须置真 ——
    /// ① `annotate_unit`（注解单元那一遍不走作用域收尾的落点冲刷 ✗）；
    /// ② **副本重放**（`flush_condition_copies` 里 `emit_block(rest)` 会把余部再发一遍，
    ///    那一遍若又接管，就会往**已经冲刷过**的队列里再排副本 ⇒ 悬空标签 ✗
    ///    —— 第 88 轮实测：`chained_compare` 的余部重放导致了 5 级级联 ✓）。
    pub(super) suppress_chain_takeover: bool,
}

impl Emitter {
    /// 发射一条指令并记下它的位置（`BC-18`）。
    /// 新开一个标签；返回它的编号。
    /// 发一个**条件跳转**（落在新标签上；调用方拿标签去 `mark_label`）。
    pub(super) fn emit_condition_jump(
        &mut self,
        condition: &Expression,
        jump_if_true: bool,
    ) -> Result<usize, CompileError> {
        let target = self.new_label();
        self.emit_condition_jump_to(condition, jump_if_true, target)?;
        Ok(target)
    }

    /// 发一个**条件跳转**到既有标签：正常"条件为假就跳"，`jump_if_true` 为真时反过来。
    ///
    /// **`not` 是推进跳转的**（实测 `if not a:` ⇒ `LOAD a; TO_BOOL; POP_JUMP_IF_TRUE`，
    /// **没有** `UNARY_NOT`）——同一棵树在"值上下文"（`x = not a`）与"条件上下文"两种发射形态，
    /// 这是第一处**按上下文改发射**的地方。
    pub(super) fn emit_condition_jump_to(
        &mut self,
        condition: &Expression,
        jump_if_true: bool,
        target: usize,
    ) -> Result<(), CompileError> {
        if let Expression::Not(operand, _) = condition {
            return self.emit_condition_jump_to(operand, !jump_if_true, target);
        }
        // **链式比较当条件**（实测）：每段 `COMPARE_OP |16` ＋ `POP_JUMP_IF_FALSE → target` ＋
        // `NOT_TAKEN`；末段之后 `JUMP_FORWARD` 跳过一条 `POP_TOP`（那条是**死代码**，但参照照发）。
        // 目前只接线 `jump_if_true == false`（`if`／`while` 的常见极性问题），真极性留给下一轮。
        if let Expression::ChainedCompare {
            operands,
            operators,
            span,
        } = condition
        {
            // **只要链里出现"没有 `COMPARE_OP` oparg"的运算符（`is`／`is not`／`in`／`not in`）就
            // 不走这条特化**（第 327 轮）：`a == b is c` 这类**链式比较当条件**时，先前的
            // `.expect("`is`／`in` 一族不走这里")` 直接 **panic** ✗（实测 `if a == b is c:` ✓）——
            // 而这正是上限诊断里 `<无 errmsg>：状态 1` × 29 那一族被折出来的真身 ✓
            // （`-11` 与它同源 ✓）。跳过去 ⇒ 落到下面那条**通用**链式比较路径 ✓，语义不变 ✓。
            let specialized = operators.iter().all(|operator| operator.oparg().is_some());
            // **再要求"这条 if 就是块的收尾"**（第 328 轮）：这条特化会给假出口发一份**收尾副本**
            // （`POP_TOP; LOAD_CONST None; RETURN_VALUE` ✓）—— 只有"if 之后没有别的语句"时才成立 ✓。
            // 带 else（或后面还有语句）时假出口必须落到 **else 体／后继语句** ✗，先前照样走特化 ⇒
            // 假出口直接跳到收尾副本 ⇒ 后面的语句被整段吞掉 ✓（实测：`if a < b <= c: ... else: ...`
            // 之后再 `print` ⇒ 什么都不打印、退出码 0 ✓）。`collect_condition_exits` 正是
            // "本 if 处于尾位且没有 else"这个标志 ✓（由 if 臂按 block_tail／rest／else_body 算好 ✓）。
            if !jump_if_true && specialized && self.collect_condition_exits {
                self.in_condition = true; // `COMPARE_OP` 的 `|16` 由这里决定
                let result = (|| -> Result<(), CompileError> {
                    self.emit_expression(&operands[0])?;
                    // **非末链的假出口**落在**就地的一份收尾副本**上（实测 `if a < b < c:` ⇒
                    // `POP_JUMP_IF_FALSE → 34`（`POP_TOP; LOAD_CONST None; RETURN_VALUE`）✓，
                    // 里层那条 `JUMP_FORWARD` 再跳过这份副本进体 ✓）。只有**量过的两链**形态
                    // 这样发；更长的链仍走"所有出口都由收尾副本机制兜"的旧形态 ✓（未量 ✗）。
                    let dead = (operators.len() == 2).then(|| self.new_label());
                    for (index, operator) in operators.iter().enumerate() {
                        self.emit_expression(&operands[index + 1])?;
                        if index + 1 != operators.len() {
                            self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                            self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 2);
                        }
                        let base = operator.oparg().expect("`is`／`in` 一族不走这里");
                        self.emit_at(
                            *span,
                            opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
                            base | 16,
                        );
                        let landing = match (&dead, index + 1 == operators.len()) {
                            (Some(label), false) => *label,
                            _ => target,
                        };
                        self.emit_jump(
                            *span,
                            opcode::opcode("POP_JUMP_IF_FALSE").expect("条件跳转在表里"),
                            landing,
                        );
                        self.emit_at(
                            *span,
                            opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                            0,
                        );
                    }
                    let after = self.new_label();
                    self.emit_jump(
                        *span,
                        opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                        after,
                    );
                    if let Some(label) = dead {
                        self.mark_label(label);
                    }
                    self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                    if dead.is_some() {
                        // 这份副本的位点取**链式表达式**的跨度（实测三条同为 `(1,1,3,12)` ✓）。
                        // `None` 走**延迟入池**（先占位、收尾时并入表尾）——直接 `intern_constant`
                        // 会把它排到别的常量前面 ⇒ 常量表顺序与参照不符 ✗（实测差的就是这里 ✓）
                        match self.unit.constants.iter().position(|item| *item == Constant::None) {
                            Some(index) => {
                                self.emit_indexed(*span, "LOAD_CONST", index);
                            }
                            None => {
                                let argument_byte = self.unit.code.len() + 1;
                                self.emit_at(
                                    *span,
                                    opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                    0,
                                );
                                self.pending.push((argument_byte, Constant::None));
                            }
                        }
                        self.emit_at(
                            *span,
                            opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                            0,
                        );
                    }
                    self.mark_label(after);
                    Ok(())
                })();
                self.in_condition = false;
                return result;
            }
        }
        // **裸的 `and`／`or` 条件**（实测四种形态，规则如下）：
        //   `cond` ＝「真值等于 cond 时跳到 `target`（`if`／`while` 里就是跳过体）」
        //   · 非末操作数：按**自身极性**跳（`and` ⇒ 为假跳、`or` ⇒ 为真跳）；
        //     跳哪里取决于"这个操作数的决定值是否就是 cond"：是 ⇒ `target`，否 ⇒ `other`
        //     （`other` ＝ 条件码之后那一格，`if` 里就是**体入口**）。
        //   实测：`if a and b:` 两个都跳 `target`（跳过体）；`if a or b:` 首个跳**体入口**、末个跳 `target`；
        //   `if not (a and b):` 首个跳体入口、末个跳 `target`（极性随 `not` 翻转）。
        if let Expression::BoolOp {
            conjunction,
            values,
            ..
        } = condition
        {
            if values.len() == 1 {
                return self.emit_condition_jump_to(&values[0], jump_if_true, target);
            }
            let other = self.new_label();
            // 非末操作数的决定值：`and` 是"假"、`or` 是"真"；与 cond 一致 ⇒ 直接跳 target
            let to_target = (*conjunction && !jump_if_true) || (!*conjunction && jump_if_true);
            for value in &values[..values.len() - 1] {
                // **每个走向条件出口的跳转各带一份收尾副本**（第 288/289 轮实测）：
                // 只有在"块内最后一条 `if`"时才给每个非最末操作数**自己的落点**并登记；
                // 否则出口共享块尾（`other`／`target`）。
                let landing = if to_target && self.collect_condition_exits {
                    let landing = self.new_label();
                    self.condition_landings.push(landing);
                    landing
                } else {
                    if to_target {
                        target
                    } else {
                        other
                    }
                };
                self.emit_test_bare(value, !*conjunction, landing, None)?;
            }
            let last = values.last().expect("`and`／`or` 至少一个操作数");
            self.emit_test_bare(last, jump_if_true, target, None)?;
            self.mark_label(other);
            return Ok(());
        }
        let condition_span = condition.span();
        self.in_condition = true;
        self.emit_expression(condition)?;
        self.in_condition = false;
        // 实测：条件是**比较**时**不再**补 `TO_BOOL`（比较自带的 `bool(...)` 位已经交出布尔了）；
        // 条件不是比较（如裸名字）才补（`TO_BOOL` 3 个缓存槽 ⇒ 跳转 1 个缓存槽 ⇒ `NOT_TAKEN`）
        if !matches!(
            condition,
            Expression::Compare(_, _, _, _) | Expression::ChainedCompare { .. }
        ) {
            self.emit_at(
                condition_span,
                opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"),
                0,
            );
        }
        let name = if jump_if_true {
            "POP_JUMP_IF_TRUE"
        } else {
            "POP_JUMP_IF_FALSE"
        };
        self.emit_jump(condition_span, opcode::opcode(name).expect("条件跳转在表里"), target);
        self.emit_at(
            condition_span,
            opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
            0,
        );
        Ok(())
    }

    pub(super) fn new_label(&mut self) -> usize {
        self.labels.push(None);
        self.labels.len() - 1
    }

    /// 按**名字**发一条指令（名字一定在表里；内部用）。
    pub(super) fn emit_named(&mut self, position: Span, name: &str, oparg: u8) {
        self.emit_at(
            position,
            opcode::opcode(name).expect("指令在表里"),
            oparg,
        );
    }

    /// **存一个名字的统一入口**（第 114 轮）：`global` 声明的 ⇒ `STORE_GLOBAL` ✓；
    /// 函数局部 ⇒ `STORE_FAST` ✓；其余 ⇒ `STORE_NAME` ✓（实测：`global a, b` 后 `a = 1`
    /// 在模块层与函数层都走 `STORE_GLOBAL` ✓）。
    pub(super) fn emit_store_name(&mut self, span: Span, name: &str) {
        if self.global_names.iter().any(|item| item == name) {
            let index = self.intern_name(name);
            self.emit_indexed(span, "STORE_GLOBAL", index);
        } else if let Some(slot) = self.deref_slot(name) {
            // **cell／自由变量要走 `STORE_DEREF`**（第 343 轮真 bug 修 ✗）：先前这一支**漏了**
            // ✗ ⇒ 名字是 cell（被内层函数闭包捕获 ✓）时落到最后的 `STORE_NAME` ✗ ⇒ 在**函数**帧里
            // 撞执行器的"`STORE_NAME` 需要命名空间帧（模块／类体）" ✗。
            // 实测原形 ✓：`Lib/collections/__init__.py` 的 `namedtuple` 里
            //   `_dict, _tuple, _len, _map, _zip = dict, tuple, len, map, zip`（第 437 行 ✓）——
            // 这五个名字都被它**内层那几个方法**捕获 ✓ ⇒ 是 cell ✓ ⇒ 解包赋值直接中止 ✗
            // （上限榜上 78 个模块压在它上面 ✓）。把名字换掉（不再是 cell ✓）就正常 ✓，实测过 ✓。
            self.emit_named(span, "STORE_DEREF", slot as u8);
        } else if self.kind == ScopeKind::Function
            && self.unit.varnames.iter().any(|item| item == name)
        {
            let slot = self.slot_of(name);
            self.emit_named(span, "STORE_FAST", slot as u8);
        } else {
            let index = self.intern_name(name);
            self.emit_indexed(span, "STORE_NAME", index);
        }
    }

    /// **删一个名字的统一入口**（第 202 轮）：口径与 [`Self::emit_store_name`] **对称** ✓ ——
    /// `global` 声明的 ⇒ `DELETE_GLOBAL` ✓；函数局部 ⇒ `DELETE_FAST <槽>` ✓；其余 ⇒ `DELETE_NAME` ✓。
    ///
    /// **为什么必需** ✗：`except … as 名字` 的**两处**（绑定那次 ✓ 与收尾那次 ✓）先前**写死**了
    /// `STORE_NAME`／`DELETE_NAME` ✗ ⇒ 一旦出现在**函数**里 ⇒ 撞执行器的
    /// "`STORE_NAME` 需要命名空间帧（模块／类体）" ✗（实测：函数内 `try/except … as` 直接中止 ✓）。
    pub(super) fn emit_delete_name(&mut self, span: Span, name: &str) {
        if self.global_names.iter().any(|item| item == name) {
            let index = self.intern_name(name);
            self.emit_indexed(span, "DELETE_GLOBAL", index);
        } else if self.kind == ScopeKind::Function
            && self.unit.varnames.iter().any(|item| item == name)
        {
            let slot = self.slot_of(name);
            self.emit_named(span, "DELETE_FAST", slot as u8);
        } else {
            let index = self.intern_name(name);
            self.emit_indexed(span, "DELETE_NAME", index);
        }
    }

    /// 发一条**行号有、列全空**的指令（第 124 轮）：生成器的 `RETURN_GENERATOR`／`POP_TOP`
    /// 实测位点是 `(def 行, def 行, None, None)` ✓，`emit_named` 表达不了"列空" ⇒ 单列一条 ✓。
    pub(super) fn emit_line_only(&mut self, line: u32, name: &str, oparg: u8) {
        self.unit
            .positions
            .push((Some(line), Some(line), None, None));
        let opcode = opcode::opcode(name).expect("指令在表里");
        self.unit.code.push(opcode as u8);
        self.unit.code.push(oparg);
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.unit.code.push(0);
            self.unit.code.push(0);
        }
    }

    /// **带下标的指令**（第 122 轮）：下标 > 255 时先发一条 `EXTENDED_ARG <高位>` ✓，
    /// **位点与随后那条完全相同**（实测 `STORE_NAME 301`／`LOAD_CONST 300` 都是这样 ✓）。
    ///
    /// 只用于"实参就是下标本身"的指令 ✓（`LOAD_GLOBAL`／`LOAD_ATTR` 那种带标志位的另行处理 ✗）。
    pub(super) fn emit_indexed(&mut self, span: Span, name: &str, index: usize) {
        let opcode = opcode::opcode(name).expect("指令在表里");
        if index > 255 {
            self.emit_at(
                span,
                opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里"),
                (index >> 8) as u8,
            );
        }
        self.emit_at(span, opcode, (index & 0xFF) as u8);
    }

    /// 同 [`Self::emit_indexed`]，但记**无位点**（合成指令那条路 ✓）。
    pub(super) fn emit_indexed_none(&mut self, name: &str, index: usize) {
        let opcode = opcode::opcode(name).expect("指令在表里");
        if index > 255 {
            self.emit_none(
                opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里"),
                (index >> 8) as u8,
            );
        }
        self.emit_none(opcode, (index & 0xFF) as u8);
    }

    /// **`with` 的一项退出调用**：三条 `LOAD_CONST None` ＋ `CALL 3` ＋ `POP_TOP`，位点＝该项的
    /// 上下文跨度（正常路径与**体内 `return` 的复制件**共用 ⇒ 一处真相）。
    pub(super) fn emit_with_exit_call(&mut self, span: Span, none_index: usize) {
        for _ in 0..3 {
            self.emit_indexed(span, "LOAD_CONST", none_index);
        }
        self.emit_at(span, opcode::opcode("CALL").expect("CALL 在表里"), 3);
        self.emit_at(span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
    }

    /// 按作用域存一个名字：模块／类体走 `STORE_NAME`，函数里走 `STORE_FAST <槽>`。

    /// 发射 `with` 的**清理块**（冷块）。返回"重放的余部＋收尾是否终止"。
    pub(super) fn emit_with_cleanups(&mut self, plan: &WithCleanupPlan) -> Result<bool, CompileError> {
        let mut terminated = true;
        let count = plan.context_spans.len();
        let mut cleanup_starts: Vec<usize> = vec![0; count];
        let mut cleanup_ends: Vec<usize> = vec![0; count];
        for index in (0..count).rev() {
            let context_span = plan.context_spans[index];
            cleanup_starts[index] = self.unit.code.len();
            self.emit_at(
                context_span,
                opcode::opcode("PUSH_EXC_INFO").expect("PUSH_EXC_INFO 在表里"),
                0,
            );
            self.emit_at(
                context_span,
                opcode::opcode("WITH_EXCEPT_START").expect("WITH_EXCEPT_START 在表里"),
                0,
            );
            self.emit_at(context_span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
            let handled = self.new_label();
            self.emit_jump(
                context_span,
                opcode::opcode("POP_JUMP_IF_TRUE").expect("POP_JUMP_IF_TRUE 在表里"),
                handled,
            );
            self.emit_at(
                context_span,
                opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                0,
            );
            self.emit_at(context_span, opcode::opcode("RERAISE").expect("RERAISE 在表里"), 2);
            self.mark_label(handled);
            self.emit_at(context_span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
            // **handler 区的右端＝`POP_EXCEPT` 之前** ✓（第 170 轮：照参照 dis ✓ —— `with … return y` 的
            //   第二条目是 `(24, 11, 41)` 码元 ✓，右端 35 正是那条 `POP_EXCEPT` ✓；而我们先前记在
            //   `POP_EXCEPT` ＋ 三个 `POP_TOP` **之后** ✗ ⇒ 长了 4 码元 ✗）。
            cleanup_ends[index] = self.unit.code.len();
            self.emit_at(
                context_span,
                opcode::opcode("POP_EXCEPT").expect("POP_EXCEPT 在表里"),
                0,
            );
            for _ in 0..3 {
                self.emit_at(context_span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
            }
            if index > 0 {
                self.emit_directed_jump(
                    context_span,
                    opcode::opcode("JUMP_BACKWARD_NO_INTERRUPT")
                        .expect("JUMP_BACKWARD_NO_INTERRUPT 在表里"),
                    plan.exit_labels[index - 1],
                    true,
                );
            } else {
                terminated &= self.emit_rest_and_tail(&plan.rest, plan.span)?;
            }
            // **每一层清理块后面各跟一份自己的末尾清理**（实测：两层时内层的 `COPY 3;…`
            // 紧跟在 JUMP_BACKWARD_NO_INTERRUPT 之后，然后才是外层的清理块）
            let layer_cleanup = self.unit.code.len();
            self.emit_named_none("COPY", 3);
            self.emit_named_none("POP_EXCEPT", 0);
            self.emit_named_none("RERAISE", 1);
            self.record_exception(
                plan.region_starts[index],
                plan.region_end,
                cleanup_starts[index],
                self.handler_depth + 2 * (index + 1),
                true,
            );
            self.record_exception(
                cleanup_starts[index],
                cleanup_ends[index],
                layer_cleanup,
                2 * (index + 1) + 2,
                true,
            );
        }
        Ok(terminated)
    }

    pub(super) fn store_target(&mut self, span: Span, name: &str) {
        match self.kind {
            ScopeKind::Module | ScopeKind::Class => {
                self.emit_store_name(span, name);
            }
            ScopeKind::Function => {
                // **`global` 声明的名字不能走 `STORE_FAST`** ✓（第 263 轮真 bug ✗）：先前这里**直接**
                // `slot_of` ＋ `STORE_FAST` ✗ ⇒ 它把全局名**追加成本地** ✗ ⇒ ① 布局整体错位（序言的
                // `MAKE_CELL` 槽号随之作废 ✗）；② 语义也错（写局部而没写全局 ✗）。实测：
                // `Lib/posixpath.py` 的 `expandvars` 有 `global _varsub, _varsubb` ✓，正是它坏掉的 ✓。
                if self.global_names.iter().any(|item| item == name) {
                    self.emit_store_name(span, name);
                } else if self.deref_slot(name).is_some() {
                    // **cell／自由变量同理**（第 343 轮）：它也不进 `varnames` ✗ ⇒ 先前这条快路
                    // 会给它发 `STORE_FAST <一个不是它的槽>` ✗ ⇒ 闭包读到的永远是空 cell ✓
                    // （实测：`def outer(): total = 0; def bump(): return total; total = 10` ⇒
                    //  `bump()` 报 `cannot access free variable 'total'` ✗）。交给 `emit_store_name`
                    // 统一走 `STORE_DEREF` ✓。
                    self.emit_store_name(span, name);
                } else {
                    let slot = self.slot_of(name);
                    self.emit_named(span, "STORE_FAST", slot as u8);
                }
            }
        }
    }

    /// 同 `emit_named`，但记**无位点**（`BC-4` 扩：参照给合成指令的是全 `None`）。
    pub(super) fn emit_named_none(&mut self, name: &str, oparg: u8) {
        self.emit_none(opcode::opcode(name).expect("指令在表里"), oparg);
    }

    /// 记下标签落在**当前**码元处。
    pub(super) fn mark_label(&mut self, label: usize) {
        self.labels[label] = Some(self.unit.code.len() / 2);
    }

    /// 发一条**前向跳转**（目标标签先占位、收尾时回填）。
    pub(super) fn emit_jump(&mut self, position: Span, opcode: u16, label: usize) {
        self.emit_directed_jump(position, opcode, label, false);
    }

    /// 发一条跳转；`backward` 为真时 oparg 是**往回**的距离
    /// （实测 `JUMP_BACKWARD` 的 oparg ＝ `当前码元 + 占用码元数 − 目标码元`，方向是 opcode 本身定的）。
    pub(super) fn emit_directed_jump(&mut self, position: Span, opcode: u16, label: usize, backward: bool) {
        let argument_byte = self.unit.code.len() + 1;
        let size = 1 + opcode::inline_cache_entries(opcode) as usize;
        self.emit_at(position, opcode, 0);
        self.jumps.push((argument_byte, label, size | (usize::from(backward) << 16)));
    }

    /// 收尾时把跳转实参回填（`BC-55` 的公式反过来用）。
    pub(super) fn flush_jumps(&mut self) {
        let jumps = core::mem::take(&mut self.jumps);
        for (argument_byte, label, packed) in jumps {
            let size = packed & 0xFFFF;
            let backward = packed >> 16 != 0;
            // **自带诊断**（第 87 轮）：原先只有一句 `标签必须已经落点`，定位它得插桩六次 ✗
            // ⇒ 现在把**标签号**、**跳转指令的码元**、**已落点集合**一起报出来；
            // 成因几乎总是"外提的落点只在一个发射路径上冲刷"（如链式比较/条件副本 ✗
            // 与 `annotate_unit` 那样的第二遍发射器）。
            let target = match self.labels[label] {
                Some(target) => target,
                None => {
                    let marked: Vec<usize> = self
                        .labels
                        .iter()
                        .enumerate()
                        .filter(|(_, value)| value.is_some())
                        .map(|(index, _)| index)
                        .collect();
                    // **自带源码位置**（第 122 轮）：按码元偏移反查指令下标 ⇒ 报出那一条的位点 ✓
                    let wanted = argument_byte / 2;
                    let mut word = 0usize;
                    let mut instruction = 0usize;
                    let position = loop {
                        if word >= wanted || word * 2 + 1 >= self.unit.code.len() {
                            break self.unit.positions.get(instruction).copied();
                        }
                        let opcode = u16::from(self.unit.code[word * 2]);
                        let size = 1 + opcode::inline_cache_entries(opcode) as usize;
                        if word + size > wanted {
                            break self.unit.positions.get(instruction).copied();
                        }
                        word += size;
                        instruction += 1;
                    };
                    panic!(
                        "跳转目标标签 {label} 从未落点（跳转指令在码元 {}，位点 {position:?}；已落点：{marked:?}）",
                        wanted
                    )
                }
            };
            let here = argument_byte / 2; // 该指令的 opcode 所在码元
            let argument = if backward {
                (here + size) as i64 - target as i64
            } else {
                target as i64 - (here + size) as i64
            };
            // **实参 > 255 ⇒ 记下来，收尾时在前面插一个 `EXTENDED_ARG`**（第 121 轮；
            //   实测 CPython：`EXTENDED_ARG 3` 在码元 1604、`JUMP_BACKWARD 804` 在 1606 ✓，
            //   两条**位点相同**＝跳转自身那条 ✓；距离公式不变 ✓ —— 前缀在 `here` 之前 ✓）。
            if !(0..=255).contains(&argument) {
                self.wide_jumps.push((here, argument as u16));
            }
            self.unit.code[argument_byte] = (argument & 0xFF) as u8;
        }
    }

    /// **把需要加宽的跳转补上 `EXTENDED_ARG` 前缀**（第 121 轮）。
    ///
    /// 必须在**所有回填之后、`encode_exceptiontable` 之前**调用 ✓：插词会移动其后全部码元
    /// ⇒ 这里同步做三件事：重建 `code`、给 `positions` 插同一条位点、按插入数**平移异常表偏移** ✓。
    pub(super) fn widen_extended_args(&mut self) {
        if self.wide_jumps.is_empty() {
            return;
        }
        let wide = core::mem::take(&mut self.wide_jumps);
        let word_count = self.unit.code.len() / 2;
        let mut high: Vec<Option<u16>> = vec![None; word_count];
        for (word, argument) in &wide {
            if let Some(slot) = high.get_mut(*word) {
                *slot = Some(*argument);
            }
        }
        // `shift[word]` ＝ 该码元**之前**插入了几个词（供异常表平移）
        let mut shift: Vec<usize> = vec![0; word_count + 1];
        let mut code: Vec<u8> = Vec::with_capacity(self.unit.code.len() + 2 * wide.len());
        let mut positions: Vec<(Option<u32>, Option<u32>, Option<u32>, Option<u32>)> =
            Vec::with_capacity(self.unit.positions.len() + wide.len());
        let mut inserted = 0usize;
        let mut word = 0usize;
        let mut instruction = 0usize;
        while word < word_count {
            let opcode = u16::from(self.unit.code[word * 2]);
            let argument = self.unit.code[word * 2 + 1];
            let size = 1 + opcode::inline_cache_entries(opcode) as usize;
            shift[word] = inserted;
            if let Some(Some(full)) = high.get(word) {
                code.push(opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里") as u8);
                code.push((full >> 8) as u8);
                // 前缀与随后的指令**同一条位点**（实测 ✓）
                positions.push(self.unit.positions[instruction]);
                inserted += 1;
            }
            code.push(opcode as u8);
            code.push(argument);
            positions.push(self.unit.positions[instruction]);
            for cached in 1..size {
                code.push(self.unit.code[(word + cached) * 2]);
                code.push(self.unit.code[(word + cached) * 2 + 1]);
                shift[(word + cached).min(word_count)] = inserted;
            }
            word += size;
            instruction += 1;
        }
        shift[word_count] = inserted;
        // **异常表偏移平移**（条目里存的是字节偏移 ⇒ 乘 2 换成码元再查插入数 ✓）
        for entry in &mut self.exception_entries {
            for field in [&mut entry.0, &mut entry.1, &mut entry.2] {
                let unit_index = (*field / 2).min(word_count);
                *field += 2 * shift[unit_index];
            }
        }
        self.unit.code = code;
        self.unit.positions = positions;
    }

    /// 发射一条指令并记位点：`position = None` ⇒ 四元组**全 `None`**（`BC-4` 扩的合成指令）。
    pub(super) fn emit_core(&mut self, position: Option<Span>, opcode: u16, oparg: u8) {
        self.unit.positions.push(match position {
            Some(span) => (
                Some(span.line_start),
                Some(span.line_end),
                Some(span.col_start),
                Some(span.col_end),
            ),
            None => (None, None, None, None),
        });
        if let Some(span) = position {
            self.last_span = span;
        }
        self.unit.code.push(opcode as u8);
        self.unit.code.push(oparg);
        // `BC-35`／`BC-36`：带缓存的指令后必须留等宽**零填充**码元
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.unit.code.push(0);
            self.unit.code.push(0);
        }
    }

    /// 发射一条**带位点**的指令（绝大多数情况）。
    pub(super) fn emit_at(&mut self, position: Span, opcode: u16, oparg: u8) {
        self.emit_core(Some(position), opcode, oparg);
    }

    /// 发射一条**没有位点**的合成指令（`BC-4` 扩：四元组全 `None`）。
    pub(super) fn emit_none(&mut self, opcode: u16, oparg: u8) {
        self.emit_core(None, opcode, oparg);
    }

    /// 收尾时把"待定常量"登记进表并回填实参。
    /// **延迟入池**的常量（字面量默认值折出来的元组）：参照把它们排在常量表**最后**
    /// （`x = 200 + 100` ⇒ `[200, None, 300]`；`return 200 + 100` ⇒ `[200, 300]`）。
    /// **每个作用域末尾都必须冲刷** —— 此前这段只在"模块且要收尾"那一支里 ✗，
    /// 于是**函数里嵌套 `def` 的默认值元组会丢**（第 280 轮修）。
    /// **条件出口副本的冲刷**：给每个走向条件出口的跳转发一份收尾（在**作用域收尾之后**；
    /// 必须**显式**发那两条：`emit_rest_and_tail`／`emit_implicit_return` 在这个时机都发不出 ✗）。
    pub(super) fn flush_condition_copies(&mut self) -> Result<(), CompileError> {
        // **链式比较的续部副本**（第 85 轮）：`SWAP 2; POP_TOP` ＋ 存入 ＋ 余部 ＋ 收尾
        let chains = core::mem::take(&mut self.pending_chain_copies);
        for (landing, target, target_span, rest, span) in &chains {
            self.mark_label(*landing);
            self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
            self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
            if self.kind == ScopeKind::Module {
                let index = self.intern_name(target);
                self.emit_indexed(*target_span, "STORE_NAME", index);
            } else {
                let slot = self.slot_of(target);
                self.emit_at(
                    *target_span,
                    opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                    slot as u8,
                );
            }
            // **重放期间抑制接管**（否则余部里的链式会再排队 ⇒ 悬空标签 ✗）
            let saved = self.suppress_chain_takeover;
            self.suppress_chain_takeover = true;
            self.emit_block(rest, false)?;
            self.suppress_chain_takeover = saved;
            let none_index = self.intern_constant(Constant::None);
            // 收尾那两条的位点取**目标**（实测复制块是 `STORE_NAME x; LOAD_CONST None; RETURN_VALUE`
            // 三条同为 `(1,1,0,1)` ✓，不是语句跨度 ✗）
            self.emit_indexed(*target_span, "LOAD_CONST", none_index);
            self.emit_at(*target_span, opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"), 0);
        }
        let copies = core::mem::take(&mut self.pending_condition_copies);
        for (landing, rest, span) in &copies {
            self.mark_label(*landing);
            self.emit_block(rest, false)?;
            let none_index = self.intern_constant(Constant::None);
            self.emit_indexed(*span, "LOAD_CONST", none_index);
            self.emit_at(
                *span,
                opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                0,
            );
        }
        Ok(())
    }

    pub(super) fn flush_deferred(&mut self) {
        for (offset, constant) in core::mem::take(&mut self.deferred) {
            let index = self.intern_constant(constant);
            self.unit.code[offset] = index as u8;
        }
    }

    pub(super) fn flush_pending(&mut self) {
        let pending = core::mem::take(&mut self.pending);
        for (argument_byte, constant) in pending {
            let index = self.intern_constant(constant);
            self.unit.code[argument_byte] = index as u8;
        }
    }

    pub(super) fn intern_constant(&mut self, constant: Constant) -> usize {
        if let Some(index) = self.unit.constants.iter().position(|item| *item == constant) {
            return index;
        }
        self.unit.constants.push(constant);
        self.unit.constants.len() - 1
    }

    /// 登记一个字面量，规矩照实测：小整数**只在常量表还是空的时候**才登记；大整数与字符串总是登记。
    pub(super) fn intern_literal(&mut self, constant: Constant) {
        if let Constant::Int(value) = constant {
            if (0..=255).contains(&value) && !self.unit.constants.is_empty() {
                return;
            }
        }
        self.intern_constant(constant);
    }

    pub(super) fn intern_name(&mut self, name: &str) -> usize {
        if let Some(index) = self.unit.names.iter().position(|item| item == name) {
            return index;
        }
        self.unit.names.push(name.to_owned());
        self.unit.names.len() - 1
    }

    /// 局部槽位（没有就按首次出现顺序追加——形参已经在前面）。
    /// **本作用域里这个名是不是 cell／free**（第 292 轮）：是 ⇒ 返回 `LOAD_DEREF`／`STORE_DEREF`
    /// 的 localsplus 索引（cell 排在 `varnames` 之后；free 就在自己的自由变量表里）。
    pub(super) fn deref_slot(&self, name: &str) -> Option<usize> {
        if self.unit.cellvars.iter().any(|item| item == name) {
            return Some(self.cell_slot(name).expect("刚查过在 cellvars 里"));
        }
        self.unit.freevars.iter().position(|item| item == name).map(|free| {
            // localsplus 布局：`varnames` ＋ `cellvars` ＋ `freevars`
            // **freevars 从"追加后的 cell"之后起** ✓（不是 `cellvars.len()` ✗ —— 形参 cell 不占位 ✓）。
            self.unit.varnames.len() + self.appended_cells() + free
        })
    }

    /// **cell 在 localsplus 里的槽**（实测两种）：**形参** cell 用它自己的 `varnames` 槽
    /// （`def outer(x): …` ⇒ `MAKE_CELL 0`、元组元素 `LOAD_FAST_BORROW 0`，同时 `varnames=('x','inner')`）；
    /// **局部** cell 排在 `varnames` **之后**（`def outer(): x = 1 …` ⇒ `varnames=('inner',)`、
    /// `MAKE_CELL 1`）。
    /// **`localsplus` 里"追加"的 cell 个数** ✓（第 210 轮真 bug 修复 ✗）：`cellvars` 中**已经是形参**
    /// 的那些**复用** `varnames` 槽 ✓、**不占**追加位 ⇒ 只有**非形参**的 cell 才追加 ✓
    /// （与 [`crate::CodeObject::localsplus_kinds`] **同一条规矩** ✓ —— 一处真相 ✓）。
    /// 先前这里直接用 `cellvars` 的**下标**当偏移 ✗ ⇒ 一个形参 cell 混在里面就**整体多算一格** ✗
    /// （实测 `Lib/types.py` 的 `coroutine`：`cellvars=[func(形参), co_flags, _collections_abc]` ⇒
    /// 第三个 cell 发成槽 6、而 localsplus 只有 6 ✗ ⇒ 差一 ✓）。
    pub(super) fn appended_cells(&self) -> usize {
        self.unit
            .cellvars
            .iter()
            .filter(|name| !self.unit.varnames.contains(name))
            .count()
    }

    /// `cellvars` 里**第 `cell` 个**之前的"追加"cell 个数 ✓。
    fn appended_cells_before(&self, cell: usize) -> usize {
        self.unit
            .cellvars
            .iter()
            .take(cell)
            .filter(|name| !self.unit.varnames.contains(name))
            .count()
    }

    pub(super) fn cell_slot(&self, name: &str) -> Option<usize> {
        let cell = self.unit.cellvars.iter().position(|item| item == name)?;
        if let Some(local) = self.unit.varnames.iter().position(|item| item == name) {
            return Some(local); // 形参（`varnames` 前缀）——它的实参槽就是 cell 槽
        }
        Some(self.unit.varnames.len() + self.appended_cells_before(cell))
    }

    /// `varnames` 里属于**参数**的个数（前缀：仅位置 + 位置或关键字 + 关键字）。
    pub(super) fn parameter_count(&self) -> usize {
        (self.unit.posonlyargcount + self.unit.argcount + self.unit.kwonlyargcount) as usize
    }

    pub(super) fn slot_of(&mut self, name: &str) -> usize {
        // **cell／自由变量优先**：它们都不进 `varnames`（`nonlocal` 声明的名字因此不会被
        // `collect_locals` 当成局部 ✓）
        if let Some(slot) = self.deref_slot(name) {
            return slot;
        }
        if let Some(index) = self.unit.varnames.iter().position(|item| item == name) {
            return index;
        }
        // **cell 名不进 `varnames`**（它在 localsplus 里排在 varnames 之后）⇒ 返回 cell 索引，
        // 免得 `collect_locals` 二次调用时把它又加回 `varnames` ✗（索引就乱了）
        if let Some(cell) = self.unit.cellvars.iter().position(|item| item == name) {
            // **与 `cell_slot` 同一条公式** ✓（第 261 轮：这里先前用 `cellvars` 的**下标** ✗ ⇒
            // 有**形参 cell**时会整体多算一格 ✗ ⇒ 与 `cell_slot` 的口径不一致 ✓）。
            return self.unit.varnames.len() + self.appended_cells_before(cell);
        }
        self.unit.varnames.push(name.to_owned());
        self.unit.nlocals = self.unit.varnames.len();
        self.unit.varnames.len() - 1
    }

    pub(super) fn emit_statement(
        &mut self,
        statement: &Statement,
        rest: &[Statement],
    ) -> Result<(), CompileError> {
        // **作用域最后一条语句 ⇒ 默认"能落到末尾"** ✓（第 199 轮真 bug 修复 ✗）：`epilogue_needed`
        // 是**结构字段** ✓，各臂按"本条语句能不能落到末尾"改写它 ✓ ⇒ 但**非终局臂从不清回** ✗ ⇒
        // 前面若出现过 `raise`／`return`（例如 `if/elif/else` 里 `else: raise` ✓）就会**连坐**后面 ✗
        // ⇒ 作用域**漏发**收尾那两条 ✗（实测最小复现：跑完却报「码元跑完却没有 RETURN_VALUE」✗）。
        // ⇒ 只在"**本条是作用域最后一条**"时置回 `true` ✓（嵌套块的 `rest` 是**块内**余部 ✓，
        //   置了也会被外层臂覆盖 ✓ ⇒ 安全 ✓）。
        if rest.is_empty() {
            self.epilogue_needed = true;
        }
        match statement {
            // **`match <主语>: case …`**（第 290 轮，最小面：字面量／捕获／通配／或 ＋ 守卫）
            Statement::Match {
                span,
                subject,
                cases,
            } => {
                self.emit_expression(subject)?;
                let end = self.new_label();
                for case in cases {
                    let next_case = self.new_label();
                    let matched = self.new_label();
                    // 模式判定：命中 ⇒ 落到 `matched`；不命中 ⇒ 跳到 `next_case` ✓
                    //（主语**一直留在栈上** ✓ —— 字面量那一档照参照用 `COPY 1` 复制一份再比 ✓）
                    self.emit_pattern_test(&case.pattern, next_case, matched)?;
                    self.mark_label(matched);
                    // **命中之后的次序**（照参照逐条 `dis` 实测）：
                    // ① **捕获先绑** —— `COPY 1; STORE <名字>` ✓（复制一份来绑 ⇒ 守卫看得见这个名字 ✓，
                    //    而且守卫不过时**主语还留在栈上** ✓ ⇒ 下一条 `case` 照常判 ✓）；
                    // ② 再判**守卫**（本层没有 `TO_BOOL` ✓ ⇒ 直接 `POP_JUMP_IF_FALSE` 取真值 ✓，语义相同 ✓）；
                    // ③ 最后把主语 `POP_TOP` 掉 ✓（命中后栈上不留东西 ✓）。
                    if let Pattern::Capture(name, name_span) = &case.pattern {
                        self.emit_named(*name_span, "COPY", 1);
                        self.emit_store_name(*name_span, name);
                    }
                    if let Some(guard) = &case.guard {
                        self.emit_expression(guard)?;
                        self.emit_jump(*span, opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"), next_case);
                        self.emit_named(*span, "NOT_TAKEN", 0);
                    }
                    self.emit_named(*span, "POP_TOP", 0);
                    self.emit_block(&case.body, false)?;
                    self.emit_jump(*span, opcode::opcode("JUMP_FORWARD").expect("表里有"), end);
                    self.emit_named(*span, "NOT_TAKEN", 0);
                    self.mark_label(next_case);
                }
                // 一条 `case` 都没中 ⇒ 主语还在栈上 ✓ 丢掉 ✓
                self.emit_named(*span, "POP_TOP", 0);
                self.mark_label(end);
                Ok(())
            }
            // **`yield [值]`**（第 124 轮实测）：值 ⇒ `YIELD_VALUE 0` ⇒ `RESUME 5` ⇒ `POP_TOP`；
            //   三条位点全取**整条 `yield`** ✓；收尾也取它 ✓（生成器的收尾块由作用域收口另发 ✓）。
            Statement::Yield(value, span) => {
                if let Some(value) = value {
                    self.emit_expression(value)?;
                } else {
                    let index = self.intern_constant(Constant::None);
                    self.emit_indexed(*span, "LOAD_CONST", index);
                }
                self.emit_named(*span, "YIELD_VALUE", 0);
                self.emit_named(*span, "RESUME", 5);
                self.emit_named(*span, "POP_TOP", 0);
                self.last_span = *span;
                self.epilogue_span = *span;
                Ok(())
            }
            // **`yield from <表达式>` 的语句形态**（第 315 轮）：表达式的值丢掉 ⇒ 末尾补 `POP_TOP` ✓
            //（照参照实测：`GET_YIELD_FROM_ITER; L1: LOAD_CONST None; SEND L2; YIELD_VALUE 1; RESUME 2;
            //  JUMP_BACKWARD_NO_INTERRUPT L1; L2: END_SEND; POP_TOP` ✓）。
            Statement::YieldFrom(value, span) => {
                self.emit_yield_from(value, *span, true)?;
                self.last_span = *span;
                self.epilogue_span = *span;
                Ok(())
            }
            // **`import`**（逐条实测）：每条 `LOAD_SMALL_INT 0; LOAD_CONST None; IMPORT_NAME <模块>`
            //   ＋（有 `as` ⇒ `IMPORT_FROM <末段>; STORE <别名>; POP_TOP`；否则 `STORE <顶层名>`）；
            //   位点整条都用**语句**那段。
            Statement::Import { items, span } => {
                for (module, alias) in items {
                    self.intern_literal(Constant::Int(0));
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                        0,
                    );
                    let none_index = self.intern_constant(Constant::None);
                    self.emit_indexed(*span, "LOAD_CONST", none_index);
                    let module_index = self.intern_name(module);
                    self.emit_indexed(*span, "IMPORT_NAME", module_index);
                    match alias {
                        // **含点的模块**才要 `IMPORT_FROM` 取最后一段（实测 `import a.b as c` ⇒
                        // `IMPORT_NAME a.b; IMPORT_FROM b; STORE c; POP_TOP`）；`import b as c` 直接 `STORE c`
                        Some(alias) if module.contains('.') => {
                            let last = module.rsplit('.').next().unwrap_or(module);
                            let last_index = self.intern_name(last);
                            self.emit_indexed(*span, "IMPORT_FROM", last_index);
                            self.store_target(*span, alias);
                            self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                        }
                        Some(alias) => self.store_target(*span, alias),
                        None => {
                            let top = module.split('.').next().unwrap_or(module);
                            self.store_target(*span, top);
                        }
                    }
                }
                // 收尾跟着**本条语句**的跨度（实测 `def f():\n    import a\n` 的收尾是 `(2,2)`）
                self.epilogue_span = *span;
                Ok(())
            }
            // **`from … import …`**（逐条实测）：`LOAD_SMALL_INT <层级>; LOAD_CONST (<名字>, …);
            //   IMPORT_NAME <模块>` ＋ 逐名字 `IMPORT_FROM; STORE` ＋ 末尾一条 `POP_TOP`；
            //   `*` 走 `CALL_INTRINSIC_1 2`（`INTRINSIC_IMPORT_STAR`）再 `POP_TOP`。
            Statement::ImportFrom {
                module,
                level,
                names,
                star,
                span,
            } => {
                self.intern_literal(Constant::Int(i64::from(*level)));
                self.emit_at(
                    *span,
                    opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                    *level,
                );
                let fromlist: Vec<String> = if *star {
                    vec!["*".to_owned()]
                } else {
                    names.iter().map(|(name, _)| name.clone()).collect()
                };
                let list_index = self.intern_constant(Constant::Names(fromlist));
                self.emit_indexed(*span, "LOAD_CONST", list_index);
                let module_index = self.intern_name(module);
                self.emit_indexed(*span, "IMPORT_NAME", module_index);
                if *star {
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL_INTRINSIC_1").expect("CALL_INTRINSIC_1 在表里"),
                        2,
                    );
                    self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                } else {
                    for (name, alias) in names {
                        let name_index = self.intern_name(name);
                        self.emit_indexed(*span, "IMPORT_FROM", name_index);
                        self.store_target(*span, alias.as_deref().unwrap_or(name));
                    }
                    self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                }
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::With {
                items,
                body,
                span,
                is_async,
            } => {
                // **3.14 的 `with` 骨架**（逐条实测，支持多项）：
                //   逐项 `上下文; COPY 1; LOAD_SPECIAL __exit__; SWAP 2; SWAP 3;
                //   LOAD_SPECIAL __enter__; CALL 0; STORE <目标>／POP_TOP`（各项**受保护区**
                //   从自己的 `STORE`／`POP_TOP` 起，嵌套覆盖）
                //   体；**逆序**的退出调用 `LOAD_CONST None×3; CALL 3; POP_TOP`；余部＋收尾
                //   **逆序**的清理块：`PUSH_EXC_INFO; WITH_EXCEPT_START; TO_BOOL; POP_JUMP_IF_TRUE;
                //   NOT_TAKEN; RERAISE 2; <处理过> POP_TOP; POP_EXCEPT; POP_TOP×3`；处理过之后
                //   **内层跳回外层的退出调用**（实测 `JUMP_BACKWARD_NO_INTERRUPT`），最外层接余部＋收尾
                //   末尾 `COPY 3; POP_EXCEPT; RERAISE 1`
                // 异常表：各项受保护区 → 自己的清理块（`depth` ＝ 2×该层项数、`lasti` 打开）；
                //         各清理块 → 末尾（`depth` ＋2）。
                let context_spans: Vec<Span> =
                    items.iter().map(|(context, _)| context.span()).collect();
                let mut region_starts: Vec<usize> = Vec::with_capacity(items.len());
                for (index, (context, target)) in items.iter().enumerate() {
                    let context_span = context_spans[index];
                    self.emit_expression(context)?;
                    self.emit_at(context_span, opcode::opcode("COPY").expect("COPY 在表里"), 1);
                    self.emit_at(
                        context_span,
                        opcode::opcode("LOAD_SPECIAL").expect("LOAD_SPECIAL 在表里"),
                        // **`async with`**：特殊方法表下标 2／3 是 `__aenter__`／`__aexit__` ✓
                        // （0／1 是 `__enter__`／`__exit__` ✓ —— 表见 `opcode_metadata.rs` ✓）。
                        if *is_async { 3 } else { 1 },
                    );
                    self.emit_at(context_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                    self.emit_at(context_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 3);
                    self.emit_at(
                        context_span,
                        opcode::opcode("LOAD_SPECIAL").expect("LOAD_SPECIAL 在表里"),
                        if *is_async { 2 } else { 0 },
                    );
                    self.emit_at(context_span, opcode::opcode("CALL").expect("CALL 在表里"), 0);
                    // **`async with` 的进入要等一次**（第 307 轮，照参照实测的形状）：
                    // `GET_AWAITABLE 1; LOAD_CONST None; SEND <出>; YIELD_VALUE 1; RESUME 3;
                    //  JUMP_BACKWARD_NO_INTERRUPT <回>; <出> END_SEND` ✓ —— 与 `async for` 同一套近似 ✓
                    // （`async def` 在本层是生成器 ✓ ⇒ `SEND` 一步就把值拿到 ✓）。
                    if *is_async {
                        let await_loop = self.new_label();
                        let await_done = self.new_label();
                        self.emit_at(
                            context_span,
                            opcode::opcode("GET_AWAITABLE").expect("GET_AWAITABLE 在表里"),
                            1,
                        );
                        let none_index = self.intern_constant(Constant::None);
                        self.emit_at(
                            context_span,
                            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                            none_index as u8,
                        );
                        self.emit_directed_jump(
                            context_span,
                            opcode::opcode("SEND").expect("SEND 在表里"),
                            await_done,
                            false,
                        );
                        self.mark_label(await_loop);
                        self.emit_at(
                            context_span,
                            opcode::opcode("YIELD_VALUE").expect("YIELD_VALUE 在表里"),
                            1,
                        );
                        self.emit_at(
                            context_span,
                            opcode::opcode("RESUME").expect("RESUME 在表里"),
                            3,
                        );
                        self.emit_at(
                            context_span,
                            opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                            0,
                        );
                        self.emit_directed_jump(
                            context_span,
                            opcode::opcode("JUMP_BACKWARD_NO_INTERRUPT")
                                .expect("JUMP_BACKWARD_NO_INTERRUPT 在表里"),
                            await_loop,
                            true,
                        );
                        self.mark_label(await_done);
                        self.emit_at(
                            context_span,
                            opcode::opcode("END_SEND").expect("END_SEND 在表里"),
                            0,
                        );
                    }
                    region_starts.push(self.unit.code.len());
                    if let Some((target, target_span)) = target {
                        match self.kind {
                            ScopeKind::Module | ScopeKind::Class => {
                                let name_index = self.intern_name(target);
                                self.emit_indexed(*target_span, "STORE_NAME", name_index);
                            }
                            ScopeKind::Function => {
                                let slot = self.slot_of(target);
                                self.emit_at(
                                    *target_span,
                                    opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                    slot as u8,
                                );
                            }
                        }
                    } else {
                        self.emit_at(
                            context_span,
                            opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                            0,
                        );
                    }
                }
                // **`with` 体内的 `RETURN_VALUE`**：取最外层 `with` 的**第一项上下文**跨度
                // （实测：嵌套时外层不被内层覆盖；多项时取第一项——退出调用逆序发，最后发它）
                let saved_with_return = self.with_return_span;
                if saved_with_return.is_none() {
                    self.with_return_span = context_spans.first().copied();
                }
                // 体期间记下这一层（`return` 要逐层跑退出调用；内层先 ⇒ 用栈的**逆序**遍历）
                let saved_with_exits = self.with_exit_stack.clone();
                self.with_exit_stack.push(context_spans.clone());
                let saved_epilogue_body = self.in_epilogue_body;
                self.in_epilogue_body = true;
                self.emit_block(body, false)?;
                self.in_epilogue_body = saved_epilogue_body;
                self.with_exit_stack = saved_with_exits;
                self.with_return_span = saved_with_return;
                // **受保护区止于"正常流末尾"** ✓（第 166 轮）：体内有 `return` 时，参照的右端是**退出调用之前**
                // 那一点 ✓（用 `return` 路径记下的 `with_body_end` ✓）；没有 `return` 就仍是当前长度 ✓。
                let region_end = self
                    .with_body_end
                    .take()
                    .unwrap_or_else(|| self.unit.code.len());
                let none_index = self.intern_constant(Constant::None);
                // **`NOP`**（实测，第 256 轮逐案收窄）：只当体里存在**跳向块尾的分支**时才发
                // ——典型是"最后一条是**无 `else` 的 `if`**"（`if flag: return tag` 的假分支就跳到
                // 这条 NOP，再往下才是退出调用）。反例（都不发）：`with cm as y: return y`（体必然终止）、
                // `with a as x, b as y: z = 1`（体直接落下来、没有分支跳过来）
                // ——后者是第 256 轮被**既有用例**当场抓住的过度发射。
                let tail_if = matches!(body.last(), Some(Statement::If { else_body, .. }) if else_body.is_empty());
                if !block_terminates(body) && tail_if {
                    // 位点取体末那条 `if` 的**条件**跨度（实测 `if flag: return tag` 的 `NOP` 是
                    // `(10, 10, 11, 15)`＝`flag` 那段，**不是**"上一条指令"——体里 `return` 的退出
                    // 复制件用的是上下文跨度，用 `last_span` 会落到 `with` 那一行）
                    let nop_span = match body.last() {
                        Some(Statement::If { condition, .. }) => condition.span(),
                        _ => self.last_span,
                    };
                    self.emit_at(nop_span, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                }
                // **单项 `with` 且体必然终止** ⇒ 参照把**正常退出路径整块省掉**（死代码：体里那条
                // `return` 已经跑过退出调用），只留异常路径的清理块（它自带一份退出＋收尾）。
                // 实测 `def f(cm):\n    with cm:\n        return 1\n` 的产物里**没有**第二组
                // `LOAD_CONST×3; CALL 3; POP_TOP`，也没有随之的那对收尾。
                let dead_normal_exit = block_terminates(body) && items.len() == 1;
                // **逆序**的退出调用（内层先退）；每条记一个标签，供清理块跳回
                let mut exit_labels: Vec<usize> = vec![0; items.len()];
                let mut terminated = true;
                if !dead_normal_exit {
                    for index in (0..items.len()).rev() {
                        let context_span = context_spans[index];
                        let label = self.new_label();
                        self.mark_label(label);
                        exit_labels[index] = label;
                        self.emit_with_exit_call(context_span, none_index);
                    }
                    terminated = self.emit_rest_and_tail(rest, *span)?;
                }
                // **逆序**的清理块**冷块**（发射计划见 `WithCleanupPlan`）。
                // 本轮是**纯重构**：内容与顺序都不变，只是把"攒计划"与"发射"分开，
                // 好让下一步把发射时机挪到作用域正常路径之后（参照的冷块外提）。
                let plan = WithCleanupPlan {
                    context_spans: context_spans.clone(),
                    region_starts: region_starts.clone(),
                    region_end,
                    exit_labels: exit_labels.clone(),
                    rest: rest.to_vec(),
                    span: *span,
                };
                terminated &= self.emit_with_cleanups(&plan)?;
                self.epilogue_span = context_spans[0];
                self.epilogue_needed = !terminated;
                Ok(())
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                span,
            } => {
                // **体必然终止 ⇒ 正常路径整体不可达**（余部、收尾、以及"在正常路径就地发一遍 finally"
                // 都是死代码）——参照实测：`def f(a): try: return 1 finally: y = 2` 之后接 `z = 3`，
                // 产物里没有 `STORE_FAST z`、常量表也没有 `3`。
                // **正常路径那份 finally**：体只要"终止"（`return`／`raise`／`break`／`continue` 都算）
                // 就走不到它 ⇒ 死代码（`break`／`continue` 的出口各自内联了一份）。
                // 注意与 `unreachable_rest` 的区别：那个还要看**作用域是否落得到底**，只有
                // `return`／`raise` 才算（`break`／`continue` 之后收尾仍要发）。
                let normal_finally_dead = handlers.is_empty()
                    && block_terminates(body)
                    && !block_terminates(finally_body);
                // **块结构模型**（第 229 轮）：照参照实测的布局——
                //   开头 `NOP`（位点 ＝ **整条 `try` 语句**）；套体；套体出口**重放余部＋收尾**
                //   `PUSH_EXC_INFO`（**无位点**的合成指令）；各处理块的类型检查链
                //   （不匹配 → 下一块的检查；最后一块不匹配 → `RERAISE 0`）
                //   每个处理块：体 → `POP_EXCEPT` →（`as 名字` 时清理）→ 重放余部＋收尾
                //   清理块 `COPY 3; POP_EXCEPT; RERAISE 1`（同样无位点）
                self.emit_at(*span, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                let body_start = self.unit.code.len();
                let saved_epilogue_body = self.in_epilogue_body;
                if !finally_body.is_empty() {
                    self.in_epilogue_body = true;
                }
                // 体内的 `return` 要先把 finally 跑一遍（参照实测；见 `finally_stack` 的说明）
                if !finally_body.is_empty() {
                    self.finally_stack.push(finally_body.clone());
                }
                self.emit_block(body, false)?;
                if !finally_body.is_empty() {
                    self.finally_stack.pop();
                }
                self.in_epilogue_body = saved_epilogue_body;
                let body_end = self.unit.code.len();
                // **`else`**（实测）：紧跟套体（套体正常走完才有它）；它**不在**受保护区内
                //（异常表只盖 `body` ⇒ 所以 `body_end` 要在 else 之前采）
                if !else_body.is_empty() {
                    self.emit_block(else_body, false)?;
                }
                // **`finally`** 的正常路径：就地发一遍（异常路径会经 `PUSH_EXC_INFO` 再发一遍）
                let has_finally = !finally_body.is_empty();
                let finally_label = self.new_label();
                if has_finally {
                    self.mark_label(finally_label);
                // 体必然终止时这一份是**死代码**（`return` 已在 finally 之后返回）⇒ 不发
                    if !normal_finally_dead {
                        self.emit_block(finally_body, false)?;
                    }
                }
                // 套体正常跑完的出口（重放余部＋收尾）——它**不属于**受保护区
                // **纯 `try/finally` 且体必然终止** ⇒ 余部（连同收尾）**不可达**，参照**根本不发**：
                // 实测 `def f(a):\n    try:\n        return 1\n    finally:\n        y = 2\n    z = 3\n`
                // 的产物里既没有 `STORE_FAST z`，常量表也没有 `3`（`co_consts` 只有 `(2,)`）；
                // 体不终止时照旧（`co_consts` = `(1, None)`，即正常路径＋收尾都在）。
                // 只收"没有 `except`"这一支：有处理块时余部可能由处理块**正常完成**而到达。
                let unreachable_rest = handlers.is_empty()
                    && block_returns_or_raises(body)
                    && !block_terminates(finally_body);
                let mut all_terminate = if unreachable_rest {
                    true
                } else if self.block_depth == 1 {
                    self.emit_rest_and_tail(rest, *span)?
                } else {
                    // **嵌套** ✓（第 202 轮修 ✗）：只发**余部** ✓、**不发**作用域收尾 ✓，
                    // 但仍要**跳到块尾** ✓（否则处理块会**落进**余部 ✗ ⇒ 实测 `StackUnderflow` ✗），
                    // 并**如实返回"没终止"** ✗ —— 先前返回 `true` ✗ ⇒ 外层（`if` 那一臂）以为整条
                    // `if` 已终止 ⇒ **不再发模块余部** ✗（`print` 就是这样消失的 ✓）。
                    self.emit_block(rest, false)?;
                    if block_terminates(rest) {
                        true
                    } else {
                        if let Some(end) = self.block_end_labels.last().copied() {
                            self.emit_jump(
                                *span,
                                opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                                end,
                            );
                        }
                        false
                    }
                };
                // **`try/finally`**（没有 `except`）：异常路径＝`PUSH_EXC_INFO`（无位点）＋ finally
                // 再来一遍 ＋ `RERAISE`（粘性位点）＋ 清理三连（实测）
                if handlers.is_empty() {
                    let exception_path = self.unit.code.len();
                    self.emit_named_none("PUSH_EXC_INFO", 0);
                    // **区间起点含 `PUSH_EXC_INFO`** ✓（第 168 轮：与第 ④ 处同款 ✗ —— 照参照 `(10, 4, 14)`
                    //   码元 ✓，区间是 `PUSH_EXC_INFO` 起 ✓）。
                    let finally_region_start = exception_path;
                    self.emit_block(finally_body, false)?;
                    let sticky = self.last_span;
                    self.emit_at(sticky, opcode::opcode("RERAISE").expect("RERAISE 在表里"), 0);
                    let cleanup = self.unit.code.len();
                    // **区间右端＝清理之前（含那条 `RERAISE 0`）** ✓（第 168 轮：参照的右端正是 14 ✓）。
                    let finally_region_end = cleanup;
                    self.emit_named_none("COPY", 3);
                    self.emit_named_none("POP_EXCEPT", 0);
                    self.emit_named_none("RERAISE", 1);
                    self.record_exception(body_start, body_end, exception_path, self.handler_depth, false);
                    self.record_exception(finally_region_start, finally_region_end, cleanup, self.handler_depth + 1, true);
                    self.epilogue_span = *span;
                    self.epilogue_needed = !all_terminate;
                    return Ok(());
                }
                // **处理块入口**＝`PUSH_EXC_INFO` 那条（异常表的 target 就是它；必须采在重放之后）
                // ——它也是**合成指令**：参照给全 `None`（`BC-4` 扩）
                let handler_start = self.unit.code.len();
                // **第一个处理块的区间要包含这条 `PUSH_EXC_INFO`** ✓（第 167 轮：与参照逐字节对齐后
                //   现形 ✓ —— 我们先前从它**之后**起算 ✗，整段晚 1 码元 ✓）。
                let push_exc_offset = self.unit.code.len();
                self.emit_named_none("PUSH_EXC_INFO", 0);
                let mut first_segment_start = Some(push_exc_offset);
                let mut pending_unmatched: Vec<usize> = Vec::new();
                // 最后一个处理块"体后清理"那段的起点（有 `finally` 时异常表第 3 条要用）
                let mut handler_cleanup_start = 0usize;
                for handler in handlers {
                    for skip in pending_unmatched.drain(..) {
                        self.mark_label(skip);
                    }
                    let segment_start = first_segment_start
                        .take()
                        .unwrap_or_else(|| self.unit.code.len());
                    if let Some(exception_type) = &handler.type_ {
                        self.emit_expression(exception_type)?;
                        self.emit_at(
                            handler.span,
                            opcode::opcode("CHECK_EXC_MATCH").expect("CHECK_EXC_MATCH 在表里"),
                            0,
                        );
                        let skip = self.new_label();
                        self.emit_jump(
                            handler.span,
                            opcode::opcode("POP_JUMP_IF_FALSE").expect("POP_JUMP_IF_FALSE 在表里"),
                            skip,
                        );
                        self.emit_at(
                            handler.span,
                            opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                            0,
                        );
                        pending_unmatched.push(skip);
                    }
                    // 匹配上了：栈顶是异常实例（有 `as 名字` ⇒ `STORE` 直接吃掉它；否则 `POP_TOP`）
                    if let Some(name) = &handler.name {
                        // **走统一入口** ✓（第 202 轮真 bug 修复 ✗：先前写死 `STORE_NAME` ⇒ 函数里必炸 ✓）。
                        self.emit_store_name(handler.span, name);
                    } else {
                        self.emit_at(
                            handler.span,
                            opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                            0,
                        );
                    }
                    // **处理块里的出口**（`return`）同样要先跑 finally。只罩**处理块体**：
                    // 正常路径那份 finally 有自己的位置，不能一起罩（上一轮把 `else` 一起罩住，
                    // 当场弄坏既有语料 `try_else_finally` ✗）。
                    if !finally_body.is_empty() {
                        self.finally_stack.push(finally_body.clone());
                    }
                    // **压一层处理器** ✓（第 203 轮）：里面的 `return` 要按参照收尾 ✓。
                    self.handler_stack.push(handler.name.clone());
                    self.handler_depth += 1;
                    self.emit_block(&handler.body, false)?;
                    self.handler_depth -= 1;
                    self.handler_stack.pop();
                    if !finally_body.is_empty() {
                        self.finally_stack.pop();
                    }
                    // **粘性位点**（实测）：体末那条之后的 `POP_EXCEPT`／`as 名字` 清理／收尾都取
                    // **上一条指令**的跨度（`except … as e` 的例子是 `(4,4,4,5)`＝名字 `y` 那段），
                    // 不是处理块语句那段的跨度
                    let sticky = self.last_span;
                    // 有 `finally` 时，"处理块跑完"那段也要兜进 finally 的异常路径（异常表第 3 条）
                    handler_cleanup_start = self.unit.code.len();
                    self.emit_at(
                        sticky,
                        opcode::opcode("POP_EXCEPT").expect("POP_EXCEPT 在表里"),
                        0,
                    );
                    if let Some(name) = &handler.name {
                        let none_index = self.intern_constant(Constant::None);
                        self.emit_indexed(sticky, "LOAD_CONST", none_index);
                        // **两条都走统一入口** ✓（同上 ✗）。
                        self.emit_store_name(sticky, name);
                        self.emit_delete_name(sticky, name);
                    }
                    // **无 `as 名字` 时，区间不含体后的那条 `POP_EXCEPT` 清理** ✓（第 167 轮：逐字节对拍
                    //   后现形 ✓ —— 我们先前把它也圈进去 ✗，长度多 1 码元 ✓）。
                    let segment_end = if handler.name.is_some() {
                        self.unit.code.len()
                    } else {
                        handler_cleanup_start
                    };
                    self.record_handler_segment(
                        segment_start,
                        segment_end,
                        handler.name.is_some(),
                    );
                    if has_finally {
                        // 处理块路径：**跳回正常路径的 finally＋余部＋收尾**（实测
                        // `JUMP_BACKWARD_NO_INTERRUPT`，位点取粘性那条）
                        self.emit_directed_jump(
                            self.last_span,
                            opcode::opcode("JUMP_BACKWARD_NO_INTERRUPT")
                                .expect("JUMP_BACKWARD_NO_INTERRUPT 在表里"),
                            finally_label,
                            true,
                        );
                    } else if self.block_depth == 1 {
                        all_terminate &= self.emit_rest_and_tail(rest, *span)?;
                    } else {
                        // **嵌套（`P3-20` 的真 bug ✗，第 353 轮修）**：处理块路径**必须**与套体出口那条
                        // **同一规矩** ✓ —— 只发**余部** ✓、**不发作用域收尾** ✗，然后跳到块尾 ✓。
                        // 先前这里不分深度、一律走 `emit_rest_and_tail` ✗ ⇒ 在**处理块里**发出一条
                        // `LOAD_CONST None; RETURN_VALUE` ✗ ⇒ **模块提前返回** ✓ ⇒ 其后的语句全丢 ✓。
                        // 实测原形（`P3-20` 的最小复现 ✓）：
                        //   try: raise ValueError
                        //   except ValueError:
                        //       print("in handler")
                        //       try: raise TypeError
                        //       except TypeError as exc: b = 2
                        //       print("after nested")
                        //   print("after")          ← 先前这一条跑不到 ✓
                        // 码元证据：处理块里 `print("after nested")` 之后紧跟
                        // `62 LOAD_CONST None; 63 RETURN_VALUE` ✗ ⇒ 正是它 ✓。
                        self.emit_block(rest, false)?;
                        if block_terminates(rest) {
                            all_terminate = true;
                        } else if let Some(end) = self.block_end_labels.last().copied() {
                            self.emit_jump(
                                *span,
                                opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                                end,
                            );
                        }
                    }
                }
                // **有 `as 名字` 的处理块**：清理区先来一遍"名字清理 ＋ `RERAISE 1`"（实测），
                // 处理块段的异常表目标就指到这里；之后才是"不匹配"的 `RERAISE 0` 与最后的清理块
                let mut name_cleanup = None;
                let named: Vec<String> = handlers
                    .iter()
                    .filter_map(|handler| handler.name.clone())
                    .collect();
                if !named.is_empty() {
                    name_cleanup = Some(self.unit.code.len());
                    for name in named {
                        let none_index = self.intern_constant(Constant::None);
                        // 这一份是**清理块里的合成副本**：参照给全 `None`（`BC-4` 扩）
                        self.emit_indexed_none("LOAD_CONST", none_index);
                        // **按作用域** ✓（第 202 轮真 bug 修复 ✗：先前一律 `STORE_NAME`／`DELETE_NAME`
                        // ⇒ 函数里 `except … as 名字` 的**清理副本**照样会撞"需要命名空间帧" ✓）。
                        if self.global_names.iter().any(|item| item == &name) {
                            let index = self.intern_name(&name);
                            self.emit_indexed_none("STORE_GLOBAL", index);
                            self.emit_indexed_none("DELETE_GLOBAL", index);
                        } else if self.kind == ScopeKind::Function
                            && self.unit.varnames.iter().any(|item| item == &name)
                        {
                            let slot = self.slot_of(&name);
                            self.emit_named_none("STORE_FAST", slot as u8);
                            self.emit_named_none("DELETE_FAST", slot as u8);
                        } else {
                            let index = self.intern_name(&name);
                            self.emit_indexed_none("STORE_NAME", index);
                            self.emit_indexed_none("DELETE_NAME", index);
                        }
                    }
                    self.emit_named_none("RERAISE", 1);
                }
                // **最后一个处理块是裸 `except:`** ⇒ 没有"不匹配"这条路 ⇒ 不发 `RERAISE 0`
                // （实测 `try: x = 1 except: y = 2` 的产物里没有它）
                let last_is_bare = handlers.last().is_some_and(|handler| handler.type_.is_none());
                for skip in pending_unmatched.drain(..) {
                    self.mark_label(skip);
                }
                if !last_is_bare {
                    // 不匹配那条 `RERAISE 0` 取**最后一个处理块**那段的跨度（实测 `except … as e`
                    // 的例子是 `(3,4,0,9)`＝`except` 子句），不是整条 `try` 语句的
                    let raise_span = handlers
                        .last()
                        .map(|handler| handler.span)
                        .unwrap_or(*span);
                    self.emit_at(raise_span, opcode::opcode("RERAISE").expect("RERAISE 在表里"), 0);
                }
                let cleanup = self.unit.code.len();
                // 最终清理三连同样是**合成指令**（参照给全 `None`）
                self.emit_named_none("COPY", 3);
                self.emit_named_none("POP_EXCEPT", 0);
                self.emit_named_none("RERAISE", 1);
                self.finish_handler_segments(cleanup, name_cleanup);
                self.record_exception(body_start, body_end, handler_start, self.handler_depth, false);
                if has_finally {
                    // **`except … finally`**（实测）：处理块链之后再发一遍 `finally` 的异常路径
                    //（`PUSH_EXC_INFO` ＋ finally ＋ `RERAISE` ＋ 清理三连），并把
                    //"处理块跑完那段"兜过去（异常表第 3 条）
                    let finally_path = self.unit.code.len();
                    self.emit_named_none("PUSH_EXC_INFO", 0);
                    let finally_region_start = self.unit.code.len();
                    self.emit_block(finally_body, false)?;
                    let finally_region_end = self.unit.code.len();
                    let sticky = self.last_span;
                    self.emit_at(sticky, opcode::opcode("RERAISE").expect("RERAISE 在表里"), 0);
                    let finally_cleanup = self.unit.code.len();
                    self.emit_named_none("COPY", 3);
                    self.emit_named_none("POP_EXCEPT", 0);
                    self.emit_named_none("RERAISE", 1);
                    self.record_exception(finally_region_start, finally_region_end, finally_cleanup, self.handler_depth + 1, true);
                    self.record_exception(
                        handler_cleanup_start,
                        finally_path,
                        finally_path,
                        self.handler_depth + 0,
                        false,
                    );
                }
                self.epilogue_span = *span;
                // **每条出口都终止**（复制件各带收尾／余部本身终止）⇒ 作用域落不到末尾 ⇒
                // 不用再补收尾（实测 `try: x = 1 except Exception as e: y = 2` 的产物末尾
                // 没有多余的那对 `LOAD_CONST None; RETURN_VALUE`）
                self.epilogue_needed = !all_terminate;
                Ok(())
            }
            Statement::Break(position) => {
                let Some(frame) = self.loops.last().cloned() else {
                    return Err(CompileError::Syntax("'break' outside loop".to_owned()));
                };
                // **块结构模型**：`break` ＝ `POP_TOP`（`for`：弹迭代器）／`NOP`（`while`）
                // ＋ **就地复制"循环之后的语句"** ＋ 作用域收尾 ⇒ 退出路径终止，不回循环尾
                if frame.is_for {
                    self.emit_at(*position, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                } else if !frame.rest.is_empty() {
                    // **`while` 里的 `break`：只有"循环之后还有代码"时才发这条 `NOP`**（第 284 轮实测）——
                    // `while a: break`（无后接）与 `while a: break else: y = 1` **都没有** NOP ✗，
                    // 而 `while a: break` 后接 `z = 2` **有** ✓（那条 NOP 是给**重放的余部**做位置标记）。
                    self.emit_at(*position, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                }
                // **复制路径在循环外**：迭代器已经被 `POP_TOP` 掉 ⇒ 复制件里的 `return` **不**该再丢
                // 迭代器 ⇒ 临时把循环帧出栈再发（实测 `for …: break` 之后的 `return x` 没有 `SWAP/POP_TOP`）
                let popped = self.loops.pop();
                let outcome = self.emit_rest_and_tail(&frame.rest, *position);
                if let Some(popped) = popped {
                    self.loops.push(popped);
                }
                let _ = outcome?;
                Ok(())
            }
            Statement::Continue(position) => {
                let Some(frame) = self.loops.last().cloned() else {
                    return Err(CompileError::Syntax(
                        "'continue' not properly in loop".to_owned(),
                    ));
                };
                // **`try/finally` 体内的 `continue`**：先把各层 finally 跑一遍再回跳。参照实测
                // （`def f(x):\n    while x:\n        try:\n            continue\n        finally:\n            y = 1\n`）：
                // `NOP; NOP; <finally 体>; JUMP_BACKWARD to L1`。
                // 只罩 `try` **体**——`else`／处理块里的出口留给后续（上一轮把 `else` 一起罩住，
                // 当场弄坏了既有语料 `try_else_finally` ✗）。
                if !self.finally_stack.is_empty() {
                    // 体末那条终止语句先留一条 `NOP` 标记（与 `return` 那条同规矩；实测
                    // `try: continue finally: y = 1` 的参照是 `NOP`(位点＝`continue`) ＋ finally ＋ 回跳）
                    self.emit_at(*position, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                    let pending: Vec<Vec<Statement>> = self.finally_stack.clone();
                    for finally_body in pending.iter().rev() {
                        self.emit_block(finally_body, false)?;
                    }
                }
                self.emit_directed_jump(
                    *position,
                    opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                    frame.continue_target,
                    true,
                );
                Ok(())
            }
            Statement::AssignChained {
                targets,
                value,
                span,
            } => {
                // 实测（`a = b = c = x`）：值先压 ⇒ **每个目标前发一条 `COPY 1`**（位点＝**整条语句** ✓），
                //   **最后一个目标不发** ✓；目标按**从左到右**存 ✓（`Name` ⇒ `STORE_NAME`／`STORE_FAST`，
                //   `obj.b` ⇒ 先 `LOAD obj` 再 `STORE_ATTR` ✓，`b[0]` ⇒ 对象＋键＋`STORE_SUBSCR` ✓）；
                //   收尾取**最后一个目标** ✓。
                self.emit_expression(value)?;
                let last_index = targets.len().saturating_sub(1);
                // **末尾两个局部目标融合**（实测 `def f(): a = b = x` ⇒ `COPY 1` ＋
                //   `STORE_FAST_STORE_FAST(1)` ✓，arg ＝ `(slot_{n-2} << 4) | slot_{n-1}` ✓）——
                //   与元组解包／推导式**同一条编码口径** ✓。
                let fused_tail: Option<(usize, usize)> = if targets.len() >= 2 {
                    match (&targets[targets.len() - 2], &targets[targets.len() - 1]) {
                        (Expression::Name(penultimate, _), Expression::Name(ultimate, _))
                            if self.kind == ScopeKind::Function
                                && self.unit.varnames.iter().any(|item| item == penultimate)
                                && self.unit.varnames.iter().any(|item| item == ultimate) =>
                        {
                            let first_slot = self.slot_of(penultimate);
                            let second_slot = self.slot_of(ultimate);
                            if first_slot <= 15 && second_slot <= 15 {
                                Some((first_slot, second_slot))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                let mut skip_index: Option<usize> = None;
                for (index, target) in targets.iter().enumerate() {
                    if Some(index) == skip_index {
                        continue;
                    }
                    if index != last_index {
                        self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 1);
                    }
                    let target_span = target.span();
                    match target {
                        Expression::Name(name, name_span) => {
                            if self.kind == ScopeKind::Function
                                && self.unit.varnames.iter().any(|item| item == name)
                            {
                                if let Some((first_slot, second_slot)) = fused_tail {
                                    if index == targets.len() - 2 && self.slot_of(name) == first_slot {
                                        self.emit_at(
                                            *name_span,
                                            opcode::opcode("STORE_FAST_STORE_FAST")
                                                .expect("STORE_FAST_STORE_FAST 在表里"),
                                            ((first_slot << 4) | second_slot) as u8,
                                        );
                                        skip_index = Some(targets.len() - 1);
                                        continue;
                                    }
                                }
                                let slot = self.slot_of(name);
                                self.emit_named(*name_span, "STORE_FAST", slot as u8);
                            } else {
                                let index = self.intern_name(name);
                                self.emit_indexed(*name_span, "STORE_NAME", index);
                            }
                        }
                        Expression::Attribute(object, name, _) => {
                            self.emit_expression(object)?;
                            let index = self.intern_name(name);
                            self.emit_indexed(target_span, "STORE_ATTR", index);
                        }
                        Expression::Subscript(object, key, _) => {
                            // **切片赋值**（第 175 轮，照实测 ✓）：两段 ⇒ `STORE_SLICE` ✓（**没有**
                            //   `BUILD_SLICE` ✓）；三段 ⇒ `BUILD_SLICE 3` ＋ `STORE_SUBSCR` ✓
                            //   —— 先前一律走 `emit_expression(key)` ✗，于是切片字面量被当独立表达式 ⇒
                            //   报「切片字面量只能出现在下标里」✗（`Lib/types.py:105` 正卡它 ✓）。
                            match &**key {
                                Expression::SliceLiteral {
                                    lower,
                                    upper,
                                    step: None,
                                    span: slice_span,
                                } => {
                                    self.emit_expression(object)?;
                                    self.emit_optional(key, lower)?;
                                    self.emit_optional(key, upper)?;
                                    self.emit_named(*slice_span, "STORE_SLICE", 0);
                                }
                                Expression::SliceLiteral {
                                    lower,
                                    upper,
                                    step: Some(step),
                                    span: slice_span,
                                } => {
                                    self.emit_expression(object)?;
                                    self.emit_optional(key, lower)?;
                                    self.emit_optional(key, upper)?;
                                    self.emit_expression(step)?;
                                    self.emit_named(*slice_span, "BUILD_SLICE", 3);
                                    self.emit_named(target_span, "STORE_SUBSCR", 0);
                                }
                                _ => {
                                    self.emit_expression(object)?;
                                    self.emit_expression(key)?;
                                    self.emit_named(target_span, "STORE_SUBSCR", 0);
                                }
                            }
                        }
                        _ => {
                            return Err(CompileError::Unsupported(
                                "链式赋值只接线了名字／属性／下标三种目标（其余如实报未接线 ✓）"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                if let Some(last_target) = targets.last() {
                    self.last_span = last_target.span();
                    self.epilogue_span = last_target.span();
                }
                Ok(())
            }
            Statement::AssignTuple {
                targets,
                value,
                target_span,
                span,
            } => {
                // 实测：值先压 ⇒ `UNPACK_SEQUENCE <个数>`（位点＝**目标那一段** ✓，如 `a, b` ✓）
                //   或 `EXTENDED_ARG ＋ UNPACK_EX (前个数 | 后个数<<8)` ✓ ⇒ 再按**源码序**存各目标
                //   （`Name` ⇒ `STORE_NAME`／`STORE_FAST`；`a[0]` ⇒ 对象＋键＋`STORE_SUBSCR`；
                //   `a.b` ⇒ 对象＋`STORE_ATTR` ✓）；收尾取**最后一个目标** ✓。
                // **等长窥孔**（实测）：值是**元组显示**且项数与目标数相同时，CPython **不**折常量、
                //   也不发 `UNPACK_SEQUENCE`，而是把各项按**源码序**压栈 ⇒ `SWAP 2`（位点＝目标段 ✓）
                //   ⇒ 再按目标序存 ✓（`a, b = 1, 2`／`a, b = b, a` 都是它 ✓）。
                let matching_tuple = match value {
                    Expression::TupleLiteral(items, _) => items.len() == targets.len(),
                    _ => false,
                };
                if matching_tuple {
                    if let Expression::TupleLiteral(items, _) = value {
                        for item in items {
                            self.emit_expression(item)?;
                        }
                    }
                    self.emit_named(*target_span, "SWAP", 2);
                } else {
                    self.emit_expression(value)?;
                }
                let starred: Vec<usize> = targets
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, star))| *star)
                    .map(|(index, _)| index)
                    .collect();
                if matching_tuple {
                    // 窥孔路径不发解包指令（值已经在栈上按目标序排好 ✓）
                } else if starred.is_empty() {
                    let count = targets.len();
                    if count > 255 {
                        return Err(CompileError::Unsupported(
                            "元组解包目标多于 255 个：`EXTENDED_ARG` 随后补（如实报未接线 ✓）".to_owned(),
                        ));
                    }
                    self.emit_named(*target_span, "UNPACK_SEQUENCE", count as u8);
                } else {
                    if starred.len() != 1 {
                        return Err(CompileError::Unsupported(
                            "元组解包只允许一个 `*` 目标（如实报未接线 ✓）".to_owned(),
                        ));
                    }
                    let before = starred[0];
                    let after = targets.len() - before - 1;
                    let argument = (before | (after << 8)) as u16;
                    if argument > 255 {
                        self.emit_named(*target_span, "EXTENDED_ARG", (argument >> 8) as u8);
                    }
                    self.emit_named(*target_span, "UNPACK_EX", (argument & 0xFF) as u8);
                }
                // **超指令融合**（实测，与推导式的元组目标同一条口径 ✓）：函数内**恰好两个**局部
                //   名字目标 ⇒ 合成 `STORE_FAST_STORE_FAST`，arg ＝ `(slot0 << 4) | slot1`，
                //   位点＝**首个目标**的跨度 ✓（`STORE_FAST k, v` ⇒ `(14,15)` ✓）。
                //   槽号要 4 位装得下（≤ 15 ✓），否则退回两条 `STORE_FAST` ✓。
                let fused_slots: Option<(usize, usize)> = if starred.is_empty() {
                    match (targets.as_slice(), self.kind) {
                        ([(Expression::Name(first, _), false), (Expression::Name(second, _), false)], ScopeKind::Function)
                            if self.unit.varnames.iter().any(|item| item == first)
                                && self.unit.varnames.iter().any(|item| item == second) =>
                        {
                            let first_slot = self.slot_of(first);
                            let second_slot = self.slot_of(second);
                            if first_slot <= 15 && second_slot <= 15 {
                                Some((first_slot, second_slot))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                for (target, _) in targets {
                    let target_span = target.span();
                    match target {
                        Expression::Name(name, name_span) => {
                            if let Some((first_slot, second_slot)) = fused_slots {
                                if self.unit.varnames.iter().any(|item| item == name)
                                    && self.slot_of(name) == second_slot
                                {
                                    // 融合已在首个目标处发出 ⇒ 第二个跳过 ✓
                                    continue;
                                }
                                if self.slot_of(name) == first_slot {
                                    self.emit_at(
                                        *name_span,
                                        opcode::opcode("STORE_FAST_STORE_FAST")
                                            .expect("STORE_FAST_STORE_FAST 在表里"),
                                        ((first_slot << 4) | second_slot) as u8,
                                    );
                                    continue;
                                }
                            }
                            if self.kind == ScopeKind::Function
                                && self.unit.varnames.iter().any(|item| item == name)
                            {
                                let slot = self.slot_of(name);
                                self.emit_named(*name_span, "STORE_FAST", slot as u8);
                            } else {
                                self.emit_store_name(*name_span, name);
                            }
                        }
                        Expression::Subscript(object, key, _) => {
                            // **切片赋值**（第 175 轮，照实测 ✓）：两段 ⇒ `STORE_SLICE` ✓（**没有**
                            //   `BUILD_SLICE` ✓）；三段 ⇒ `BUILD_SLICE 3` ＋ `STORE_SUBSCR` ✓
                            //   —— 先前一律走 `emit_expression(key)` ✗，于是切片字面量被当独立表达式 ⇒
                            //   报「切片字面量只能出现在下标里」✗（`Lib/types.py:105` 正卡它 ✓）。
                            match &**key {
                                Expression::SliceLiteral {
                                    lower,
                                    upper,
                                    step: None,
                                    span: slice_span,
                                } => {
                                    self.emit_expression(object)?;
                                    self.emit_optional(key, lower)?;
                                    self.emit_optional(key, upper)?;
                                    self.emit_named(*slice_span, "STORE_SLICE", 0);
                                }
                                Expression::SliceLiteral {
                                    lower,
                                    upper,
                                    step: Some(step),
                                    span: slice_span,
                                } => {
                                    self.emit_expression(object)?;
                                    self.emit_optional(key, lower)?;
                                    self.emit_optional(key, upper)?;
                                    self.emit_expression(step)?;
                                    self.emit_named(*slice_span, "BUILD_SLICE", 3);
                                    self.emit_named(target_span, "STORE_SUBSCR", 0);
                                }
                                _ => {
                                    self.emit_expression(object)?;
                                    self.emit_expression(key)?;
                                    self.emit_named(target_span, "STORE_SUBSCR", 0);
                                }
                            }
                        }
                        Expression::Attribute(object, name, _) => {
                            self.emit_expression(object)?;
                            let index = self.intern_name(name);
                            self.emit_indexed(target_span, "STORE_ATTR", index);
                        }
                        _ => {
                            return Err(CompileError::Unsupported(
                                "元组解包只接线了名字／属性／下标三种目标（其余如实报未接线 ✓）"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                if let Some((last, _)) = targets.last() {
                    self.last_span = last.span();
                    self.epilogue_span = last.span();
                }
                let _ = span;
                Ok(())
            }
            // `global a, b`（第 115 轮）：**不发指令** ✓，但**必须在场上登记** ✓ ——
            // 实测"分析趟收在 A 实例、发存储的是 B 实例"✗ ⇒ 只靠构造期扫描不够 ✓。
            // CPython 要求 `global` 先于该作用域里的使用（否则 `SyntaxError` ✓）⇒ 顺序天然成立 ✓。
            Statement::Global(names, _) => {
                for name in names {
                    if !self.global_names.iter().any(|item| item == name) {
                        self.global_names.push(name.clone());
                    }
                }
                Ok(())
            }
            Statement::Delete { targets, span } => {
                // 实测四种目标：`del x`（模块）⇒ `DELETE_NAME`（位点＝**名字** ✓）；
                //   函数局部 ⇒ `DELETE_FAST`（位点＝名字 ✓）；`del a[0]` ⇒ `LOAD a; <键>; DELETE_SUBSCR`
                //   （位点＝**整个目标** ✓）；`del a.b` ⇒ `LOAD a; DELETE_ATTR <名字下标>`（同上 ✓）；
                //   `del a, b` ⇒ 逐个发，**收尾取最后一个目标** ✓。
                for target in targets {
                    let target_span = target.span();
                    match target {
                        Expression::Name(name, name_span) => {
                            // **与处理器收尾同一处** ✓（第 202 轮收归统一入口 ✓）。
                            self.emit_delete_name(*name_span, name);
                        }
                        Expression::Attribute(object, name, _) => {
                            self.emit_expression(object)?;
                            // 实测：`DELETE_ATTR` 的 oparg 是**名字下标本身**（不移位 ✓）
                            let index = self.intern_name(name);
                            self.emit_named(target_span, "DELETE_ATTR", index as u8);
                        }
                        Expression::Subscript(object, key, _) => {
                            self.emit_expression(object)?;
                            // **切片键**（第 311 轮，照参照实测）：`del x[a:b]` ⇒
                            //   `LOAD a; LOAD b; BUILD_SLICE 2; DELETE_SUBSCR` ✓（带步长 ⇒ `BUILD_SLICE 3` ✓；
                            //   全常量的那一档在解析期已折成 `Constant::Slice` ⇒ 这里只处理动态界 ✓）。
                            // 先前把切片键当**普通表达式**发 ✗ ⇒ 报"切片字面量只能出现在下标里" ✗ ——
                            // `Lib/asyncio/base_events.py:173` 的 `del addrinfos_lists[0][:count - 1]`
                            // 正卡在这（那一族 **35** 个模块）✓。
                            match &**key {
                                Expression::SliceLiteral {
                                    lower,
                                    upper,
                                    step,
                                    ..
                                } => {
                                    self.emit_optional(key, lower)?;
                                    self.emit_optional(key, upper)?;
                                    match step {
                                        Some(step) => {
                                            self.emit_expression(step)?;
                                            self.emit_named(target_span, "BUILD_SLICE", 3);
                                        }
                                        None => {
                                            self.emit_named(target_span, "BUILD_SLICE", 2);
                                        }
                                    }
                                }
                                other => self.emit_expression(other)?,
                            }
                            self.emit_named(target_span, "DELETE_SUBSCR", 0);
                        }
                        _ => {
                            return Err(CompileError::Unsupported(
                                "`del` 只接线了名字／属性／下标三种目标（其余如实报未接线 ✓）"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                // 收尾取**最后一个目标**（实测 `del a, b` ⇒ `(1,1,7,8)`＝`b` ✓）
                if let Some(last) = targets.last() {
                    self.last_span = last.span();
                    self.epilogue_span = last.span();
                }
                let _ = span;
                Ok(())
            }
            Statement::Assert {
                test,
                message,
                span,
            } => {
                // 3.14 实测（`assert x`）：`LOAD x; TO_BOOL; POP_JUMP_IF_TRUE → 尾; NOT_TAKEN;
                //   LOAD_COMMON_CONSTANT 0`（AssertionError，**位点＝整条语句** ✓）`; RAISE_VARARGS 1`
                //   （位点＝**测试表达式** ✓）；带消息时中间插 `LOAD <消息>`（位点＝消息表达式 ✓）
                //   ＋ `CALL 0`（位点＝整条语句 ✓）。
                let test_span = test.span();
                self.emit_expression(test)?;
                let end = self.new_label();
                self.emit_at(test_span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
                self.emit_jump(
                    test_span,
                    opcode::opcode("POP_JUMP_IF_TRUE").expect("POP_JUMP_IF_TRUE 在表里"),
                    end,
                );
                self.emit_at(test_span, opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"), 0);
                self.emit_named(*span, "LOAD_COMMON_CONSTANT", 0);
                if let Some(message) = message {
                    // 消息那条  的位点由**表达式自己**决定（实测＝消息表达式的跨度 ✓）
                    self.emit_expression(message)?;
                    self.emit_named(*span, "CALL", 0);
                }
                self.emit_named(test_span, "RAISE_VARARGS", 1);
                self.mark_label(end);
                // **收尾取测试表达式的跨度**（实测：`assert x` 的 `LOAD_CONST None; RETURN_VALUE`
                // 是 `(1,1,7,8)` ✓，而不是模块起点 ✗）
                self.last_span = test_span;
                self.epilogue_span = test_span;
                Ok(())
            }
            Statement::Pass(position) => {
                // 不发指令：只把位置留给**收尾**（模块／函数那条隐式 return 取它的行，实测）
                self.last_span = *position;
                self.epilogue_span = *position;
                Ok(())
            }
            Statement::AugAssign {
                target,
                operator,
                value,
                span,
            } => {
                // 三种目标的栈序**逐一实测**（见 `Statement::AugAssign` 的文档）；`oparg` 由
                // **符号**从 `get_nb_ops()` 查（`BC-39`），不写死
                let symbol = operator.symbol();
                let oparg = crate::opcode::get_nb_ops()
                    .iter()
                    .position(|entry| entry.1 == symbol)
                    .unwrap_or_else(|| panic!("nb_ops 里应当有 {symbol}"))
                    as u8;
                match target {
                    AugTarget::Name(name, name_span) => {
                        if self.kind == ScopeKind::Function
                            && self.unit.varnames.iter().any(|item| item == name)
                        {
                            let slot = self.slot_of(name);
                            self.emit_at(
                                *name_span,
                                opcode::opcode("LOAD_FAST_BORROW")
                                    .expect("LOAD_FAST_BORROW 在表里"),
                                slot as u8,
                            );
                        } else {
                            let index = self.intern_name(name);
                            self.emit_indexed(*name_span, "LOAD_NAME", index);
                        }
                        self.emit_expression(value)?;
                        self.emit_at(
                            *span,
                            opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                            oparg,
                        );
                        if self.kind == ScopeKind::Function
                            && self.unit.varnames.iter().any(|item| item == name)
                        {
                            let slot = self.slot_of(name);
                            // 存入取**目标**跨度（第 228 轮按正确配对重测：`x %= 2` ⇒ `STORE_NAME`
                            // 是 `(0,1)`；`BINARY_OP` 才是整条语句 `(0,6)`）——旧注释源自错位测量
                            self.emit_at(
                                *name_span,
                                opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                slot as u8,
                            );
                        } else {
                            let index = self.intern_name(name);
                            self.emit_indexed(*name_span, "STORE_NAME", index);
                        }
                        // 收尾取**目标**（实测 `x %= 2` 的收尾是 `(0,1)`）
                        self.epilogue_span = *name_span;
                    }
                    AugTarget::Attribute {
                        object,
                        name,
                        span: target_span,
                    } => {
                        self.emit_expression(object)?;
                        self.emit_at(
                            *target_span,
                            opcode::opcode("COPY").expect("COPY 在表里"),
                            1,
                        );
                        let index = self.intern_name(name);
                        // `LOAD_ATTR` 的 oparg 低位是"取方法"标志 ⇒ 纯取值就是 `下标 << 1`（实测）
                        self.emit_at(
                            *target_span,
                            opcode::opcode("LOAD_ATTR").expect("LOAD_ATTR 在表里"),
                            (index << 1) as u8,
                        );
                        self.emit_expression(value)?;
                        self.emit_at(
                            *span,
                            opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                            oparg,
                        );
                        self.emit_at(*target_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                        self.emit_indexed(*target_span, "STORE_ATTR", index);
                        // 收尾取**目标链**那段（实测 `a.b += 2` 的收尾是 `(0,3)`）
                        self.epilogue_span = *target_span;
                    }
                    AugTarget::Subscript {
                        container,
                        key,
                        target_span,
                        span: _,
                    } => {
                        self.emit_expression(container)?;
                        self.emit_expression(key)?;
                        self.emit_at(
                            *target_span,
                            opcode::opcode("COPY").expect("COPY 在表里"),
                            2,
                        );
                        self.emit_at(
                            *target_span,
                            opcode::opcode("COPY").expect("COPY 在表里"),
                            2,
                        );
                        self.emit_binary_op_subscript(*target_span);
                        self.emit_expression(value)?;
                        self.emit_at(
                            *span,
                            opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                            oparg,
                        );
                        self.emit_at(*target_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 3);
                        self.emit_at(*target_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                        self.emit_at(
                            *target_span,
                            opcode::opcode("STORE_SUBSCR").expect("STORE_SUBSCR 在表里"),
                            0,
                        );
                        // 收尾取**目标链**那段（实测 `a[i] += 2` 的收尾是 `(0,4)`）
                        self.epilogue_span = *target_span;
                    }
                }
                Ok(())
            }
            Statement::Assign {
                target,
                target_span,
                value,
                span,
            } => {
                // 右值最外层是局部时用 `LOAD_FAST`（实测：`x = a` ⇒ `LOAD_FAST 0`，位置是那个名字的）
                let store_span;
                match (self.kind, value) {
                    (ScopeKind::Function, Expression::Name(name, name_span))
                        if self.unit.varnames.iter().any(|item| item == name) =>
                    {
                        let slot = self.slot_of(name);
                        self.emit_at(
                            *name_span,
                            opcode::opcode("LOAD_FAST").expect("LOAD_FAST 在表里"),
                            slot as u8,
                        );
                        store_span = *target_span;
                    }
                    _ => {
                        // **链式比较的失败块外提**（第 85 轮）：右值最外层是链式比较时，
                        // 失败落点在**收尾之后**（`pending_chain_copies`），成功路径直接续下去 ✓
                        let mut chained = None;
                        // **只在模块体里接管**：`annotate_unit`（注解单元）会把函数体**再编一遍**，
                        // 而它不走作用域冲刷 ⇒ 在那里排队会让标签悬空 ✗（第 87 轮实测：第二遍给
                        // `bad_left`/`bad_right`/`mixed` 又分配了标签却从不落点 ✓）。
                        if self.kind == ScopeKind::Module
                            && !self.suppress_chain_takeover
                            && matches!(value, Expression::ChainedCompare { .. })
                        {
                            let label = self.new_label();
                            self.chain_takeover =
                                Some((label, rest.to_vec(), target.clone(), *target_span));
                            chained = Some(label);
                        }
                        self.emit_expression(value)?;
                        // **只有"接管真被取走"才排队**：值可能走的是快路径（`emit_operand` 一族）
                        // ⇒ 那时标签从没落点 ⇒ 排队会在冲刷时报「标签必须已经落点」✗（第 86 轮实测）
                        let taken = self.chain_takeover.is_none();
                        let _ = self.chain_takeover.take();
                        if let Some(label) = chained.filter(|_| taken) {
                            // 第 5 项是**链式表达式**的跨度（实测复制块的 `SWAP 2; POP_TOP` 取
                            // `(1,1,4,13)` ✓，不是语句跨度 ✗）
                            self.pending_chain_copies.push((
                                label,
                                target.clone(),
                                *target_span,
                                rest.to_vec(),
                                value.span(),
                            ));
                        }
                        // **`STORE_NAME` 的位置逐形态实测**（六例吻合）：右值是**非常量**的
                        // 复合表达式（未折叠的 `+`、比较）⇒ 取整段表达式
                        // （`z = w + 2` ⇒ `(1,1,4,9)`、`x = 1 < 2` ⇒ `(1,1,4,9)`）；
                        // 其余（字面量、名字、折叠结果）⇒ 取**目标**
                        // （`x = 1` ⇒ `(0,1)`、`y = x` ⇒ `(7,8)`）
                        // 实测（第 228 轮按**正确配对**重测）：普通赋值的 `STORE_*` **一律取目标**跨度
                        // （`x = 1`／`x = a`／`x = a[1]`／`x = a.b`／`x = a + 1`／`x = a < b`／`x = f()`
                        // 全是 `(0,1)`）——此前"复合右值取整段"是**错位测量**的产物
                        store_span = *target_span;
                    }
                }
                        // 收尾两条也**一律取目标**（同上，实测 `x = a + 1` 的收尾是 `(0,1)`）
                self.epilogue_span = *target_span;
                let _ = span;
                match self.kind {
                    ScopeKind::Module | ScopeKind::Class => {
                        // **走统一入口**（第 117 轮）：`global` 声明的名字发 `STORE_GLOBAL` ✓
                        self.emit_store_name(store_span, target);
                    }
                    ScopeKind::Function => {
                        // **`global` 声明的名字走 `STORE_GLOBAL`** ✓（第 263 轮真 bug ✗）：先前这里
                        // 直接 `slot_of` ＋ `STORE_FAST` ✗ ⇒ ① 把全局名**追加成本地**（布局错位 ✗）；
                        // ② 语义错（写了局部、没写全局 ✗）。实测：`Lib/posixpath.py:301` 的
                        // `_varsubb = re.compile(…)`（`expandvars` 里有 `global _varsubb` ✓）正是它 ✓。
                        if self.global_names.iter().any(|item| item == target) {
                            self.emit_store_name(store_span, target);
                            return Ok(());
                        }
                        if let Some(slot) = self.deref_slot(target) {
                            // cell／自由变量 ⇒ `STORE_DEREF`（闭包；第 292 轮）
                            self.emit_at(
                                *target_span,
                                opcode::opcode("STORE_DEREF").expect("STORE_DEREF 在表里"),
                                slot as u8,
                            );
                            return Ok(());
                        }
                        let slot = self.slot_of(target);
                        // 实测：`STORE_FAST` 的位置**总是目标**（`x = a` ⇒ `x` 那一格）
                        self.emit_at(
                            *target_span,
                            opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                            slot as u8,
                        );
                    }
                }
                Ok(())
            }
            Statement::Expression(value, span) => {
                // **裸常量表达式语句整个丢掉** ✓（第 178 轮实证 ✓）：参照对 `...`／`1` 这种**一条都不发** ✗
                // （模块／类体／函数里都一样 ✓ —— 实测 `...` 与 `1` 的 `co_consts` 里连常量都没有 ✓）。
                // **字符串除外** ✓（它可能是文档串 ✓，由别处管 ✓）。
                if let Expression::Constant(constant, _) = value {
                    if !matches!(constant, crate::compile::Constant::Str(_)) {
                        return Ok(());
                    }
                }
                self.emit_expression(value)?;
                // 实测：表达式语句算完 `POP_TOP` 丢掉，位置是整段表达式
                self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                // 收尾两条跟这段表达式走（实测 `f()` 语句 ⇒ 收尾位置 (1,1,0,3)）
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::NonLocal(..) => {
                // **不发任何指令**（纯声明 ✓）：作用全在分析层与 `deref_slot` 的分流里
                Ok(())
            }
            Statement::Raise { value, cause, span } => {
                match (value, cause) {
                    (Some(value), cause) => {
                        self.emit_expression(value)?;
                        let count = if let Some(cause) = cause {
                            self.emit_expression(cause)?;
                            2
                        } else {
                            1
                        };
                        self.emit_named(*span, "RAISE_VARARGS", count);
                    }
                    (None, _) => self.emit_named(*span, "RAISE_VARARGS", 0),
                }
                // `raise` 不落到末尾 ⇒ 不补隐式返回；**收尾两条的位点跟本条语句**
                // （模块收尾读的就是这个字段：每条语句臂都要把它设成自己的跨度）
                self.epilogue_needed = false;
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::Return(value, span) => {
                // **循环体内的 `return`**（第 247 轮实测）：每个外层 **`for`** 的迭代器都要丢掉。
                //   值是**常量**（折成一条 `LOAD_SMALL_INT`／`LOAD_CONST`）⇒ **先** `POP_TOP`×n 再取值；
                //   其余 ⇒ 先取值，再 `SWAP 2; POP_TOP`×n（把迭代器从值下面抽走）。
                //   `while` 没有迭代器 ⇒ 不计；嵌套 `for` ⇒ 每个丢一次（实测 `SWAP 2` 的 arg 恒为 2）。
                let for_depth = self.loops.iter().filter(|frame| frame.is_for).count();
                let constant_value = fold_constant(value)?.is_some();
                // 丢弃指令的位点与 `RETURN_VALUE` **同一条规则**（实测 `for …: return 1` 的
                // `POP_TOP` 是 `(3,3,15,16)`＝字面量 `1` 那段）
                let value_span = match value {
                    Expression::Int(_, _)
                    | Expression::Str(_, _)
                    | Expression::Bytes(_, _)
                    | Expression::Constant(_, _) => value.span(),
                    _ => *span,
                };
                // `return <字面量>` 在**需要收尾机制的块体**里：字面量常量走延迟（小整数不入池）
                let saved_defer = self.defer_return_literal;
                self.defer_return_literal = self.in_epilogue_body;
                if for_depth > 0 && constant_value {
                    for _ in 0..for_depth {
                        self.emit_at(
                            value_span,
                            opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                            0,
                        );
                    }
                }
                // **处理器里返回**（第 203 轮）：值若是**字面量常量** ⇒ 参照**先** `POP_EXCEPT`
                // ＋ 名字清理 ✓，再压值 ✓（实测 `except … as exc: return 1` ⇒
                // `POP_EXCEPT; LOAD_CONST None; STORE_FAST ex; DELETE_FAST ex; LOAD_SMALL_INT 1; RETURN_VALUE`）。
                if !self.handler_stack.is_empty() && constant_value {
                    self.emit_at(value_span, opcode::opcode("POP_EXCEPT").expect("POP_EXCEPT 在表里"), 0);
                    if let Some(name) = self.handler_stack.last().cloned().flatten() {
                        let none_index = self.intern_constant(Constant::None);
                        self.emit_indexed(value_span, "LOAD_CONST", none_index);
                        self.emit_store_name(value_span, &name);
                        self.emit_delete_name(value_span, &name);
                    }
                }
                // **`with` 体内的 `return`**（实测）：值先入栈，然后**逐层**（内层先、每层按 item 逆序）
                // 发 `SWAP 2; SWAP 2` ＋ 退出调用，最后才 `RETURN_VALUE`；值是**字面量常量**时反过来——
                // 退出调用全发完再取值（`with cm: return 1` ⇒ `… CALL 3; POP_TOP; LOAD_SMALL_INT; RETURN`）。
                let with_levels = self.with_exit_stack.clone();
                // **受保护区右端＝退出调用之前** ✓（第 169／170 轮）：两种发射顺序分别记点 ✓ ——
                //   值是**常量**时（退出调用在前 ✗）**先**记 ✓；值是**表达式**时（值在前 ✗）记在**值之后** ✓
                //   （否则值那条指令会漏在区外 ✓，长度少 1 ✓）。
                if !with_levels.is_empty() && constant_value && self.with_body_end.is_none() {
                    self.with_body_end = Some(self.unit.code.len());
                }
                if with_levels.is_empty() || constant_value {
                    // 没有 `with`：照旧；有 `with` 且值是常量：值放到退出调用**之后**
                } else {
                    self.emit_expression(value)?;
                    if self.with_body_end.is_none() {
                        self.with_body_end = Some(self.unit.code.len());
                    }
                }
                // 值是**字面量**（退出调用在前、值在后）时，参照在退出调用之前还发一条 `NOP`
                // （实测 `with cm as y: if y: return 1` ⇒ `NOP` 在三条 `LOAD_CONST` 之前）
                if !with_levels.is_empty() && constant_value {
                    let nop_span = self.last_span;
                    self.emit_at(nop_span, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                }
                for level in with_levels.iter().rev() {
                    for span in level.iter().rev() {
                        // **只有值已在栈上时才要这对 `SWAP`**（字面量走"退出调用在前、值在后"⇒ 不需要）
                        if !constant_value {
                            self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 3);
                            self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                        }
                        let none_index = self.intern_constant(Constant::None);
                        self.emit_with_exit_call(*span, none_index);
                    }
                }
                // **`finally` 副本**：`try/finally` 体内的 `return` 先把 finally 跑一遍再返回
                // （参照实测形状 `NOP; NOP; <finally 体>; LOAD_SMALL_INT 1; RETURN_VALUE`）。
                // 位置由 finally 体自己的语句跨度决定（重放语句即可），顺序上排在 `with` 退出之后。
                if !self.finally_stack.is_empty() {
                    // 值是**字面量**时，参照在 finally 副本之前发一条 `NOP`，位点＝**值自己**的跨度
                    // （实测 `def f(x): try: return 1 finally: y = 2` ⇒ `NOP` 位点 `(3,3,15,16)` 即
                    // 那个 `1`；随后的 `LOAD_SMALL_INT` 位点是**粘性**的 ⇒ 属 `BC-4` 允许的传播精度面）
                    if constant_value {
                        let value_span = value.span();
                        self.emit_at(value_span, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                    }
                    let pending: Vec<Vec<Statement>> = self.finally_stack.clone();
                    for finally_body in pending.iter().rev() {
                        self.emit_block(finally_body, false)?;
                    }
                }
                if with_levels.is_empty() || constant_value {
                    self.emit_expression(value)?;
                }
                self.defer_return_literal = saved_defer;
                if for_depth > 0 && !constant_value {
                    for _ in 0..for_depth {
                        self.emit_at(value_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                        self.emit_at(
                            value_span,
                            opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                            0,
                        );
                    }
                }
                // **处理器里返回**（第 203 轮）：值是**表达式** ⇒ 值已在栈顶 ✓ ⇒
                // `SWAP 2`（把异常换上来）→ `POP_EXCEPT` → 名字清理 ✓（实测 `except … as exc: return exc`）。
                if !self.handler_stack.is_empty() && !constant_value {
                    self.emit_at(value_span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                    self.emit_at(value_span, opcode::opcode("POP_EXCEPT").expect("POP_EXCEPT 在表里"), 0);
                    if let Some(name) = self.handler_stack.last().cloned().flatten() {
                        let none_index = self.intern_constant(Constant::None);
                        self.emit_indexed(value_span, "LOAD_CONST", none_index);
                        self.emit_store_name(value_span, &name);
                        self.emit_delete_name(value_span, &name);
                    }
                }
                // `BC-23` 的 `CHECK_BOUNDARY_OUT`：**在返回值压栈之后、`RETURN_VALUE` 之前**
                // （它看的是栈顶且**不弹出**）
                if let Some(index) = self.boundary_out {
                    self.emit_at(
                        *span,
                        opcode::opcode("CHECK_BOUNDARY_OUT").expect("专有指令在表里"),
                        index as u8,
                    );
                }
                // **`RETURN_VALUE` 的位置**（第 226 轮按**正确配对**重测，规则只有一条）：
                // **只有字面量常量**取**值自身**的跨度（`return 'a'` ⇒ `(2,2,11,14)`、`return 1` ⇒
                // `(2,2,11,12)`、`return None` ⇒ `(2,2,11,15)`）；其余一切形态取**整条 `return`**
                // （`return a` ⇒ `(2,2,4,12)`、`return a + 1` ⇒ `(2,2,4,16)`、`return 200 * 300` ⇒
                // `(2,2,4,20)`、`return a.b` ⇒ `(2,2,4,14)`、`return a[0]` ⇒ `(2,2,4,15)`、
                // `return f()` ⇒ `(2,2,4,14)`、`return a, b` ⇒ `(2,2,4,15)`）。
                // 第 221 轮那两条"下标／属性取值跨度"是从**错位**的测量推出来的 ⇒ 已撤。
                // `with` 体内优先用外层 `with` 的上下文跨度（实测；其余形态照旧）
                let position = self.with_return_span.unwrap_or(value_span);
                self.emit_at(
                    position,
                    opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                    0,
                );
                Ok(())
            }
            Statement::For {
                span: for_span,
                target,
                target_span,
                tuple_targets,
                iterable,
                body,
                else_body,
                is_async,
            } => {

                // **`.0` 两处特例**（第 139／140 轮实测）：生成器表达式体的 `for … in .0` ✓
                //   ① **不发 `GET_ITER`** ✗（`.0` 本身就是迭代器 ✓，普通 `for` 才要 ✓）；
                //   ② 参照对它的读取是 **`LOAD_FAST`**（非 borrow ✗）。
                let loop_label = self.new_label();
                let exhausted = self.new_label();
                if *is_async {
                    // **`async for`**（第 306 轮）：照参照 `dis` 实测的骨架 ——
                    // `<可迭代>; GET_AITER; [循环] GET_ANEXT; LOAD_CONST None; SEND <出>; YIELD_VALUE 1;
                    //  RESUME 3; JUMP_BACKWARD_NO_INTERRUPT <循环>; <出> END_SEND; NOT_TAKEN`，
                    // 之后的目标绑定／体／回跳**与普通 `for` 走同一条路** ✓（回跳落在 `GET_ANEXT` 上 ✓）。
                    // **两条如实登记的偏差**：① 参照把 `CLEANUP_THROW`／`END_ASYNC_FOR` 接在一条**异常表**
                    // 条目上（耗尽路径 ✓）——本层**没有**发那条条目 ⇒ 异步迭代器耗尽时异常会上抛；
                    // ② `async def` 在本层本就是**生成器近似**。⇒ 这一格先求"**能编译、能 import**"
                    // （上限诊断里 `asyncio`／`contextlib` 两族共 **53** 个模块压在它上面）。
                    self.emit_expression(iterable)?;
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("GET_AITER").expect("GET_AITER 在表里"),
                        0,
                    );
                    self.mark_label(loop_label);
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("GET_ANEXT").expect("GET_ANEXT 在表里"),
                        0,
                    );
                    let none_index = self.intern_constant(Constant::None);
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                        none_index as u8,
                    );
                    // **有内联缓存的跳转要走 `emit_directed_jump`**（第 306 轮实测：
                    // `SEND` 带 1 格缓存 ✓ ⇒ 用 `emit_jump` 落点会偏 4 字节 ✗）。
                    self.emit_directed_jump(
                        iterable.span(),
                        opcode::opcode("SEND").expect("SEND 在表里"),
                        exhausted,
                        false,
                    );
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("YIELD_VALUE").expect("YIELD_VALUE 在表里"),
                        1,
                    );
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("RESUME").expect("RESUME 在表里"),
                        3,
                    );
                    // **丢掉"送进来的值"**（第 306 轮，本层近似）：`RESUME` 之后栈顶是恢复时送进来的
                    // 那个值 ✓，而下一轮的 `GET_ANEXT` 要的是**迭代器**在栈顶 ✗ ⇒ 不收走就报
                    // `'async for' requires an object with __anext__ method, got NoneType` ✗。
                    // 参照那一格由 `YIELD_VALUE` 的 oparg 语义处理 ✓（本层先如实按近似做 ✓）。
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                        0,
                    );
                    // **回跳要带方向** ✓（`emit_jump` 一律按前向算 ⇒ 实参成了 65529 ✗）。
                    self.emit_directed_jump(
                        iterable.span(),
                        opcode::opcode("JUMP_BACKWARD_NO_INTERRUPT")
                            .expect("JUMP_BACKWARD_NO_INTERRUPT 在表里"),
                        loop_label,
                        true,
                    );
                    self.mark_label(exhausted);
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("END_SEND").expect("END_SEND 在表里"),
                        0,
                    );
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                        0,
                    );
                } else {
                if matches!(iterable, Expression::Name(name, _) if name == ".0") {
                    let slot = self.slot_of(".0");
                    // 位点＝**整条 `for` 语句**（实测 `(i for i in g)` 里是生成器表达式的 `(11,25)` ✓）；
                    // 而紧随的 `FOR_ITER` 仍取 `iterable.span()` ＝原可迭代的 `(23,24)` ✓（两者不同 ✓）。
                    self.emit_at(
                        *for_span,
                        opcode::opcode("LOAD_FAST").expect("LOAD_FAST 在表里"),
                        slot as u8,
                    );
                } else {
                    self.emit_expression(iterable)?;
                    self.emit_at(
                        iterable.span(),
                        opcode::opcode("GET_ITER").expect("GET_ITER 在表里"),
                        0,
                    );
                }
                self.mark_label(loop_label);
                self.emit_jump(
                    iterable.span(),
                    opcode::opcode("FOR_ITER").expect("FOR_ITER 在表里"),
                    exhausted,
                );
                }
                // **元组目标**（第 118 轮）：先 `UNPACK_SEQUENCE <个数>`（位点＝**整段目标** ✓，
                //   实测 `n, line` ⇒ `(2,2,4,11)` ✓），再**按目标序**逐个存 ✓（走统一入口 ✓）。
                if !tuple_targets.is_empty() {
                    self.emit_named(*target_span, "UNPACK_SEQUENCE", tuple_targets.len() as u8);
                    // **两个局部目标的超指令融合**（实测 `for a, b in xs` ⇒ `STORE_FAST_STORE_FAST`
                    //   ✓，与元组解包／推导式**同一编码口径** ✓，arg ＝ `(slot0 << 4) | slot1` ✓）
                    // **超指令融合只认"两项都是名字"** ✓（第 289 轮：目标现在可嵌套 ✗ ⇒
                    // 括号那一层要走递归 UNPACK ✓，不能融成一条 STORE_FAST_STORE_FAST ✗）。
                    let fused = if let [ForTarget::Name(first, _), ForTarget::Name(second, _)] =
                        tuple_targets.as_slice()
                    {
                        if self.kind == ScopeKind::Function
                            && self.unit.varnames.iter().any(|item| item == first)
                            && self.unit.varnames.iter().any(|item| item == second)
                        {
                            let slot0 = self.slot_of(first);
                            let slot1 = self.slot_of(second);
                            if slot0 <= 15 && slot1 <= 15 {
                                Some((slot0, slot1))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let first_span = match &tuple_targets[0] {
                        ForTarget::Name(_, span) => *span,
                        ForTarget::Group(_, span) => *span,
                    };
                    if let Some((slot0, slot1)) = fused {
                        self.emit_at(
                            first_span,
                            opcode::opcode("STORE_FAST_STORE_FAST")
                                .expect("STORE_FAST_STORE_FAST 在表里"),
                            ((slot0 << 4) | slot1) as u8,
                        );
                    } else {
                        for item in tuple_targets {
                            self.emit_for_target(item)?;
                        }
                    }
                } else {
                    match self.kind {
                        ScopeKind::Module | ScopeKind::Class => {
                            // **走统一入口**（第 117 轮）：`global` 声明的名字发 `STORE_GLOBAL` ✓
                            self.emit_store_name(*target_span, target);
                        }
                        ScopeKind::Function => {
                            let slot = self.slot_of(target);
                            // **超指令融合**（第 140 轮实测：生成器体 `for i in .0: yield i` ⇒
                            //   `STORE_FAST_LOAD_FAST 17` ＝ 槽1存、槽1取 ✓）。**保守**只认
                            //   "体第一条就是 `yield <同名局部>`"这一形态 ✓，别的一律发两条 ✓。
                            // **必须同时是"生成器体"**（可迭代是 `.0` ✓）：普通函数里
                            // `for i in x: yield i` 参照**不融合** ✗（夹具当场抓到 ✓）。
                            // 体第一条**立刻读同一槽**的两种形态（第 141 轮放宽 ✓）：
                            //   ① 直接 `yield i`；② `if cond: yield i`（生成器表达式带条件那条 ✓）。
                            let yields_target = |statement: &Statement| {
                                matches!(
                                    statement,
                                    Statement::Yield(Some(Expression::Name(name, _)), _)
                                        if name == target
                                )
                            };
                            let reads_target_first = match body.first() {
                                Some(statement) => {
                                    yields_target(statement)
                                        || matches!(
                                            statement,
                                            Statement::If { then_body, else_body, .. }
                                                if else_body.is_empty()
                                                    && then_body.first().is_some_and(yields_target)
                                        )
                                }
                                None => false,
                            };
                            let fuses = matches!(iterable, Expression::Name(name, _) if name == ".0")
                                && reads_target_first;
                            if fuses {
                                self.emit_at(
                                    *target_span,
                                    opcode::opcode("STORE_FAST_LOAD_FAST")
                                        .expect("STORE_FAST_LOAD_FAST 在表里"),
                                    ((slot << 4) | slot) as u8,
                                );
                                // 体里那次读由这条抵消 ⇒ 记下来让 `Name` 发射处**免发** ✓
                                self.pending_fused_load = Some(slot);
                            } else {
                                self.emit_at(
                                    *target_span,
                                    opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                    slot as u8,
                                );
                            }
                        }
                    }
                }
                self.loops.push(LoopFrame {
                    continue_target: loop_label,
                    is_for: true,
                    rest: rest.to_vec(),
                });
                self.in_loop_body = true;
                self.emit_block(body, false)?;
                self.loops.pop();
                // 体**必然终止**时这条回跳不可达 ⇒ 参照不发（实测 `for i in s:\n    continue\n`）；
                // 末尾那条"体不落到末尾的 `if`"由 `If` 臂代发 ⇒ 这里让位
                let tail_owned_by_if = body.last().is_some_and(|statement| match statement {
                    Statement::If {
                        then_body,
                        else_body,
                        ..
                    } => else_body.is_empty() && block_terminates(then_body),
                    _ => false,
                });
                if !loop_body_terminates(body) && !tail_owned_by_if {
                    // 位置**沿用上一条指令**（参照的粘性 loc：回跳是合成指令，继承前一条的位点；
                    // 实测 `for i in s:\n    x = i\n` 的回跳与 `STORE_NAME x` 同为 `(2,2,4,5)`）
                    let back_span = self.last_span;
                    self.emit_directed_jump(
                        back_span,
                        opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                        loop_label,
                        true,
                    );
                }
                self.mark_label(exhausted);
                // 实测：耗尽后 `END_FOR` ＋ `POP_ITER`（`END_FOR` 在 `POP_ITER` 之前，不是反过来）
                self.emit_at(
                    iterable.span(),
                    opcode::opcode("END_FOR").expect("END_FOR 在表里"),
                    0,
                );
                self.emit_at(
                    iterable.span(),
                    opcode::opcode("POP_ITER").expect("POP_ITER 在表里"),
                    0,
                );
                // 实测：`for … else` 的 else 体紧接 `POP_ITER`（正常耗尽才走到这里）
                if !else_body.is_empty() {
                    self.emit_block(else_body, false)?;
                }
                // 无 `else` 时收尾取**可迭代对象**那段（实测 `for i in s:\n    x = i\n` 的收尾是 `s`
                // 的跨度）；有 `else` 时收尾跟着 else 那条路的最后一条走（实测 `…else:\n    y = 1\n`
                // 的收尾是 `y` 的跨度）⇒ 不覆盖
                if else_body.is_empty() {
                    self.epilogue_span = iterable.span();
                }
                Ok(())
            }
            Statement::While {
                span: _,
                condition,
                body,
                else_body,
            } => {
                let condition_span = condition.span();
                let start = self.new_label();
                let after = self.new_label();
                // **常量条件 `while True:`**（第 123 轮实测）：条件那串换一条 `NOP`（位点＝`True` 那条 ✓）
                //   ⇒ 体 ⇒ 回跳指到 `NOP` ✓（实测 `JUMP_BACKWARD 5` 回到 `NOP`）。只在无 `else` 时 ✓。
                if else_body.is_empty()
                    && matches!(condition, Expression::Constant(Constant::Bool(true), _))
                {
                    self.mark_label(start);
                    // **条件那条常量仍要入池**（实测参照常量池里有 bool:True ✓，即使测试被消去 ✓）
                    self.intern_constant(Constant::Bool(true));
                    self.emit_at(condition_span, opcode::opcode("NOP").expect("NOP 在表里"), 0);
                    self.loops.push(LoopFrame {
                        continue_target: start,
                        is_for: false,
                        rest: rest.to_vec(),
                    });
                    self.in_loop_body = true;
                    self.emit_block(body, false)?;
                    self.loops.pop();
                    self.emit_directed_jump(
                        self.last_span,
                        opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                        start,
                        true,
                    );
                    self.mark_label(after);
                    return Ok(());
                }
                self.mark_label(start);
                self.emit_condition_jump_to(condition, false, after)?;
                self.loops.push(LoopFrame {
                    continue_target: start,
                    is_for: false,
                    rest: rest.to_vec(),
                });
                self.in_loop_body = true;
                self.emit_block(body, false)?;
                self.loops.pop();
                let tail_owned_by_if = body.last().is_some_and(|statement| match statement {
                    Statement::If {
                        then_body,
                        else_body,
                        ..
                    } => else_body.is_empty() && block_terminates(then_body),
                    _ => false,
                });
                if !loop_body_terminates(body) && !tail_owned_by_if {
                    // 位置**沿用上一条指令**（同 `for`；实测与 `STORE_NAME x` 同为 `(2,2,4,5)`）
                    let back_span = self.last_span;
                    self.emit_directed_jump(
                        back_span,
                        opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                        start,
                        true,
                    );
                }
                self.mark_label(after);
                // 实测：`while … else` 的 else 体**紧接退出标签**（没有额外跳转）
                if !else_body.is_empty() {
                    self.emit_block(else_body, false)?;
                }
                // 无 `else` 时收尾取**条件那一段**（实测 `while a:\n    x = 1\n` 的收尾是条件的跨度）；
                // 有 `else` 时收尾跟着 else 那条路的最后一条走（实测 `while a:\n    x = 1\nelse:\n    y = 2\n`）
                if else_body.is_empty() {
                    self.epilogue_span = condition_span;
                }
                Ok(())
            }
            Statement::If {
                span: _,
                condition,
                then_body,
                else_body,
            } => {
                // `JUMP_FORWARD`（有 else 且无隐式 return 时那条）要用条件跨度
                let condition_span = condition.span();
                // **先读**"这是作用域末尾那条 `if`"的标志：发体的 `emit_block` 会把它重置
                // （`emit_block` 现在按语句自己维护该标志）⇒ 发完再读就永远是 false
                let implicit = self.if_implicit_return;
                // **For/If 联合窥孔**（第 248 轮实测）：循环体**最后一条**、无 `else`、体**不落到末尾**
                // （`return`／`break`／`continue`）⇒ 参照把条件**取反**、**回边放在不成立那条**，
                // 体直接落到末尾：`POP_JUMP_IF_TRUE → 体; NOT_TAKEN; JUMP_BACKWARD → 循环头; 体`
                // **第 141 轮放宽**：不再要求体"必然终止" ✓ —— 生成器体里 `if cond: yield i` 的体
                // 是**落到末尾**的 ✓，参照照样反转（实测 `POP_JUMP_IF_TRUE → 体; NOT_TAKEN;
                //   JUMP_BACKWARD → 循环头; 体` ✓）。
                let inverted = self.loop_last_if
                    && else_body.is_empty()
                    && self.loops.last().is_some();
                let saved_collect = self.collect_condition_exits;
                // **"块尾的条件出口各带一份收尾副本"还得块本身在尾位**（第 279 轮修 ✗）：
                // 先前只看"本条 `if` 是块里最后一条" ✗ ⇒ **嵌套**在非尾块里的 `if` 也会给出口
                // 建独立落点 ✗，而那些落点由 `flush_condition_copies` 排在**收尾之后** ✗
                // ⇒ 跳转落到 `LOAD_CONST None; RETURN_VALUE`（**空栈** ⇒ `StackUnderflow` ✗，
                //   或返回值被当成 `None` ✗）。实测原形：`Lib/importlib/_bootstrap.py` 的
                // `_spec_from_module`（`if origin is None:` 里那段 `if not origin and …` ✓）。
                self.collect_condition_exits =
                    else_body.is_empty() && rest.is_empty() && self.block_tail && !inverted;
                let skip = self.emit_condition_jump(condition, inverted)?;
                self.collect_condition_exits = saved_collect;
                // **粘性继承**：条件那串发完之后"最后一条指令"的位置（`if a:` 是 `a`、`if not a:`
                // 是 `a`（`not` 被折进跳转 ⇒ 末条是操作数））。无 `else` 的 `if` 收尾就用它（实测）
                let condition_tail = self.last_span;
                if inverted {
                    // 回边由**本臂**代发（循环臂会让位）；位点沿用上一条（合成指令的粘性）
                    let loop_target = self
                        .loops
                        .last()
                        .expect("刚判过在循环里")
                        .continue_target;
                    let back_span = self.last_span;
                    self.emit_directed_jump(
                        back_span,
                        opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                        loop_target,
                        true,
                    );
                    // **反转分支里三条合成指令的位点取"体那边"**（第 142 轮实测：参照给体那条语句的
                    //   跨度 `(2,2,15,16)` ✓，而不是条件那段 `(2,2,31,36)` ✗）⇒ 条件跳转与 `NOT_TAKEN`
                    //   是按条件跨度发的 ✓，这里把**末尾三条**（跳转／`NOT_TAKEN`／回边）改写为体的跨度 ✓。
                    // 只认我们合成的生成器体形态（`Yield(_, span)` ✓），其它形态**一律不动** ✓（保守 ✓）
                    let body_span = match then_body.first() {
                        Some(Statement::Yield(_, span)) => Some(*span),
                        _ => None,
                    };
                    if let Some(body_span) = body_span {
                        let count = self.unit.positions.len();
                        for index in count.saturating_sub(3)..count {
                            self.unit.positions[index] = (
                                Some(body_span.line_start),
                                Some(body_span.line_end),
                                Some(body_span.col_start),
                                Some(body_span.col_end),
                            );
                        }
                    }
                    self.mark_label(skip);
                    self.emit_block(then_body, false)?;
                    return Ok(());
                }
                self.clause_condition_tail = condition_tail;
                self.clause_had_else = !else_body.is_empty();
                // **分支体是不是尾块**（第 279 轮）：本条 `if` 是块里最后一条（`rest` 空 ✓）**且**
                // 本块本身在尾位 ⇒ 分支落下去就走到作用域末尾 ✓（只有这时才补隐式 `return` ✓）。
                let branch_tail = self.block_tail && rest.is_empty();
                self.emit_block(then_body, branch_tail)?;
                // **体那条路能落下来才补隐式 return**（实测 `def f(x):\n    if x:\n        return 1\n`
                // 的体里没有收尾对；`class C: def m(self, x): if x: self.a = 1` 才有）
                if implicit && !block_terminates(then_body) {
                    self.emit_implicit_return();
                }
                if else_body.is_empty() {
                    let landings = core::mem::take(&mut self.condition_landings);
                    if rest.is_empty() {
                        for landing in landings {
                            self.pending_condition_copies
                                .push((landing, Vec::new(), condition_span));
                        }
                    } else {
                        // **`rest` 非空时，落点就是"条件出口"＝块尾**（第 129 轮实测修）：
                        // 此前这些标签被 `take` 之后**静默丢弃** ✗ ⇒ 收尾回填时报
                        // "跳转目标标签从未落点" ✓（最小复现 target/probe/x1.py ✓：
                        //  嵌套 if ＋ 条件带 and ＋ 体内终止语句之后还有语句 ✓）。
                        // 把它们**就地落到当前位置**（与 `skip` 同一处）✓ —— 它们本就是这个出口 ✓。
                        for landing in landings {
                            self.mark_label(landing);
                        }
                    }
                    self.mark_label(skip);
                } else if implicit {
                    // 分支末尾有隐式 `return` ⇒ then 分支**不会**落到 else（实测：这条 `if` 不发
                    // `JUMP_FORWARD`）；两个分支都 return ⇒ 模块末尾也没有可落到的路径
                    // ⇒ 收尾那两条也**不补**（实测）
                    self.mark_label(skip);
                    // `elif` 链（`else:` 里**只有**一条 `if`）：整条链的**尾巴（末尾那对隐式 return）
                    // 由最外层补一次**——嵌套的那条 `if` 自己在 else 路径上也要补，那就会一层一层多出来
                    // （实测：n 条分支的参照收尾 pair 数 ＝ n ＋ 1）
                    let chain = else_body.len() == 1
                        && matches!(else_body.first(), Some(Statement::If { .. }));
                    let saved = self.suppress_chain_tail;
                    self.suppress_chain_tail = chain;
                    self.emit_block(else_body, branch_tail)?;
                    self.suppress_chain_tail = saved;
                    if !saved {
                        // **`elif` 链**的尾巴取**最后一个子句的条件尾**（实测 `if/elif` 的尾巴是 `elif`
                        // 那个条件）；`if/else` 的尾巴**不覆盖**（它跟着 else 那条路的最后一条走）
                        if chain && !self.clause_had_else {
                            self.last_span = self.clause_condition_tail;
                        }
                        if !block_terminates(else_body) {
                            self.emit_implicit_return();
                        }
                    }
                    // **尾巴只由一边拥有** ✓（第 199 轮修 ✗）：`else` **不终止** ⇒ 上面那条**隐式 return**
                    // 已经把尾巴发了 ✓ ⇒ 这里 `false` ✓；`else` **终止**而 `then` 不终止 ⇒ 尾巴交给
                    // **作用域收尾** ✓ ⇒ `true` ✓（参照实测：`if …: x = 1 / else: raise` 在模块末尾
                    // **发了** `LOAD_CONST None; RETURN_VALUE` ✓，位置取**最后一条语句**的跨度 ✓）。
                    self.epilogue_needed = if block_terminates(else_body) {
                        !block_terminates(then_body)
                    } else {
                        false
                    };
                } else {
                    let after = self.new_label();
                    self.emit_jump(
                        condition_span,
                        opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                        after,
                    );
                    self.mark_label(skip);
                    self.emit_block(else_body, branch_tail)?;
                    self.mark_label(after);
                }
                if else_body.is_empty() {
                    // 无 `else` 时收尾**沿用条件那串的最后一条指令**（实测 `if a:\n    x = 1\n`
                    // 的收尾是 `(1,1,3,4)`、`if not a:` 是 `(1,1,7,8)` = 那个 `a`；
                    // 有 `else` 时收尾跟着 else 那条路的最后一条走，故不覆盖）
                    self.epilogue_span = condition_tail;
                }
                Ok(())
            }
            // **属性赋值**（实测）：先压**值**，再压**对象**，然后 `STORE_ATTR <名字下标>`
            Statement::AssignSubscript {
                container,
                key,
                value,
                target_span,
                span: _,
            } => {
                // 实测顺序：**值先**，再容器、再键，最后 `STORE_SUBSCR`（与执行器的栈序一致）；
                // 位置取**目标下标**那段（`a[1] = 2` ⇒ `(0,4)`，不是整条语句）
                self.emit_expression(value)?;
                // **切片赋值**（第 175 轮，照实测 ✓）：两段 ⇒ `[值, 容器, 下界, 上界]` ＋ `STORE_SLICE` ✓
                //（**没有** `BUILD_SLICE` ✓）；三段 ⇒ `BUILD_SLICE 3` ＋ `STORE_SUBSCR` ✓。
                // 先前一律 `emit_expression(key)` ✗ ⇒ 切片字面量被当独立表达式 ⇒
                // 报「切片字面量只能出现在下标里」✗（`Lib/types.py:105` 正卡它 ✓）。
                match &key {
                    Expression::SliceLiteral {
                        lower,
                        upper,
                        step: None,
                        span: slice_span,
                    } => {
                        self.emit_expression(container)?;
                        self.emit_optional(&key, lower)?;
                        self.emit_optional(&key, upper)?;
                        self.emit_named(*slice_span, "STORE_SLICE", 0);
                    }
                    Expression::SliceLiteral {
                        lower,
                        upper,
                        step: Some(step),
                        span: slice_span,
                    } => {
                        self.emit_expression(container)?;
                        self.emit_optional(&key, lower)?;
                        self.emit_optional(&key, upper)?;
                        self.emit_expression(step)?;
                        self.emit_named(*slice_span, "BUILD_SLICE", 3);
                        self.emit_named(*target_span, "STORE_SUBSCR", 0);
                    }
                    _ => {
                        self.emit_expression(container)?;
                        self.emit_expression(key)?;
                        self.emit_named(*target_span, "STORE_SUBSCR", 0);
                    }
                }
                self.epilogue_span = *target_span;
                Ok(())
            }
            Statement::AssignAttr {
                object,
                name,
                value,
                span: _,
                target_span,
            } => {
                // **值＋对象两个局部名**打成超指令（实测 `self.b = i` ⇒
                // `LOAD_FAST_BORROW_LOAD_FAST_BORROW i, self`，先值后对象）
                let fused_pair = match (value, object) {
                    (Expression::Name(value_name, _), Expression::Name(object_name, _)) => {
                        let slots = &self.unit.varnames;
                        match (
                            slots.iter().position(|item| item == value_name),
                            slots.iter().position(|item| item == object_name),
                        ) {
                            (Some(first), Some(second)) => Some((first, second)),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                match fused_pair {
                    Some((first, second)) => {
                        self.emit_at(
                            value.span(),
                            opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                                .expect("超指令在表里"),
                            ((first << 4) | second) as u8,
                        );
                    }
                    None => {
                        self.emit_expression(value)?;
                        // 对象读用**它自己的**跨度（实测 `self.v = 5` 的 `LOAD_FAST_BORROW self`
                        // 是 `(3,3,8,12)`）
                        self.emit_expression(object)?;
                    }
                }
                let index = self.intern_name(name);
                self.emit_indexed(*target_span, "STORE_ATTR", index);
                self.epilogue_span = *target_span;
                Ok(())
            }
            // **类体**（`class C[(B)]: …`）：模块级形态逐条实测——
            // `LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <体 code>; MAKE_FUNCTION;
            //  LOAD_CONST 'C'; [每个基类一条；`LOAD_NAME` 形态见下]; CALL 2+n; STORE_NAME C`
            Statement::Class {
                name,
                span,
                first_line,
                bases,
                keywords,
                body,
                decorators,
            } => {
                // **装饰器求值**（源码序 ✓，与 `Def` 同口径 ✓）：先算出来 ✓，类建好之后再逆序套 ✓。
                let decorator_spans: Vec<Span> =
                    decorators.iter().map(|decorator| decorator.span()).collect();
                for decorator in decorators {
                    self.emit_expression(decorator)?;
                }
                // 实测：基类是用 **`LOAD_NAME`** 压栈的（不是 `LOAD_CONST`）
                // **`BC-4` 的 qualname 规则** ✓（第 202 轮真 bug 修复 ✗：先前一律把**裸名字**当
                // `co_qualname` ⇒ "函数里定义类"与参照对不上 ✓）：模块级 ⇒ `C` ✓；
                // 类体里 ⇒ `外.类` ✓；函数里 ⇒ `外.<locals>.C` ✓（**与 `Def` 同一口径** ✓）。
                let class_qualname = match self.kind {
                    ScopeKind::Module => name.clone(),
                    ScopeKind::Class => format!("{}.{name}", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
                };
                let nested = compile_class_scope(
                    name,
                    &class_qualname,
                    self.mode,
                    self.tier,
                    body,
                    *first_line,
                )?;
                let index = self.intern_constant(Constant::Code(Box::new(nested)));
                self.emit_named(*span, "LOAD_BUILD_CLASS", 0);
                self.emit_named(*span, "PUSH_NULL", 0);
                self.emit_indexed(*span, "LOAD_CONST", index);
                self.emit_named(*span, "MAKE_FUNCTION", 0);
                if keywords.is_empty() {
                    let name_const = self.intern_constant(Constant::Str(name.clone()));
                    self.emit_indexed(*span, "LOAD_CONST", name_const);
                } else {
                    // **有关键字时，类名要"最后"登记** ✓（夹具实测 `co_names` 是 `[B, M, C]` ✓ ——
                    //   类名排在**基类与关键字之后** ✓）⇒ 先占位、收尾回填 ✓（`pending` 那条路 ✓）。
                    let name_byte = self.unit.code.len() + 1;
                    self.emit_indexed(*span, "LOAD_CONST", 0);
                    self.pending
                        .push((name_byte, Constant::Str(name.clone())));
                }
                for base in bases {
                    self.emit_expression(base)?;
                }
                if keywords.is_empty() {
                    self.emit_named(*span, "CALL", (2 + bases.len()) as u8);
                // **套上装饰器**（逆序 ✓，与 `Def` 同口径 ✓）：此时类对象在 TOS ✓。
                // **纠错** ✓（第 193 轮）：我一度把它改成 `CALL 1` ✗ —— 参照实测（`dis` ✓）就是
                // **`CALL arg=0`** ✓（`… CALL arg=2 ／ CALL arg=0 ／ STORE_NAME` ✓）⇒ 发射器**原本是对的** ✓，
                // 病根在**运行期**（原生没收到那个实参 ✗）⇒ 已改回 ✓。
                for decorator_span in decorator_spans.iter().rev() {
                    self.emit_named(*decorator_span, "CALL", 0);
                }
                } else {
                    // **类关键字**（第 157 轮，实测形状）：
                    //   `LOAD_CONST 'C'; <基类…>; <关键字值…>; LOAD_CONST <名字元组>; CALL_KW <位置＋关键字>`
                    //   ⇒ 名字元组常量**最后**登记 ✓（实测 consts ＝ `[code, 'C', names]` ✓）。
                    for (_, value) in keywords {
                        self.emit_expression(value)?;
                    }
                    let names: Vec<Constant> = keywords
                        .iter()
                        .map(|(key, _)| Constant::Str(key.clone()))
                        .collect();
                    let names_index = self.intern_constant(Constant::Tuple(names));
                    self.emit_indexed(*span, "LOAD_CONST", names_index);
                    self.emit_named(
                        *span,
                        "CALL_KW",
                        (2 + bases.len() + keywords.len()) as u8,
                    );
                }
                // **走统一入口** ✓（第 202 轮真 bug 修复 ✗：先前写死 `STORE_NAME` ⇒
                // "函数里定义类"当场撞"需要命名空间帧" ✓ —— 与处理器那两处同族 ✓）。
                self.emit_store_name(*span, name);
                // 收尾两条跟整段（与 `def` 同规则，实测）
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::Def {
                name,
                decorators,
                span,
                first_line,
                parameters,
                kwonly,
                returns,
                returns_span,
                varargs,
                varkw,
                body,
            } => {
                // **装饰器**：参照实测 ⇒ 先按**源码序**把各装饰器求值压栈 ✓
                // （`@dec` ⇒ `LOAD_NAME dec` 位点＝`dec` ✓；`@a.b` ⇒ `LOAD_NAME a; LOAD_ATTR b` ✓；
                //   `@dec(1)` ⇒ `LOAD_NAME dec; PUSH_NULL; …; CALL 1` 位点＝整个装饰器 ✓）
                let decorator_spans: Vec<Span> =
                    decorators.iter().map(|decorator| decorator.span()).collect();
                for decorator in decorators {
                    self.emit_expression(decorator)?;
                }
                // **函数里嵌套 `def`**（第 278 轮接线）：无闭包时与模块／类体同一套形态——
                // `LOAD_CONST <code>; MAKE_FUNCTION; STORE_FAST`（`co_flags` 的 `CO_NESTED` 由
                // 限定名里的 `.<locals>.` 自动置位，实测 `def outer(): def inner(): …` ⇒ flags 19）。
                // **闭包**（内层引用外层局部 ⇒ 要 `cellvars`／`freevars`／`MAKE_CELL`）**尚未接线**，
                // 而且**没有拦截**：那种名字会按全局发（运行期 `NameError`，不是静默改语义，
                // 但仍是错的）——要闭包得另做一层，见待接线清单。
                // `BC-4` 的 qualname 规则（实测）：模块级 `def f` ⇒ `f`；函数**里**的定义
                // 走 `<locals>` 段（`f.<locals>.g`）；类体（编译器尚未接线）则是 `C.m`。
                // 实测：类体里的 `def m` ⇒ `C.m`；函数里的 `def g` ⇒ `f.<locals>.g`
                let nested_qualname = match self.kind {
                    ScopeKind::Module => name.clone(),
                    ScopeKind::Class => format!("{}.{name}", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
                };
                let nested = compile_scope(
                    name,
                    &nested_qualname,
                    parameters,
                    kwonly,
                    returns.as_ref(),
                    varargs.as_deref(),
                    varkw.as_deref(),
                    self.mode,
                    self.tier,
                    body,
                    ScopeKind::Function,
                    // **类体里定义** ⇒ 方法（多置 `0x8000000`）
                    self.kind == ScopeKind::Class,
                    &[],
                    Span::new(*first_line, *first_line, 0, 0),
                )?;
                // **闭包（第 292 轮）**：内层若把本作用域的局部名当成了**全局**（`co_names` 里出现
                // 本作用域的 `varnames`／`cellvars`），那它就是本作用域的自由变量 ⇒ 用带 `freevars`
                // 的参数**重编一次**内层单元（`COPY_FREE_VARS`／`LOAD_DEREF` 才发得出 ✓）。
                // （顺序取内层 `co_names` 的出现序 —— 实测参照的自由变量表就是引用序）
                let mut nested = nested;
                let mut closure_freevars: Vec<String> = Vec::new();
                if self.kind == ScopeKind::Function {
                    // **`nonlocal` 声明的名字**也是内层的自由变量（它的 `co_names` 里没有，
                    // 光靠名字交集会漏 ✓）
                    collect_nonlocals(body, &mut closure_freevars);
                    // **内层向外索取的名字**（含隔着若干层的需求 ✓）
                    for wanted in nested.demanded.clone() {
                        let from_here = self.unit.varnames.iter().any(|local| local == &wanted)
                            || self.unit.cellvars.iter().any(|cell| cell == &wanted)
                            || self.unit.freevars.iter().any(|free| free == &wanted);
                        if from_here && !closure_freevars.iter().any(|item| item == &wanted) {
                            closure_freevars.push(wanted);
                        }
                    }
                    for referenced in nested.names.clone() {
                        let from_here = self.unit.varnames.iter().any(|local| local == &referenced)
                            || self.unit.cellvars.iter().any(|cell| cell == &referenced)
                            || self.unit.freevars.iter().any(|free| free == &referenced);
                        if from_here && !closure_freevars.iter().any(|item| item == &referenced) {
                            closure_freevars.push(referenced);
                        }
                    }
                    if !closure_freevars.is_empty() {
                        nested = compile_scope(
                            name,
                            &nested_qualname,
                            parameters,
                            kwonly,
                            returns.as_ref(),
                            varargs.as_deref(),
                            varkw.as_deref(),
                            self.mode,
                            self.tier,
                            body,
                            ScopeKind::Function,
                            false,
                            &closure_freevars,
                            Span::new(*first_line, *first_line, 0, 0),
                        )?;
                    }
                }
                // **闭包元组**（实测顺序：`LOAD_FAST_BORROW <cell slot>; BUILD_TUPLE n;` 排在
                // `LOAD_CONST <code>`／`MAKE_FUNCTION` **之前**，`SET_FUNCTION_ATTRIBUTE 8` 在其**之后**）
                if !closure_freevars.is_empty() {
                    for free in &closure_freevars {
                        // 元组元素一律是 **`LOAD_FAST_BORROW <localsplus 槽>`**（本层 cell 用它的
                        // cell 槽；若来自更外层则是它自己的自由槽 = `varnames.len()+cellvars.len()+序号` ✓）。
                        // **两层闭包尚未接线** ✗：探针分析只能看到**直接**内层 `def` 的需求，
                        // 传不到隔一层的外层（实测 `def a(): x=1; def b(): def c(): return x` —— `b`
                        // 自己不引用 `x` ⇒ `a` 的探针看不到需求 ⇒ `x` 不会被移出 `varnames`、
                        // `nlocals` 差 1 ✗）。那需要真正的**符号表前向分析**（自由名逐层上浮 ✓），
                        // 不是探针能顶的 ⇒ 如实报错，不静默发错代码 ✗。
                        let slot = self
                            .deref_slot(free)
                            .expect("自由变量必在本层 cell／free 表里");
                        self.emit_named(*span, "LOAD_FAST_BORROW", slot as u8);
                    }
                    self.emit_named(*span, "BUILD_TUPLE", closure_freevars.len() as u8);
                }
                self.emit_function_object(nested, parameters, kwonly, returns.as_ref(), *returns_span, *span)?;
                if !closure_freevars.is_empty() {
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
                }
                // **逆序裹上装饰器**（`CALL 0` 的位点＝各自装饰器的跨度 ✓）：最近的那条先裹 ✓；
                // 模块体与函数体**都要**裹（第一版只写在函数分支里 ✗ ⇒ 模块级的 `def` 漏裹 ✓）
                for decorator_span in decorator_spans.iter().rev() {
                    self.emit_named(*decorator_span, "CALL", 0);
                }
                if self.kind == ScopeKind::Function {
                    // **名字是 cell／自由变量时走 `STORE_DEREF`** ✓（第 267 轮真 bug ✗）：先前这里
                    // 直接 `slot_of` ＋ `STORE_FAST` ✗ ⇒ **嵌在函数里的 `def`** 若其名字是 cell ✓
                    // （会被内层捕获 ✓），就**从没写进 cell** ✗ ⇒ 后面 `LOAD_DEREF` 读到**空 cell** ✗
                    //（实测 `Lib/os.py` 的 `_create_environ_mapping`：`def encode(...)` 是 cell ✓ ⇒
                    //  读到 `NameError: cannot access free variable 'encode'` ✗）。
                    if let Some(slot) = self.deref_slot(name) {
                        self.emit_at(
                            *span,
                            opcode::opcode("STORE_DEREF").expect("STORE_DEREF 在表里"),
                            slot as u8,
                        );
                    } else {
                        // 函数里嵌的函数存**局部**（实测 `STORE_FAST inner`）
                        let slot = self.slot_of(name);
                        self.emit_named(*span, "STORE_FAST", slot as u8);
                    }
                } else {
                    let name_index = self.intern_name(name);
                    self.emit_indexed(*span, "STORE_NAME", name_index);
                }
                // 收尾两条跟 `def` 的整段（实测：`def f(): return 1` 的五条位置都是它）
                self.epilogue_span = *span;
                Ok(())
            }
        }
    }

    /// 发 f-string 的**一段**（字面量／插值；格式规格本身也是若干段）。
    pub(super) fn emit_fstring_part(&mut self, part: &FStringPart, span: Span) -> Result<(), CompileError> {
        match part {
            FStringPart::Literal { text, span: literal_span } => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_indexed(*literal_span, "LOAD_CONST", index);
                Ok(())
            }
            FStringPart::Formatted {
                expression,
                conversion,
                spec,
                spec_span,
                span: part_span,
            } => {
                self.emit_expression(expression)?;
                if let Some(conversion) = conversion {
                    self.emit_at(
                        *part_span,
                        opcode::opcode("CONVERT_VALUE").expect("CONVERT_VALUE 在表里"),
                        *conversion,
                    );
                }
                match spec {
                    None => {
                        self.emit_at(
                            *part_span,
                            opcode::opcode("FORMAT_SIMPLE").expect("FORMAT_SIMPLE 在表里"),
                            0,
                        );
                    }
                    Some(parts) => {
                        if parts.is_empty() {
                            // **空规格**（`f"{x:}"`／`f"{x!r:}"`）—— 第 286 轮真 bug 修复 ✗：
                            // 参照这里是 `LOAD_CONST ''` ＋ `FORMAT_WITH_SPEC` ✓（逐条 `dis` 实测 ✓）；
                            // 先前**什么都不发** ✗ ⇒ 栈顶的**值本身**被当成规格 ✗ ⇒
                            // `TypeError: format spec must be a str` ✗（`f"{x=:}"` 也就走不通 ✓）。
                            let index = self.intern_constant(Constant::Str(String::new()));
                            self.emit_indexed(*part_span, "LOAD_CONST", index);
                        }
                        for part in parts {
                            self.emit_fstring_part(part, span)?;
                        }
                        if parts.len() > 1 {
                            // 规格内部的 `BUILD_STRING` 取**规格那一段**的跨度（实测）
                            self.emit_at(
                                spec_span.unwrap_or(*part_span),
                                opcode::opcode("BUILD_STRING").expect("BUILD_STRING 在表里"),
                                parts.len() as u8,
                            );
                        }
                        self.emit_at(
                            *part_span,
                            opcode::opcode("FORMAT_WITH_SPEC").expect("FORMAT_WITH_SPEC 在表里"),
                            0,
                        );
                    }
                }
                Ok(())
            }
        }
    }

    /// **造一个函数对象**（`def` 与 `lambda` 共用）：默认值元组／仅关键字默认值映射 →
    /// （有注解时先造 `__annotate__` 单元）→ `LOAD_CONST <code>` → `MAKE_FUNCTION` →
    /// `SET_FUNCTION_ATTRIBUTE`（实测挂载次序 **16 → 2 → 1**）。
    /// **不做 `STORE_*`**：`def` 之后自己存；`lambda` 把它当作表达式的值留在栈上。
    pub(super) fn emit_function_object(
        &mut self,
        nested: CompiledUnit,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        returns: Option<&Constant>,
        returns_span: Option<Span>,
        span: Span,
    ) -> Result<(), CompileError> {
                // **默认值**（实测）：`def f(a, b=x)` ⇒ 逐个求值默认值再 `BUILD_TUPLE n`，
        // 排在 `LOAD_CONST <code>` **之前**；挂载在 `MAKE_FUNCTION` 之后
        // （有注解时次序是 `SET_FUNCTION_ATTRIBUTE 16` 再 `1`）。
        let defaults: Vec<&Expression> = parameters
            .iter()
            .filter_map(|parameter| parameter.default.as_ref())
            .collect();
        if !defaults.is_empty() {
            // **字面量默认值折叠**（实测：`def f(a, b=2)` ⇒ 一条 `LOAD_CONST (2,)`，
            // 常量表里那个元组排在**最后**，位点取**体末句**）
            let literals: Option<Vec<Constant>> =
                defaults.iter().map(|expression| constant_expression(expression)).collect();
            match literals {
                Some(constants) => {
                    // 实测：参照折叠时**先把字面量本身入池**（这些槽没人引用，是折叠的
                    // 痕渍）⇒ 要照做，否则常量表对不上
                    for constant in &constants {
                        self.intern_constant(constant.clone());
                    }
                    let offset = self.unit.code.len() + 1;
                    self.emit_named(span, "LOAD_CONST", 0);
                    self.deferred.push((offset, Constant::Tuple(constants)));
                }
                None => {
                    for expression in &defaults {
                        self.emit_expression(expression)?;
                    }
                    self.emit_named(span, "BUILD_TUPLE", defaults.len() as u8);
                }
            }
        }
        // **仅关键字默认值**（实测）：`LOAD_CONST 'c'; <值>; …; BUILD_MAP n`，
        // 排在位置默认值元组之后、code 之前；**不折叠**（字面量也走 `LOAD_SMALL_INT`）
        let kwdefaults: Vec<&Parameter> = kwonly
            .iter()
            .filter(|parameter| parameter.default.is_some())
            .collect();
        if !kwdefaults.is_empty() {
            for parameter in &kwdefaults {
                let key =
                    self.intern_constant(Constant::Str(parameter.name.clone()));
                self.emit_indexed(span, "LOAD_CONST", key);
                if let Some(default) = parameter.default.as_ref() {
                    self.emit_expression(default)?;
                }
            }
            self.emit_named(span, "BUILD_MAP", kwdefaults.len() as u8);
        }
        // **PEP 649**：带注解的 `def` 先造 `__annotate__` 单元（实测：它在常量表里
        // 排在函数 code **之前**，随即 `MAKE_FUNCTION` ＋ `SET_FUNCTION_ATTRIBUTE 16`）
        let annotated = parameters
            .iter()
            .chain(kwonly.iter())
            .any(|parameter| parameter.annotation.is_some())
            || returns.is_some();
        if annotated {
            let annotate_qualname = if self.qualname == "<module>" {
                "__annotate__".to_owned()
            } else {
                format!("{}.<locals>.__annotate__", self.qualname)
            };
            let unit = self.annotate_unit(
                &annotate_qualname,
                parameters,
                kwonly,
                returns,
                returns_span,
                span,
            );
            let annotate_index = self.intern_constant(Constant::Code(Box::new(unit)));
            self.emit_indexed(span, "LOAD_CONST", annotate_index);
            self.emit_named(span, "MAKE_FUNCTION", 0);
        }
        let index = self.intern_constant(Constant::Code(Box::new(nested)));
        // 实测：`def` 的三条指令（＋收尾）位置都是**整个 `def` 语句**
        self.emit_indexed(span, "LOAD_CONST", index);
        // 3.14 的 `MAKE_FUNCTION` **没有 oparg**（`dis` 显示 `arg=None`）
        self.emit_named(span, "MAKE_FUNCTION", 0);
        if annotated {
            // bit4 `annotate`（`SPEC-bytecode.md` 的属性位表）
            self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 16);
        }
        if !kwdefaults.is_empty() {
            // bit1 `kwdefaults`；实测的挂载次序是 **16 → 2 → 1**
            self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 2);
        }
        if !defaults.is_empty() {
            // bit0 `defaults`（同一张位表）
            self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 1);
        }
        Ok(())
    }

    /// 造一个 **`__annotate__` 单元**（PEP 649 的 3.14 形态，逐条实测）。
    ///
    /// 形态：`format` 参数的守卫（`format > 2` ⇒ `NotImplementedError`）＋ 注解字典
    /// （键＝形参名，最后 `'return'`；值＝注解表达式）＋ `RETURN_VALUE`；
    /// `argcount = 1`、`varnames = ('format',)`、`flags = 0x3`。
    pub(super) fn annotate_unit(
        &self,
        qualname: &str,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        returns: Option<&Constant>,
        returns_span: Option<Span>,
        span: Span,
    ) -> CompiledUnit {
        let mut emitter = Emitter {
            block_depth: 0,
            handler_depth: 0,
        comprehension_locals: Vec::new(),
        pending_cleanups: Vec::new(),
        pending_fused_load: None,
            mode: self.mode,
            tier: self.tier,
            qualname: qualname.to_owned(),
            global_names: Vec::new(),
            wide_jumps: Vec::new(),
            boundary_out: None,
            deferred: Vec::new(),
            pending: Vec::new(),
            jumps: Vec::new(),
            labels: Vec::new(),
            if_implicit_return: false,
            block_tail: false,
            in_loop_body: false,
            loop_last_if: false,
            with_return_span: None,
            in_epilogue_body: false,
            defer_return_literal: false,
            with_exit_stack: Vec::new(),
            with_body_end: None,
            finally_stack: Vec::new(),
            handler_stack: Vec::new(),
            condition_landings: Vec::new(),
            collect_condition_exits: false,
            pending_condition_copies: Vec::new(),
            pending_chain_copies: Vec::new(),
            chain_takeover: None,
            suppress_chain_takeover: true,
            suppress_chain_tail: false,
            loops: Vec::new(),
            block_end_labels: Vec::new(),
            exception_entries: Vec::new(),
            handler_segments: Vec::new(),
            clause_condition_tail: Span::synthetic(),
            clause_had_else: false,
            boolop_scaffold_span: None,
            in_condition: false,
            epilogue_needed: false,
            epilogue_span: span,
            last_span: span,
            kind: ScopeKind::Function,
            unit: CompiledUnit {
                name: "__annotate__".to_owned(),
                qualname: qualname.to_owned(),
                argcount: 1,
                posonlyargcount: 0,
                kwonlyargcount: 0,
                nlocals: 1,
                flags: 0x3,
                names: Vec::new(),
                varnames: vec!["format".to_owned()],
                cellvars: Vec::new(),
                freevars: Vec::new(),
                demanded: Vec::new(),
                constants: Vec::new(),
                code: Vec::new(),
                positions: Vec::new(),
                exceptiontable: Vec::new(),
            },
        };
        // 实测：`__annotate__` 的 `RESUME` 取**合成**位点（`(def 行, def 行, 0, 0)`）
        emitter.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        emitter.emit_named(span, "LOAD_FAST_BORROW", 0);
        // 实测：守卫的 `2` **也**在常量表里（下标 0），尽管指令用的是 `LOAD_SMALL_INT`
        emitter.intern_constant(Constant::Int(2));
        emitter.emit_named(span, "LOAD_SMALL_INT", 2);
        // 实测：`COMPARE_OP 132` 就是 `>`（比较下标 << 5 ｜ 提示位）
        emitter.emit_named(span, "COMPARE_OP", 132);
        let end = emitter.new_label();
        emitter.emit_jump(
            span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            end,
        );
        emitter.emit_named(span, "NOT_TAKEN", 0);
        emitter.emit_named(span, "LOAD_COMMON_CONSTANT", 1);
        emitter.emit_named(span, "RAISE_VARARGS", 1);
        emitter.mark_label(end);
        let mut count = 0usize;
        // 注解字典按**声明次序**：位置参数 → 仅关键字 → `'return'`（实测）
        for parameter in parameters.iter().chain(kwonly.iter()) {
            let Some(annotation) = parameter.annotation.as_ref() else {
                continue;
            };
            let key = emitter.intern_constant(Constant::Str(parameter.name.clone()));
            emitter.emit_named(span, "LOAD_CONST", key as u8);
            // 注解表达式取**注解自身**的跨度（实测 `def f(a: int):` 的 `LOAD_GLOBAL` 是 `(1,1,9,12)`）
            let annotation_span = parameter.annotation_span.unwrap_or(span);
            emitter.emit_annotation_expression(annotation, annotation_span);
            count += 1;
        }
        if let Some(annotation) = returns {
            let key = emitter.intern_constant(Constant::Str("return".to_owned()));
            emitter.emit_named(span, "LOAD_CONST", key as u8);
            // 同上：返回注解取注解自身的跨度（实测 `def f() -> int:` 的 `LOAD_GLOBAL` 是 `(1,1,11,14)`）
            emitter.emit_annotation_expression(annotation, returns_span.unwrap_or(span));
            count += 1;
        }
        emitter.emit_named(span, "BUILD_MAP", count as u8);
        emitter.emit_named(span, "RETURN_VALUE", 0);
        // __annotate__ 单元不会有待发的条件副本
        emitter.flush_jumps();
        emitter.unit
    }

    /// **`match` 模式的判定**（第 290 轮）：命中 ⇒ 落到 `matched` ✓；不命中 ⇒ 跳 `next_case` ✓；
    /// **主语留在栈上** ✓（字面量那一档用 `COPY 1` 复制一份比 ✓，照参照 `dis` 实测 ✓）。
    fn emit_pattern_test(
        &mut self,
        pattern: &Pattern,
        next_case: usize,
        matched: usize,
    ) -> Result<(), CompileError> {
        match pattern {
            Pattern::Literal(constant, span) => {
                self.emit_named(*span, "COPY", 1);
                let index = self.intern_constant(constant.clone());
                self.emit_indexed(*span, "LOAD_CONST", index);
                // 参照实测：`None`／`True`／`False` 走 `IS_OP 0` ✓，其余走 `COMPARE_OP 88`（`bool(==)` ✓）
                if matches!(constant, Constant::None | Constant::Bool(_)) {
                    self.emit_named(*span, "IS_OP", 0);
                } else {
                    self.emit_named(*span, "COMPARE_OP", 88);
                }
                self.emit_jump(*span, opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"), next_case);
                self.emit_named(*span, "NOT_TAKEN", 0);
                Ok(())
            }
            // **值模式**（第 300 轮）：与字面量模式**同形**（逐条 `dis` 实测）。
            Pattern::Value(expression, span) => {
                self.emit_named(*span, "COPY", 1);
                self.emit_expression(expression)?;
                self.emit_named(*span, "COMPARE_OP", 88);
                self.emit_jump(
                    *span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
                    next_case,
                );
                self.emit_named(*span, "NOT_TAKEN", 0);
                Ok(())
            }
            // **类模式**（第 300 轮那档；第 301 轮补上**子模式**）：形状照参照 `dis` 实测走，
            // 进入时**栈顶就是待测的值**（主语或某个子值），出来时它被**消费干净** ✓。
            Pattern::Class { .. } => {
                // **顶层的待测值就是主语** ✓ ⇒ `false`：不在这里收走它 ✓（这一条 `case`
                // "命中之后"那段会 `POP_TOP` 主语 ✓ —— 这里再收一次就是**双重弹栈** ⇒ `StackUnderflow` ✗）。
                self.emit_class_pattern(pattern, next_case, matched, false)?;
                Ok(())
            }
            // 捕获与通配**一定命中** ✓（没有判定指令 ✓）
            Pattern::Capture(_, _) | Pattern::Wildcard(_) => Ok(()),
            Pattern::Or(alternatives, span) => {
                let count = alternatives.len();
                for (index, alternative) in alternatives.iter().enumerate() {
                    // 最后一个备选的不命中目标是**下一条 `case`** ✓；前面几个先落到本地失败标签 ✓
                    let failure = if index + 1 == count {
                        next_case
                    } else {
                        self.new_label()
                    };
                    self.emit_pattern_test(alternative, failure, matched)?;
                    if index + 1 < count {
                        // 这个备选命中 ⇒ 直接去 `matched` ✓
                        self.emit_jump(*span, opcode::opcode("JUMP_FORWARD").expect("表里有"), matched);
                        self.emit_named(*span, "NOT_TAKEN", 0);
                        self.mark_label(failure);
                    }
                }
                Ok(())
            }
        }
    }

    /// **类模式的判定**（第 300／301 轮）：进入时栈顶是待测值，**出来时它被消费干净** ✓；
    /// 命中走 `on_match`、不命中走 `failure` ✓（两条路的栈都已清平 ✓）。
    ///
    /// 形状照参照 `dis` 实测：
    /// `COPY 1; <类>; LOAD_CONST <关键字名元组>; MATCH_CLASS <位置个数>; COPY 1;
    ///  POP_JUMP_IF_NONE <不命中>; NOT_TAKEN; UNPACK_SEQUENCE <总数>`，随后逐个子模式判定 ✓。
    /// 子模式的失败点**各自就地清理**（不另设共享清理块 —— 语义同参照 ✓，形状如实登记为不同 ✓）。
    fn emit_class_pattern(
        &mut self,
        pattern: &Pattern,
        failure: usize,
        on_match: usize,
        consume_value: bool,
    ) -> Result<(), CompileError> {
        let Pattern::Class {
            class,
            positional,
            keywords,
            span,
        } = pattern
        else {
            unreachable!("只给类模式调")
        };
        let total = positional.len() + keywords.len();
        self.emit_named(*span, "COPY", 1);
        self.emit_expression(class)?;
        let names: Vec<Constant> = keywords
            .iter()
            .map(|(name, _)| Constant::Str(name.clone()))
            .collect();
        let names_index = self.intern_constant(Constant::Tuple(names));
        self.emit_indexed(*span, "LOAD_CONST", names_index);
        self.emit_named(*span, "MATCH_CLASS", positional.len() as u8);
        self.emit_named(*span, "COPY", 1);
        let none_path = self.new_label();
        self.emit_jump(
            *span,
            opcode::opcode("POP_JUMP_IF_NONE").expect("表里有"),
            none_path,
        );
        self.emit_named(*span, "NOT_TAKEN", 0);
        if total > 0 {
            self.emit_named(*span, "UNPACK_SEQUENCE", total as u8);
        }
        // 逐个测：第 k 个子模式失败时，栈上还剩 `total - k - 1` 个已展开值，外加**待测值本身** ✓
        let mut sub_patterns: Vec<&Pattern> = positional.iter().collect();
        sub_patterns.extend(keywords.iter().map(|(_, sub)| sub));
        for (index, sub) in sub_patterns.iter().enumerate() {
            let remaining = total - index - 1;
            let failed = self.new_label();
            let next = self.new_label();
            self.emit_subpattern_test(sub, failed, next)?;
            self.emit_jump(*span, opcode::opcode("JUMP_FORWARD").expect("表里有"), next);
            self.mark_label(failed);
            // 失败现场：已展开的 `remaining` 个值 ✓ ＋ **只有嵌套**才连待测值一起收 ✓
            //（顶层那份待测值就是**主语** ⇒ 下一条 `case` 要用它 ✗ —— 多收一格就是 `StackUnderflow` ✗）。
            let pops = remaining + usize::from(consume_value);
            for _ in 0..pops {
                self.emit_named(*span, "POP_TOP", 0);
            }
            self.emit_jump(
                *span,
                opcode::opcode("JUMP_FORWARD").expect("表里有"),
                failure,
            );
            self.mark_label(next);
        }
        // 全过 ⇒ 待测值还在栈上：**嵌套子模式**要把它收走 ✓（`consume_value`）；
        // **顶层**那份是主语、留给这条 `case` 的"命中之后"收 ✓。
        if consume_value {
            self.emit_named(*span, "POP_TOP", 0);
        }
        self.emit_jump(*span, opcode::opcode("JUMP_FORWARD").expect("表里有"), on_match);
        // **`MATCH_CLASS` 说不命中**：`POP_JUMP_IF_NONE` 已经弹掉一格 ✓ ⇒ 这里**只**弹剩下那个
        // `None` ✓；**待测值要留着** ✗（顶层的待测值就是主语 ⇒ 下一条 `case` 要用它 ✓；
        // 嵌套时由**外层**的失败清理连它一起收 ✓）。第 301 轮在这里多弹了一格 ⇒ `StackUnderflow` ✗。
        self.mark_label(none_path);
        self.emit_named(*span, "POP_TOP", 0);
        // **嵌套**时待测值（子值）也要在这里收掉 ✓：它的失败出口直接去**外层**的清理 ✓，
        // 而外层只按"已展开的剩余值"计数 ✗（把子值留成残渣 ⇒ 下一条 `case` 的栈被污染 ✗ ——
        // 实测症状：`case Box():` 对 `Box(9)` 明明该命中却落到 `_` ✗）。
        if consume_value {
            self.emit_named(*span, "POP_TOP", 0);
        }
        self.emit_jump(
            *span,
            opcode::opcode("JUMP_FORWARD").expect("表里有"),
            failure,
        );
        Ok(())
    }

    /// **类模式里的一个子模式**（第 301 轮）：栈顶是它的值，**测完消费掉** ✓（不留残渣 ✓）；
    /// 命中走 `on_match`、不命中走 `failure` ✓。
    fn emit_subpattern_test(
        &mut self,
        pattern: &Pattern,
        failure: usize,
        on_match: usize,
    ) -> Result<(), CompileError> {
        match pattern {
            // 字面量／值：**把栈顶那个值直接比掉**（参照实测：子模式这里**没有** `COPY 1` ✓）
            Pattern::Literal(constant, span) => {
                let index = self.intern_constant(constant.clone());
                self.emit_indexed(*span, "LOAD_CONST", index);
                if matches!(constant, Constant::None | Constant::Bool(_)) {
                    self.emit_named(*span, "IS_OP", 0);
                } else {
                    self.emit_named(*span, "COMPARE_OP", 88);
                }
                self.emit_jump(
                    *span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
                    failure,
                );
                self.emit_named(*span, "NOT_TAKEN", 0);
                Ok(())
            }
            Pattern::Value(expression, span) => {
                self.emit_expression(expression)?;
                self.emit_named(*span, "COMPARE_OP", 88);
                self.emit_jump(
                    *span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
                    failure,
                );
                self.emit_named(*span, "NOT_TAKEN", 0);
                Ok(())
            }
            // 捕获：`STORE` 直接吃掉它 ✓（一定命中）；通配：`POP_TOP` ✓
            Pattern::Capture(name, span) => {
                self.emit_store_name(*span, name);
                Ok(())
            }
            Pattern::Wildcard(span) => {
                self.emit_named(*span, "POP_TOP", 0);
                Ok(())
            }
            // 嵌套类模式 ✓（`case ast.Return(value=ast.Name())` 就是它）
            Pattern::Class { .. } => self.emit_class_pattern(pattern, failure, on_match, true),
            Pattern::Or(_, span) => Err(CompileError::Unsupported(format!(
                "`match` 子模式里的或模式尚未接线（第 {} 行）",
                span.line_start
            ))),
        }
    }

    /// **`for` 目标的一项**（第 289 轮）：名字 ⇒ 按作用域存 ✓；括号元组 ⇒ 先 `UNPACK_SEQUENCE 个数`
    /// （位点＝**那一层**的跨度 ✓，`dis` 实测）再递归 ✓。
    fn emit_for_target(&mut self, target: &ForTarget) -> Result<(), CompileError> {
        match target {
            ForTarget::Name(name, span) => {
                self.emit_store_name(*span, name);
            }
            ForTarget::Group(items, span) => {
                self.emit_named(*span, "UNPACK_SEQUENCE", items.len() as u8);
                for item in items {
                    self.emit_for_target(item)?;
                }
            }
        }
        Ok(())
    }

    /// 注解**表达式**的发射（实测）：类型名走 `LOAD_GLOBAL`（oparg ＝ `名字下标 << 1`）；
    /// `None` ⇒ `LOAD_CONST None`；`X[...]` ⇒ 先外后内再 `BINARY_OP 26`（`[]`）。
    pub(super) fn emit_annotation_expression(&mut self, annotation: &Constant, span: Span) {
        match annotation {
            Constant::Type(name) if name == "NoneType" => {
                let index = self.intern_constant(Constant::None);
                self.emit_indexed(span, "LOAD_CONST", index);
            }
            Constant::Type(name) | Constant::Str(name) => {
                let index = self.intern_name(name);
                self.emit_named(span, "LOAD_GLOBAL", (index << 1) as u8);
            }
            Constant::Tuple(parts) => {
                for part in parts {
                    self.emit_annotation_expression(part, span);
                }
                // 实测：`BINARY_OP 26` 的 argrepr 是 `[]`
                self.emit_named(span, "BINARY_OP", 26);
            }
            // **`名字[实参…]`** ✓（第 287 轮，`dis` 逐条实测）：一个实参**不**发 `BUILD_TUPLE` ✓
            //（`list[int]` ⇒ `LOAD_GLOBAL list; LOAD_GLOBAL int; BINARY_OP 26` ✓）、
            // 两个及以上 ⇒ `BUILD_TUPLE n` ✓（`dict[str, object]` ✓）。
            Constant::AnnSubscript { base, arguments } => {
                self.emit_annotation_expression(base, span);
                for argument in arguments {
                    self.emit_annotation_expression(argument, span);
                }
                if arguments.len() > 1 {
                    self.emit_named(span, "BUILD_TUPLE", arguments.len() as u8);
                }
                self.emit_named(span, "BINARY_OP", 26);
            }
            // **注解里的 `|`** ✓（实测 `int | None` ⇒ `…; BINARY_OP 7` ✓）
            Constant::AnnUnion { left, right } => {
                self.emit_annotation_expression(left, span);
                self.emit_annotation_expression(right, span);
                self.emit_named(span, "BINARY_OP", 7);
            }
            // **注解里的列表** ✓（`Callable[[int, str], None]` 的头一个实参 ⇒ `BUILD_LIST n` ✓）
            Constant::AnnList(items) => {
                for item in items {
                    self.emit_annotation_expression(item, span);
                }
                self.emit_named(span, "BUILD_LIST", items.len() as u8);
            }
            // **点号** ✓（参照发 `LOAD_ATTR`，oparg ＝ 名字下标 `<< 1` ✓，`dis` 实测 ✓）
            Constant::AnnAttribute { base, name } => {
                self.emit_annotation_expression(base, span);
                let index = self.intern_name(name);
                self.emit_named(span, "LOAD_ATTR", (index << 1) as u8);
            }
            // **前向引用的字符串** ✓（参照发 `LOAD_CONST 'X'` ✓）
            Constant::AnnString(text) => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_indexed(span, "LOAD_CONST", index);
            }
            // **`...`** ✓（参照发 `LOAD_CONST Ellipsis` ✓）
            Constant::Ellipsis => {
                let index = self.intern_constant(Constant::Ellipsis);
                self.emit_indexed(span, "LOAD_CONST", index);
            }
            _ => {}
        }
    }

    /// **重放"余部 ＋ 作用域收尾"**（块结构模型的退出路径）：余部不终止时接收尾；
    /// 都没有 ⇒ 跳到本层块尾（不落进后面的块）。
    pub(super) fn emit_rest_and_tail(&mut self, rest: &[Statement], position: Span) -> Result<bool, CompileError> {
        self.emit_block(rest, false)?;
        if block_terminates(rest) {
            return Ok(true);
        }
        if self.emit_scope_tail(self.last_span) {
            return Ok(true);
        }
        if let Some(end) = self.block_end_labels.last().copied() {
            self.emit_jump(
                position,
                opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                end,
            );
        }
        Ok(false)
    }

    /// **作用域收尾**（模块／函数的隐式 `LOAD_CONST None; RETURN_VALUE`）：正常路径在
    /// `compile_scope` 末尾发一次；`break`／`try` 的退出路径**也各发一次**（块结构模型）。
    /// 返回"是否真的发了"（没发就该由调用方跳到块尾）。
    pub(super) fn emit_scope_tail(&mut self, span: Span) -> bool {
        if !self.epilogue_needed {
            return false;
        }
        match self.kind {
            ScopeKind::Module | ScopeKind::Function => {
                // `None` 已登记就直接用；否则**延迟入池**（参照在作用域末尾才登记它，
                // 提前登记会把后面才登记的小整数挤到后面；实测
                // `for i in s:\n    break\nelse:\n    y = 1\n` ⇒ `('int:1', None)`）
                let none_index = match self
                    .unit
                    .constants
                    .iter()
                    .position(|item| *item == Constant::None)
                {
                    Some(index) => index,
                    None => {
                        let argument_byte = self.unit.code.len() + 1;
                        self.emit_at(span, opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"), 0);
                        self.pending.push((argument_byte, Constant::None));
                        self.emit_at(
                            span,
                            opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                            0,
                        );
                        return true;
                    }
                };
                self.emit_indexed(span, "LOAD_CONST", none_index);
                self.emit_at(
                    span,
                    opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                    0,
                );
                true
            }
            // 类体的收尾是"挂 `__static_attributes__` 等"那几条，形状不同 ⇒ 暂不重放
            ScopeKind::Class => false,
        }
    }

    /// 发一条**隐式** `LOAD_CONST None; RETURN_VALUE`（位置取最后一条真指令的）。
    pub(super) fn emit_implicit_return(&mut self) {
        let index = self.intern_constant(Constant::None);
        let position = self.last_span;
        self.emit_indexed(position, "LOAD_CONST", index);
        self.emit_at(
            position,
            opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
            0,
        );
    }

    /// 发一段语句；`implicit_return` 为真时，若最后一条是 `if`，它的**每个分支**末尾
    /// 各补一条 `LOAD_CONST None; RETURN_VALUE`（实测：末尾的 `if` 会这样，非末尾的不会）。
    /// 推导式里"紧接着会被读的那个局部槽"（用来决定要不要打成 `STORE_FAST_LOAD_FAST`）：
    /// 还有下一层生成器时看**下一层可迭代表达式**的最左名字（实测多重 `for` 的外层因此**不**融合），
    /// 否则看本层的 `if` 子句、再看元素；名字没有快速槽（全局）就不融合。
    pub(super) fn next_read_slot(
        &self,
        element: &Expression,
        generators: &[Generator],
        index: usize,
    ) -> Option<usize> {
        let candidate = if index + 1 < generators.len() {
            leftmost_name(&generators[index + 1].iterable)
        } else {
            generators[index]
                .conditions
                .first()
                .and_then(leftmost_name)
                .or_else(|| leftmost_name(element))
        }?;
        self.unit
            .varnames
            .iter()
            .position(|item| item == candidate)
    }

    /// 发推导式的**元素**：列表／集合是一条表达式；字典是"键 ＋ 值"，两边最左都是局部名时打成
    /// 超指令 `LOAD_FAST_BORROW_LOAD_FAST_BORROW <键槽, 值最左槽>`（实测 `{k: k + 1 …}` 就是这个形状），
    /// 值的最左那次读取由它抵消。
    pub(super) fn emit_comprehension_element(
        &mut self,
        kind: ComprehensionKind,
        element: &Expression,
        value: Option<&Expression>,
    ) -> Result<(), CompileError> {
        if kind != ComprehensionKind::Dict {
            return self.emit_expression(element);
        }
        let value = value.ok_or_else(|| {
            CompileError::Unsupported("字典推导式缺了值那一半".to_owned())
        })?;
        let key_slot = leftmost_name(element)
            .and_then(|name| self.unit.varnames.iter().position(|item| item == name));
        let value_slot = leftmost_name(value)
            .and_then(|name| self.unit.varnames.iter().position(|item| item == name));
        // **键已经由 `STORE_FAST_LOAD_FAST` 压回来了**（无 `if` 子句时就是这种情况）⇒ 它就是键，
        // 别再为键发一次读（否则栈上会多一份、`MAP_ADD` 取错位置）
        if let (Some(pending), Some(key_slot)) = (self.pending_fused_load, key_slot) {
            if pending == key_slot {
                self.pending_fused_load = None;
                return self.emit_expression(value);
            }
        }
        if let (Some(key_slot), Some(value_slot)) = (key_slot, value_slot) {
            self.emit_at(
                element.span(),
                opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                    .expect("超指令在表里"),
                ((key_slot << 4) | value_slot) as u8,
            );
            self.pending_fused_load = Some(value_slot);
            return self.emit_expression(value);
        }
        self.emit_expression(element)?;
        self.emit_expression(value)
    }

    /// 把待外提的推导式清理块发出来（`emit_block` 收尾时调用）。
    pub(super) fn flush_pending_cleanups(&mut self) -> Result<(), CompileError> {
        for cleanup in core::mem::take(&mut self.pending_cleanups) {
            let span = cleanup.scaffold;
            self.mark_label(cleanup.label);
            let target = self.unit.code.len();
            // 前两条是**合成指令**（参照给全 `None`），后面的还原序列有位点（`BC-4` 扩）
            self.emit_named_none("SWAP", 2);
            self.emit_named_none("POP_TOP", 0);
            self.emit_at(span, opcode::opcode("SWAP").expect("SWAP 在表里"), cleanup.depth);
            for slot in cleanup.slots.iter().rev() {
                self.emit_at(
                    span,
                    opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                    *slot as u8,
                );
            }
            self.emit_at(span, opcode::opcode("RERAISE").expect("RERAISE 在表里"), 0);
            self.record_exception(cleanup.region_start, cleanup.region_end, target, self.handler_depth + 2, false);
        }
        Ok(())
    }

    pub(super) fn emit_block(
        &mut self,
        statements: &[Statement],
        tail: bool,
    ) -> Result<(), CompileError> {
        // **块深度** ✓：深度 1 ＝ 作用域自己的体 ✓（第 202 轮修 ✗）。
        self.block_depth += 1;
        // **尾块标记**（第 279 轮）：只对**本块**有效 ⇒ 进块时换、出块时还原 ✓
        let outer_tail = self.block_tail;
        self.block_tail = tail;
        // **循环体**标记只对紧随其后的这一次 `emit_block` 生效（嵌套块不会再看到）
        let in_loop_body = self.in_loop_body;
        self.in_loop_body = false;
        // 本块的"块尾"标签：`break`／`try` 的退出路径重放余部后若**不终止**，要跳到块尾
        let block_end = self.new_label();
        self.block_end_labels.push(block_end);
        let last_index = statements.len().saturating_sub(1);
        for (index, statement) in statements.iter().enumerate() {
            let rest = &statements[index + 1..];
            // 末尾那条 `if` 的分支要补隐式 return（实测；逻辑原在 `compile_scope` 的循环里）。
            // **模块与函数都算**——`class C:\n    def m(self, x):\n        if x:\n            self.a = 1\n`
            // 的参照产物在体的出口也补了一对 `LOAD_CONST None; RETURN_VALUE`（第 243 轮暴露）。
            // 类体**没有**隐式 return ⇒ 不算。
            // **还必须是尾块**（第 279 轮修 ✗）：块里最后一条 `if` 后面若还有代码（那就是外层
            // 块的事 ✓），补出来的 `LOAD_CONST None; RETURN_VALUE` 会**提前返回** ✗ ——
            // 见 `block_tail` 的文档 ✓。
            self.if_implicit_return = matches!(self.kind, ScopeKind::Module | ScopeKind::Function)
                && self.block_tail
                && index == last_index
                && matches!(statement, Statement::If { .. });
            // 循环体**最后一条**、且是**无 `else` 的 `if`** ⇒ 窥孔候选（`If` 臂自己读）
            self.loop_last_if = in_loop_body
                && index == last_index
                && matches!(statement, Statement::If { else_body, .. } if else_body.is_empty());
            self.emit_statement(statement, rest)?;
            self.if_implicit_return = false;
            self.loop_last_if = false;
            // **死代码**：无条件终止语句之后的同块语句参照**不发射**（实测
            // `for i in s:\n    break\n    x = 1\n` 的产物里没有 `x = 1`）
            if matches!(
                statement,
                Statement::Break(_)
                    | Statement::Continue(_)
                    | Statement::Return(_, _)
                    | Statement::Raise { .. }
                    // **`try` 也一样**：它的**每条**出口（套体、各处理块）都已经重放了余部
                    // ＋ 收尾 ⇒ 外层块不能再发第三份（实测参照只有两份：套体一份、处理块一份）
                    | Statement::Try { .. }
                    // `with` 同 `try`：正常出口与清理出口**各自**重放了余部＋收尾
                    | Statement::With { .. }
            ) {
                break;
            }
        }
        self.block_depth -= 1;
        self.block_tail = outer_tail;
        self.block_end_labels.pop();
        self.mark_label(block_end);
        Ok(())
    }

    /// 压一个**boolop 的直接操作数**：裸的局部名用 **`LOAD_FAST`**（拥有加载，因为 `COPY` 要
    /// 求有两份引用）；其余交给普通发射（子表达式照旧走借用加载，实测 `(a < b) and c` 里那对
    /// 仍是 `LOAD_FAST_BORROW_LOAD_FAST_BORROW`）。
    pub(super) fn emit_operand(&mut self, value: &Expression) -> Result<(), CompileError> {
        if let Expression::Name(name, span) = value {
            // **融合读取**：`STORE_FAST_LOAD_FAST` 压回的那份值先看这里（这条快路径会**绕过**
            // 普通发射的 `Name` 分支，第 235 轮实测：函数作用域的推导式因此多压了一份元素值）
            if let Some(slot) = self.pending_fused_load.take() {
                if self.unit.varnames.get(slot).is_some_and(|item| item == name) {
                    return Ok(());
                }
            }
            if self.kind == ScopeKind::Function && self.unit.varnames.iter().any(|item| item == name)
            {
                let slot = self.slot_of(name);
                self.emit_at(
                    *span,
                    opcode::opcode("LOAD_FAST").expect("LOAD_FAST 在表里"),
                    slot as u8,
                );
                return Ok(());
            }
        }
        self.emit_expression(value)
    }

    /// **值上下文**里的"测真值并跳转"（结果值留在栈上）：`值; COPY 1; TO_BOOL; POP_JUMP_IF_*; NOT_TAKEN; POP_TOP`。
    ///
    /// 嵌套 `and`／`or` 时**融合**（实测 `x = a and b or c`／`x = (a or b) and c`）：
    /// 内层非末操作数跳到 `fresh`（标在内层**末**操作数 `NOT_TAKEN` 之后、`POP_TOP` 之前），
    /// 内层末操作数按**外层继承的条件** `jump_if_true` 跳到 `target`。
    pub(super) fn emit_test_value(
        &mut self,
        value: &Expression,
        jump_if_true: bool,
        target: usize,
        cleanup: Option<usize>,
    ) -> Result<(), CompileError> {
        if let Expression::BoolOp {
            conjunction,
            values,
            span,
        } = value
        {
            // **骨架指令取"拥有该操作数的那个布尔节点"的跨度**（实测，逐层递归）：
            // `a and b or c` 里 `a` 之后的骨架是 `(4,11)`（内层 `and` 的跨度）、
            // `b` 之后的是 `(4,16)`（外层 `or` 的跨度）；`a and (b or c)` 则是 `(4,18)`／`(11,17)`。
            let saved = self.boolop_scaffold_span;
            self.boolop_scaffold_span = Some(*span);
            let fresh = self.new_label();
            for inner in &values[..values.len() - 1] {
                // 内层操作数按**内层自身的极性**跳（`and` ⇒ 假就跳、`or` ⇒ 真就跳）
                self.emit_test_value(inner, !*conjunction, fresh, None)?;
            }
            // **末操作数**之后的骨架属于**父层**（实测 `a and b or c` 里 `b` 之后是外层 `or` 的
            // `(4,16)`，而不是内层 `and` 的 `(4,11)`）⇒ 恢复父层的跨度再递归
            self.boolop_scaffold_span = saved;
            let last = values.last().expect("`and`／`or` 至少一个操作数");
            return self.emit_test_value(last, jump_if_true, target, Some(fresh));
        }
        let span = self.boolop_scaffold_span.unwrap_or_else(|| value.span());
        self.emit_operand(value)?;
        self.emit_at(span, opcode::opcode("COPY").expect("COPY 在表里"), 1);
        self.emit_at(span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
        let name = if jump_if_true {
            "POP_JUMP_IF_TRUE"
        } else {
            "POP_JUMP_IF_FALSE"
        };
        self.emit_jump(span, opcode::opcode(name).expect("条件跳转在表里"), target);
        self.emit_at(span, opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"), 0);
        if let Some(label) = cleanup {
            self.mark_label(label);
        }
        self.emit_at(span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
        Ok(())
    }

    /// **条件上下文**里的"测真值并跳转"（值**不**保留 ⇒ 没有 `COPY`／`POP_TOP`）：
    /// `值; TO_BOOL; POP_JUMP_IF_*; NOT_TAKEN`（实测 `if a and b:` 两个操作数都跳同一个目标）。
    pub(super) fn emit_test_bare(
        &mut self,
        value: &Expression,
        jump_if_true: bool,
        target: usize,
        cleanup: Option<usize>,
    ) -> Result<(), CompileError> {
        // **`not` 折进跳转极性**（实测：`if not a and not b:` 的参照产物是
        // `TO_BOOL; POP_JUMP_IF_TRUE`，没有 `UNARY_NOT`）——与 `emit_condition_jump_to` 同一条规则
        if let Expression::Not(operand, _) = value {
            return self.emit_test_bare(operand, !jump_if_true, target, cleanup);
        }
        if let Expression::BoolOp {
            conjunction,
            values,
            ..
        } = value
        {
            let fresh = self.new_label();
            for inner in &values[..values.len() - 1] {
                self.emit_test_bare(inner, !*conjunction, fresh, None)?;
            }
            let last = values.last().expect("`and`／`or` 至少一个操作数");
            return self.emit_test_bare(last, jump_if_true, target, Some(fresh));
        }
        let span = value.span();
        self.emit_expression(value)?;
        self.emit_at(span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
        let name = if jump_if_true {
            "POP_JUMP_IF_TRUE"
        } else {
            "POP_JUMP_IF_FALSE"
        };
        self.emit_jump(span, opcode::opcode(name).expect("条件跳转在表里"), target);
        self.emit_at(span, opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"), 0);
        if let Some(label) = cleanup {
            self.mark_label(label);
        }
        Ok(())
    }

    /// 记一条异常表条目（**字节**偏移；编码时换成码元）。
    pub(super) fn record_exception(
        &mut self,
        start: usize,
        end: usize,
        target: usize,
        depth: usize,
        lasti: bool,
    ) {
        self.exception_entries.push((start, end, target, depth, lasti));
    }

    /// 处理块段（处理块里再抛要落到清理块）：先记字节区间 ＋ "有没有 `as 名字`"，
    /// `finish_handler_segments` 再补目标（有名字的落到**名字清理**那条，没有的落到 `COPY 3`）。
    pub(super) fn record_handler_segment(&mut self, start: usize, end: usize, has_name: bool) {
        self.handler_segments.push((start, end, has_name));
    }

    /// 给所有处理块段补上清理块目标（`depth` 1、`lasti` 打开，与参照的 cleanup 条目同形）。
    /// `name_cleanup` 是"名字清理"那段的起点（有 `as 名字` 的处理块落到它，其余落到 `cleanup`）。
    pub(super) fn finish_handler_segments(&mut self, cleanup: usize, name_cleanup: Option<usize>) {
        for (start, end, has_name) in core::mem::take(&mut self.handler_segments) {
            let target = match (has_name, name_cleanup) {
                (true, Some(offset)) => offset,
                _ => cleanup,
            };
            self.record_exception(start, end, target, self.handler_depth + 1, true);
        }
    }

    /// 把异常表条目编码成 `BC-54` 的字节串（4 个 6-bit varint／条，**码元**为单位）。
    pub(super) fn encode_exceptiontable(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // **条目要按 `start` 递增排序** ✓（第 167 轮：与参照逐字节对齐后才现形 ✓ —— 内容相同但顺序相反 ✗；
        //   参照是递增 ✓，我们是记录序 ✗）。用**稳定排序** ✓，同 `start` 时保持记录序 ✓。
        // **按 `start` 稳定排序** ✓（第 167 轮发现参照是递增序 ✓；当时排序引发**语料回归** ✗ ⇒ 留到
        //   `finally` 区间两端也修好之后再启用 ✓ —— 顺序是**语义**（先匹配者胜 ✓），所以必须与参照一致 ✓）。
        for (start, end, target, depth, lasti) in &self.exception_entries {
            let length = end.saturating_sub(*start);
            // **每条目的首字节带 `0x80` 标志** ✓（第 167 轮：与参照逐字节对齐后才现形 ✓ ——
            //   我们先前少这一位 ✗。参照的解析器**忽略**这一位 ✓，但产物要逐字节相同 ✓。）
            let entry_start = out.len();
            for value in [
                start / 2,
                length / 2,
                target / 2,
                (depth << 1) | usize::from(*lasti),
            ] {
                write_exception_varint(&mut out, value);
            }
            out[entry_start] |= 0x80;
        }
        out
    }

    /// 发一条**比较**：`COMPARE_OP`（六个）或 `IS_OP`／`CONTAINS_OP`（`is`／`in` 两族）。
    pub(super) fn emit_compare(
        &mut self,
        left: &Expression,
        operator: &CompareOperator,
        right: &Expression,
        span: Span,
    ) -> Result<(), CompileError> {
        if matches!(
            operator,
            CompareOperator::Is
                | CompareOperator::IsNot
                | CompareOperator::In
                | CompareOperator::NotIn
        ) {
            self.emit_two_operands(left, right)?;
            let (name, oparg) = match operator {
                CompareOperator::Is => ("IS_OP", 0),
                CompareOperator::IsNot => ("IS_OP", 1),
                CompareOperator::In => ("CONTAINS_OP", 0),
                _ => ("CONTAINS_OP", 1),
            };
            self.emit_at(span, opcode::opcode(name).expect("比较指令在表里"), oparg);
            return Ok(());
        }
        self.emit_compare_plain(left, operator, right, span)
    }

    /// `COMPARE_OP`，并按当前上下文决定是否带 `bool(...)` 位（`|16`）。
    pub(super) fn emit_compare_plain(
        &mut self,
        left: &Expression,
        operator: &CompareOperator,
        right: &Expression,
        span: Span,
    ) -> Result<(), CompileError> {
        self.emit_two_operands(left, right)?;
        let base = operator
            .oparg()
            .expect("`is`／`in` 一族不走 `COMPARE_OP`（调用方已分流）");
        let oparg = if self.in_condition { base | 16 } else { base };
        self.emit_at(
            span,
            opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
            oparg,
        );
        Ok(())
    }

    /// `COMPARE_OP` **强制带** `bool(...)` 位（`not` 推进比较时用：参照实测 `not a < b` ⇒ 18）。
    pub(super) fn emit_compare_with_bool(
        &mut self,
        left: &Expression,
        operator: &CompareOperator,
        right: &Expression,
        span: Span,
    ) -> Result<(), CompileError> {
        self.emit_two_operands(left, right)?;
        let base = operator
            .oparg()
            .expect("`is`／`in` 一族不走 `COMPARE_OP`（调用方已分流）");
        self.emit_at(
            span,
            opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
            base | 16,
        );
        Ok(())
    }

    /// 发一条 `BINARY_OP`（`NB_SUBSCR`，即 `[]`）。
    pub(super) fn emit_binary_op_subscript(&mut self, span: Span) {
        let index = crate::opcode::get_nb_ops()
            .iter()
            .position(|entry| entry.1 == "[]")
            .expect("nb_ops 里应当有 []") as u8;
        self.emit_at(
            span,
            opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
            index,
        );
    }

    /// 压两个操作数：两边都是**本函数的局部**时打成超指令（位置取先压的那个名字，实测）。
    pub(super) fn emit_two_operands(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> Result<(), CompileError> {
        // **待抵消的融合读**：`STORE_FAST_LOAD_FAST` 已经把左操作数压回来了 ⇒ 不能再打成
        // "两个局部名的超指令"（否则会**多压一份**，第 235 轮实测：函数作用域的推导式因此在
        // `LIST_APPEND` 时栈上多一个值、取到迭代器而报错）——只发右操作数
        if let Expression::Name(a, _) = left {
            if let Some(slot) = self.pending_fused_load {
                if self.unit.varnames.get(slot).is_some_and(|item| item == a) {
                    self.pending_fused_load = None;
                    return self.emit_expression(right);
                }
            }
        }
        let pack = match (self.kind, left, right) {
            (ScopeKind::Function, Expression::Name(a, _), Expression::Name(b, _)) => {
                let slots = &self.unit.varnames;
                match (
                    slots.iter().position(|item| item == a),
                    slots.iter().position(|item| item == b),
                ) {
                    (Some(first), Some(second)) => Some((first, second)),
                    _ => None,
                }
            }
            _ => None,
        };
        match pack {
            Some((first, second)) => {
                self.emit_at(
                    left.span(),
                    opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                        .expect("超指令在表里"),
                    ((first << 4) | second) as u8,
                );
                Ok(())
            }
            None => {
                self.emit_expression(left)?;
                self.emit_expression(right)
            }
        }
    }

    /// 发射一个**可缺省**的表达式：缺省时压 `LOAD_CONST None`（切片缺界的实测形态）。
    pub(super) fn emit_optional(
        &mut self,
        owner: &Expression,
        part: &Option<Box<Expression>>,
    ) -> Result<(), CompileError> {
        match part {
            Some(expression) => self.emit_expression(expression),
            None => {
                let index = self.intern_constant(Constant::None);
                self.emit_indexed(owner.span(), "LOAD_CONST", index);
                Ok(())
            }
        }
    }

    /// **`yield from`** 的公共发射（第 315 轮）：语句形态末尾补 `POP_TOP`（值丢掉）、表达式形态不补。
    /// 形状照参照实测：`<被委派的表达式>; GET_YIELD_FROM_ITER; [L1] LOAD_CONST None; SEND <L2>;
    /// YIELD_VALUE 1; RESUME 2; POP_TOP; JUMP_BACKWARD_NO_INTERRUPT <L1>; [L2] END_SEND`。
    /// 中间那条 `POP_TOP` 与 `async for` 同款（本层 `YIELD_VALUE`／`RESUME` 的语义与参照不同，
    /// 不收走恢复时送进来的值，下一轮就会把它当迭代器），**如实登记**。
    fn emit_yield_from(
        &mut self,
        value: &Expression,
        span: Span,
        discard: bool,
    ) -> Result<(), CompileError> {
        self.emit_expression(value)?;
        self.emit_named(span, "GET_YIELD_FROM_ITER", 0);
        let loop_label = self.new_label();
        let done = self.new_label();
        self.mark_label(loop_label);
        let none_index = self.intern_constant(Constant::None);
        self.emit_indexed(span, "LOAD_CONST", none_index);
        self.emit_directed_jump(span, opcode::opcode("SEND").expect("SEND 在表里"), done, false);
        self.emit_named(span, "YIELD_VALUE", 1);
        self.emit_named(span, "RESUME", 2);
        // **收走"恢复时送进来的值"**：本层 `YIELD_VALUE`／`RESUME` 的语义与参照不同 ⇒ 不收走的话，
        // 下一轮 `SEND` 会把那个值当迭代器 ⇒ 报"既不是内建迭代器，也没有 `__next__`" ✗
        // （实测：去掉它 `yield from [1, 2]` 当场坏 ✓）。
        self.emit_named(span, "POP_TOP", 0);
        self.emit_directed_jump(
            span,
            opcode::opcode("JUMP_BACKWARD_NO_INTERRUPT").expect("JUMP_BACKWARD_NO_INTERRUPT 在表里"),
            loop_label,
            true,
        );
        self.mark_label(done);
        self.emit_named(span, "END_SEND", 0);
        if discard {
            self.emit_named(span, "POP_TOP", 0);
        }
        Ok(())
    }

    pub(super) fn emit_expression(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            // **浮点字面量**（第 127 轮实测）：`x = 1.5` ⇒ `LOAD_CONST <下标>`（**总**入常量池 ✓，
            //   连 `2.0` 这种也走常量池 ✓），位点取字面量自身 ✓。
            Expression::Float(bits, span) => {
                let index = self.intern_constant(Constant::Float(*bits));
                self.emit_indexed(*span, "LOAD_CONST", index);
                Ok(())
            }
            Expression::Map(items, span) => {
                // **纯键值对**（没有 `**`）：形状照第 282 轮**一字不动** ✓（夹具守着 ✓）——
                // 15 对及以下一条 `BUILD_MAP n` ✓、16 对及以上走增量形态
                //（`BUILD_MAP 0` ＋ 逐对 `MAP_ADD 1` ✓，阈值逐条 `dis` 实测 ✓）。
                if items.iter().all(|item| matches!(item, MapItem::Pair(_, _))) {
                    if items.len() >= 16 {
                        self.emit_named(*span, "BUILD_MAP", 0);
                        for item in items {
                            let MapItem::Pair(key, value) = item else {
                                unreachable!("上面刚判过全是键值对")
                            };
                            self.emit_expression(key)?;
                            self.emit_expression(value)?;
                            self.emit_named(*span, "MAP_ADD", 1);
                        }
                        return Ok(());
                    }
                    for item in items {
                        let MapItem::Pair(key, value) = item else {
                            unreachable!("上面刚判过全是键值对")
                        };
                        self.emit_expression(key)?;
                        self.emit_expression(value)?;
                    }
                    self.emit_named(*span, "BUILD_MAP", items.len() as u8);
                    return Ok(());
                }
                // **有 `**` 解包**（第 293 轮）：统一用"累加器"形态 —— `BUILD_MAP 0` 起头 ✓，
                // 键值对走 `MAP_ADD 1` ✓、解包走 `LOAD <映射>; DICT_UPDATE 1` ✓
                //（`DICT_UPDATE` 把 TOS 并进 TOS1 再弹掉 TOS ✓ ⇒ 累加器一直在栈上 ✓）。
                // **如实说** ✗：参照这一档是"每串键值对各发一条 `BUILD_MAP n`"，与这条形状**不逐字节同形** ✓
                //（语义相同 ✓；夹具里没有 `**` 的用例 ✓ ⇒ 不影响逐字节对拍 ✓）。
                self.emit_named(*span, "BUILD_MAP", 0);
                for item in items {
                    match item {
                        MapItem::Pair(key, value) => {
                            self.emit_expression(key)?;
                            self.emit_expression(value)?;
                            self.emit_named(*span, "MAP_ADD", 1);
                        }
                        MapItem::Unpack(value) => {
                            self.emit_expression(value)?;
                            self.emit_named(*span, "DICT_UPDATE", 1);
                        }
                    }
                }
                Ok(())
            }
            Expression::TupleLiteral(items, span) => {
                if items
                    .iter()
                    .any(|item| matches!(item, Expression::Starred(_, _)))
                {
                    // **星号解包**（第 120 轮，实测形态）：前面的非星号项先压 ⇒
                    // `BUILD_LIST <前项数>`（位点＝**整个显示**）⇒ 每个星号项：表达式 ＋
                    // `LIST_EXTEND 1`（位点同上）；元组末尾再 `CALL_INTRINSIC_1 6`。
                    // **交错形态也接** ✓（第 224 轮）：星号后面还有项 ⇒ 参照发的是 `LIST_APPEND 1` ✓
                    //（实测 `(*a, b)` ⇒ `BUILD_LIST 0; <a>; LIST_EXTEND 1; <b>; LIST_APPEND 1` ✓；
                    //  `[a, *b, c]` ⇒ `BUILD_LIST 1; <b>; LIST_EXTEND 1; <c>; LIST_APPEND 1` ✓）。
                    let leading = items
                        .iter()
                        .take_while(|item| !matches!(item, Expression::Starred(_, _)))
                        .count();
                    // 前导（非星号）项**先按各自位点压栈**（实测  的 
                    //   位点是  ＝  自身，不是整个显示）
                    for item in &items[..leading] {
                        self.emit_expression(item)?;
                    }
                    self.emit_named(*span, "BUILD_LIST", leading as u8);
                    for item in &items[leading..] {
                        match item {
                            Expression::Starred(value, _) => {
                                self.emit_expression(value)?;
                                self.emit_named(*span, "LIST_EXTEND", 1);
                            }
                            // **星号后面还有项** ✓（第 224 轮）：参照发 `LIST_APPEND 1` ✓
                            //（`i` 是目标列表相对 TOS 的深度 ✓，这里恒为 1 ✓）。
                            _ => {
                                self.emit_expression(item)?;
                                self.emit_named(*span, "LIST_APPEND", 1);
                            }
                        }
                    }
                    // 元组末尾把列表转成元组（实测 `CALL_INTRINSIC_1 6`）；**必须在 return 之前** ✓
                    self.emit_named(*span, "CALL_INTRINSIC_1", 6);
                    return Ok(());
                }

                // 全常量 ⇒ 折叠成**常量元组**（实测 `x = (1, 2)` 的 `co_consts` 里有它，
                // 且登记在收尾（`LOAD_CONST None`）**之后** ⇒ 走 `pending` 那条延迟路径）
                if let Some(folded) = fold_constant(expression)? {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    let argument_byte = self.unit.code.len() + 1;
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                        0,
                    );
                    self.pending.push((argument_byte, folded));
                    return Ok(());
                }
                // **超指令融合**（第 139 轮实测）：前两项都是**裸局部名字**时，参照把它们打成
                //   `LOAD_FAST_BORROW_LOAD_FAST_BORROW <高4位先压 | 低4位后压>` ✓
                //   （`yield a, b` ⇒ 一条融合 ＋ `BUILD_TUPLE 2` ✓）。只认 `Expression::Name`
                //   ⇒ 不误融合（`a.b, c` 的首条是 `LOAD_FAST_BORROW a` ＋ `LOAD_ATTR` ✗ ✓）。
                let mut start = 0;
                if items.len() >= 2 {
                    let slot_of = |item: &Expression| match item {
                        Expression::Name(name, _) => self
                            .unit
                            .varnames
                            .iter()
                            .position(|candidate| candidate == name),
                        _ => None,
                    };
                    if let (Some(first), Some(second)) = (slot_of(&items[0]), slot_of(&items[1])) {
                        self.emit_at(
                            items[0].span(),
                            opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                                .expect("超指令在表里"),
                            ((first << 4) | second) as u8,
                        );
                        start = 2;
                    }
                }
                for item in &items[start..] {
                    self.emit_expression(item)?;
                }
                let count = u8::try_from(items.len()).map_err(|_| {
                    CompileError::Unsupported("元组字面量超过 255 项尚未接线".to_owned())
                })?;
                self.emit_named(*span, "BUILD_TUPLE", count);
                Ok(())
            }
            // 切片字面量**只能**当下标用（`a[b:c]`）；单独出现是内部错误，别静默发错指令
            // **带上位点**（第 311 轮）：先前只有一句"只能出现在下标里" ⇒ 定不了是哪一行
            // （上限榜上那一族 35 个模块就是被它挡着）⇒ 现在给行／列，便于顺着修。
            Expression::SliceLiteral { span, .. } => Err(CompileError::Unsupported(format!(
                "切片字面量只能出现在下标里（`a[b:c]`；第 {} 行，列 {}-{}）",
                span.line_start, span.col_start, span.col_end
            ))),
            Expression::Subscript(container, key, span) => {
                // 容器**不能**先单独发射：普通下标与两段切片都要把"容器＋下一个操作数"
                // 交给 `emit_two_operands`（两者都是局部时要打成超指令，实测）
                match &**key {
                    // **两段**（有界、没有步长）：参照发 `BINARY_SLICE`——它**自己就是取下标**，
                    // 所以这里**不再**补 `BINARY_OP []`（第一版多发了一条，被夹具打回）
                    Expression::SliceLiteral {
                        lower,
                        upper,
                        step: None,
                        ..
                    } => {
                        match lower {
                            // 前两个操作数都是局部时打成超指令（实测 `a[b:c]` 在函数里就是）
                            Some(lower) => self.emit_two_operands(container, lower)?,
                            None => {
                                self.emit_expression(container)?;
                                // **缺的界要显式压 `None`**（实测 `a[:c]` 就是 `LOAD a; LOAD None; LOAD c`）
                                self.emit_optional(key, lower)?;
                            }
                        }
                        self.emit_optional(key, upper)?;
                        self.emit_named(*span, "BINARY_SLICE", 0);
                    }
                    // **三段**（有步长表达式）：`BUILD_SLICE 3` 之后照常取下标
                    Expression::SliceLiteral {
                        lower,
                        upper,
                        step: Some(step),
                        span: slice_span,
                    } => {
                        // 四个操作数（容器、下界、上界、步长）按**相邻两两**打包：实测函数里
                        // `a[b:c:d]` 是 `PAIR(a,b)` ＋ `PAIR(c,d)`（两对，`None` 会打断打包）
                        match (lower, upper) {
                            (Some(lower), Some(upper)) => {
                                self.emit_two_operands(container, lower)?;
                                self.emit_two_operands(upper, step)?;
                            }
                            (Some(lower), None) => {
                                self.emit_two_operands(container, lower)?;
                                self.emit_optional(key, upper)?;
                                self.emit_expression(step)?;
                            }
                            (None, Some(upper)) => {
                                self.emit_expression(container)?;
                                self.emit_optional(key, lower)?;
                                self.emit_two_operands(upper, step)?;
                            }
                            (None, None) => {
                                self.emit_expression(container)?;
                                self.emit_optional(key, lower)?;
                                self.emit_optional(key, upper)?;
                                self.emit_expression(step)?;
                            }
                        }
                        self.emit_named(*slice_span, "BUILD_SLICE", 3);
                        self.emit_binary_op_subscript(*span);
                    }
                    // 普通键（含**常量切片**键：`LOAD_CONST slice(…)` 之后照常 `BINARY_OP []`）
                    _ => {
                        self.emit_two_operands(container, key)?;
                        self.emit_binary_op_subscript(*span);
                    }
                }
                Ok(())
            }
            Expression::List(items, span) => {
                if items
                    .iter()
                    .any(|item| matches!(item, Expression::Starred(_, _)))
                {
                    // **星号解包**（第 120 轮，实测形态）：前面的非星号项先压 ⇒
                    // `BUILD_LIST <前项数>`（位点＝**整个显示**）⇒ 每个星号项：表达式 ＋
                    // `LIST_EXTEND 1`（位点同上）；元组末尾再 `CALL_INTRINSIC_1 6`。
                    // **交错形态也接** ✓（第 224 轮）：星号后面还有项 ⇒ 参照发的是 `LIST_APPEND 1` ✓
                    //（实测 `(*a, b)` ⇒ `BUILD_LIST 0; <a>; LIST_EXTEND 1; <b>; LIST_APPEND 1` ✓；
                    //  `[a, *b, c]` ⇒ `BUILD_LIST 1; <b>; LIST_EXTEND 1; <c>; LIST_APPEND 1` ✓）。
                    let leading = items
                        .iter()
                        .take_while(|item| !matches!(item, Expression::Starred(_, _)))
                        .count();
                    // 前导（非星号）项**先按各自位点压栈**（实测  的 
                    //   位点是  ＝  自身，不是整个显示）
                    for item in &items[..leading] {
                        self.emit_expression(item)?;
                    }
                    self.emit_named(*span, "BUILD_LIST", leading as u8);
                    for item in &items[leading..] {
                        match item {
                            Expression::Starred(value, _) => {
                                self.emit_expression(value)?;
                                self.emit_named(*span, "LIST_EXTEND", 1);
                            }
                            // **星号后面还有项** ✓（第 224 轮）：参照发 `LIST_APPEND 1` ✓
                            //（`i` 是目标列表相对 TOS 的深度 ✓，这里恒为 1 ✓）。
                            _ => {
                                self.emit_expression(item)?;
                                self.emit_named(*span, "LIST_APPEND", 1);
                            }
                        }
                    }
                    return Ok(());
                }

                for item in items {
                    self.emit_expression(item)?;
                }
                let count = u8::try_from(items.len()).map_err(|_| {
                    CompileError::Unsupported("列表字面量超过 255 项尚未接线".to_owned())
                })?;
                self.emit_named(*span, "BUILD_LIST", count);
                Ok(())
            }
            Expression::Constant(constant, span) => {
                let index = self.intern_constant(constant.clone());
                self.emit_indexed(*span, "LOAD_CONST", index);
                Ok(())
            }
            // **清单推导式**（3.12+ 内联；照实测骨架）：
            //   `可迭代; GET_ITER; LOAD_FAST_AND_CLEAR <目标槽>; SWAP 2; BUILD_LIST 0; SWAP 2;
            //    L1: FOR_ITER → L2; STORE_FAST_LOAD_FAST <槽,槽>; [if 条件…];
            //    <元素>; LIST_APPEND 2; JUMP_BACKWARD → L1; L2: END_FOR; POP_ITER;
            //    SWAP 2; STORE_FAST <槽>`（还原外层同名局部）
            //   **整段受异常表保护**，清理块 `SWAP 2; POP_TOP; SWAP 2; STORE_FAST <槽>; RERAISE 0`
            //   （异常表：起点＝`BUILD_LIST`、到还原前为止，目标＝清理块，`depth` 2、`lasti` 关闭）
            // **生成器表达式**（第 125 轮）：**不内联** —— 编成独立 code object ＋ 生成器协议 ✓。
            //   外层四步（实测）：`LOAD_CONST <code>; MAKE_FUNCTION; <可迭代>; GET_ITER; CALL 0` ✓，
            //   其中 `LOAD_CONST` 与 `CALL` 位点＝**表达式自身** ✓、`GET_ITER` 位点＝可迭代那段 ✓。
            Expression::Comprehension {
                kind: ComprehensionKind::Generator,
                element,
                generators,
                span,
                ..
            } => {
                // **多层 `for` 也接线** ✓（第 173 轮：`Lib/_weakrefset.py` 的
                //   `e for s in (self, other) for e in s` 就撞在这条上 ✓）。照清单推导式那条路 ✓：
                //   **从最内层往外**包 —— 每层的 `if 条件` 留在**自己那层的 `for`** 里 ✓，
                //   **只有最外层**（`generators[0]`）的可迭代对象是 **`.0`** ✓，其余用各自表达式 ✓。
                let iterator = ".0".to_owned();
                let mut inner = vec![Statement::Yield(Some((**element).clone()), element.span())];
                for (index, generator) in generators.iter().enumerate().rev() {
                    for condition in generator.conditions.iter().rev() {
                        inner = vec![Statement::If {
                            span: condition.span(),
                            condition: condition.clone(),
                            then_body: inner,
                            else_body: Vec::new(),
                        }];
                    }
                    let (target, target_span, tuple_targets) = match &generator.target {
                        ComprehensionTarget::Name(name, target_span) => {
                            (name.clone(), *target_span, Vec::new())
                        }
                        ComprehensionTarget::Tuple(items) => {
                            let first = items.first().expect("元组目标不为空");
                            let last = items.last().expect("刚判过");
                            (first.0.clone(), first.1.to(last.1), items.clone())
                        }
                    };
                    // **`.0` 的位点取原可迭代表达式的跨度**（第 140 轮实测 ✓），且只给最外层 ✓。
                    let iterable = if index == 0 {
                        Expression::Name(iterator.clone(), generator.iterable.span())
                    } else {
                        generator.iterable.clone()
                    };
                    inner = vec![Statement::For {
                        is_async: false,
                        span: *span,
                        target,
                        target_span,
                        // 推导式那条临时 `for` 的目标**只有名字** ✓（`for .0 in …` 一类 ✓）
                        tuple_targets: tuple_targets
                            .into_iter()
                            .map(|(name, span)| ForTarget::Name(name, span))
                            .collect(),
                        iterable,
                        body: inner,
                        else_body: Vec::new(),
                    }];
                }
                let body = inner;
                // **外层那两条发射用最外层的可迭代对象** ✓（此处**只**改这一处 ✓ —— 上一版把尾段
                //   整段替换 ✗，漏进了清单／字典推导式那两支 ✓，实测 `comprehension_dict` 当场回归 ✓）。
                let outer_iterable = generators[0].iterable.clone();
                let nested_qualname = match self.kind {
                    ScopeKind::Module => "<genexpr>".to_owned(),
                    ScopeKind::Class => format!("{}.<genexpr>", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.<genexpr>", self.qualname),
                };
                let parameters = vec![Parameter {
                    name: iterator,
                    posonly: false,
                    annotation: None,
                    annotation_span: None,
                    default: None,
                }];
                let unit = compile_scope(
                    "<genexpr>",
                    &nested_qualname,
                    &parameters,
                    &[],
                    None,
                    None,
                    None,
                    self.mode,
                    self.tier,
                    &body,
                    ScopeKind::Function,
                    self.kind == ScopeKind::Class,
                    &[],
                    Span::new(span.line_start, span.line_start, 0, 0),
                )?;
                let index = self.intern_constant(Constant::Code(Box::new(unit)));
                self.emit_indexed(*span, "LOAD_CONST", index);
                self.emit_named(*span, "MAKE_FUNCTION", 0);
                self.emit_expression(&outer_iterable)?;
                self.emit_named(outer_iterable.span(), "GET_ITER", 0);
                self.emit_named(*span, "CALL", 0);
                self.last_span = *span;
                self.epilogue_span = *span;
                Ok(())
            }
            Expression::Comprehension {
                kind,
                element,
                value,
                generators,
                span,
            } => {
                // **3.14 内联推导式**（逐条实测；支持多重 `for`／元组目标／字典）：
                //   ① 先求**第一个**可迭代对象并 `GET_ITER`；
                //   ② 把所有**目标**逐个 `LOAD_FAST_AND_CLEAR`（"变量不外泄"），再 `SWAP 目标数+1`；
                //   ③ `BUILD_<容器> 0; SWAP 2`；
                //   ④ 每层 `FOR_ITER → L2_i; <存目标_i>`，内层再套下一层；
                //   ⑤ 最内层：各 `if` 子句（`TO_BOOL; POP_JUMP_IF_TRUE → 元素; NOT_TAKEN;
                //      JUMP_BACKWARD → 本层 L1`）→ 元素（字典还有值）→ `ADD`（oparg ＝ 1＋层数）；
                //   ⑥ 收尾 `END_FOR; POP_ITER`（逐层）→ `SWAP 目标数+1` → **逆序**还原目标；
                //   ⑦ 整段受异常表保护，清理块 `SWAP 2; POP_TOP; SWAP 目标数+1; <逐目标还原>; RERAISE 0`
                //      （外提到所在语句块末尾，见 `pending_cleanups`）。
                let (build_op, add_op, arity) = match kind {
                    // 生成器表达式在**上面**那条分支就已返回 ✓（这里到不了 ✓）
                    ComprehensionKind::Generator => {
                        unreachable!("生成器表达式不走内联路径")
                    }
                    ComprehensionKind::List => ("BUILD_LIST", "LIST_APPEND", 1usize),
                    ComprehensionKind::Set => ("BUILD_SET", "SET_ADD", 1),
                    ComprehensionKind::Dict => ("BUILD_MAP", "MAP_ADD", 2),
                };
                let target_names: Vec<(&str, Span)> = generators
                    .iter()
                    .flat_map(|generator| match &generator.target {
                        ComprehensionTarget::Name(name, span) => vec![(name.as_str(), *span)],
                        ComprehensionTarget::Tuple(items) => {
                            items.iter().map(|(name, span)| (name.as_str(), *span)).collect()
                        }
                    })
                    .collect();
                // 推导式内部：目标名按**局部**读（模块级也一样）——记下起点，收尾时截断
                let locals_saved = self.comprehension_locals.len();
                for (name, _) in &target_names {
                    self.comprehension_locals.push((*name).to_owned());
                }
                // 元素那段的跨度：字典是**键:值**整段（实测 `{k: k + 1 …}` 的
                // `POP_JUMP_IF_TRUE`／`MAP_ADD` 都是 `(5,13)`），列表／集合就是元素自己
                let element_span = match (kind, value.as_deref()) {
                    (ComprehensionKind::Dict, Some(value)) => element.span().to(value.span()),
                    _ => element.span(),
                };
                let first_scaffold = generators[0].iterable.span();
                self.emit_expression(&generators[0].iterable)?;
                self.emit_at(
                    first_scaffold,
                    opcode::opcode("GET_ITER").expect("GET_ITER 在表里"),
                    0,
                );
                let mut slots: Vec<usize> = Vec::with_capacity(target_names.len());
                for (name, _) in &target_names {
                    let slot = self.slot_of(name);
                    // 保存块用**整条推导式**的跨度（实测 `[a + b for a in s for b in t]` 的
                    // `LOAD_FAST_AND_CLEAR`／`SWAP`／`BUILD_LIST` 都是 `(1,1,4,33)`）
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_FAST_AND_CLEAR").expect("LOAD_FAST_AND_CLEAR 在表里"),
                        slot as u8,
                    );
                    slots.push(slot);
                }
                let depth = (target_names.len() + 1) as u8;
                self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), depth);
                let region_start = self.unit.code.len();
                self.emit_at(
                    *span,
                    opcode::opcode(build_op).expect("建容器指令在表里"),
                    0,
                );
                self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                // 逐层：`FOR_ITER → 本层出口` ＋ 存目标（内层继续递归）
                let mut exhausted: Vec<usize> = Vec::with_capacity(generators.len());
                let mut loops: Vec<usize> = Vec::with_capacity(generators.len());
                let mut slot_index = 0usize;
                let element_label;
                for (index, generator) in generators.iter().enumerate() {
                    let scaffold = generator.iterable.span();
                    if index > 0 {
                        self.emit_expression(&generator.iterable)?;
                        self.emit_at(scaffold, opcode::opcode("GET_ITER").expect("GET_ITER 在表里"), 0);
                    }
                    let loop_start = self.new_label();
                    let out = self.new_label();
                    loops.push(loop_start);
                    exhausted.push(out);
                    self.mark_label(loop_start);
                    self.emit_jump(
                        scaffold,
                        opcode::opcode("FOR_ITER").expect("FOR_ITER 在表里"),
                        out,
                    );
                    // 存目标：名字可直接与"紧接着的那次读取"打成 `STORE_FAST_LOAD_FAST`（实测）；元组先解包
                    match &generator.target {
                        ComprehensionTarget::Name(name, span) => {
                            let slot = slots[slot_index];
                            slot_index += 1;
                            if let Some(next_slot) = self.next_read_slot(element, generators, index) {
                                self.emit_at(
                                    *span,
                                    opcode::opcode("STORE_FAST_LOAD_FAST")
                                        .expect("STORE_FAST_LOAD_FAST 在表里"),
                                    ((slot << 4) | next_slot) as u8,
                                );
                                self.pending_fused_load = Some(next_slot);
                            } else {
                                let _ = name;
                                self.emit_at(
                                    *span,
                                    opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                    slot as u8,
                                );
                            }
                        }
                        ComprehensionTarget::Tuple(items) => {
                            let mut item_slots: Vec<usize> = Vec::new();
                            for _ in items {
                                item_slots.push(slots[slot_index]);
                                slot_index += 1;
                            }
                            // `UNPACK_SEQUENCE` 取**元组目标**那段的跨度（实测 `k, v` ⇒ `(14,18)`）
                            let target_span = items
                                .first()
                                .map(|(_, span)| *span)
                                .zip(items.last().map(|(_, span)| *span))
                                .map(|(first, last)| first.to(last))
                                .unwrap_or(scaffold);
                            // 随后的融合存取取**首个目标名**的跨度（实测 `STORE_FAST_STORE_FAST k, v`
                            // 是 `(14,15)`）
                            let first_slot_span = items
                                .first()
                                .map(|(_, span)| *span)
                                .unwrap_or(scaffold);
                            self.emit_at(
                                target_span,
                                opcode::opcode("UNPACK_SEQUENCE")
                                    .expect("UNPACK_SEQUENCE 在表里"),
                                item_slots.len() as u8,
                            );
                            // **任意项数**的元组目标 ✓（第 281 轮修 ✗；先前写死"两项" ✗）——
                            // 形状逐条 `dis` 实测 ✓：每两项一条 `STORE_FAST_STORE_FAST`
                            // （**高 4 位收 TOS** ✓），余下的一项走 `STORE_FAST` ✓
                            //（`a, b, c` ⇒ `STORE_FAST_STORE_FAST (a,b)` ＋ `STORE_FAST c` ✓；
                            //  `a, b, c, d` ⇒ 两条融合 ✓）。**槽号 ≥ 16 时**融不进 4 位 ⇒ 退回逐条 ✓。
                            let mut store_index = 0usize;
                            while store_index + 1 < item_slots.len() {
                                let (high, low) = (item_slots[store_index], item_slots[store_index + 1]);
                                if high < 16 && low < 16 {
                                    self.emit_at(
                                        first_slot_span,
                                        opcode::opcode("STORE_FAST_STORE_FAST")
                                            .expect("STORE_FAST_STORE_FAST 在表里"),
                                        ((high << 4) | low) as u8,
                                    );
                                } else {
                                    self.emit_at(
                                        first_slot_span,
                                        opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                        high as u8,
                                    );
                                    self.emit_at(
                                        first_slot_span,
                                        opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                        low as u8,
                                    );
                                }
                                store_index += 2;
                            }
                            if store_index < item_slots.len() {
                                self.emit_at(
                                    first_slot_span,
                                    opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                                    item_slots[store_index] as u8,
                                );
                            }
                        }
                    }
                }
                // 最内层：条件 → 元素
                element_label = self.new_label();
                // 注：`ADD`（`LIST_APPEND`／`SET_ADD`／`MAP_ADD`）与跳转都用上面算好的
                // `element_span`——列表／集合是元素自己，字典是"键:值"整段（实测）
                // **条件链**（实测 `[x for x in s if p if q]`）：每条 `if` 为真就跳去**下一条**
                // （最后一条跳去元素）；为假则 `JUMP_BACKWARD` 回本层循环
                let conditions: Vec<&Expression> = generators
                    .iter()
                    .flat_map(|generator| generator.conditions.iter())
                    .collect();
                let condition_labels: Vec<usize> =
                    conditions.iter().map(|_| self.new_label()).collect();
                for (index, condition) in conditions.iter().enumerate() {
                    self.mark_label(condition_labels[index]);
                    let target = if index + 1 < conditions.len() {
                        condition_labels[index + 1]
                    } else {
                        element_label
                    };
                    // **粘性位点**（实测）：`TO_BOOL` 取**条件**那段的、跳转三条取**元素**那段的
                    self.emit_expression(condition)?;
                    self.emit_at(
                        condition.span(),
                        opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"),
                        0,
                    );
                    self.emit_jump(
                        element_span,
                        opcode::opcode("POP_JUMP_IF_TRUE").expect("POP_JUMP_IF_TRUE 在表里"),
                        target,
                    );
                    self.emit_at(
                        element_span,
                        opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                        0,
                    );
                    self.emit_directed_jump(
                        element_span,
                        opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                        *loops.last().expect("至少一层"),
                        true,
                    );
                }
                self.mark_label(element_label);
                // 元素（字典是"键 ＋ 值"）
                self.emit_comprehension_element(*kind, element, value.as_deref())?;
                let inner_scaffold = element_span;
                self.emit_at(inner_scaffold, opcode::opcode(add_op).expect("加元素指令在表里"), (1 + generators.len()) as u8);
                // 元素之后**跳回最内层循环**
                self.emit_directed_jump(
                    inner_scaffold,
                    opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                    *loops.last().expect("至少一层"),
                    true,
                );
                // 逐层收尾（内层先）
                for index in (0..generators.len()).rev() {
                    let scaffold = generators[index].iterable.span();
                    self.mark_label(exhausted[index]);
                    self.emit_at(scaffold, opcode::opcode("END_FOR").expect("END_FOR 在表里"), 0);
                    self.emit_at(scaffold, opcode::opcode("POP_ITER").expect("POP_ITER 在表里"), 0);
                    if index > 0 {
                        self.emit_directed_jump(
                            element_span,
                            opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                            loops[index - 1],
                            true,
                        );
                    }
                }
                let region_end = self.unit.code.len();
                self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), depth);
                // **逆序**还原目标（同样取整条推导式的跨度）
                for name in target_names.iter().rev() {
                    let slot = self.slot_of(name.0);
                    self.emit_at(
                        *span,
                        opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                        slot as u8,
                    );
                }
                let label = self.new_label();
                self.pending_cleanups.push(PendingCleanup {
                    slots: slots.clone(),
                    depth,
                    scaffold: *span,
                    region_start,
                    region_end,
                    label,
                });
                let _ = arity;
                self.comprehension_locals.truncate(locals_saved);
                Ok(())
            }
            // **集合字面量**（实测 `{a, b}` ⇒ 逐元素后 `BUILD_SET 2`；`{}` 是空**字典**）
            Expression::SetLiteral(items, span) => {
                if items
                    .iter()
                    .any(|item| matches!(item, Expression::Starred(_, _)))
                {
                    // **星号解包**（第 120 轮，实测形态）：前面的非星号项先压 ⇒
                    // `BUILD_LIST <前项数>`（位点＝**整个显示**）⇒ 每个星号项：表达式 ＋
                    // `SET_UPDATE 1`（位点同上）；元组末尾再 `CALL_INTRINSIC_1 6`。
                    // **交错形态也接** ✓（第 224 轮）：星号后面还有项 ⇒ 参照发的是 `LIST_APPEND 1` ✓
                    //（实测 `(*a, b)` ⇒ `BUILD_LIST 0; <a>; LIST_EXTEND 1; <b>; LIST_APPEND 1` ✓；
                    //  `[a, *b, c]` ⇒ `BUILD_LIST 1; <b>; LIST_EXTEND 1; <c>; LIST_APPEND 1` ✓）。
                    let leading = items
                        .iter()
                        .take_while(|item| !matches!(item, Expression::Starred(_, _)))
                        .count();
                    for item in &items[..leading] {
                        self.emit_expression(item)?;
                    }
                    self.emit_named(*span, "BUILD_SET", leading as u8);
                    for item in &items[leading..] {
                        match item {
                            Expression::Starred(value, _) => {
                                self.emit_expression(value)?;
                                self.emit_named(*span, "SET_UPDATE", 1);
                            }
                            // **星号后面还有项** ✓（第 224 轮）：参照发 `LIST_APPEND 1` ✓
                            //（`i` 是目标列表相对 TOS 的深度 ✓，这里恒为 1 ✓）。
                            _ => {
                                self.emit_expression(item)?;
                                self.emit_named(*span, "LIST_APPEND", 1);
                            }
                        }
                    }
                    return Ok(());
                }

                // **≥3 个元素且全常量** ⇒ 参照折叠成 `frozenset` 常量（实测 `{1, 2, 3}` ⇒
                // `BUILD_SET 0; LOAD_CONST frozenset({1, 2, 3}); SET_UPDATE 1`；`{1, 1, 2}` 也折、
                // 去重后是 `frozenset({1, 2})`）；`{1}`／`{1, 2}`／含非常量 ⇒ 照旧逐元素 `BUILD_SET n`。
                if items.len() >= 3 {
                    let mut folded: Vec<Constant> = Vec::with_capacity(items.len());
                    let mut all_constant = true;
                    for item in items {
                        match fold_constant(item)? {
                            Some(constant) => folded.push(constant),
                            None => {
                                all_constant = false;
                                break;
                            }
                        }
                    }
                    if all_constant {
                        // **最左叶子**照样进常量表（实测常量表是 `(1, None, frozenset(…))`）
                        if let Some(first) = folded.first() {
                            self.intern_literal(first.clone());
                        }
                        // 集合语义：去重（`frozenset` 的元素序不进观测面——渲染时排序）
                        let mut unique: Vec<Constant> = Vec::new();
                        for constant in folded {
                            if !unique.contains(&constant) {
                                unique.push(constant);
                            }
                        }
                        self.emit_at(
                            *span,
                            opcode::opcode("BUILD_SET").expect("BUILD_SET 在表里"),
                            0,
                        );
                        // 折叠出来的 `frozenset` 与其它折叠常量**同一条路**：延迟到收尾之后入池
                        // （实测 `x = {1, 2, 3}` ⇒ `[1, None, frozenset]`；`x = 200 + 100` ⇒ `[200, None, 300]`）
                        let argument_byte = self.unit.code.len() + 1;
                        self.emit_at(
                            *span,
                            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                            0,
                        );
                        self.pending
                            .push((argument_byte, Constant::FrozenSet(unique)));
                        self.emit_at(
                            *span,
                            opcode::opcode("SET_UPDATE").expect("SET_UPDATE 在表里"),
                            1,
                        );
                        return Ok(());
                    }
                }
                for item in items {
                    self.emit_expression(item)?;
                }
                self.emit_at(
                    *span,
                    opcode::opcode("BUILD_SET").expect("BUILD_SET 在表里"),
                    items.len() as u8,
                );
                Ok(())
            }
            // **f-string**（3.14 实测）：逐段求值——字面段 `LOAD_CONST`、插值段"表达式 ＋
            // `CONVERT_VALUE`（有转换时）＋ `FORMAT_SIMPLE`／`FORMAT_WITH_SPEC`"；**多于一段**
            // 再 `BUILD_STRING n`（纯字面量在解析时已降成 `Str`）
            Expression::FString { parts, span } => {
                for part in parts {
                    self.emit_fstring_part(part, *span)?;
                }
                if parts.len() > 1 {
                    self.emit_at(
                        *span,
                        opcode::opcode("BUILD_STRING").expect("BUILD_STRING 在表里"),
                        parts.len() as u8,
                    );
                }
                Ok(())
            }
            // **`lambda`**（实测）：嵌套单元名／qualname 都是 `<lambda>`（函数里是
            // `<f>.<locals>.<lambda>`）；体 ＝ 那条表达式的 `Return`；随后与 `def` 共用
            // "造函数对象"（默认值 → `LOAD_CONST <code>` → `MAKE_FUNCTION` → 挂属性）
            // **`yield` 当表达式** ✓（第 219 轮）：与语句版同源 ✓，只是**不丢**那个值 ✓
            // —— `YIELD_VALUE` 之后 `RESUME 5` 把"送进来的值"留在栈上 ✓（语句版随后 `POP_TOP` 丢掉 ✓）。
            Expression::Yield(value, span) => {
                if let Some(value) = value {
                    self.emit_expression(value)?;
                } else {
                    let index = self.intern_constant(Constant::None);
                    self.emit_indexed(*span, "LOAD_CONST", index);
                }
                self.emit_named(*span, "YIELD_VALUE", 0);
                self.emit_named(*span, "RESUME", 5);
                self.last_span = *span;
                Ok(())
            }
            // **`yield from <表达式>`**（第 315 轮）：值**留着**（表达式形态）⇒ 末尾不补 `POP_TOP` ✓。
            Expression::YieldFrom(value, span) => {
                self.emit_yield_from(value, *span, false)?;
                self.last_span = *span;
                Ok(())
            }
            Expression::Lambda {
                parameters,
                kwonly,
                varargs,
                varkw,
                body,
                span,
            } => {
                let nested_qualname = match self.kind {
                    ScopeKind::Module => "<lambda>".to_owned(),
                    ScopeKind::Class => format!("{}.<lambda>", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.<lambda>", self.qualname),
                };
                let returned = Statement::Return((**body).clone(), body.span());
                let unit = [returned];
                // **lambda 的闭包**（第 297 轮）：它要的自由变量 = 体内引用的名字（扣掉自己的形参）
                // ∩（本层 `varnames`／`cellvars`／`freevars`）；实测 `return lambda: x` ⇒
                // lambda `co_freevars=('x',)`、外层 `cellvars=('x',)`、元组 `LOAD_FAST_BORROW 0` ✓
                // 要的是**这个 lambda 自己的体**所需的自由变量 = 体内引用的名字 − 自己的形参，
                // 再与本层 cell／free 表求交 ✓（`collect_lambda_demands` 是"在语句里找 lambda"，
                // 用在这里会找不到 ✗ —— 第 297 轮踩过）
                let mut inner_names: Vec<String> = Vec::new();
                collect_names_in_expression(body, &mut inner_names);
                let mut lambda_freevars: Vec<String> = Vec::new();
                for name in inner_names {
                    let is_parameter = parameters.iter().any(|item| item.name == name)
                        || kwonly.iter().any(|item| item.name == name)
                        || varargs.as_deref() == Some(name.as_str())
                        || varkw.as_deref() == Some(name.as_str());
                    if !is_parameter && self.deref_slot(&name).is_some() && !lambda_freevars.contains(&name) {
                        lambda_freevars.push(name);
                    }
                }
                let nested = compile_scope(
                    "<lambda>",
                    &nested_qualname,
                    parameters,
                    kwonly,
                    None,
                    varargs.as_deref(),
                    varkw.as_deref(),
                    self.mode,
                    self.tier,
                    &unit,
                    ScopeKind::Function,
                    self.kind == ScopeKind::Class,
                    // 嵌套单元的 `RESUME` 取**合成位点**（`lambda` 那一行、列 0..0；实测
                    // `def outer(): return lambda v: v` 的 lambda `RESUME` 是 `(2,2,0,0)`）
                    &lambda_freevars,
                    Span::new(span.line_start, span.line_start, 0, 0),
                )?;
                if !lambda_freevars.is_empty() {
                    for free in &lambda_freevars {
                        let slot = self.deref_slot(free).expect("刚筛过在本层表里");
                        self.emit_named(*span, "LOAD_FAST_BORROW", slot as u8);
                    }
                    self.emit_named(*span, "BUILD_TUPLE", lambda_freevars.len() as u8);
                }
                self.emit_function_object(nested, parameters, kwonly, None, None, *span)?;
                if !lambda_freevars.is_empty() {
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
                }
                Ok(())
            }
            // **属性读**（实测）：`LOAD_FAST_BORROW 0; LOAD_ATTR <名字下标>`；
            // `LOAD_ATTR` 的 oparg 低位是"取方法"标志 ⇒ 纯取值就是 `下标 << 1`
            Expression::Attribute(target, name, span) => {
                self.emit_expression(target)?;
                let index = self.intern_name(name);
                // 位置取**属性表达式自身**的跨度（第 226 轮按正确配对重测：`x = a.b` ⇒ `(4,7)`、
                // `x = a.b.c` ⇒ 两条 `LOAD_ATTR` 分别是 `(4,7)`／`(4,9)`、`x = a[0].b` ⇒ `(4,10)`）；
                // 原来取的是**对象**的跨度（`target.span()`）——那是从错位的测量里留下的错规则
                self.emit_named(*span, "LOAD_ATTR", (index << 1) as u8);
                Ok(())
            }

            // **装不进 `i64` 的整数字面量**（第 285 轮）：照参照直接进常量池 ✓
            //（`LOAD_CONST`；`LOAD_SMALL_INT` 那条优化只给 0..=255 ✓）。
            Expression::BigInt(text, span) => {
                let index = self.intern_constant(Constant::BigInt(text.clone()));
                self.emit_indexed(*span, "LOAD_CONST", index);
                Ok(())
            }

            Expression::Int(value, span) => {
                if (0..=255).contains(value) {
                    // **延迟分支**（实测）：需要收尾机制的块体里的 `return <小整数>` **不入常量表**
                    // （`with a: return 1` ⇒ `co_consts` 只有 `none`；不带 `with` 的 `return 1` ⇒ 入池）
                    if !self.defer_return_literal {
                        self.intern_literal(Constant::Int(*value));
                    }
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                        *value as u8,
                    );
                } else {
                    let index = self.intern_constant(Constant::Int(*value));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                }
                Ok(())
            }
            Expression::Str(text, span) => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_indexed(*span, "LOAD_CONST", index);
                Ok(())
            }
            Expression::Bytes(value, span) => {
                // `bytes` 字面量与字符串同形：一条 `LOAD_CONST`（实例化时常量池里那项建 `BytesObject`）
                let index = self.intern_constant(Constant::Bytes(value.clone()));
                self.emit_indexed(*span, "LOAD_CONST", index);
                Ok(())
            }
            Expression::Name(name, span) => {
                // **融合指令提供的那份值**：`STORE_FAST_LOAD_FAST` 刚把这个槽压回栈顶 ⇒
                // **紧接着的那一次**读取直接用它，不再发 `LOAD_FAST_BORROW`（实测的融合选择）
                if let Some(slot) = self.pending_fused_load.take() {
                    if self.unit.varnames.get(slot).is_some_and(|item| item == name) {
                        return Ok(());
                    }
                }
                // **推导式内部**：目标名是局部槽（模块级也一样，实测 `[x for x in s]` 的 `x`
                // 进 `co_varnames`、读它是 `LOAD_FAST_BORROW`）——但**只在推导式内部**这样；
                // 模块级同名变量在别处照旧走 `LOAD_NAME`
                if self.comprehension_locals.iter().any(|item| item == name) {
                    let slot = self.slot_of(name);
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_FAST_BORROW").expect("LOAD_FAST_BORROW 在表里"),
                        slot as u8,
                    );
                    return Ok(());
                }
                if let Some(slot) = self.deref_slot(name) {
                    // cell／自由变量 ⇒ `LOAD_DEREF`（闭包；第 292 轮）
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_DEREF").expect("LOAD_DEREF 在表里"),
                        slot as u8,
                    );
                    return Ok(());
                }
                if self.kind == ScopeKind::Function && self.unit.varnames.iter().any(|item| item == name)
                {
                    let slot = self.slot_of(name);
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_FAST_BORROW").expect("LOAD_FAST_BORROW 在表里"),
                        slot as u8,
                    );
                    return Ok(());
                }
                // **`global` 声明的名字**（第 115 轮）：**任何作用域**（含模块层 ✓）读它都走
                //   `LOAD_GLOBAL` ✓ —— 实测 `global a` 后 `print(a)` 是 `LOAD_GLOBAL` ✓。
                if self.kind == ScopeKind::Function
                    || self.global_names.iter().any(|item| item == name)
                {
                    // **`LOAD_GLOBAL`**（`BC-57`）：读非局部名走它——
                    // oparg 的低位是"压 NULL"标志 ⇒ 纯取值就是 `下标 << 1`（实测）
                    let index = self.intern_name(name);
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_GLOBAL").expect("LOAD_GLOBAL 在表里"),
                        (index << 1) as u8,
                    );
                    return Ok(());
                }
                let index = self.intern_name(name);
                self.emit_indexed(*span, "LOAD_NAME", index);
                Ok(())
            }
            Expression::Call {
                function,
                arguments,
                star_arguments,
                keywords,
                dict_arguments,
                callee_span,
                span,
            } => {
                // 实测形状（无关键字）：`<可调用>; PUSH_NULL; <实参…>; CALL <个数>`
                // 实测形状（带关键字）：`… ; <关键字值…>; LOAD_CONST <名元组>; CALL_KW <位置+关键字>`
                //
                // **例外（实测）**：**函数作用域里对全局名发调用**时，"压 NULL"由 `LOAD_GLOBAL`
                // 的**低位**承担（`LOAD_GLOBAL <下标 << 1 | 1>`），**不再**发单独的 `PUSH_NULL`
                // ——参照的 `def f(): raise ValueError(1)` 就是这样，而 `def f(): return g(1)`
                // 与模块级的 `LOAD_NAME; PUSH_NULL` 形态照旧。
                let global_callee = matches!(self.kind, ScopeKind::Function)
                    && matches!(
                        function.as_ref(),
                        Expression::Name(name, _)
                            if !self.unit.varnames.iter().any(|item| item == name)
                    );
                if global_callee {
                    if let Expression::Name(name, name_span) = function.as_ref() {
                        let index = self.intern_name(name);
                        self.emit_named(*name_span, "LOAD_GLOBAL", ((index << 1) | 1) as u8);
                    }
                } else {
                    self.emit_expression(function)?;
                    self.emit_at(
                        *callee_span,
                        opcode::opcode("PUSH_NULL").expect("PUSH_NULL 在表里"),
                        0,
                    );
                }
                // **`CALL_FUNCTION_EX` 形状下位置实参不单独压栈**（第 150 轮实测：参照把位置实参
                //   化成"元组那一格" ✓ —— `f(1, **kw)` ⇒ `LOAD_CONST (1,)` ✓；`f(x, **kw)` ⇒
                //   `LOAD x; BUILD_TUPLE 1` ✓）⇒ 这里先跳过，交给下面那一格发 ✓。
                let ex_shape = !star_arguments.is_empty() || !dict_arguments.is_empty();
                let tuple_slot = ex_shape && star_arguments.is_empty();
                if !tuple_slot {
                    for argument in arguments {
                        self.emit_expression(argument)?;
                    }
                }
                let total = arguments.len() + keywords.len() + star_arguments.len();
                if star_arguments.is_empty()
                    && dict_arguments.is_empty()
                    && keywords.is_empty()
                {
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL").expect("CALL 在表里"),
                        u8::try_from(total).map_err(|_| {
                            CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                        })?,
                    );
                    return Ok(());
                }
                if !star_arguments.is_empty() || !dict_arguments.is_empty() {
                    // **`CALL_FUNCTION_EX`**（实测四种形状）：
                    //   位置部分：没有 `*` 但有关键字 ⇒ `LOAD_CONST ()`；
                    //     只有一个 `*` 且没有前置位置实参 ⇒ 直接把那个可迭代对象交上去；
                    //     只有一个 `*` 且有前置位置实参 ⇒ `BUILD_LIST n; <* 对象>; LIST_EXTEND 1;
                    //       CALL_INTRINSIC_1 6`（LIST_TO_TUPLE）
                    //   关键字部分：`名字=值` 逐对压栈后 `BUILD_MAP <对数>`（一对都没有就先
                    //     `BUILD_MAP 0`），随后每个 `**` 压栈 ＋ `DICT_MERGE 1`；一个关键字都没有
                    //     就压 `PUSH_NULL`（"没有关键字"那一格）
                    // **多个 `*` 实参**（第 326 轮，照参照实测）：`f(*a, *b)` ⇒
                    //   `BUILD_LIST 0; LOAD a; LIST_EXTEND 1; LOAD b; LIST_EXTEND 1;
                    //    CALL_INTRINSIC_1 6`；有前置位置实参时把 `BUILD_LIST` 的个数换成它们 ✓。
                    // 先前这一支直接报"多个 `*` 实参尚未接线" ✗（`functools` 那一族 **11** 个模块卡它 ✓）。
                    if star_arguments.len() > 1 {
                        let head = u8::try_from(arguments.len()).map_err(|_| {
                            CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                        })?;
                        self.emit_at(
                            *span,
                            opcode::opcode("BUILD_LIST").expect("BUILD_LIST 在表里"),
                            head,
                        );
                        for star in star_arguments {
                            self.emit_expression(star)?;
                            self.emit_at(
                                *span,
                                opcode::opcode("LIST_EXTEND").expect("LIST_EXTEND 在表里"),
                                1,
                            );
                        }
                        self.emit_at(
                            *span,
                            opcode::opcode("CALL_INTRINSIC_1").expect("CALL_INTRINSIC_1 在表里"),
                            6, // INTRINSIC_LIST_TO_TUPLE
                        );
                    } else if star_arguments.is_empty() {
                        // **位置实参化成"元组那一格"**（第 150 轮实测 ✓，**只在没有 `*` 时** ✓）：
                        //   **全常量**就折成一个元组
                        //   常量 ✓（`f(1, 2, **kw)` ⇒ `LOAD_CONST (1, 2)` ✓）；否则逐个压栈后
                        //   `BUILD_TUPLE n` ✓（`f(x, **kw)` ⇒ `LOAD x; BUILD_TUPLE 1` ✓）；空表 ⇒
                        //   空元组常量 ✓（`f(**kw)`／`f(a=1, **kw)` ⇒ `LOAD_CONST ()` ✓）。
                        let mut parts: Vec<Constant> = Vec::with_capacity(arguments.len());
                        let mut all_constant = true;
                        for argument in arguments {
                            match super::fold_constant(argument)? {
                                Some(constant) => parts.push(constant),
                                None => {
                                    all_constant = false;
                                    break;
                                }
                            }
                        }
                        if all_constant {
                            // **登记顺序照参照** ✓：这个元组常量**收尾之后**才登记 ✗（实测
                            //   `f(a=1, **kw)` 的常量池是 `[code, str:a, none, names:]` ✓ ——
                            //   元组排在**最后** ✓）⇒ 与折叠常量同一条路：先占位、收尾回填 ✓。
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, Constant::Tuple(parts)));
                        } else {
                            for argument in arguments {
                                self.emit_expression(argument)?;
                            }
                            let count = u8::try_from(arguments.len()).map_err(|_| {
                                CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                            })?;
                            self.emit_named(*span, "BUILD_TUPLE", count);
                        }
                    } else if arguments.is_empty() {
                        self.emit_expression(&star_arguments[0])?;
                    } else {
                        // 前置位置实参**已经压过栈了**（函数入口处统一压的），这里只把它们收进列表
                        self.emit_at(
                            *span,
                            opcode::opcode("BUILD_LIST").expect("BUILD_LIST 在表里"),
                            u8::try_from(arguments.len()).map_err(|_| {
                                CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                            })?,
                        );
                        self.emit_expression(&star_arguments[0])?;
                        self.emit_at(
                            *span,
                            opcode::opcode("LIST_EXTEND").expect("LIST_EXTEND 在表里"),
                            1,
                        );
                        self.emit_at(
                            *span,
                            opcode::opcode("CALL_INTRINSIC_1")
                                .expect("CALL_INTRINSIC_1 在表里"),
                            6, // INTRINSIC_LIST_TO_TUPLE
                        );
                    }
                    if keywords.is_empty() && dict_arguments.is_empty() {
                        self.emit_at(
                            *span,
                            opcode::opcode("PUSH_NULL").expect("PUSH_NULL 在表里"),
                            0,
                        );
                    } else {
                        for (name, value) in keywords {
                            let index = self.intern_constant(Constant::Str(name.clone()));
                            self.emit_indexed(*span, "LOAD_CONST", index);
                            self.emit_expression(value)?;
                        }
                        self.emit_at(
                            *span,
                            opcode::opcode("BUILD_MAP").expect("BUILD_MAP 在表里"),
                            u8::try_from(keywords.len()).map_err(|_| {
                                CompileError::Unsupported(
                                    "关键字超过 255 个尚未接线".to_owned(),
                                )
                            })?,
                        );
                        for source in dict_arguments {
                            self.emit_expression(source)?;
                            self.emit_at(
                                *span,
                                opcode::opcode("DICT_MERGE").expect("DICT_MERGE 在表里"),
                                1,
                            );
                        }
                    }
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL_FUNCTION_EX")
                            .expect("CALL_FUNCTION_EX 在表里"),
                        0,
                    );
                    return Ok(());
                }
                {
                    for (_, value) in keywords {
                        self.emit_expression(value)?;
                    }
                    let names: Vec<String> =
                        keywords.iter().map(|(name, _)| name.clone()).collect();
                    let _ = &names;
                    let index = self.intern_constant(Constant::Names(names));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL_KW").expect("CALL_KW 在表里"),
                        u8::try_from(total).map_err(|_| {
                            CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                        })?,
                    );
                }
                Ok(())
            }
            // **链式比较**（实测骨架见 AST 注释；位点整段都取**整条链**，操作数各取自身）
            // `*表达式` 只该出现在**显示**里（由显示那几处的路径消费）；单独走到这里就取内层
            Expression::Starred(value, _) => self.emit_expression(value),
            Expression::Walrus {
                target,
                target_span,
                value,
                span,
            } => {
                // 实测（`x = (y := 3)`）：先求值 ⇒ `COPY 1`（位点＝**海象表达式** ✓）⇒ 存目标
                //   （位点＝**目标名** ✓）；`COPY` 留的那一份就是表达式的值 ✓。
                self.emit_expression(value)?;
                self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 1);
                if self.kind == ScopeKind::Function {
                    // **cell／自由变量**（第 119 轮）：闭包里的 `:=` 目标走 `STORE_DEREF` ✓
                    //（与赋值臂同一条口径 ✓；实测 `def outer(): x = 0; def inner(): return x;
                    //   if (x := 1): pass` ⇒ `STORE_DEREF` ✓）。
                    if let Some(slot) = self.deref_slot(target) {
                        self.emit_at(
                            *target_span,
                            opcode::opcode("STORE_DEREF").expect("STORE_DEREF 在表里"),
                            slot as u8,
                        );
                    } else if self.unit.varnames.iter().any(|item| item == target) {
                        let slot = self.slot_of(target);
                        self.emit_named(*target_span, "STORE_FAST", slot as u8);
                    } else {
                        return Err(CompileError::Unsupported(
                            "海象的目标既不是局部也不是 cell／自由变量：随后补（如实报未接线 ✓）"
                                .to_owned(),
                        ));
                    }
                } else {
                    let index = self.intern_name(target);
                    self.emit_indexed(*target_span, "STORE_NAME", index);
                }
                Ok(())
            }
            Expression::ChainedCompare {
                operands,
                operators,
                span,
            } => {
                self.emit_expression(&operands[0])?;
                let takeover = self.chain_takeover.take();
                let failed = match &takeover {
                    Some((label, _, _, _)) => *label,
                    None => self.new_label(),
                };
                for (index, operator) in operators.iter().enumerate() {
                    self.emit_expression(&operands[index + 1])?;
                    let last = index + 1 == operators.len();
                    if !last {
                        // 保住**中间操作数**：`SWAP 2; COPY 2`（实测实参都是 2）
                        self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                        self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 2);
                    }
                    // **`is`／`in` 两族在链式比较里也走 `IS_OP`／`CONTAINS_OP`**（第 327 轮）：
                    // 它们**没有** `COMPARE_OP` 的 oparg ✗ ⇒ 先前这里 `.expect(...)` 直接 panic ✗
                    //（实测 `if a == b is c:` ✓ —— 正是上限诊断里 29 个模块那一族的真身 ✓）。
                    // 与 [`Emitter::emit_compare`] 的约定一致：`is`⇒`IS_OP 0`／`is not`⇒`IS_OP 1`／
                    // `in`⇒`CONTAINS_OP 0`／`not in`⇒`CONTAINS_OP 1` ✓。
                    let is_in_family = matches!(
                        operator,
                        crate::compile::CompareOperator::Is
                            | crate::compile::CompareOperator::IsNot
                            | crate::compile::CompareOperator::In
                            | crate::compile::CompareOperator::NotIn
                    );
                    if is_in_family {
                        let (name, oparg) = match operator {
                            crate::compile::CompareOperator::Is => ("IS_OP", 0u8),
                            crate::compile::CompareOperator::IsNot => ("IS_OP", 1),
                            crate::compile::CompareOperator::In => ("CONTAINS_OP", 0),
                            _ => ("CONTAINS_OP", 1),
                        };
                        self.emit_at(*span, opcode::opcode(name).expect("比较指令在表里"), oparg);
                    } else {
                        let base = operator.oparg().expect("六个 `COMPARE_OP` 运算符之一");
                        // 非末段的结果立刻转布尔 ⇒ 不带 `|16`；末段按上下文（实测量到 2 / 18）
                        let oparg = if last && self.in_condition { base | 16 } else { base };
                        self.emit_at(
                            *span,
                            opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
                            oparg,
                        );
                    }
                    if !last {
                        self.emit_at(*span, opcode::opcode("COPY").expect("COPY 在表里"), 1);
                        self.emit_at(*span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
                        self.emit_jump(
                            *span,
                            opcode::opcode("POP_JUMP_IF_FALSE").expect("条件跳转在表里"),
                            failed,
                        );
                        self.emit_at(
                            *span,
                            opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                            0,
                        );
                        self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                    }
                }
                // **失败路径**的清理（`SWAP 2; POP_TOP`：丢掉左操作数、留下 `False`）——参照把它
                // **外提**到语句之后，本层就地在表达式尾发出，因此**成功路径必须跳过它**
                // （否则成功时栈上只有一个结果，`SWAP 2` 会 `StackUnderflow`；第 262 轮的语料
                // `chained_compare` 正是这么抓出来的）。
                // **赋值臂接管时**（第 85 轮）：失败块由 `pending_chain_copies` 在**收尾之后**发出
                // ⇒ 成功路径直接续下去，**不发** `JUMP_FORWARD`、也不就地发 `SWAP/POP_TOP` ✓
                if takeover.is_some() {
                    return Ok(());
                }
                let done = self.new_label();
                self.emit_jump(
                    *span,
                    opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                    done,
                );
                self.mark_label(failed);
                self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
                self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                self.mark_label(done);
                Ok(())
            }
            // **三元表达式**：`<条件>; TO_BOOL; POP_JUMP_IF_FALSE → else; NOT_TAKEN;
            // <then>; JUMP_FORWARD → end; else: <else>; end:`（语义等价；参照把余部复制进两分支）
            Expression::Conditional {
                condition,
                then_value,
                else_value,
                span: _,
            } => {
                let condition_span = condition.span();
                let else_label = self.new_label();
                self.emit_expression(condition)?;
                self.emit_at(
                    condition_span,
                    opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"),
                    0,
                );
                self.emit_jump(
                    condition_span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("条件跳转在表里"),
                    else_label,
                );
                self.emit_at(
                    condition_span,
                    opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                    0,
                );
                self.emit_expression(then_value)?;
                let end_label = self.new_label();
                self.emit_jump(
                    condition_span,
                    opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                    end_label,
                );
                self.mark_label(else_label);
                self.emit_expression(else_value)?;
                self.mark_label(end_label);
                Ok(())
            }
            Expression::Compare(left, operator, right, span) => {
                self.emit_compare(left, operator, right, *span)?;
                Ok(())
            }

            Expression::BoolOp {
                conjunction,
                values,
                ..
            } => {
                // **值上下文**（实测模板，3.14 用 `COPY`／`TO_BOOL`／`POP_JUMP_IF_*`／`NOT_TAKEN`／`POP_TOP`）：
                //   非末操作数：`值; COPY 1; TO_BOOL; POP_JUMP_IF_<短路方向>; NOT_TAKEN; POP_TOP`
                //   末操作数  ：当作**值**求（不再测真值）
                // 嵌套时**融合**：内层非末操作数跳到内层末操作数 `NOT_TAKEN` 之后的落点（实测
                // `x = (a or b) and c`），内层末操作数按**外层继承的条件**跳（实测 `x = a and b or c`）。
                // 折叠：常量短路（实测 `1 and 2` ⇒ `2`、`0 and 3` ⇒ `0`），且**加载位置取
                // 「决定结果的那个操作数」**（`x = 0 and 3` 的 `LOAD_SMALL_INT` 位置是 `0` 那段，
                // 不是整段表达式）
                let folded = {
                    let mut deciding: Option<Span> = None;
                    let mut last_span = expression.span();
                    let mut result: Option<Constant> = None;
                    for value in values {
                        let Some(constant) = fold_constant(value)? else {
                            result = None;
                            break;
                        };
                        last_span = value.span();
                        let Some(truth) = truthiness(&constant) else {
                            result = None;
                            break;
                        };
                        let short_circuit = if *conjunction { !truth } else { truth };
                        result = Some(constant);
                        if short_circuit {
                            deciding = Some(value.span());
                            break;
                        }
                    }
                    result.map(|constant| (constant, deciding.unwrap_or(last_span)))
                };
                if let Some((folded, span)) = folded {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    match folded {
                        Constant::Int(value) if (0..=255).contains(&value) => {
                            self.emit_at(
                                span,
                                opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                                value as u8,
                            );
                        }
                        other => {
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, other));
                        }
                    }
                    return Ok(());
                }
                // 条件上下文（`if`／`while`）：不保留值 ⇒ 由 `emit_condition_jump_to` 走另一条路；
                // 这里只处理"值上下文"
                let end = self.new_label();
                // 骨架指令取**整个布尔表达式**的跨度（实测；操作数的 `LOAD` 仍各自取）
                let saved_scaffold = self.boolop_scaffold_span;
                self.boolop_scaffold_span = Some(expression.span());
                for value in &values[..values.len() - 1] {
                    self.emit_test_value(value, !*conjunction, end, None)?;
                }
                self.boolop_scaffold_span = saved_scaffold;
                self.emit_operand(values.last().expect("`and`／`or` 至少一个操作数"))?;
                self.mark_label(end);
                Ok(())
            }
            Expression::Not(_, span) => {
                // **`not` 的三种下场**（逐条实测）：
                //   `not <名字等>`       ⇒ `TO_BOOL; UNARY_NOT`
                //   `not (a is b)`／`in` ⇒ **翻转比较**（`IS_OP 1`／`CONTAINS_OP 1`，不"产出布尔再取反"）
                //   `not (a < b)`        ⇒ 比较带 `bool(...)` 位（`|16`）＋ `UNARY_NOT`
                //   双重 `not` **抵消**（偶数个：只留 `TO_BOOL`／只留 `bool(...)` 位，无 `UNARY_NOT`）
                if let Some(folded) = fold_constant(expression)? {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    match folded {
                        Constant::Int(value) if (0..=255).contains(&value) => {
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                                value as u8,
                            );
                        }
                        other => {
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, other));
                        }
                    }
                    return Ok(());
                }
                // 数 `not` 的个数并剥掉（偶数个相互抵消）
                let mut depth = 0usize;
                let mut operand: &Expression = expression;
                let mut deepest_span = *span;
                while let Expression::Not(inner, inner_span) = operand {
                    depth += 1;
                    deepest_span = *inner_span;
                    operand = inner;
                }
                let odd = depth % 2 == 1;
                // **跨度**（实测）：奇数个 `not` ⇒ 取**最外层**那段的；偶数个（相互抵消）⇒ 取
                // **最内层** `not` 那段的（`x = not not a` 的 `TO_BOOL` 是 `(1,1,8,13)`）
                let not_span = if odd { *span } else { deepest_span };
                match operand {
                    Expression::Compare(left, operator, right, _compare_span) => {
                        let flipped = match operator {
                            CompareOperator::Is => CompareOperator::IsNot,
                            CompareOperator::IsNot => CompareOperator::Is,
                            CompareOperator::In => CompareOperator::NotIn,
                            CompareOperator::NotIn => CompareOperator::In,
                            other => *other,
                        };
                        let identity = matches!(
                            operator,
                            CompareOperator::Is
                                | CompareOperator::IsNot
                                | CompareOperator::In
                                | CompareOperator::NotIn
                        );
                        // 推进之后，比较**整段**是那个 `not` 表达式（实测 `x = not a is b`
                        // 的 `IS_OP` 位置是 `(1,1,4,14)`＝整个 `not a is b`）
                        if identity {
                            // `is`／`in` 族：奇数翻参数、偶数原样；**都不"产出布尔再取反"**
                            let chosen = if odd { flipped } else { *operator };
                            // `is`／`in` 族**一律取最外层**那段的跨度（实测 `x = not not a is b`
                            // 的 `IS_OP` 是 `(1,1,4,18)`）
                            self.emit_compare(left, &chosen, right, *span)?;
                        } else {
                            // `COMPARE_OP` 族：一律带 `bool(...)` 位；奇数再补 `UNARY_NOT`
                            self.emit_compare_with_bool(left, operator, right, not_span)?;
                            if odd {
                                self.emit_at(
                                    not_span,
                                    opcode::opcode("UNARY_NOT").expect("UNARY_NOT 在表里"),
                                    0,
                                );
                            }
                        }
                    }
                    _ => {
                        self.emit_expression(operand)?;
                        self.emit_at(not_span, opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"), 0);
                        if odd {
                            self.emit_at(
                                not_span,
                                opcode::opcode("UNARY_NOT").expect("UNARY_NOT 在表里"),
                                0,
                            );
                        }
                    }
                }
                Ok(())
            }
            Expression::Binary(operator, left, right, span) => {
                // **常量折叠**（实测三条规则）：只有最左叶子进常量表；结果是小整数走
                // `LOAD_SMALL_INT` 不进表；否则该常量在收尾之后才登记。**折叠出的加载位置是
                // 整段表达式**（实测 `x = 1 + 2` ⇒ `(1,1,4,9)`）。
                if let Some(folded) = fold_constant(expression)? {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    match folded {
                        Constant::Int(value) if (0..=255).contains(&value) => {
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_SMALL_INT")
                                    .expect("LOAD_SMALL_INT 在表里"),
                                value as u8,
                            );
                        }
                        other => {
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, other));
                        }
                    }
                    return Ok(());
                }
                self.emit_two_operands(left, right)?;
                let symbol = operator.symbol();
                let index = crate::opcode::get_nb_ops()
                    .iter()
                    .position(|entry| entry.1 == symbol)
                    .unwrap_or_else(|| panic!("nb_ops 里应当有 {symbol}"))
                    as u8;
                // 实测：`BINARY_OP` 的位置是**整段 `a op b`**
                self.emit_at(
                    *span,
                    opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                    index,
                );
                Ok(())
            }
            Expression::Unary(operator, operand, span) => {
                // 常量折叠：`x = -5` ⇒ `LOAD_CONST -5`（折叠规则同二元：最左叶子也进表）
                if let Some(folded) = fold_constant(expression)? {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    match folded {
                        Constant::Int(value) if (0..=255).contains(&value) => {
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                                value as u8,
                            );
                        }
                        other => {
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, other));
                        }
                    }
                    return Ok(());
                }
                self.emit_expression(operand)?;
                // `+x` 在 3.14 里**不是** `UNARY_POSITIVE`（那条约 3.12 就没了）：
                // 实测是 `CALL_INTRINSIC_1 INTRINSIC_UNARY_POSITIVE`（下表按**名字**取，`BC-39`）
                let opcode_number = match operator {
                    UnaryOperator::Positive => opcode::opcode("CALL_INTRINSIC_1")
                        .expect("CALL_INTRINSIC_1 在表里"),
                    UnaryOperator::Negative => {
                        opcode::opcode("UNARY_NEGATIVE").expect("UNARY_NEGATIVE 在表里")
                    }
                    UnaryOperator::Invert => {
                        opcode::opcode("UNARY_INVERT").expect("UNARY_INVERT 在表里")
                    }
                };
                let argument = match operator {
                    UnaryOperator::Positive => crate::opcode::get_intrinsic1_descs()
                        .iter()
                        .position(|name| *name == "INTRINSIC_UNARY_POSITIVE")
                        .expect("intrinsic1 表里应当有 INTRINSIC_UNARY_POSITIVE")
                        as u8,
                    _ => 0,
                };
                self.emit_at(*span, opcode_number, argument);
                Ok(())
            }
        }
    }
}

// ---- 语法树 ----
