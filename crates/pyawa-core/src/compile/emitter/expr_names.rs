//! `emitter` 的 `expr_names` 域：名字／属性／下标／调用／lambda／海象。

use super::*;

impl Emitter {
    /// 分派：每个变体一个发射函数（见本文件下方）。
    pub(crate) fn emit_expression_names(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Subscript (..) => self.emit_expression_subscript(expression),
            Expression::ChainSubject (..) => self.emit_expression_chain_subject(expression),
            Expression::Lambda { .. } => self.emit_expression_lambda(expression),
            Expression::Attribute (..) => self.emit_expression_attribute(expression),
            Expression::Name (..) => self.emit_expression_name(expression),
            Expression::Call { .. } => self.emit_expression_call(expression),
            Expression::Starred (..) => self.emit_expression_starred(expression),
            Expression::Walrus { .. } => self.emit_expression_walrus(expression),
            _ => unreachable!("emit_expression_names 只接收本族的 Expression"),
        }
    }

    /// `Subscript` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_subscript(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
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
            _ => unreachable!("emit_expression_subscript 只接收 Statement::Subscript"),
        }
    }

    /// `ChainSubject` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_chain_subject(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::ChainSubject(_) => Ok(()),
            _ => unreachable!("emit_expression_chain_subject 只接收 Statement::ChainSubject"),
        }
    }

    /// `Lambda` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_lambda(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
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
                // **`BC-62`**：体有两种形态 —— 表达式（原有）／语句序列（扩展模式新形态）。
                // 语句序列编译成一个函数单元：**末表达式**的那条表达式语句变成 `Return`，
                // 没有末表达式就 `Return None`（与 `def` 末尾的隐式返回同形）。
                let unit: Vec<Statement> = match body {
                    LambdaBody::Expression(expression) => {
                        vec![Statement::Return((**expression).clone(), expression.span())]
                    }
                    LambdaBody::Statements(statements) => {
                        let mut unit = statements.clone();
                        let tail = match unit.pop() {
                            Some(Statement::Expression(expression, expression_span)) => {
                                Statement::Return(expression, expression_span)
                            }
                            other => {
                                if let Some(statement) = other {
                                    unit.push(statement);
                                }
                                Statement::Return(
                                    Expression::Constant(Constant::None, *span),
                                    *span,
                                )
                            }
                        };
                        unit.push(tail);
                        unit
                    }
                };
                // **lambda 的闭包**（第 297 轮）：它要的自由变量 = 体内引用的名字（扣掉自己的形参）
                // ∩（本层 `varnames`／`cellvars`／`freevars`）；实测 `return lambda: x` ⇒
                // lambda `co_freevars=('x',)`、外层 `cellvars=('x',)`、元组 `LOAD_FAST_BORROW 0` ✓
                // 要的是**这个 lambda 自己的体**所需的自由变量 = 体内引用的名字 − 自己的形参，
                // 再与本层 cell／free 表求交 ✓（`collect_lambda_demands` 是"在语句里找 lambda"，
                // 用在这里会找不到 ✗ —— 第 297 轮踩过）
                let mut inner_names: Vec<String> = Vec::new();
                collect_names_in_lambda_body(body, &mut inner_names);
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
                        self.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
                    }
                    self.emit_indexed(*span, "BUILD_TUPLE", lambda_freevars.len());
                }
                self.emit_function_object(nested, parameters, kwonly, None, None, &[], *span)?;
                if !lambda_freevars.is_empty() {
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
                }
                Ok(())
            }
            // **属性读**（实测）：`LOAD_FAST_BORROW 0; LOAD_ATTR <名字下标>`；
            // `LOAD_ATTR` 的 oparg 低位是"取方法"标志 ⇒ 纯取值就是 `下标 << 1`
            _ => unreachable!("emit_expression_lambda 只接收 Statement::Lambda"),
        }
    }

    /// `Attribute` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_attribute(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Attribute(target, name, span) => {
                self.emit_expression(target)?;
                let index = self.intern_name(name);
                // 位置取**属性表达式自身**的跨度（第 226 轮按正确配对重测：`x = a.b` ⇒ `(4,7)`、
                // `x = a.b.c` ⇒ 两条 `LOAD_ATTR` 分别是 `(4,7)`／`(4,9)`、`x = a[0].b` ⇒ `(4,10)`）；
                // 原来取的是**对象**的跨度（`target.span()`）——那是从错位的测量里留下的错规则
                self.emit_shifted(*span, "LOAD_ATTR", index << 1);
                Ok(())
            }

            // **装不进 `i64` 的整数字面量**（第 285 轮）：照参照直接进常量池 ✓
            //（`LOAD_CONST`；`LOAD_SMALL_INT` 那条优化只给 0..=255 ✓）。
            _ => unreachable!("emit_expression_attribute 只接收 Statement::Attribute"),
        }
    }

    /// `Name` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_name(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
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
                    self.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
                    return Ok(());
                }
                if let Some(slot) = self.deref_slot(name) {
                    // cell／自由变量 ⇒ `LOAD_DEREF`（闭包；第 292 轮）
                    self.emit_indexed(*span, "LOAD_DEREF", slot);
                    return Ok(());
                }
                if self.kind == ScopeKind::Function && self.unit.varnames.iter().any(|item| item == name)
                {
                    let slot = self.slot_of(name);
                    self.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
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
                    self.emit_shifted(*span, "LOAD_GLOBAL", index << 1);
                    return Ok(());
                }
                let index = self.intern_name(name);
                self.emit_indexed(*span, "LOAD_NAME", index);
                Ok(())
            }
            _ => unreachable!("emit_expression_name 只接收 Statement::Name"),
        }
    }

    /// `Call` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_call(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
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
                // **cell／free（被闭包捕获）的名字不是全局** ✗（第 625 轮真 bug 修 ✗）：先前只查
                // `varnames`（本层局部 ✗），没排掉捕获来的名字 ✓ ⇒ `def inner(): return x(1)`（`x` 来自外层 ✓）
                // 被当成全局发 `LOAD_GLOBAL` ✗ ⇒ 运行时 `NameError: name 'x' is not defined` ✓
                //（实测：`functools._warn_python_reduce_kwargs` 的 `py_reduce(*args, **kwargs)` ✓、
                //  `re` 里同类闭包调用 ✓，都断在这一格 ✓）。
                let global_callee = matches!(self.kind, ScopeKind::Function)
                    && matches!(
                        function.as_ref(),
                        Expression::Name(name, _)
                            if !self.unit.varnames.iter().any(|item| item == name)
                                && self.deref_slot(name).is_none()
                    );
                if global_callee {
                    if let Expression::Name(name, name_span) = function.as_ref() {
                        let index = self.intern_name(name);
                        self.emit_shifted(*name_span, "LOAD_GLOBAL", (index << 1) | 1);
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
            _ => unreachable!("emit_expression_call 只接收 Statement::Call"),
        }
    }

    /// `Starred` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_starred(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Starred(value, _) => self.emit_expression(value),
            _ => unreachable!("emit_expression_starred 只接收 Statement::Starred"),
        }
    }

    /// `Walrus` 的发射（从 `emit_expression_names` 的臂原样搬来）。
    pub(crate) fn emit_expression_walrus(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
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
                        self.emit_indexed(*target_span, "STORE_DEREF", slot);
                    } else if self.unit.varnames.iter().any(|item| item == target) {
                        let slot = self.slot_of(target);
                        self.emit_indexed(*target_span, "STORE_FAST", slot);
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
            _ => unreachable!("emit_expression_walrus 只接收 Statement::Walrus"),
        }
    }
}
