//! `emitter` 的 `stmt_misc` 域：其余简单语句（`yield`／`return`／`raise`／`assert`／`del`／`global` 等）。

use super::*;

impl Emitter {
    pub(crate) fn emit_statement_misc(&mut self, statement: &Statement, _rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
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
                Statement::Break(position) => {
                    let Some(frame) = self.loops.last().cloned() else {
                        return Err(CompileError::Syntax("'break' outside loop".to_owned()));
                    };
                    // **跳出 `except` 处理块 ⇒ 逐层 `POP_EXCEPT`** ✓（与 `continue` 同一条规矩 ✓）。
                    for _ in frame.handler_depth..self.handler_depth {
                        self.emit_at(*position, opcode::opcode("POP_EXCEPT").expect("POP_EXCEPT 在表里"), 0);
                    }
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
                    // **复制"循环之后的语句"时，按"已经在处理块之外"发** ✓（task-22 ✗）：处理块里的
                    // `break` 那段复制件（可能带 `return`／隐式收尾 ✓）若仍算在 `handler_depth` 里，
                    // 会给它多发 `POP_EXCEPT`／`SWAP` ✗（实测 `except: break` 复制件多一条 `POP_EXCEPT`
                    // ＋一条 `SWAP 2` ⇒ `StackUnderflow` ✗ —— 参照那里只有 `POP_EXCEPT; POP_TOP; …
                    // RETURN_VALUE` ✓）。
                    let saved_handler_depth = self.handler_depth;
                    let saved_handler_stack = core::mem::take(&mut self.handler_stack);
                    self.handler_depth = frame.handler_depth;
                    let outcome = self.emit_rest_and_tail(&frame.rest, *position, Some(frame.block_depth));
                    self.handler_depth = saved_handler_depth;
                    self.handler_stack = saved_handler_stack;
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
                    // **跳出 `except` 处理块 ⇒ 逐层 `POP_EXCEPT`** ✓（task-22 真 bug 修 ✗）：参照在
                    // `continue` 的回跳**之前**发它 ✓（`except` 内 1 层就 1 条 ✓）—— 少了它，
                    // 异常状态（`prev_exc` ＋ 异常对象 ✓）会**留在栈上** ✓ ⇒ 外层 `FOR_ITER` 把异常
                    // 当迭代器 ✗（实测 `re.compile(r'[a-z]', re.I)` ✓）。
                    for _ in frame.handler_depth..self.handler_depth {
                        self.emit_at(*position, opcode::opcode("POP_EXCEPT").expect("POP_EXCEPT 在表里"), 0);
                    }
                    self.emit_directed_jump(
                        *position,
                        opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                        frame.continue_target,
                        true,
                    );
                    Ok(())
                }
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
                                self.emit_indexed(target_span, "DELETE_ATTR", index);
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
                        self.emit_indexed(*span, "CHECK_BOUNDARY_OUT", index);
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
            _ => unreachable!("emit_statement_misc 只接收 stmt_misc 族的 Statement"),
        }
    }
}
