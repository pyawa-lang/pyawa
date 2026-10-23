//! `emitter` 的 `stmt_compound` 域：复合语句（`if`／`while`／`for`／`with`／`try`／`match`）的发射。

use super::*;

impl Emitter {
    /// 复合语句分派：每个语句族一个发射函数（见本文件下方各 `emit_statement_*`）。
    pub(crate) fn emit_statement_compound(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
            Statement::Match { .. } => self.emit_statement_match(statement),
            Statement::With { .. } => self.emit_statement_with(statement, rest),
            Statement::Try { .. } => self.emit_statement_try(statement, rest),
            Statement::For { .. } => self.emit_statement_for(statement, rest),
            Statement::While { .. } => self.emit_statement_while(statement, rest),
            Statement::If { .. } => self.emit_statement_if(statement, rest),
            _ => unreachable!("emit_statement_compound 只接收本族的 Statement"),
        }
    }

    /// `Match` 语句的发射（从 `emit_statement_compound` 的臂原样搬来）。
    pub(crate) fn emit_statement_match(&mut self, statement: &Statement) -> Result<(), CompileError> {
        match statement {
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
            _ => unreachable!("emit_statement_match 只接收 Statement::Match"),
        }
    }

    /// `With` 语句的发射（从 `emit_statement_compound` 的臂原样搬来）。
    pub(crate) fn emit_statement_with(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
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
                        self.emit_indexed(context_span, "LOAD_CONST", none_index);
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
                    if let Some(target) = target {
                        // **存储走 `for` 目标那条统一入口**（第 405 轮）：名字 ⇒ 按作用域存；
                        // 括号元组 ⇒ `UNPACK_SEQUENCE`（位点＝**组**的跨度 ✓，参照 `dis` 实测）
                        // ＋ 逐名字存 ✓ —— `with … as (a, b)` 直接复用 ✓。
                        self.emit_for_target(target)?;
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
                // **体期间栈上多出 `2 × items.len()` 格**（上下文 ＋ `__exit__`）⇒ 体内嵌套
                // `try`／`with` 的异常表 `depth` 必须算上它们（`record_exception` 读 `with_depth`）。
                // 少了这层记账 ⇒ 体内 `try` 的 `depth` 少 2 ⇒ 抛出时 `dispatch_raise` 把 with 的
                // 两格一起弹掉 ⇒ 退出调用 `CALL 3` 栈空（StackUnderflow）。
                // 记在单独的 `with_depth` 而不是 `handler_depth`：后者还给 `break`／`continue`
                // 数 `POP_EXCEPT` 用，`with` 的格不能算进去（见字段注释）。
                let saved_with_depth = self.with_depth;
                self.with_depth += 2 * items.len();
                self.emit_block(body, false)?;
                self.with_depth = saved_with_depth;
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
                    terminated = self.emit_rest_and_tail(rest, *span, None)?;
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
            _ => unreachable!("emit_statement_with 只接收 Statement::With"),
        }
    }

    /// `Try` 语句的发射（从 `emit_statement_compound` 的臂原样搬来）。
    pub(crate) fn emit_statement_try(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
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
                    self.emit_rest_and_tail(rest, *span, None)?
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
                        all_terminate &= self.emit_rest_and_tail(rest, *span, None)?;
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
                            self.emit_indexed_none("STORE_FAST", slot);
                            self.emit_indexed_none("DELETE_FAST", slot);
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
                self.record_exception(
                        body_start,
                        body_end,
                        handler_start,
                        self.handler_depth,
                        false,
                    );
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
                    self.record_exception(
                        finally_region_start,
                        finally_region_end,
                        finally_cleanup,
                        self.handler_depth + 1,
                        true,
                    );
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
            _ => unreachable!("emit_statement_try 只接收 Statement::Try"),
        }
    }

    /// `For` 语句的发射（从 `emit_statement_compound` 的臂原样搬来）。
    pub(crate) fn emit_statement_for(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
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
                    // **`LOAD_CONST` 要走带 `EXTENDED_ARG` 的那条** ✓（`as u8` 会截断 ✗；
                    // 这里当场入池 ✓ —— 与原来"先 `intern_constant` 再发"同一顺序 ✓）。
                    let none_index = self.intern_constant(Constant::None);
                    self.emit_indexed(iterable.span(), "LOAD_CONST", none_index);
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
                    self.emit_indexed(*for_span, "LOAD_FAST", slot);
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
                    self.emit_indexed(*target_span, "UNPACK_SEQUENCE", tuple_targets.len());
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
                            // **cell／free 目标同元组解包那条** ✗（槽里是 cell 对象 ✓ ⇒ 不能 `STORE_FAST`）。
                            && self.deref_slot(first).is_none()
                            && self.deref_slot(second).is_none()
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
                            // **cell／free 目标 ⇒ `STORE_DEREF`** ✓（task-25 ② 真 bug 修 ✓）：
                            // 这类目标的 localsplus 槽里放的是 **cell 对象** ✓ ⇒ `STORE_FAST` 会把它打掉 ✗
                            //（参照实测 `def f(xs): def g(): return x; for x in xs: …` ⇒ `cellvars=('x',)`
                            // ＋ `STORE_DEREF 2 x` ✓）；融合路（`STORE_FAST_LOAD_FAST`）对 cell 也不成立 ✓。
                            if let Some(deref) = self.deref_slot(target) {
                                self.emit_indexed(*target_span, "STORE_DEREF", deref);
                            } else {
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
                                            Statement::If { condition, then_body, else_body, .. }
                                                if else_body.is_empty()
                                                    // **条件的最左读必须是目标** ✓（第 412 轮，task-24 顺带 ✓）：
                                                    // 先前只看体里第一个 `yield` ✗ ⇒ `if len(n): yield n`
                                                    // 也融合 ✗ ⇒ `STORE_FAST_LOAD_FAST` 预压的 `n` 与
                                                    // 随后 `LOAD_GLOBAL len` 的次序相反 ✗ ⇒ 栈上顺序错 ⇒
                                                    // `list(x for x in xs if len(x))` 报
                                                    // `TypeError: 'str' object is not callable` ✗；
                                                    // 参照实测：条件以目标开头（`if n`／`if n > 0`／`if not n`）
                                                    // 才融合 `STORE_FAST_LOAD_FAST`，以全局名开头（`if len(n)`）
                                                    // 则发普通 `STORE_FAST` ✓。
                                                    && leftmost_name(condition)
                                                        .is_some_and(|name| name == target)
                                                    && then_body.first().is_some_and(yields_target)
                                        )
                                }
                                None => false,
                            };
                            // **槽 ≥ 16 不融合** ✓（同上：4 位装不下 ⇒ `as u8` 截断 ✗）。
                            let fuses = slot < 16
                                && matches!(iterable, Expression::Name(name, _) if name == ".0")
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
                                self.emit_indexed(*target_span, "STORE_FAST", slot);
                            }
                            }
                        }
                    }
                }
                self.loops.push(LoopFrame {
                    continue_target: loop_label,
                    handler_depth: self.handler_depth,
                    is_for: true,
                    rest: rest.to_vec(),
                    block_depth: self.block_depth,
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
            _ => unreachable!("emit_statement_for 只接收 Statement::For"),
        }
    }

    /// `While` 语句的发射（从 `emit_statement_compound` 的臂原样搬来）。
    pub(crate) fn emit_statement_while(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
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
                        handler_depth: self.handler_depth,
                        is_for: false,
                        rest: rest.to_vec(),
                        block_depth: self.block_depth,
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
                    handler_depth: self.handler_depth,
                    is_for: false,
                    rest: rest.to_vec(),
                    block_depth: self.block_depth,
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
            _ => unreachable!("emit_statement_while 只接收 Statement::While"),
        }
    }

    /// `If` 语句的发射（从 `emit_statement_compound` 的臂原样搬来）。
    pub(crate) fn emit_statement_if(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
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
                if crate::diag::flag("PYAWA_INVERTED_DEBUG") {
                    eprintln!(
                        "[inverted] loop_last_if={} else_empty={} loops_len={}",
                        self.loop_last_if, else_body.is_empty(), self.loops.len()
                    );
                }
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
            _ => unreachable!("emit_statement_if 只接收 Statement::If"),
        }
    }
}
