//! `emitter` 的 `expr_comprehensions` 域：推导式（list／set／dict／generator）的发射。

use super::*;

use super::defs::global_load_names;
use super::patterns::{collect_comprehension_target_names, comprehension_target_as_for_target, first_for_target_name};
impl Emitter {
    pub(crate) fn emit_expression_comprehensions(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
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
                        // 目标换成本层 `Statement::For` 那套（名字／可再嵌的组 ✓）
                        let converted = comprehension_target_as_for_target(&generator.target);
                        let (target, target_span, tuple_targets) = match converted {
                            ForTarget::Name(name, target_span) => (name, target_span, Vec::new()),
                            // 组：`for` 那条发射路把"组的内容"放 `tuple_targets`、`UNPACK` 用组跨度 ✓；
                            // `target` 字段**不参与发射**（见 `Statement::For` 的发射臂 ✓），但它是
                            // **本层局部**、要参与 `varnames` 登记 ⇒ 取组里**第一个叶子名字** ✓
                            //（写空串会把空的 varname 塞进去 ✗ ⇒ 槽号整体错位 ✓）。
                            // 星号／属性／下标目标不会出现在推导式里（`ComprehensionTarget` 只有
                            // 名字与组 ✓）⇒ 如实报未接线，别静默走错栈序 ✓。
                            ForTarget::Starred(..)
                            | ForTarget::Attribute(..)
                            | ForTarget::Subscript(..) => {
                                return Err(CompileError::Unsupported(
                                    "推导式目标里出现星号／属性／下标（如实报未接线 ✓）".to_owned(),
                                ));
                            }
                            ForTarget::Group(items, group_span) => {
                                let Some((name, _)) = first_for_target_name(&items) else {
                                    return Err(CompileError::Unsupported(
                                        "推导式的组目标里没有名字（如实报未接线 ✓）".to_owned(),
                                    ));
                                };
                                (name, group_span, items)
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
                            // 组的内容直接交过去（可再嵌 ✓）
                            tuple_targets,
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
                    // **闭包（task-25 ④）** ✓：生成器表达式**不内联** ⇒ 体外引用的名字要成为本层 **cell** ✓、
                    // 生成器单元要带 `freevars`（`COPY_FREE_VARS`／`LOAD_DEREF` ✓）。与 `def` 同一条探针 ✓：
                    // 内层 `co_names` 里出现本层局部／cell／自由名 ⇒ 它就是本层的自由变量 ✓。
                    let mut unit = unit;
                    let mut closure_freevars: Vec<String> = Vec::new();
                    for referenced in global_load_names(&unit)
                        .into_iter()
                        .chain(unit.demanded.clone())
                    {
                        let from_here = self.unit.varnames.iter().any(|local| local == &referenced)
                            || self.unit.cellvars.iter().any(|cell| cell == &referenced)
                            || self.unit.freevars.iter().any(|free| free == &referenced);
                        if from_here && !closure_freevars.iter().any(|item| item == &referenced) {
                            closure_freevars.push(referenced);
                        }
                    }
                    if !closure_freevars.is_empty() {
                        unit = compile_scope(
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
                            &closure_freevars,
                            Span::new(span.line_start, span.line_start, 0, 0),
                        )?;
                    }
                    // **闭包元组**在 `LOAD_CONST` **之前** ✓、`SET_FUNCTION_ATTRIBUTE 8` 在 `MAKE_FUNCTION`
                    // **之后** ✓（参照实测 ✓）。
                    if !closure_freevars.is_empty() {
                        for free in &closure_freevars {
                            let slot = self
                                .deref_slot(free)
                                .unwrap_or_else(|| panic!("生成器表达式的自由变量必在本层 cell／free 表里：{free:?}（varnames={:?} cellvars={:?} freevars={:?}）", self.unit.varnames, self.unit.cellvars, self.unit.freevars));
                            self.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
                        }
                        self.emit_indexed(*span, "BUILD_TUPLE", closure_freevars.len());
                    }
                    let index = self.intern_constant(Constant::Code(Box::new(unit)));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    self.emit_named(*span, "MAKE_FUNCTION", 0);
                    if !closure_freevars.is_empty() {
                        self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
                    }
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
                    let mut target_names: Vec<(&str, Span)> = Vec::new();
                    for generator in generators {
                        collect_comprehension_target_names(&generator.target, &mut target_names);
                    }
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
                        self.emit_indexed(*span, "LOAD_FAST_AND_CLEAR", slot);
                        slots.push(slot);
                    }
                    // **`SWAP` 的深度也走带 `EXTENDED_ARG` 的那条** ✓（`as u8` 会截断 ✗）。
                    let depth = target_names.len() + 1;
                    self.emit_indexed(*span, "SWAP", depth);
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
                        // 存目标：名字可直接与"紧接着的那次读取"打成 `STORE_FAST_LOAD_FAST`（实测）；
                        // 组先 `UNPACK_SEQUENCE` 再递归（第 405 轮：`for [off] in …` 也走这条 ✓）
                        match &generator.target {
                            ComprehensionTarget::Name(name, span) => {
                                self.emit_comprehension_store_name(
                                    name,
                                    *span,
                                    element,
                                    generators,
                                    index,
                                    &slots,
                                    &mut slot_index,
                                );
                            }
                            ComprehensionTarget::Group(items, group_span) => {
                                self.emit_comprehension_target_group(
                                    items,
                                    *group_span,
                                    element,
                                    generators,
                                    index,
                                    &slots,
                                    &mut slot_index,
                                    scaffold,
                                )?;
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
                    self.emit_indexed(inner_scaffold, add_op, 1 + generators.len());
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
                    self.emit_indexed(*span, "SWAP", depth);
                    // **逆序**还原目标（同样取整条推导式的跨度）
                    for name in target_names.iter().rev() {
                        let slot = self.slot_of(name.0);
                        self.emit_indexed(*span, "STORE_FAST", slot);
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
            _ => unreachable!("emit_expression_comprehensions 只接收 expr_comprehensions 族的 Expression"),
        }
    }
}
