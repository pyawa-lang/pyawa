//! `emitter` 的 `patterns` 域：赋值／解包／`match` 模式／推导式目标的发射辅助。

use super::*;

/// 解包目标里**最后一个叶子**的跨度（嵌套组取组内最后一个 ✓）：赋值语句的收尾位点用它
/// （参照实测：`(a, b), (c, d) = …` 的隐式 `RETURN_VALUE` 位点是 `d` 那段 ✓）。
pub(crate) fn assign_target_leaf_span(target: &Expression) -> Span {
    match target {
        Expression::TupleLiteral(items, _) | Expression::List(items, _) => items
            .last()
            .map(assign_target_leaf_span)
            .unwrap_or_else(|| target.span()),
        Expression::Starred(inner, _) => assign_target_leaf_span(inner),
        other => other.span(),
    }
}

/// 推导式目标 ⇒ `for` 目标（生成器表达式那条要把它塞进一个临时 `Statement::For` ✓）。
pub(crate) fn comprehension_target_as_for_target(target: &ComprehensionTarget) -> ForTarget {
    match target {
        ComprehensionTarget::Name(name, span) => ForTarget::Name(name.clone(), *span),
        ComprehensionTarget::Group(items, span) => ForTarget::Group(
            items
                .iter()
                .map(comprehension_target_as_for_target)
                .collect(),
            *span,
        ),
        // **属性／下标目标**（task-45 ✓）：直接映射到 `ForTarget` 的同名形态（发射同口径 ✓）。
        ComprehensionTarget::Attribute(object, name, span) => {
            ForTarget::Attribute(Box::new(object.clone()), name.clone(), *span)
        }
        ComprehensionTarget::Subscript(object, key, span) => {
            ForTarget::Subscript(Box::new(object.clone()), Box::new(key.clone()), *span)
        }
    }
}

/// `for` 目标组里**第一个叶子名字**（组／嵌套组递归 ✓）——生成器表达式那条的
/// `Statement::For.target` 字段要用一个**真实的局部名**登记 ✓。
pub(crate) fn first_for_target_name(items: &[ForTarget]) -> Option<(String, Span)> {
    items.iter().find_map(|item| match item {
        ForTarget::Name(name, span) => Some((name.clone(), *span)),
        ForTarget::Group(items, _) => first_for_target_name(items),
        ForTarget::Starred(inner, _) => first_for_target_name(core::slice::from_ref(inner)),
        ForTarget::Attribute(..) | ForTarget::Subscript(..) => None,
    })
}

/// 推导式目标里的名字（按书写顺序，递归 ✓）——保存／还原那几个块用 ✓。
pub(crate) fn collect_comprehension_target_names<'a>(
    target: &'a ComprehensionTarget,
    out: &mut Vec<(&'a str, Span)>,
) {
    match target {
        ComprehensionTarget::Name(name, span) => out.push((name.as_str(), *span)),
        ComprehensionTarget::Group(items, _) => {
            for item in items {
                collect_comprehension_target_names(item, out);
            }
        }
        // 属性／下标目标**不占名字槽**（不绑定局部 ✓）
        ComprehensionTarget::Attribute(..) | ComprehensionTarget::Subscript(..) => {}
    }
}

/// 一项推导式目标的跨度（`UNPACK_SEQUENCE` 的位点用 ✓）。
pub(crate) fn comprehension_target_span_of(target: &ComprehensionTarget) -> Span {
    match target {
        ComprehensionTarget::Name(_, span)
        | ComprehensionTarget::Group(_, span)
        | ComprehensionTarget::Attribute(_, _, span)
        | ComprehensionTarget::Subscript(_, _, span) => *span,
    }
}


