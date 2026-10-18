//! `emitter` 的 `jumps` 域：跳转与 `EXTENDED_ARG` 回填、条件副本与待定收尾。

use super::*;

impl Emitter {
    /// 发一个**条件跳转**到既有标签：正常"条件为假就跳"，`jump_if_true` 为真时反过来。
    ///
    /// **`not` 是推进跳转的**（实测 `if not a:` ⇒ `LOAD a; TO_BOOL; POP_JUMP_IF_TRUE`，
    /// **没有** `UNARY_NOT`）——同一棵树在"值上下文"（`x = not a`）与"条件上下文"两种发射形态，
    /// 这是第一处**按上下文改发射**的地方。
    pub(crate) fn emit_condition_jump_to(
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
                                self.emit_constant(*span, Constant::None);
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

    /// 收尾时把跳转实参回填（`BC-55` 的公式反过来用）。
    pub(crate) fn flush_jumps(&mut self) {
        // **字节码转储**（第 390 轮，门控 `PYAWA_DUMP_CODE=1`）：放在 `flush_jumps` 开头 ——
        // 此刻 `self.labels` **已全部落点** ✓ ⇒ 每条跳转都能解析出目标 unit ✓（与参照 dis 逐条对照用 ✓）。
        if crate::diag::flag("PYAWA_DUMP_CODE") {
            eprintln!(
                // **带代码对象名** ✓（第 651 轮）：`Lib/re` 一次 dump 上万个单元 ✗ ⇒ 没有名字
                // 根本对应不回具体函数 ✓（判据② 的岔口就是这么找的 ✓）。
                "[code] ===== 代码对象 name={} 字节={} 常量={}",
                self.unit.name,
                self.unit.code.len(),
                self.unit.constants.len()
            );
            let mut word = 0usize;
            while word * 2 + 1 < self.unit.code.len() {
                let op = u16::from(self.unit.code[word * 2]);
                let arg = self.unit.code[word * 2 + 1];
                eprintln!("[code] unit{word} op={op} arg={arg}");
                word += 1 + opcode::inline_cache_entries(op) as usize;
            }
        }
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
                    let origin = self
                        .label_origins
                        .get(label)
                        .map(|origin| origin.to_string())
                        .unwrap_or_else(|| "<无记录>".to_owned());
                    panic!(
                        "跳转目标标签 {label} 从未落点（标签创建处：{origin}；跳转指令在码元 {}，位点 {position:?}；已落点：{marked:?}）",
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
            // **跳转落点诊断** ✓（第 629 轮，门控 `PYAWA_JUMP_DEBUG=1`）：把"指令码元 → 目标码元 → 相对实参"
            // 打出来 ✓ —— 量长跳落点用 ✓（`widen_extended_args` 插 `EXTENDED_ARG` 时若没修正实参 ✗，
            // 这里就是那把尺子 ✓：`Lib/re` 的长循环正是这么跳进体里的 ✓）。
            if crate::diag::flag("PYAWA_JUMP_DEBUG") {
                eprintln!(
                    "[jump] name={} 码元 {here} 目标码元 {target} 向后={backward} 实参={argument} size={size}",
                    self.unit.name
                );
            }
            if !(0..=255).contains(&argument) {
                self.wide_jumps.push((here, argument as u16));
            }
            self.resolved_jumps
                .push((here, size, target, backward, argument_byte));
            self.unit.code[argument_byte] = (argument & 0xFF) as u8;
        }
    }

    /// **把需要加宽的跳转补上 `EXTENDED_ARG` 前缀**（第 121 轮）。
    ///
    /// 必须在**所有回填之后、`encode_exceptiontable` 之前**调用 ✓：插词会移动其后全部码元
    /// ⇒ 这里同步做三件事：重建 `code`、给 `positions` 插同一条位点、按插入数**平移异常表偏移** ✓。
    pub(crate) fn widen_extended_args(&mut self) {
        if self.wide_jumps.is_empty() {
            return;
        }
        // **迭代到不动点** ✓（第 630 轮 ✓）：插 `EXTENDED_ARG` 会把某些跳转的实参**顶过 1 字节** ✗
        //（`Lib/` 里实测有一条重算后正好 256 ✓）⇒ 加宽之后要看下一轮还需不需要插 ✓；
        // 每轮只增不减 ⇒ 必然收敛 ✓（上限 4 轮 ✓）。重复前缀**不会**发生 ✓：上一轮插的前缀是**独立的词** ✓
        // 会被重建原样拷贝 ✓，而本轮计划里没有它就自然不插 ✓。
        for _ in 0..4 {
            self.widen_once();
            if self.wide_jumps.is_empty() {
                break;
            }
        }
    }

    /// **一轮加宽** ✓（`widen_extended_args` 的主体 ✓）。
    pub(crate) fn widen_once(&mut self) {
        if self.wide_jumps.is_empty() {
            return;
        }
        let wide = core::mem::take(&mut self.wide_jumps);
        self.widen_high = vec![None; self.unit.code.len() / 2];
        for (word, full) in &wide {
            if let Some(slot) = self.widen_high.get_mut(*word) {
                *slot = Some(*full);
            }
        }
        if crate::diag::flag("PYAWA_JUMP_DEBUG") {
            eprintln!("[jump] name={} 需要加宽的跳转 {} 条：{:?}", self.unit.name, wide.len(), wide);
        }
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
        // **插词会移动其后全部码元** ✗ ⇒ 把 `labels` 一起搬到新坐标系 ✓（回填按它算 ✓），
        // 并把平移表留给重建后的**实参重算** ✓。
        for label in &mut self.labels {
            if let Some(word) = *label {
                *label = Some(word + shift[word.min(word_count)]);
            }
        }
        self.widen_shift = shift.clone();
        if crate::diag::flag("PYAWA_JUMP_DEBUG") {
            eprintln!("[jump] 加宽插入 {} 个词（码元总数 {word_count} ⇒ {}）", inserted, word_count + inserted);
        }
        // **异常表偏移平移**（条目里存的是字节偏移 ⇒ 乘 2 换成码元再查插入数 ✓）
        for entry in &mut self.exception_entries {
            for field in [&mut entry.0, &mut entry.1, &mut entry.2] {
                let unit_index = (*field / 2).min(word_count);
                *field += 2 * shift[unit_index];
            }
        }
        self.unit.code = code;
        self.unit.positions = positions;
        // **重建之后按新坐标重算实参** ✓（第 630 轮真 bug 修 ✗）—— 见 `refill_jump_args` ✓。
        self.refill_jump_args();
    }

    /// **按加宽后的坐标重算跳转实参** ✓（第 630 轮真 bug 修 ✗）：插 `EXTENDED_ARG` 会把它**之后**的
    /// 全部码元往后挪 ✗ ⇒ 先前实参原样照抄 ✓ ⇒ 每条插在"跳转与目标之间"的加宽都让落点短一截 ✓
    /// （实测：10／14 臂 `帧操作失败：StackUnderflow` ✗、18 臂直接跳进循环体 ✗；`Lib/re` 的
    /// `SubPattern.getwidth` 空迭代同源 ✓；修好后 `import re` 又推进一层 ✓）。
    ///
    /// 坐标：自己有没有前缀**从 code 里认** ✓（`new_here` 那一词是不是 `EXTENDED_ARG` ✓）——
    /// 不依赖本轮计划 ✓（上一轮插的前缀不会被本轮计划覆盖 ✓）。
    pub(crate) fn refill_jump_args(&mut self) {
        if self.widen_shift.is_empty() {
            return;
        }
        let word_count = self.widen_shift.len().saturating_sub(1);
        let extended_opcode = u16::from(opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里")) as u8;
        let mut index = 0usize;
        while index < self.resolved_jumps.len() {
            let (here, size, target, backward, _argument_byte) = self.resolved_jumps[index];
            let shift_here = self.widen_shift[here.min(word_count)];
            let new_here = here + shift_here;
            let own_prefix = self.unit.code.get(new_here * 2).copied() == Some(extended_opcode);
            let new_base = new_here + usize::from(own_prefix) + size;
            let new_target = target + self.widen_shift[target.min(word_count)];
            let new_argument = if backward {
                new_base as i64 - new_target as i64
            } else {
                new_target as i64 - new_base as i64
            };
            assert!(new_argument >= 0, "跳转实参重算为负：{new_argument}");
            let new_argument = new_argument as usize;
            let new_argument_byte = new_here * 2 + 1 + 2 * usize::from(own_prefix);
            if new_argument > 0xFF {
                if own_prefix {
                    // 原位改高位字节 ✓（前缀就是 `new_here` 那一词 ✓）。
                    self.unit.code[new_here * 2 + 1] = ((new_argument >> 8) & 0xFF) as u8;
                    self.unit.code[new_argument_byte] = (new_argument & 0xFF) as u8;
                } else {
                    // **本轮没排上加宽 ⇒ 记进下一轮** ✓（外壳会再跑一轮 ✓），并把记录改到新坐标 ✓。
                    self.wide_jumps.push((new_here, new_argument as u16));
                }
            } else {
                self.unit.code[new_argument_byte] = (new_argument & 0xFF) as u8;
            }
            self.resolved_jumps[index] = (new_here, size, new_target, backward, new_argument_byte);
            index += 1;
        }
        if self.wide_jumps.is_empty() {
            self.resolved_jumps.clear();
        }
    }

    /// 收尾时把"待定常量"登记进表并回填实参。
    /// **延迟入池**的常量（字面量默认值折出来的元组）：参照把它们排在常量表**最后**
    /// （`x = 200 + 100` ⇒ `[200, None, 300]`；`return 200 + 100` ⇒ `[200, 300]`）。
    /// **每个作用域末尾都必须冲刷** —— 此前这段只在"模块且要收尾"那一支里 ✗，
    /// 于是**函数里嵌套 `def` 的默认值元组会丢**（第 280 轮修）。
    /// **条件出口副本的冲刷**：给每个走向条件出口的跳转发一份收尾（在**作用域收尾之后**；
    /// 必须**显式**发那两条：`emit_rest_and_tail`／`emit_implicit_return` 在这个时机都发不出 ✗）。
    pub(crate) fn flush_condition_copies(&mut self) -> Result<(), CompileError> {
        // **链式比较的续部副本**（第 85 轮）：`SWAP 2; POP_TOP` ＋ 存入 ＋ 余部 ＋ 收尾
        let chains = core::mem::take(&mut self.pending_chain_copies);
        for (landing, target, target_span, rest, span) in &chains {
            self.mark_label(*landing);
            self.emit_at(*span, opcode::opcode("SWAP").expect("SWAP 在表里"), 2);
            self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
            if self.kind == ScopeKind::Module {
                let index = self.intern_name(target);
                self.emit_indexed(*target_span, "STORE_NAME", index);
            } else if let Some(deref) = self.deref_slot(target) {
                // **重放路径同样要认 cell／自由变量** ✓（第 520 轮 ✓，口径同 `emit_store_name` ✓）。
                self.emit_indexed(*target_span, "STORE_DEREF", deref);
            } else {
                let slot = self.slot_of(target);
                self.emit_indexed(*target_span, "STORE_FAST", slot);
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

    pub(crate) fn flush_deferred(&mut self) {
        for (offset, constant) in core::mem::take(&mut self.deferred) {
            let index = self.intern_constant(constant);
            self.patch_constant_argument(offset, index);
        }
    }

    pub(crate) fn flush_pending(&mut self) {
        let pending = core::mem::take(&mut self.pending);
        for (argument_byte, constant) in pending {
            let index = self.intern_constant(constant);
            self.patch_constant_argument(argument_byte, index);
        }
    }

    /// **回填一个延迟的 `LOAD_CONST` 实参**（第 406 轮真 bug 修 ✗）：常量下标可能 ≥ 256 ✓
    /// （`pending` 的"上界 ≤ 191"只是**发射时**的估计 ✗ —— 之后编译嵌套 `def`／`class` 时
    /// `intern_constant(Code(…))` 会**当场**往常量表里加 ✓，于是早先排队的下标会越过 255 ✗）。
    /// 先前这里直接 `index as u8` ✗ ⇒ 高字节丢掉 ✓（`pickletools.py:165` 的 `-1` 真下标 379
    /// ⇒ 低字节 123 ⇒ 读到**别的字符串常量** ✓ ⇒ `UP_TO_NEWLINE` 变成本文件后面某段文档串 ✓）。
    /// 现在：低字节照写，**并登记进 `wide_jumps`** ⇒ `widen_extended_args` 会在那条
    /// `LOAD_CONST` 前补 `EXTENDED_ARG`（与跳转加宽同一套机制 ✓，`refill_jump_args` 会跟着平移 ✓）。
    pub(crate) fn patch_constant_argument(&mut self, argument_byte: usize, index: usize) {
        self.unit.code[argument_byte] = (index & 0xFF) as u8;
        if index > 0xFF {
            // 指令字起点（操作码在偶数字节、实参在奇数字节 ✓）
            let word = (argument_byte - 1) / 2;
            debug_assert!(
                index <= 0xFFFF,
                "常量下标超过 65535：两级 EXTENDED_ARG 随后补（如实报未接线 ✓）"
            );
            self.wide_jumps.push((word, index as u16));
        }
    }

    /// **`TS-47`／`BC-65`**：登记本作用域的一个**已标注变量**（名字 ⇒ 编译期类型标签）。
    ///
    /// 只在**扩展模式**生效（`TS-5`：纯 Python 模式不产生任何检查 ⇒ 表永远为空）。
    /// 重复标注按**后者**覆盖（Python 允许对同一名字重写标注；此后赋值按新标签查）。
    pub(crate) fn register_assignment_label(&mut self, name: &str, label: Constant) {
        if self.mode != Mode::Extension {
            return;
        }
        if let Some(entry) = self
            .assignment_labels
            .iter_mut()
            .find(|(candidate, _)| candidate == name)
        {
            entry.1 = label;
        } else {
            self.assignment_labels.push((name.to_owned(), label));
        }
    }

    /// **`TS-47` 的运行期一半**（`BC-65`）：对**名字目标**的赋值，在 `STORE_*` **之前**发一条
    /// `CHECK_ASSIGNMENT <常量下标>`。
    ///
    /// - oparg 是常量表下标，常量是**单标签元组**（标签形态同 `BC-24`：类型对象／`"Any"`／
    ///   `(外, 内)`／union 元组）——执行器读栈顶**不弹**（紧随的存储还要用这个值）；
    /// - 只在扩展模式、且名字在本作用域**已标注**时发：**未标注的名字不推断**（`TS-2`）；
    /// - 常量下标走 [`Self::intern_constant`] ＋ [`Self::emit_indexed`]（`> 255` 自动 `EXTENDED_ARG`，
    ///   与 `CHECK_BOUNDARY_*` 的进池口径一致，不做 `as u8` 截断）。
    pub(crate) fn emit_assignment_check(&mut self, span: Span, name: &str) {
        if self.mode != Mode::Extension {
            return;
        }
        let Some((_, label)) = self
            .assignment_labels
            .iter()
            .find(|(candidate, _)| candidate == name)
        else {
            return;
        };
        let label = label.clone();
        let index = self.intern_constant(Constant::Tuple(vec![label]));
        self.emit_indexed(span, "CHECK_ASSIGNMENT", index);
    }

}
