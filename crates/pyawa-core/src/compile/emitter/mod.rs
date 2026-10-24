//! `compile` 的子模块：字节码发射器（拆分自单文件时期；按域拆成 `emitter/` 目录）。
//!
//! 入口仍是 `Emitter::emit_statement`／`Emitter::emit_expression`（本文件里的分派）；
//! 各语句／表达式族在 `stmt_*`／`expr_*` 文件，发射原语在 `primitives`／`jumps` 等文件。

use super::*;

mod primitives;
mod jumps;
mod defs;
// **`nested_unit_names` 再导出**（task-12 ✓）：`compile::symbols` 的 `analyze_cells` 也要用
// （类体里**方法单元**引用的外层局部 ⇒ 外层函数得把它变 cell ✓）；`defs` 是私有子模块，
// 跨兄弟模块直接取不到 ⇒ 从 `emitter` 这一层 `pub(crate) use` 一次 ✓。
pub(crate) use self::defs::nested_unit_names;
mod patterns;
mod control;
mod compare;
mod fstrings;
mod stmt_compound;
mod stmt_defs;
mod stmt_imports;
mod stmt_assign;
mod stmt_misc;
mod expr_literals;
mod expr_names;
mod expr_comprehensions;
mod expr_fstrings;
mod expr_operators;
mod expr_yield;

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
    /// 每个标签**在哪儿创建的** ✓（第 359 轮；`#[track_caller]` 自动记 ✓）—— 只为诊断 ✓：
    /// "跳转目标标签 N 从未落点"这条内部断言必须能指出**是谁开了这个标签** ✗，否则只能干猜 ✓。
    pub(super) label_origins: Vec<&'static core::panic::Location<'static>>,
    /// 模块收尾还需不需要补 `LOAD_CONST None; RETURN_VALUE`。
    /// 实测：末尾的 `if/else` 两个分支都 `return` ⇒ **没有**可落到末尾的路径 ⇒ 参照不再补。
    pub(super) epilogue_needed: bool,
    /// **类体里收集到的注解声明** ✓（task-21 ✓，PEP 649）：`(名字, 注解来源, 跨度)` ✓ ——
    /// 类体收尾时统一发 `__annotate_func__` ✓（`annotate_unit` 那一族的类体版 ✓）。
    /// `TS-47`：来源是二选一（`Expression` ＝ 纯模式／回退；`Type` ＝ 扩展模式的编译期标签 ✓）。
    pub(super) class_annotations: Vec<(String, AnnotationSource, Span)>,
    /// **`TS-47` 的已标注变量表**（本作用域）：名字 ⇒ 编译期类型标签（`BC-24` 的形态）。
    /// 按**发射顺序**登记（`Statement::Annotation` 的 `Type` 源登记）；赋值时据此发
    /// `CHECK_ASSIGNMENT`（`BC-65`）。**纯模式永远为空**（`TS-5`：不产生任何检查）；
    /// 未标注的名字**不**登记（`TS-2`：不推断）。`Vec` 保序（重复标注按后者覆盖）。
    pub(super) assignment_labels: Vec<(String, Constant)>,
    /// **类体 `__annotate__` 单元的自由变量** ✓（task-24 泛型外壳 ✓）：注解里引用的类型参数名 ✓
    /// （声明序 ✓）⇒ 收尾造 `__annotate_func__` 时要压闭包元组 ＋ `SET_FUNCTION_ATTRIBUTE 8` ✓。
    /// 普通类是空表 ✓（行为不变 ✓）。
    pub(super) annotate_freevars: Vec<String>,
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
    /// **已解出的跳转记录** ✓（第 630 轮）：`(自身词位, size, 目标词位, 是否向后, opcode 实参字节下标)` ——
    /// 加宽会移动码元 ✗，所以留到重建之后按**新坐标**重算实参 ✓（不然落点短一截 ✗）。
    pub(super) resolved_jumps: Vec<(usize, usize, usize, bool, usize)>,
    /// **本轮被加宽的跳转** ✓：`词位 → 完整实参` ✓（重建时写前缀的高位字节 ✓）。
    pub(super) widen_high: Vec<Option<u16>>,
    /// **本轮平移表** ✓：`shift[旧词位]` ＝ 该词位**之前**插了几个 `EXTENDED_ARG` ✓。
    pub(super) widen_shift: Vec<usize>,
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
    /// **`with` 体在值栈上留下的格数**（＝ `2 × items.len()`：上下文 ＋ `__exit__`）。
    ///
    /// **为什么与 `handler_depth` 分开**：两者都是异常表 `depth` 要加上的额外格数，但
    /// `handler_depth` 另有用途 —— `break`／`continue` **逐层发 `POP_EXCEPT`** 的循环
    /// （`for _ in frame.handler_depth..self.handler_depth`）只该数**处理器**的
    /// `PUSH_EXC_INFO` 格；`with` 的格由退出调用 `CALL 3` 收走，**不是** `POP_EXCEPT` 的对象
    /// ⇒ 若把 `with` 的格并进 `handler_depth`，`for: with: break` 会多发 `POP_EXCEPT`。
    /// 所以这里单列一项，只在 `record_exception` 记账异常表 `depth` 时加上。
    pub(super) with_depth: usize,
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
    pub(super) fn emit_statement(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
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
            Statement::Match { .. } | Statement::With { .. } | Statement::Try { .. } | Statement::For { .. } | Statement::While { .. } | Statement::If { .. } => self.emit_statement_compound(statement, rest),
            Statement::Class { .. } | Statement::Def { .. } | Statement::TypeAlias { .. } => self.emit_statement_defs(statement, rest),
            Statement::Import { .. } | Statement::ImportFrom { .. } => self.emit_statement_imports(statement, rest),
            Statement::Annotation { .. } | Statement::AssignChained { .. } | Statement::AssignTuple { .. } | Statement::AugAssign { .. } | Statement::Assign { .. } | Statement::NullCoalesceAssign { .. } | Statement::AssignSubscript { .. } | Statement::AssignAttr { .. } => self.emit_statement_assign(statement, rest),
            Statement::Yield (..) | Statement::YieldFrom (..) | Statement::Break (..) | Statement::Continue (..) | Statement::Global (..) | Statement::Delete { .. } | Statement::Assert { .. } | Statement::Pass (..) | Statement::Expression (..) | Statement::NonLocal (..) | Statement::Raise { .. } | Statement::Return (..) => self.emit_statement_misc(statement, rest),
        }
    }

    pub(super) fn emit_expression(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Float (..) | Expression::Map (..) | Expression::TupleLiteral (..) | Expression::SliceLiteral { .. } | Expression::List (..) | Expression::Constant (..) | Expression::SetLiteral (..) | Expression::BigInt (..) | Expression::Int (..) | Expression::Str (..) | Expression::Bytes (..) => self.emit_expression_literals(expression),
            Expression::Subscript (..) | Expression::ChainSubject (..) | Expression::Lambda { .. } | Expression::Attribute (..) | Expression::Name (..) | Expression::Call { .. } | Expression::Starred (..) | Expression::Walrus { .. } => self.emit_expression_names(expression),
            Expression::Comprehension { .. } => self.emit_expression_comprehensions(expression),
            Expression::FString { .. } => self.emit_expression_fstrings(expression),
            Expression::NullCoalesce (..) | Expression::SafeChain { .. } | Expression::ChainedCompare { .. } | Expression::Conditional { .. } | Expression::Compare (..) | Expression::BoolOp { .. } | Expression::Not (..) | Expression::Binary (..) | Expression::Unary (..) => self.emit_expression_operators(expression),
            Expression::Yield (..) | Expression::YieldFrom (..) => self.emit_expression_yield(expression),
        }
    }
}