impl Emitter {
    pub(crate) fn store_target(&mut self, span: Span, name: &str) {
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
                    self.emit_indexed(span, "STORE_FAST", slot);
                }
            }
        }
    }

    /// **`match` 模式的判定**（第 290 轮）：命中 ⇒ 落到 `matched` ✓；不命中 ⇒ 跳 `next_case` ✓；
    /// **主语留在栈上** ✓（字面量那一档用 `COPY 1` 复制一份比 ✓，照参照 `dis` 实测 ✓）。
    pub(crate) fn emit_pattern_test(
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
            // **序列模式／映射模式**（task-37）：主语留在栈上（helper 自己 `COPY 1` 一份来测 ✓；
            // 失败路也把那份复制收干净 ⇒ 下一条 `case` 照常看到主语 ✓）。
            Pattern::Sequence { .. } => self.emit_sequence_pattern(pattern, next_case, matched, false),
            Pattern::Mapping { .. } => self.emit_mapping_pattern(pattern, next_case, matched, false),
            // 星号项只该在序列模式的 items 里（解析器保证 ⇒ 到这儿是内部错误）。
            Pattern::Star(_, span) => Err(CompileError::Unsupported(format!(
                "`*` 项只能出现在序列模式里（第 {} 行）",
                span.line_start
            ))),
            // **`as` 模式**（task-45 ✓）：照参照实测形状 —— 先 `COPY 1` 一份给子模式测，
            // 子模式命中后再把**主语**绑到名字（`STORE` 弹掉那份 copy ✓）。
            Pattern::As(inner, name, span) => {
                self.emit_named(*span, "COPY", 1);
                self.emit_pattern_test(inner, next_case, matched)?;
                self.store_target(*span, name);
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
    pub(crate) fn emit_class_pattern(
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
        self.emit_indexed(*span, "MATCH_CLASS", positional.len());
        self.emit_named(*span, "COPY", 1);
        let none_path = self.new_label();
        self.emit_jump(
            *span,
            opcode::opcode("POP_JUMP_IF_NONE").expect("表里有"),
            none_path,
        );
        self.emit_named(*span, "NOT_TAKEN", 0);
        if total > 0 {
            self.emit_indexed(*span, "UNPACK_SEQUENCE", total);
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
    pub(crate) fn emit_subpattern_test(
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
            // **`as` 模式**（task-45 ✓）：栈顶是待测值 ⇒ 先 `COPY 1` 给子模式，命中后
            // 把原值 `STORE` 绑定 ✓。
            Pattern::As(inner, name, span) => {
                self.emit_named(*span, "COPY", 1);
                self.emit_subpattern_test(inner, failure, on_match)?;
                self.store_target(*span, name);
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
            // **嵌套序列／映射**（task-37）：进入时栈顶是它的值；测完把它消费掉 ✓
            Pattern::Sequence { .. } => self.emit_sequence_pattern(pattern, failure, on_match, true),
            Pattern::Mapping { .. } => self.emit_mapping_pattern(pattern, failure, on_match, true),
            Pattern::Star(_, span) => Err(CompileError::Unsupported(format!(
                "`*` 项只能出现在序列模式里（第 {} 行）",
                span.line_start
            ))),
            // **子模式里的或模式**（task-37 补 ✓；参照实测形状 `_pyrepl/utils.py:242`）：
            // 每个备选先 `COPY 1` 复制一份值来测（测完那份被消费 ✓，原值留给下一个备选 ✓）；
            // 任一备选命中 ⇒ 丢掉原值、跳 `on_match` ✓；全不中 ⇒ 跳 `failure` ✓。
            Pattern::Or(alternatives, span) => {
                let success = self.new_label();
                for alternative in alternatives {
                    self.emit_named(*span, "COPY", 1);
                    let failed = self.new_label();
                    self.emit_subpattern_test(alternative, failed, success)?;
                    self.emit_jump(
                        *span,
                        opcode::opcode("JUMP_FORWARD").expect("表里有"),
                        success,
                    );
                    self.mark_label(failed);
                }
                // 全不中：**子模式的契约是"把值消费掉"** ✓ ⇒ 原值那份也弹掉再走失败 ✓
                //（否则失败路会把它留给下一条 case 的栈 ⇒ 污染 ✓）。
                self.emit_named(*span, "POP_TOP", 0);
                self.emit_jump(
                    *span,
                    opcode::opcode("JUMP_FORWARD").expect("表里有"),
                    failure,
                );
                self.mark_label(success);
                // 命中：原值那份也消费掉（子模式的契约 ✓）
                self.emit_named(*span, "POP_TOP", 0);
                self.emit_jump(
                    *span,
                    opcode::opcode("JUMP_FORWARD").expect("表里有"),
                    on_match,
                );
                Ok(())
            }
        }
    }

    /// 发**小整数常量**（`GET_LEN` 的比较用）：≤255 走 `LOAD_SMALL_INT` ✓，更大走 `LOAD_CONST`。
    fn emit_small_length(&mut self, span: Span, value: usize) {
        if value <= 255 {
            self.emit_named(span, "LOAD_SMALL_INT", value as u8);
        } else {
            let index = self.intern_constant(Constant::Int(value as i64));
            self.emit_indexed(span, "LOAD_CONST", index);
        }
    }

    /// 失败清理：先弹掉**还没测的项**，再按需弹掉保留的原值（嵌套时 ✓；顶层是主语、要留给下一条 `case` ✓）。
    fn emit_pattern_failure_cleanup(&mut self, span: Span, remaining: usize, consume_value: bool) {
        for _ in 0..remaining {
            self.emit_named(span, "POP_TOP", 0);
        }
        if consume_value {
            self.emit_named(span, "POP_TOP", 0);
        }
    }

    /// 映射模式的**键元组**：全字面量 ⇒ `LOAD_CONST (键…)`（参照同形 ✓）；含值模式 ⇒ 逐个求值再
    /// `BUILD_TUPLE` ✓。
    fn emit_mapping_keys(
        &mut self,
        span: Span,
        items: &[(Pattern, Pattern)],
    ) -> Result<(), CompileError> {
        let all_literals = items
            .iter()
            .all(|(key, _)| matches!(key, Pattern::Literal(_, _)));
        if all_literals {
            let mut constants: Vec<Constant> = Vec::with_capacity(items.len());
            for (key, _) in items {
                match key {
                    Pattern::Literal(constant, _) => constants.push(constant.clone()),
                    _ => unreachable!("上面刚判过全是字面量"),
                }
            }
            let index = self.intern_constant(Constant::Tuple(constants));
            self.emit_indexed(span, "LOAD_CONST", index);
            return Ok(());
        }
        for (key, _) in items {
            match key {
                Pattern::Literal(constant, key_span) => {
                    let index = self.intern_constant(constant.clone());
                    self.emit_indexed(*key_span, "LOAD_CONST", index);
                }
                Pattern::Value(expression, _) => self.emit_expression(expression)?,
                _ => unreachable!("解析器保证映射键只有字面量／值模式"),
            }
        }
        self.emit_indexed(span, "BUILD_TUPLE", items.len());
        Ok(())
    }

    /// **序列模式的判定**（task-37）。进入时栈顶是待测值；`consume_value` 为真时（嵌套）
    /// **连它一起消费**，为假时（顶层）**留着**（那是主语 ⇒ 下一条 `case` 也要用 ✓）。
    ///
    /// 形状照参照 3.14.4 实测：`MATCH_SEQUENCE; POP_JUMP_IF_FALSE; NOT_TAKEN; GET_LEN;
    /// LOAD_SMALL_INT n; COMPARE_OP 72(==)［有 star ⇒ 172(>=)］; POP_JUMP_IF_FALSE; NOT_TAKEN;
    /// UNPACK_SEQUENCE n`／`UNPACK_EX`／空序列 `POP_TOP`。**与参照的差别**（如实登记）：
    /// 参照把主语自己交给解包（成功路主语消失），这里 `COPY 1` 保留主语 ⇒ 与既有 `case` 骨架
    /// 的"主语一直留在栈上"约定一致 ✓（指令序列多一条 `COPY 1`）。
    pub(crate) fn emit_sequence_pattern(
        &mut self,
        pattern: &Pattern,
        failure: usize,
        on_match: usize,
        consume_value: bool,
    ) -> Result<(), CompileError> {
        let Pattern::Sequence { items, span } = pattern else {
            unreachable!("只给序列模式调")
        };
        let span = *span;
        let total_slots = items.len();
        let star_index = items
            .iter()
            .position(|item| matches!(item, Pattern::Star(_, _)));
        // [原值] ⇒ [原值, 副本]（副本交给判定／解包 ✓）
        self.emit_named(span, "COPY", 1);
        self.emit_named(span, "MATCH_SEQUENCE", 0);
        let head_failure = self.new_label();
        self.emit_jump(
            span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            head_failure,
        );
        self.emit_named(span, "NOT_TAKEN", 0);
        if let Some(index) = star_index {
            let minimum = total_slots - 1;
            if minimum > 0 {
                self.emit_named(span, "GET_LEN", 0);
                self.emit_small_length(span, minimum);
                // `>=`（参照实测 `COMPARE_OP 172` ✓）
                self.emit_named(span, "COMPARE_OP", 172);
                self.emit_jump(
                    span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
                    head_failure,
                );
                self.emit_named(span, "NOT_TAKEN", 0);
            }
            let before = index;
            let after = total_slots - index - 1;
            self.emit_indexed(span, "UNPACK_EX", before | (after << 8));
        } else {
            self.emit_named(span, "GET_LEN", 0);
            self.emit_small_length(span, total_slots);
            // `==`（参照实测 `COMPARE_OP 72` ✓）
            self.emit_named(span, "COMPARE_OP", 72);
            self.emit_jump(
                span,
                opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
                head_failure,
            );
            self.emit_named(span, "NOT_TAKEN", 0);
            if total_slots == 0 {
                self.emit_named(span, "POP_TOP", 0);
            } else {
                self.emit_indexed(span, "UNPACK_SEQUENCE", total_slots);
            }
        }
        // 逐项判定（书写顺序 ⇒ 栈顶就是 items[0] ✓）
        for (index, item) in items.iter().enumerate() {
            let remaining = total_slots - index - 1;
            let failed = self.new_label();
            let next = self.new_label();
            if let Pattern::Star(inner, _) = item {
                // 星号那一格：按捕获／通配消费栈顶 ✓（`*名字` ⇒ STORE；`*_` ⇒ POP_TOP）
                self.emit_subpattern_test(inner, failed, next)?;
            } else {
                self.emit_subpattern_test(item, failed, next)?;
            }
            self.emit_jump(
                span,
                opcode::opcode("JUMP_FORWARD").expect("表里有"),
                next,
            );
            self.mark_label(failed);
            self.emit_pattern_failure_cleanup(span, remaining, consume_value);
            self.emit_jump(
                span,
                opcode::opcode("JUMP_FORWARD").expect("表里有"),
                failure,
            );
            self.mark_label(next);
        }
        // 全部命中：栈上是原值（嵌套时消费掉它 ✓）
        if consume_value {
            self.emit_named(span, "POP_TOP", 0);
        }
        self.emit_jump(
            span,
            opcode::opcode("JUMP_FORWARD").expect("表里有"),
            on_match,
        );
        // 序列判定／长度判定失败：栈是 [原值, 副本] ⇒ 收掉副本（＋按需收原值）
        self.mark_label(head_failure);
        self.emit_named(span, "POP_TOP", 0);
        if consume_value {
            self.emit_named(span, "POP_TOP", 0);
        }
        self.emit_jump(
            span,
            opcode::opcode("JUMP_FORWARD").expect("表里有"),
            failure,
        );
        Ok(())
    }

    /// **映射模式的判定**（task-37）。进入时栈顶是待测值；消费语义同序列模式 ✓。
    ///
    /// 形状照参照 3.14.4 实测：`MATCH_MAPPING; POP_JUMP_IF_FALSE; NOT_TAKEN; GET_LEN;
    /// LOAD_SMALL_INT n; COMPARE_OP 172(>=); POP_JUMP_IF_FALSE; NOT_TAKEN; LOAD_CONST <键元组>;
    /// MATCH_KEYS; COPY 1; POP_JUMP_IF_NONE; NOT_TAKEN; UNPACK_SEQUENCE n` ✓；`case {}` 只有
    /// `MATCH_MAPPING` ＋ 收尾 ✓。**与参照的差别**（如实登记）：同序列模式，`COPY 1` 保留主语 ✓。
    pub(crate) fn emit_mapping_pattern(
        &mut self,
        pattern: &Pattern,
        failure: usize,
        on_match: usize,
        consume_value: bool,
    ) -> Result<(), CompileError> {
        let Pattern::Mapping { items, span } = pattern else {
            unreachable!("只给映射模式调")
        };
        let span = *span;
        self.emit_named(span, "COPY", 1);
        self.emit_named(span, "MATCH_MAPPING", 0);
        let head_failure = self.new_label();
        self.emit_jump(
            span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            head_failure,
        );
        self.emit_named(span, "NOT_TAKEN", 0);
        if !items.is_empty() {
            self.emit_named(span, "GET_LEN", 0);
            self.emit_small_length(span, items.len());
            self.emit_named(span, "COMPARE_OP", 172);
            self.emit_jump(
                span,
                opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
                head_failure,
            );
            self.emit_named(span, "NOT_TAKEN", 0);
            self.emit_mapping_keys(span, items)?;
            self.emit_named(span, "MATCH_KEYS", 0);
            self.emit_named(span, "COPY", 1);
            let keys_missing = self.new_label();
            self.emit_jump(
                span,
                opcode::opcode("POP_JUMP_IF_NONE").expect("表里有"),
                keys_missing,
            );
            self.emit_named(span, "NOT_TAKEN", 0);
            self.emit_indexed(span, "UNPACK_SEQUENCE", items.len());
            // 逐项判定值模式（栈：原值、副本、键元组、值… ✓）
            for (index, (_, value)) in items.iter().enumerate() {
                let remaining = items.len() - index - 1;
                let failed = self.new_label();
                let next = self.new_label();
                self.emit_subpattern_test(value, failed, next)?;
                self.emit_jump(
                    span,
                    opcode::opcode("JUMP_FORWARD").expect("表里有"),
                    next,
                );
                self.mark_label(failed);
                self.emit_pattern_failure_cleanup(span, remaining, false);
                self.emit_named(span, "POP_TOP", 0); // 键元组
                self.emit_named(span, "POP_TOP", 0); // 副本
                if consume_value {
                    self.emit_named(span, "POP_TOP", 0);
                }
                self.emit_jump(
                    span,
                    opcode::opcode("JUMP_FORWARD").expect("表里有"),
                    failure,
                );
                self.mark_label(next);
            }
            // 全部命中：收掉键元组与副本，然后直接去 `on_match`（**不能落进下面的清理块** ✗）
            self.emit_named(span, "POP_TOP", 0);
            self.emit_named(span, "POP_TOP", 0);
            if consume_value {
                self.emit_named(span, "POP_TOP", 0);
            }
            self.emit_jump(
                span,
                opcode::opcode("JUMP_FORWARD").expect("表里有"),
                on_match,
            );
            // `MATCH_KEYS` 缺键：栈 [原值, 副本, 键元组, None]
            self.mark_label(keys_missing);
            self.emit_named(span, "POP_TOP", 0);
            self.emit_named(span, "POP_TOP", 0);
            self.emit_named(span, "POP_TOP", 0);
            if consume_value {
                self.emit_named(span, "POP_TOP", 0);
            }
            self.emit_jump(
                span,
                opcode::opcode("JUMP_FORWARD").expect("表里有"),
                failure,
            );
        } else {
            // `case {}`：只剩副本
            self.emit_named(span, "POP_TOP", 0);
            if consume_value {
                self.emit_named(span, "POP_TOP", 0);
            }
            self.emit_jump(
                span,
                opcode::opcode("JUMP_FORWARD").expect("表里有"),
                on_match,
            );
        }
        // `MATCH_MAPPING`／长度失败：栈 [原值, 副本]
        self.mark_label(head_failure);
        self.emit_named(span, "POP_TOP", 0);
        if consume_value {
            self.emit_named(span, "POP_TOP", 0);
        }
        self.emit_jump(
            span,
            opcode::opcode("JUMP_FORWARD").expect("表里有"),
            failure,
        );
        Ok(())
    }

    /// **`for` 目标的一项**（第 289 轮）：名字 ⇒ 按作用域存 ✓；括号元组 ⇒ 先 `UNPACK_SEQUENCE 个数`
    /// （位点＝**那一层**的跨度 ✓，`dis` 实测）再递归 ✓。
    pub(crate) fn emit_for_target(&mut self, target: &ForTarget) -> Result<(), CompileError> {
        match target {
            ForTarget::Name(name, span) => {
                self.emit_store_name(*span, name);
            }
            ForTarget::Group(items, span) => {
                self.emit_unpack_for_targets(items, *span)?;
            }
            // 星号项本身只是"那一层解包时占一格"的标记 ✓ —— 真正存的是它包着的目标 ✓。
            ForTarget::Starred(inner, _) => self.emit_for_target(inner)?,
            // **属性目标** ✓（task-25 ✓）：`with cm as self.foo:` ⇒ 先求值对象、再 `STORE_ATTR`
            ForTarget::Attribute(object, name, span) => {
                self.emit_expression(object)?;
                let index = self.intern_name(name);
                self.emit_indexed(*span, "STORE_ATTR", index);
            }
            // **下标目标** ✓（task-25 ✓）：`with cm as a[0]:` ⇒ 对象、键、`STORE_SUBSCR`
            ForTarget::Subscript(object, key, span) => {
                self.emit_expression(object)?;
                self.emit_expression(key)?;
                self.emit_named(*span, "STORE_SUBSCR", 0);
            }
        }
        Ok(())
    }

    /// **一层 `for`／`with` 目标的解包＋逐项存**（task-25 ✓）：无星 ⇒ `UNPACK_SEQUENCE`；
    /// 有星 ⇒ `UNPACK_EX`（`before | after << 8`，只允许一个 `*` ✓，与元组赋值同一条编码口径 ✓）。
    /// 位点取**那一层目标**（整段）——与 `UNPACK_SEQUENCE` 既有口径一致 ✓。
    pub(crate) fn emit_unpack_for_targets(
        &mut self,
        targets: &[ForTarget],
        span: Span,
    ) -> Result<(), CompileError> {
        let starred: Vec<usize> = targets
            .iter()
            .enumerate()
            .filter(|(_, item)| matches!(item, ForTarget::Starred(..)))
            .map(|(index, _)| index)
            .collect();
        if starred.is_empty() {
            self.emit_indexed(span, "UNPACK_SEQUENCE", targets.len());
        } else {
            if starred.len() != 1 {
                return Err(CompileError::Unsupported(
                    "`for`／`with` 目标里只允许一个 `*`（如实报未接线 ✓）".to_owned(),
                ));
            }
            let before = starred[0];
            let after = targets.len() - before - 1;
            let argument = (before | (after << 8)) as u16;
            if argument > 255 {
                self.emit_named(span, "EXTENDED_ARG", (argument >> 8) as u8);
            }
            self.emit_named(span, "UNPACK_EX", (argument & 0xFF) as u8);
        }
        for item in targets {
            self.emit_for_target(item)?;
        }
        Ok(())
    }

    /// **一层元组解包目标的发射**（第 405 轮，`Statement::AssignTuple` 那一条的递归助手）：
    /// - `emit_unpack` 为假 ⇒ 只存（等长窥孔：值已按目标序压好 ✓）；
    /// - 否则发 `UNPACK_SEQUENCE <个数>`／`UNPACK_EX`（位点＝这一层的跨度 ✓）；
    /// - 每项按形态存：名字（含 cell／全局那几支 ✓）／属性／下标／**再一层括号元组**（递归 ✓）。
    ///
    /// 嵌套口径照参照 `dis` 实测：`a, (b, c) = x` ⇒ `UNPACK 2`＋`STORE a`＋`UNPACK 2`＋`STORE b`＋
    /// `STORE c`（函数里两个名字打成 `STORE_FAST_STORE_FAST` ✓，**每一层**都这么融 ✓）。
    pub(crate) fn emit_unpack_assign(
        &mut self,
        targets: &[(Expression, bool)],
        target_span: Span,
        emit_unpack: bool,
    ) -> Result<(), CompileError> {
        let starred: Vec<usize> = targets
            .iter()
            .enumerate()
            .filter(|(_, (_, star))| *star)
            .map(|(index, _)| index)
            .collect();
        if emit_unpack {
            if starred.is_empty() {
                let count = targets.len();
                if count > 255 {
                    return Err(CompileError::Unsupported(
                        "元组解包目标多于 255 个：`EXTENDED_ARG` 随后补（如实报未接线 ✓）".to_owned(),
                    ));
                }
                self.emit_indexed(target_span, "UNPACK_SEQUENCE", count);
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
                    self.emit_named(target_span, "EXTENDED_ARG", (argument >> 8) as u8);
                }
                self.emit_named(target_span, "UNPACK_EX", (argument & 0xFF) as u8);
            }
        }
        // **超指令融合**（实测，与推导式的元组目标同一条口径 ✓）：函数内**相邻两个**局部
        //   名字目标 ⇒ 合成 `STORE_FAST_STORE_FAST`，arg ＝ `(slot0 << 4) | slot1`，
        //   位点＝**首个目标**的跨度 ✓（`STORE_FAST k, v` ⇒ `(14,15)` ✓）。
        //   **按对融**（第 405 轮推广 ✓）：`a, b, c, d = x` 参照发两条融合 ＋ ……；
        //   **带 `*` 的也融** ✓（`a, *b = x` ⇒ `UNPACK_EX` ＋ `STORE_FAST_STORE_FAST` ✓，实测）。
        //   槽号要 4 位装得下（≤ 15 ✓），否则退回两条 `STORE_FAST` ✓。
        //   **cell／free 目标不能融** ✗（task-25 ② 真 bug 修 ✓）：形参 cell 的
        //   localsplus 槽里放的是 **cell 对象** ⇒ `STORE_FAST` 会把 cell 打掉 ✗
        //   （参照对 `argname, cls = …`（`cls` 是形参 cell ✓）发
        //   `STORE_FAST argname; STORE_DEREF cls` 两条 ✓）。
        let mut fused_arguments: Vec<Option<u8>> = vec![None; targets.len()];
        let mut fused_second: Vec<bool> = vec![false; targets.len()];
        if self.kind == ScopeKind::Function {
            let mut pair_start = 0usize;
            while pair_start + 1 < targets.len() {
                let fusable = match (&targets[pair_start].0, &targets[pair_start + 1].0) {
                    (Expression::Name(first, _), Expression::Name(second, _)) => {
                        self.unit.varnames.iter().any(|item| item == first)
                            && self.unit.varnames.iter().any(|item| item == second)
                            && self.deref_slot(first).is_none()
                            && self.deref_slot(second).is_none()
                            && {
                                let first_slot = self.slot_of(first);
                                let second_slot = self.slot_of(second);
                                if first_slot <= 15 && second_slot <= 15 {
                                    fused_arguments[pair_start] =
                                        Some(((first_slot << 4) | second_slot) as u8);
                                    fused_second[pair_start + 1] = true;
                                    true
                                } else {
                                    false
                                }
                            }
                    }
                    _ => false,
                };
                let _ = fusable;
                pair_start += 2;
            }
        }
        for (index, (target, _)) in targets.iter().enumerate() {
            let target_span = target.span();
            match target {
                Expression::Name(name, name_span) => {
                    if let Some(argument) = fused_arguments[index] {
                        self.emit_at(
                            *name_span,
                            opcode::opcode("STORE_FAST_STORE_FAST")
                                .expect("STORE_FAST_STORE_FAST 在表里"),
                            argument,
                        );
                        continue;
                    }
                    if fused_second[index] {
                        // 融合已在首个目标处发出 ⇒ 第二个跳过 ✓
                        continue;
                    }
                    // **cell／free 名 ⇒ `STORE_DEREF`** ✓（与链式赋值那条同口径 ✓）：
                    // 形参 cell 的 localsplus 槽里是 cell 对象 ⇒ `STORE_FAST` 会打掉它 ✗。
                    if let Some(slot) = self.deref_slot(name) {
                        self.emit_indexed(*name_span, "STORE_DEREF", slot);
                    } else if self.kind == ScopeKind::Function
                        && self.unit.varnames.iter().any(|item| item == name)
                    {
                        let slot = self.slot_of(name);
                        self.emit_indexed(*name_span, "STORE_FAST", slot);
                    } else {
                        self.emit_store_name(*name_span, name);
                    }
                }
                Expression::Subscript(object, key, _) => {
                    // **切片赋值**（第 175 轮，照实测 ✓）：两段 ⇒ `STORE_SLICE` ✓（**没有**
                    //   `BUILD_SLICE` ✓）；三段 ⇒ `BUILD_SLICE 3` ＋ `STORE_SUBSCR` ✓
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
                // **再一层括号／方括号元组**（第 405 轮）：那一层的 `*` 拆成星标记后递归 ✓
                Expression::TupleLiteral(items, group_span) | Expression::List(items, group_span) => {
                    let inner: Vec<(Expression, bool)> = items
                        .iter()
                        .map(|item| match item {
                            Expression::Starred(inner, _) => ((**inner).clone(), true),
                            other => (other.clone(), false),
                        })
                        .collect();
                    self.emit_unpack_assign(&inner, *group_span, true)?;
                }
                _ => {
                    return Err(CompileError::Unsupported(
                        "元组解包只接线了名字／属性／下标／嵌套元组四种目标（其余如实报未接线 ✓）"
                            .to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// 推导式里**一个名字目标**的存储（第 405 轮从原来的 `match` 里抽出来 ✓）：
    /// 能与"紧接着的那次读取"打成 `STORE_FAST_LOAD_FAST` 就打 ✓（两个槽都要装进 4 位 ✓）。
    pub(crate) fn emit_comprehension_store_name(
        &mut self,
        _name: &str,
        span: Span,
        element: &Expression,
        generators: &[Generator],
        index: usize,
        slots: &[usize],
        slot_index: &mut usize,
    ) {
        let slot = slots[*slot_index];
        *slot_index += 1;
        if let Some(next_slot) = self
            .next_read_slot(element, generators, index)
            .filter(|next_slot| slot < 16 && *next_slot < 16)
        {
            self.emit_at(
                span,
                opcode::opcode("STORE_FAST_LOAD_FAST").expect("STORE_FAST_LOAD_FAST 在表里"),
                ((slot << 4) | next_slot) as u8,
            );
            self.pending_fused_load = Some(next_slot);
        } else {
            self.emit_indexed(span, "STORE_FAST", slot);
        }
    }

    /// 推导式里**一层组目标**（`(k, v)`／`[off]`／裸 `a, b`，可再嵌 ✓）的存储：
    /// 先 `UNPACK_SEQUENCE <这一层的项数>`（位点＝这一层的跨度 ✓），
    /// 全是名字 ⇒ 每两项一条 `STORE_FAST_STORE_FAST`（参照口径 ✓）；
    /// 含嵌套组 ⇒ 逐项递归 ✓。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_comprehension_target_group(
        &mut self,
        items: &[ComprehensionTarget],
        group_span: Span,
        element: &Expression,
        generators: &[Generator],
        index: usize,
        slots: &[usize],
        slot_index: &mut usize,
        scaffold: Span,
    ) -> Result<(), CompileError> {
        self.emit_indexed(group_span, "UNPACK_SEQUENCE", items.len());
        // **单名字组**（`[off]`）：照参照"存完紧接着读"能融成 `STORE_FAST_LOAD_FAST` ✓
        //（`[off for [off] in …]` 实测 ⇒ `UNPACK_SEQUENCE 1; STORE_FAST_LOAD_FAST (off, off)` ✓）。
        if items.len() == 1 {
            if let ComprehensionTarget::Name(name, span) = &items[0] {
                self.emit_comprehension_store_name(
                    name, *span, element, generators, index, slots, slot_index,
                );
                return Ok(());
            }
        }
        if items
            .iter()
            .all(|item| matches!(item, ComprehensionTarget::Name(..)))
        {
            let first_span = items
                .first()
                .map(comprehension_target_span_of)
                .unwrap_or(scaffold);
            let mut item_slots: Vec<usize> = Vec::with_capacity(items.len());
            for _ in items {
                item_slots.push(slots[*slot_index]);
                *slot_index += 1;
            }
            let mut store_index = 0usize;
            while store_index + 1 < item_slots.len() {
                let (high, low) = (item_slots[store_index], item_slots[store_index + 1]);
                if high < 16 && low < 16 {
                    self.emit_at(
                        first_span,
                        opcode::opcode("STORE_FAST_STORE_FAST")
                            .expect("STORE_FAST_STORE_FAST 在表里"),
                        ((high << 4) | low) as u8,
                    );
                } else {
                    self.emit_indexed(first_span, "STORE_FAST", high);
                    self.emit_indexed(first_span, "STORE_FAST", low);
                }
                store_index += 2;
            }
            if store_index < item_slots.len() {
                self.emit_indexed(first_span, "STORE_FAST", item_slots[store_index]);
            }
            return Ok(());
        }
        // **含嵌套组的层**：名字项**只存不融** ✗ —— 融合会把"元素读"提前塞进存目标中间，
        // 而这一层后面还有 `UNPACK_SEQUENCE` 要弹栈顶（栈序就错了 ✓）；嵌套组自己递归 ✓。
        for item in items {
            match item {
                ComprehensionTarget::Name(_, span) => {
                    let slot = slots[*slot_index];
                    *slot_index += 1;
                    self.emit_indexed(*span, "STORE_FAST", slot);
                }
                ComprehensionTarget::Group(inner, inner_span) => {
                    self.emit_comprehension_target_group(
                        inner,
                        *inner_span,
                        element,
                        generators,
                        index,
                        slots,
                        slot_index,
                        scaffold,
                    )?;
                }
                // **属性／下标目标**（task-45 ✓）：栈顶是这一格的值 ⇒ 对象（＋键）求值后存 ✓。
                ComprehensionTarget::Attribute(object, name, span) => {
                    self.emit_expression(object)?;
                    let name_index = self.intern_name(name);
                    self.emit_indexed(*span, "STORE_ATTR", name_index);
                }
                ComprehensionTarget::Subscript(object, key, span) => {
                    self.emit_expression(object)?;
                    self.emit_expression(key)?;
                    self.emit_named(*span, "STORE_SUBSCR", 0);
                }
            }
        }
        Ok(())
    }

}
