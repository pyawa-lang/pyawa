//! `emitter` 的 `control` 域：块与控制流骨架（`with` 清理、异常表、真假判定、作用域收尾）。

use super::*;

impl Emitter {
    /// **`with` 的一项退出调用**：三条 `LOAD_CONST None` ＋ `CALL 3` ＋ `POP_TOP`，位点＝该项的
    /// 上下文跨度（正常路径与**体内 `return` 的复制件**共用 ⇒ 一处真相）。
    pub(crate) fn emit_with_exit_call(&mut self, span: Span, none_index: usize) {
        for _ in 0..3 {
            self.emit_indexed(span, "LOAD_CONST", none_index);
        }
        self.emit_at(span, opcode::opcode("CALL").expect("CALL 在表里"), 3);
        self.emit_at(span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
    }

    /// 按作用域存一个名字：模块／类体走 `STORE_NAME`，函数里走 `STORE_FAST <槽>`。

    /// 发射 `with` 的**清理块**（冷块）。返回"重放的余部＋收尾是否终止"。
    pub(crate) fn emit_with_cleanups(&mut self, plan: &WithCleanupPlan) -> Result<bool, CompileError> {
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
                terminated &= self.emit_rest_and_tail(&plan.rest, plan.span, None)?;
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

    /// **重放"余部 ＋ 作用域收尾"**（块结构模型的退出路径）：余部不终止时接收尾；
    /// 都没有 ⇒ 跳到本层块尾（不落进后面的块）。
    /// `containing_depth`：**循环语句所在块的深度**（只有 `Break` 传；其余调用点传 `None` 保持原行为 ✓）。
    /// 深度 > 1（循环嵌在 `if`／`with`／`try` 的体里）⇒ 余部抄完后跳**那个块的块尾** ✓，
    /// 而不是发作用域收尾 ✗（第 410 轮，task-21 G1 ✓）。
    pub(crate) fn emit_rest_and_tail(
        &mut self,
        rest: &[Statement],
        position: Span,
        containing_depth: Option<usize>,
    ) -> Result<bool, CompileError> {
        self.emit_block(rest, false)?;
        if block_terminates(rest) {
            return Ok(true);
        }
        // **先看循环所在的块**（第 410 轮，task-21 G1 ✓）：循环嵌在更深块里（深度 > 1）⇒
        // 抄完余部后跳**那个块的块尾** ✓（参照 `dis`：`POP_TOP; JUMP_FORWARD` 到 if 之后 ✓）。
        // 这一条必须**排在"外层循环回跳"之前** ✗：外层循环存在、但中间还夹着一层 `if`／`with` 时，
        // 余部只到"那个块的末尾"为止 ⇒ 直接回跳外层会**跳过块后的代码** ✗（实测
        // `for outer: if c: for i: break` 之后的 `print` 被跳过／`while True: if c: for i: break`
        // 直接**死循环** ✗）。先前两条也都没有 depth 判断 ⇒ 发作用域收尾 ⇒ 函数提前 return ✗。
        if let Some(depth) = containing_depth {
            if depth > 1 {
                if let Some(end) = self.block_end_labels.get(depth - 1).copied() {
                    self.emit_jump(
                        position,
                        opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                        end,
                    );
                    return Ok(false);
                }
            }
        }
        // **有外层循环 ⇒ 抄完 `rest` 之后回到外层的续点** ✓（第 267 轮修 ✗）：
        // `break` 的调用方已 `loops.pop()` 弹掉**内层**帧 ✓ ⇒ 此刻若有循环帧，它就是**外层** ✓；
        // 外层那个迭代器从未被弹 ✓ ⇒ 跳到它的 `continue_target`（`for`＝`FOR_ITER`、`while`＝条件起点 ✓）
        // 时栈契约正好对得上 ✓（跳到块尾会 `StackUnderflow` ✗ —— 第 264 轮实测 ✓）。
        if let Some(outer) = self.loops.last().cloned() {
            self.emit_directed_jump(
                    position,
                    opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                    outer.continue_target,
                    true,
                );
            return Ok(false);
        }
        // **作用域收尾只对"尾块"发** ✓（task-17 ✓）：`if/else` 的**非尾**分支体里最后一条是
        // `with` 时（`emit_block(else_body, branch_tail=false)` ⇒ 这里的 `block_tail` 是假 ✗），
        // 先前不看它 ⇒ 分支里的 `with` 正常出口发出 `LOAD_CONST None; RETURN_VALUE` ⇒
        // **提前 return None** ✗，后面真正的 `return` 成了死代码 ✗（官方套件 135 个测试签名
        // ＋ `test.support` 导入链卡在这一格 ✓）。非尾块改走下面的兜底：跳 `block_end_labels.last()`。
        //
        // **`break` 的退出路径例外** ✓：它重放的 `rest` 在语义上接回**作用域层**继续走
        // （`containing_depth = Some(1)`；嵌套的 depth > 1 已在上面提前返回）⇒ 收尾照发 ✓
        // —— 否则 `for …: break` 这类会退化成"跳块尾"，栈契约与参照都不对 ✗。
        if (self.block_tail || containing_depth.is_some())
            && self.emit_scope_tail(self.last_span)
        {
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
    pub(crate) fn emit_scope_tail(&mut self, span: Span) -> bool {
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
    pub(crate) fn emit_implicit_return(&mut self) {
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
    pub(crate) fn next_read_slot(
        &self,
        element: &Expression,
        generators: &[Generator],
        index: usize,
    ) -> Option<usize> {
        // **定点修复（第 213 轮 ✗→✓）**：有 `if` 时**只能**由**条件**的最左名决定 ✓ ——
        // 先前条件的最左名不是目标名就**回退到元素** ✗ ⇒ 条件里含调用时（如 `if len(n)`），
        // 条件先算的是全局 `len` ✗，而 `STORE_FAST_LOAD_FAST` 预压的 `n` **没人用** ✗
        // ⇒ 栈上多留一项 ⇒ 元素处的 `LIST_APPEND 2` 看到的"容器"位置错 ✗
        //（最小复现 `[n for n in xs if len(n)]` ⇒ `LIST_APPEND 的容器位置／类型不符` ✓）。
        let candidate = if index + 1 < generators.len() {
            leftmost_name(&generators[index + 1].iterable)
        } else if let Some(condition) = generators[index].conditions.first() {
            leftmost_name(condition)
        } else {
            leftmost_name(element)
        }?;
        // cell（含形参 cell）／自由变量不能走"裸槽"快路 ✗（槽里是 cell 对象 ✓）
        self.plain_local_slot(candidate)
    }

    /// 发推导式的**元素**：列表／集合是一条表达式；字典是"键 ＋ 值"，两边最左都是局部名时打成
    /// 超指令 `LOAD_FAST_BORROW_LOAD_FAST_BORROW <键槽, 值最左槽>`（实测 `{k: k + 1 …}` 就是这个形状），
    /// 值的最左那次读取由它抵消。
    pub(crate) fn emit_comprehension_element(
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
        // **只有"裸名字"才走融合快路** ✓（第 614 轮真 bug 修 ✗）：`leftmost_name` 会**钻进**
        // `Attribute`／`Subscript` ✓（`compile.rs:1960` ✓）⇒ 对 `{item.name: item for item in items}`
        // 它给出的是 **`item`** ✗（真键是 `item.name` ✓）⇒ 键发错、`pending_fused_load` 记账也错 ✗ ⇒
        // 栈不平 ✓（实测 `帧操作失败：StackUnderflow` ✓；`re/_constants.py` 的 `_makecodes` 同型 ✓）。
        // **只对"键"要求它是裸名字** ✓（第 614 轮 ✓）：`leftmost_name` 会钻进 `Attribute`／`Subscript`
        //（`compile.rs:1960` ✓）⇒ 对 `{item.name: item for item in items}` 它给出 `item` ✗ ⇒ 融合快路会把
        // 属性读**整个漏掉** ✗ ⇒ 栈不平 ✓（实测 `帧操作失败：StackUnderflow` ✓；`re/_constants.py` 同型 ✓）。
        // **值那半照参照保留** ✓：参照对 `{k: k + 1 …}` 融合的是**值的 leftmost 名字**（`k` ✓）⇒
        // 逐字节夹具（`y = {k: k + 1 for k in s if k}` ✓）就是这么要求的 ✓，不能一起闸掉 ✗。
        let key_is_plain_name = matches!(element, Expression::Name(..));
        let key_slot = if key_is_plain_name {
            leftmost_name(element).and_then(|name| self.plain_local_slot(name))
        } else {
            None
        };
        let value_slot = leftmost_name(value).and_then(|name| self.plain_local_slot(name));
        // **键已经由 `STORE_FAST_LOAD_FAST` 压回来了**（无 `if` 子句时就是这种情况）⇒ 它就是键，
        // 别再为键发一次读（否则栈上会多一份、`MAP_ADD` 取错位置）
        if let (Some(pending), Some(key_slot)) = (self.pending_fused_load, key_slot) {
            if pending == key_slot {
                self.pending_fused_load = None;
                return self.emit_expression(value);
            }
        }
        // **前提：两个槽号都要装得进 4 位** ✓（task-21 第 3 轮横扫补齐 ✗ —— 与 `pack`／
        // `fused_pair`／`fused_tail` 那几处同一条规矩 ✓）：装不下就**不融合** ✓，走下面的
        // 两条独立加载 ✓（参照同样只在 <16 时融合 ✓）。
        if let (Some(key_slot), Some(value_slot)) = (key_slot, value_slot) {
            if key_slot > 0x0F || value_slot > 0x0F {
                self.emit_expression(element)?;
                return self.emit_expression(value);
            }
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
    pub(crate) fn flush_pending_cleanups(&mut self) -> Result<(), CompileError> {
        for cleanup in core::mem::take(&mut self.pending_cleanups) {
            let span = cleanup.scaffold;
            self.mark_label(cleanup.label);
            let target = self.unit.code.len();
            // 前两条是**合成指令**（参照给全 `None`），后面的还原序列有位点（`BC-4` 扩）
            self.emit_named_none("SWAP", 2);
            self.emit_named_none("POP_TOP", 0);
            self.emit_indexed(span, "SWAP", cleanup.depth);
            for slot in cleanup.slots.iter().rev() {
                self.emit_indexed(span, "STORE_FAST", *slot);
            }
            self.emit_at(span, opcode::opcode("RERAISE").expect("RERAISE 在表里"), 0);
            self.record_exception(cleanup.region_start, cleanup.region_end, target, self.handler_depth + 2, false);
        }
        Ok(())
    }

    pub(crate) fn emit_block(
        &mut self,
        statements: &[Statement],
        tail: bool,
    ) -> Result<(), CompileError> {
        // **块深度** ✓：深度 1 ＝ 作用域自己的体 ✓（第 202 轮修 ✗）。
        self.block_depth += 1;
        // **保存"作用域还要不要收尾"** ✓（第 357 轮真 bug 修 ✗）：这个标志说的是**作用域**末尾
        // 要不要补 `LOAD_CONST None; RETURN_VALUE` ✓，而 `raise`／`return` 两条语句臂会把它置假 ✗
        // —— 那是**对"本块"说的** ✓。嵌套块（`if` 的体、`for` 的体、`try` 的套体 … ✓）里的终止
        // **连坐不了**外层 ✓ ⇒ 出块时**还原** ✓。先前不还原 ✗ ⇒ 末尾语句的**内部**终止过 ⇒
        // 标志留在假 ⇒ **漏发收尾** ⇒ 掉底（`FellOffEnd` ✓，上限榜最大的族 **104** 个模块 ✓）。
        // 实测原形（最小复现 ✓）：
        //   def m(x):
        //       if x:
        //           raise TypeError("boom")
        //   m(0)            ← 参照给 None；我们掉底 ✗
        //（`Lib/enum.py` 的 `EnumType._check_for_existing_members_` 同形 ✓。）
        let saved_epilogue_needed = self.epilogue_needed;
        let nested_block = self.block_depth > 1;
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
            if crate::diag::flag("PYAWA_LOOPLASTIF_DEBUG") {
                eprintln!(
                    "[loop_last_if] in_loop_body={in_loop_body} index={index} last={last_index} if={}",
                    matches!(statement, Statement::If { .. })
                );
            }
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
        // **嵌套块出块时还原"作用域收尾"标志** ✓（第 357 轮真 bug 修 ✗）：块内的 `raise`／`return`
        // 只说"**本块**不落到末尾" ✓，替不了**作用域**的账 ✓。深度 1（作用域自己的体）**不还原** ✓
        // —— 那一条正是要反映作用域末尾的可达性 ✓（`compile_scope` 会读它 ✓）。
        if nested_block {
            self.epilogue_needed = saved_epilogue_needed;
        }
        self.block_tail = outer_tail;
        self.block_end_labels.pop();
        self.mark_label(block_end);
        Ok(())
    }

    /// 压一个**boolop 的直接操作数**：裸的局部名用 **`LOAD_FAST`**（拥有加载，因为 `COPY` 要
    /// 求有两份引用）；其余交给普通发射（子表达式照旧走借用加载，实测 `(a < b) and c` 里那对
    /// 仍是 `LOAD_FAST_BORROW_LOAD_FAST_BORROW`）。
    pub(crate) fn emit_operand(&mut self, value: &Expression) -> Result<(), CompileError> {
        if let Expression::Name(name, span) = value {
            // **融合读取**：`STORE_FAST_LOAD_FAST` 压回的那份值先看这里（这条快路径会**绕过**
            // 普通发射的 `Name` 分支，第 235 轮实测：函数作用域的推导式因此多压了一份元素值）
            if let Some(slot) = self.pending_fused_load.take() {
                if self.unit.varnames.get(slot).is_some_and(|item| item == name) {
                    return Ok(());
                }
            }
            if self.kind == ScopeKind::Function
                && self.unit.varnames.iter().any(|item| item == name)
                // **形参 cell 也不走这条** ✓（同 `Assign` 那条：槽里是 cell 对象 ✓ ⇒ 要走 `LOAD_DEREF` ✓）。
                && self.deref_slot(name).is_none()
            {
                let slot = self.slot_of(name);
                self.emit_indexed(*span, "LOAD_FAST", slot);
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
    pub(crate) fn emit_test_value(
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
            // **本子式短路时的值极性** ✓：`and` 短路值必**假** ✓、`or` 短路值必**真** ✓。
            let short = !*conjunction;
            // **落点要用调用方给的 `cleanup`** ✓（本式是外层的**末操作数**时外层会给 ✓）：
            // 先前**无视 `cleanup`**、一律新开 `fresh` ✗ ⇒ 传入的标签**永不落点** ⇒ 回填阶段
            // panic ✗（实测 `a and (b and c) or d` ⇒ `跳转目标标签 2 从未落点` ✓）。
            // 且"本子式短路极性 **==** 外层要求的极性"时应**直接跳 `target`**（保持值 ✓）：
            // 先前一律跳 `fresh` ✗ ⇒ 同极性嵌套语义错 ✗（实测 `(a and b) and c` 在 `a` 假时
            // 错返回 `c` ✗；`a and (b and c) and d`／`a or (b and c) and d`／`(a or b) or c` 同理 ✗）。
            let fresh = cleanup.unwrap_or_else(|| self.new_label());
            let dest = if short == jump_if_true { target } else { fresh };
            for inner in &values[..values.len() - 1] {
                // 内层操作数按**内层自身的极性**跳（`and` ⇒ 假就跳、`or` ⇒ 真就跳）
                self.emit_test_value(inner, short, dest, None)?;
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
    pub(crate) fn emit_test_bare(
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
            if *conjunction {
                // `and`：非末操作数为**假** ⇒ 整个 `and` 为假 ⇒ 跳"这个 and 的假出口"（参照实测：
                // `(a and b) or c` 里 `a` 假 ⇒ `POP_JUMP_IF_FALSE` 落到 **下一个操作数 `c`** ✓）。
                // **`and` 的"假出口"** ✓（照参照：`(a and b) or c` 里 `a` 假 ⇒ 落到下一个操作数 `c` ✓）：
                //   * `jump_if_true == false`（`target` 就是**假出口** ✓）⇒ 直接跳 `target` ✓；
                //   * `jump_if_true == true` ⇒ 这个 `and` 的"假"意味着**外层还要继续** ✓ ⇒ 跳"本子式之后"
                //     —— 调用方给了 `cleanup` 就用它 ✓（那是它自己攒的落点 ✓），否则**当场开一个**
                //     并在本分支末尾落点 ✓（那就是**落下去**的位置 ✓，参照的 `L1` 正是这一格 ✓）。
                let after = if jump_if_true && cleanup.is_none() {
                    Some(self.new_label())
                } else {
                    None
                };
                let dest = if jump_if_true {
                    cleanup.or(after).unwrap_or(target)
                } else {
                    target
                };
                for inner in &values[..values.len() - 1] {
                    self.emit_test_bare(inner, false, dest, None)?;
                }
                let last = values.last().expect("`and` 至少一个操作数");
                let outcome = self.emit_test_bare(last, jump_if_true, target, cleanup)?;
                if let Some(label) = after {
                    self.mark_label(label);
                }
                return Ok(outcome);
            }
            let fresh = self.new_label();
            for inner in &values[..values.len() - 1] {
                // `or`：非末操作数为**真** ⇒ 跳 `fresh`（本子式之后 ✓）；为**假** ⇒ 继续下一个操作数 ✓
                let cont = self.new_label();
                self.emit_test_bare(inner, true, fresh, Some(cont))?;
                self.mark_label(cont);
            }
            let last = values.last().expect("`or` 至少一个操作数");
            let outcome = self.emit_test_bare(last, jump_if_true, target, Some(fresh))?;
            if let Some(label) = cleanup {
                self.mark_label(label);
            }
            return Ok(outcome);
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
    pub(crate) fn record_exception(
        &mut self,
        start: usize,
        end: usize,
        target: usize,
        depth: usize,
        lasti: bool,
    ) {
        if crate::diag::flag("PYAWA_TRY_DEPTH_DEBUG") {
            eprintln!(
                "[try_depth] 记账 depth={depth} for_depth={} handler_depth={} with_depth={}",
                self.loops.iter().filter(|f| f.is_for).count(),
                self.handler_depth,
                self.with_depth
            );
        }
        // 一处真相（第 374 轮）：循环体里的 try，其异常表 depth 还要算上每层外层 for 的 1 项。
        // 依据（第 372 轮并排打印实测）：循环体执行时栈上只有迭代器（当前值已被 STORE_FAST 收走），
        // 记账 4 与 实际 2 的差正是这两层 for 各多算的 1 项。
        // `with` 体同理：体期间栈上有 `2 × 项数` 格（上下文 ＋ `__exit__`），记账时补进 `with_depth`。
        let depth = depth + self.loops.iter().filter(|f| f.is_for).count() + self.with_depth;
        self.exception_entries.push((start, end, target, depth, lasti));
    }

    /// 处理块段（处理块里再抛要落到清理块）：先记字节区间 ＋ "有没有 `as 名字`"，
    /// `finish_handler_segments` 再补目标（有名字的落到**名字清理**那条，没有的落到 `COPY 3`）。
    pub(crate) fn record_handler_segment(&mut self, start: usize, end: usize, has_name: bool) {
        self.handler_segments.push((start, end, has_name));
    }

    /// 给所有处理块段补上清理块目标（`depth` 1、`lasti` 打开，与参照的 cleanup 条目同形）。
    /// `name_cleanup` 是"名字清理"那段的起点（有 `as 名字` 的处理块落到它，其余落到 `cleanup`）。
    pub(crate) fn finish_handler_segments(&mut self, cleanup: usize, name_cleanup: Option<usize>) {
        for (start, end, has_name) in core::mem::take(&mut self.handler_segments) {
            let target = match (has_name, name_cleanup) {
                (true, Some(offset)) => offset,
                _ => cleanup,
            };
            self.record_exception(start, end, target, self.handler_depth + 1, true);
        }
    }

    /// 把异常表条目编码成 `BC-54` 的字节串（4 个 6-bit varint／条，**码元**为单位）。
    pub(crate) fn encode_exceptiontable(&self) -> Vec<u8> {
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

    /// **`yield from`** 的公共发射（第 315 轮）：语句形态末尾补 `POP_TOP`（值丢掉）、表达式形态不补。
    /// 形状照参照实测：`<被委派的表达式>; GET_YIELD_FROM_ITER; [L1] LOAD_CONST None; SEND <L2>;
    /// YIELD_VALUE 1; RESUME 2; POP_TOP; JUMP_BACKWARD_NO_INTERRUPT <L1>; [L2] END_SEND`。
    /// 中间那条 `POP_TOP` 与 `async for` 同款（本层 `YIELD_VALUE`／`RESUME` 的语义与参照不同，
    /// 不收走恢复时送进来的值，下一轮就会把它当迭代器），**如实登记**。
    pub(crate) fn emit_yield_from(
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

}
