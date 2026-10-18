//! `emitter` 的 `stmt_assign` 域：赋值族（含增强赋值、链式／解包／下标／属性、空合并赋值、变量标注）。

use super::*;

use super::patterns::assign_target_leaf_span;
impl Emitter {
    pub(crate) fn emit_statement_assign(&mut self, statement: &Statement, rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
                Statement::Annotation {
                    name,
                    annotation,
                    span,
                    ..
                } => {
                    match annotation {
                        AnnotationSource::Expression(expression) => {
                            if self.kind == ScopeKind::Class {
                                self.class_annotations.push((
                                    name.clone(),
                                    AnnotationSource::Expression(expression.clone()),
                                    *span,
                                ));
                            }
                        }
                        AnnotationSource::Type(label) => {
                            self.register_assignment_label(name, label.clone());
                            if self.kind == ScopeKind::Class {
                                self.class_annotations.push((
                                    name.clone(),
                                    AnnotationSource::Type(label.clone()),
                                    *span,
                                ));
                            }
                        }
                    }
                    self.epilogue_span = *span;
                    return Ok(());
                }
                // **`match <主语>: case …`**（第 290 轮，最小面：字面量／捕获／通配／或 ＋ 守卫）
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
                                // **cell／free 名一律走 `STORE_DEREF`** ✓（第 638 轮真 bug 修 ✗）：链式赋值
                                // 这条路**只查了 `varnames`** ✗ ⇒ 已被移出 `varnames` 的 **cell**（典型：
                                // `hits = misses = 0` ＋ 内层 `nonlocal hits, misses` ✓）落成 `STORE_NAME` ✗
                                // ⇒ 函数体里当场报"`STORE_NAME` 需要命名空间帧" ✓
                                //（`functools._lru_cache_wrapper` 的 `hits = misses = 0` 同型 ✓，`import re` 断在这 ✓）。
                                if let Some(slot) = self.deref_slot(name) {
                                    self.emit_indexed(*name_span, "STORE_DEREF", slot);
                                } else if self.kind == ScopeKind::Function
                                    && self.unit.varnames.iter().any(|item| item == name)
                                {
                                    // **融合快路要先确认"末位也是普通局部"** ✓（否则会把 cell 那一格吞掉 ✗）。
                                    let tail_is_plain_local = matches!(
                                        &targets[targets.len() - 1],
                                        Expression::Name(last, _) if self.deref_slot(last).is_none()
                                    );
                                    if let Some((first_slot, second_slot)) = fused_tail.filter(|_| tail_is_plain_local) {
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
                                    // **统一入口** ✓（第 411 轮，task-22 B6 顺带真 bug ✗）：链式赋值的
                                    // 名字目标先前**只查了 varnames**，漏了 `global` 声明 ✗ ⇒
                                    // `global a, b; a = b = {}` 发出 `STORE_NAME` ✗ ⇒ 函数帧里当场
                                    // "STORE_NAME 需要命名空间帧" ✗（`Lib/mimetypes.py` 的
                                    // `_default_mime_types` 正是 `suffix_map = _suffix_map_default = {}`
                                    // 的形状 ⇒ `import mimetypes`／`http.server` 全被它压住 ✓）。
                                    self.emit_store_name(*name_span, name);
                                } else {
                                    self.emit_store_name(*name_span, name);
                                }
                            }
                            Expression::Attribute(object, name, _) => {
                                self.emit_expression(object)?;
                                let index = self.intern_name(name);
                                self.emit_indexed(target_span, "STORE_ATTR", index);
                            }
                            // **元组目标** ✓（第 413 轮，task-26 ④ ✓）：`(a, b) = c = expr` 的链式版。
                            Expression::TupleLiteral(items, tuple_span) => {
                                let unpack: Vec<(Expression, bool)> = items
                                    .iter()
                                    .map(|item| match item {
                                        Expression::Starred(inner, _) => ((**inner).clone(), true),
                                        other => (other.clone(), false),
                                    })
                                    .collect();
                                self.emit_unpack_assign(&unpack, *tuple_span, true)?;
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
                    chain,
                } => {
                    // 实测：值先压 ⇒ `UNPACK_SEQUENCE <个数>`（位点＝**目标那一段** ✓，如 `a, b` ✓）
                    //   或 `EXTENDED_ARG ＋ UNPACK_EX (前个数 | 后个数<<8)` ✓ ⇒ 再按**源码序**存各目标
                    //   （`Name` ⇒ `STORE_NAME`／`STORE_FAST`；`a[0]` ⇒ 对象＋键＋`STORE_SUBSCR`；
                    //   `a.b` ⇒ 对象＋`STORE_ATTR` ✓）；收尾取**最后一个目标** ✓。
                    // **等长窥孔**（实测 ✓）：值是**元组显示**且**项数 ≤ 3** 且与目标数相同时，CPython **不**发
                    //   `UNPACK_SEQUENCE`，而是把各项按**源码序**压栈 ⇒ `SWAP <个数>`（位点＝目标段 ✓）
                    //   ⇒ 再按目标序存 ✓（`a, b = 1, 2` ⇒ `SWAP 2` ✓；`a, b, c = x, y, z` ⇒ `SWAP 3` ✓）。
                    //   **n ≥ 4 不再走窥孔** ✗（task-14 真 bug 修 ✓）：CPython 对 4 项及以上改用
                    //   `值（常量元组会折叠成 LOAD_CONST）＋ UNPACK_SEQUENCE n` ✓ —— 先前这里**不看长度**
                    //   一律 `SWAP 2` ✗ ⇒ 5 项解包实测得到 `(4,5,3,2,1)` ✓（`a,b,c,d,e = 1,2,3,4,5` ✗）。
                    // **链式赋值**：值先算一次 ✓，**从右往左**先赋链上目标 ✓（参照语义 ✓
                    // —— `value, order, next = s = h[0]` 里先 `s = h[0]` ✓），再解元组目标 ✓。
                    // 参照的字节码把它优化成"一份 COPY ＋ 末位目标与链目标合存" ✓（`STORE_FAST_STORE_FAST
                    // (nxt, s)` ✓）；本层先发**等价**的 `COPY 1` ＋ `STORE <链目标>` ✓（如实记这一处
                    // 非逐字节相同 ✗：夹具里还没有这个形态 ✓，真撞上再合 ✓）。
                    let matching_tuple = match value {
                        Expression::TupleLiteral(items, _) => {
                            chain.is_empty() && items.len() == targets.len() && targets.len() <= 3
                        }
                        _ => false,
                    };
                    if matching_tuple {
                        if let Expression::TupleLiteral(items, _) = value {
                            for item in items {
                                self.emit_expression(item)?;
                            }
                        }
                        // `SWAP <目标数>` ✓（2 或 3 ✓）：把最下面那一项转到栈顶 ✓ ⇒ 目标按源码序取 ✓。
                        self.emit_indexed(*target_span, "SWAP", targets.len());
                    } else {
                        self.emit_expression(value)?;
                    }
                    // 链上目标（**从右往左** ✓）：值已经在栈顶 ⇒ 逐个 `COPY 1` ＋ 存 ✓。
                    for target in chain.iter().rev() {
                        let target_span = target.span();
                        // 值在栈顶 ⇒ 复制一份来存（原值留给后面的解包 ✓）。
                        self.emit_at(
                            target_span,
                            opcode::opcode("COPY").expect("COPY 在表里"),
                            1,
                        );
                        match target {
                            Expression::Name(name, name_span) => {
                                self.store_target(*name_span, name);
                            }
                            // **属性／下标目标** ✓（第 413 轮，task-26 ④ ✓）：`self.size, self.name =
                            // `self._state = state`（`multiprocessing/heap.py:58` ✓）／`c.x, c.y = c.z = v` ✓。
                            // 参照 `dis`：`COPY 1; LOAD obj; STORE_ATTR`／`COPY 1; LOAD obj; LOAD key; STORE_SUBSCR` ✓。
                            Expression::Attribute(object, name, _) => {
                                self.emit_expression(object)?;
                                let index = self.intern_name(name);
                                self.emit_indexed(target_span, "STORE_ATTR", index);
                            }
                            Expression::Subscript(object, key, _) => {
                                self.emit_expression(object)?;
                                self.emit_expression(key)?;
                                self.emit_named(target_span, "STORE_SUBSCR", 0);
                            }
                            // **元组目标** ✓：`(a, b) = s = expr` / `self.a, self.b = s = v` ✓ ——
                            // 复制件按解包规则逐项存（与 `AssignTuple` 的主目标同一条助手 ✓）。
                            Expression::TupleLiteral(items, span) => {
                                let unpack: Vec<(Expression, bool)> = items
                                    .iter()
                                    .map(|item| match item {
                                        Expression::Starred(inner, _) => ((**inner).clone(), true),
                                        other => (other.clone(), false),
                                    })
                                    .collect();
                                self.emit_unpack_assign(&unpack, *span, true)?;
                            }
                            // **如实范围** ✗：其余形态（如在链里再嵌套 `*` 以外的解包）随后补 ✓。
                            _ => {
                                return Err(CompileError::Unsupported(
                                    "链式赋值里遇到未接线的目标形态（名字／属性／下标／元组已接 ✓）".to_owned(),
                                ))
                            }
                        }
                    }
                    // **解包 ＋ 逐目标存储**（第 405 轮抽成递归助手 ✓）：嵌套元组／列表目标
                    // （`(a, b), (c, d) = …`／`a, (b, c) = …`）在**每一层**各发一条 `UNPACK_SEQUENCE`
                    // （位点＝那一层的跨度 ✓，参照 `dis` 实测）再递归 ✓；规则与原先的单层完全一致 ✓。
                    self.emit_unpack_assign(targets, *target_span, !matching_tuple)?;
                    if let Some((last, _)) = targets.last() {
                        // **收尾取最后一个叶子目标** ✓（参照实测：`(a, b), (c, d) = …` 的隐式
                        // `LOAD_CONST None; RETURN_VALUE` 位点是 `d` 那段 ✓，不是整个组 ✗）。
                        let leaf = assign_target_leaf_span(last);
                        self.last_span = leaf;
                        self.epilogue_span = leaf;
                    }
                    let _ = span;
                    Ok(())
                }
                // `global a, b`（第 115 轮）：**不发指令** ✓，但**必须在场上登记** ✓ ——
                // 实测"分析趟收在 A 实例、发存储的是 B 实例"✗ ⇒ 只靠构造期扫描不够 ✓。
                // CPython 要求 `global` 先于该作用域里的使用（否则 `SyntaxError` ✓）⇒ 顺序天然成立 ✓。
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
                            // **cell／自由变量优先** ✓（task-20 第 3 轮真 bug 修 ✗）：`nonlocal x; x += 1`
                            // 先前落到 `LOAD_NAME`／`STORE_NAME` ✗ ⇒ **函数帧没有命名空间** ⇒ 实测
                            // `帧操作失败`／`LOAD_NAME 需要命名空间帧` ✓（`Lib/functools.py` 的
                            // `_lru_cache_wrapper` 一族就卡在这 ✓）。
                            let deref = self.deref_slot(name);
                            if let Some(slot) = deref {
                                self.emit_indexed(*name_span, "LOAD_DEREF", slot);
                            } else if self.kind == ScopeKind::Function
                                && self.unit.varnames.iter().any(|item| item == name)
                            {
                                let slot = self.slot_of(name);
                                self.emit_indexed(*name_span, "LOAD_FAST_BORROW", slot);
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
                            // **`TS-47`／`BC-65`**：`x += v` 的结果同样要过已标注变量的赋值检查
                            // （检查读栈顶不弹 ⇒ 下面的 `STORE_*` 照旧拿得到结果）。
                            self.emit_assignment_check(*name_span, name);
                            if let Some(slot) = deref {
                                self.emit_indexed(*name_span, "STORE_DEREF", slot);
                            } else if self.kind == ScopeKind::Function
                                && self.unit.varnames.iter().any(|item| item == name)
                            {
                                let slot = self.slot_of(name);
                                // 存入取**目标**跨度（第 228 轮按正确配对重测：`x %= 2` ⇒ `STORE_NAME`
                                // 是 `(0,1)`；`BINARY_OP` 才是整条语句 `(0,6)`）——旧注释源自错位测量
                                self.emit_indexed(*name_span, "STORE_FAST", slot);
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
                            self.emit_shifted(*target_span, "LOAD_ATTR", index << 1);
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
                            if self.unit.varnames.iter().any(|item| item == name)
                                // **形参 cell 不能走这条** ✗（task-25 ② 真 bug 修 ✓）：`cls` 既是 `varnames`
                                // 又是 `cellvars` ⇒ 它的 localsplus 槽里放的是 **cell 对象** ✓
                                //（`LOAD_FAST 0` 会把 cell 本身压栈 ✗）。实测
                                // `Lib/functools.py::singledispatch.register` 的 `func = cls`（`cls` 被内部
                                // lambda 捕获 ⇒ 形参 cell ✓）⇒ `func` 拿到 cell ⇒ `get_type_hints(cell)` ⇒
                                // `importlib.resources` 的 `<cell object> does not have annotations` ✓。
                                && self.deref_slot(name).is_none() =>
                        {
                            let slot = self.slot_of(name);
                            self.emit_indexed(*name_span, "LOAD_FAST", slot);
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
                    // **`TS-47`／`BC-65`**：已标注变量的赋值 ⇒ 在 `STORE_*` **之前**检查栈顶
                    // （读而不弹）。名字目标在模块／类体／函数三档都走这里；属性／下标目标
                    // 本轮**不发**（如实留缺：那些不是名字目标的赋值检查随后补）。
                    self.emit_assignment_check(*target_span, target);
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
                                self.emit_indexed(*target_span, "STORE_DEREF", slot);
                                return Ok(());
                            }
                            let slot = self.slot_of(target);
                            // 实测：`STORE_FAST` 的位置**总是目标**（`x = a` ⇒ `x` 那一格）
                            self.emit_indexed(*target_span, "STORE_FAST", slot);
                        }
                    }
                    Ok(())
                }
                // **`BC-63` 空合并赋值 `a ??= b`**（语句）：`a` 只读一次；`None` 时才执行 `a = b`
                //（含 `TS-47` 的运行期 `CHECK_ASSIGNMENT`：`TS-49` 说它**不改变标注类型**，检查照原标注）。
                //
                // 形态：`<读 a>; POP_JUMP_IF_NOT_NONE 结束; <b>; [CHECK_ASSIGNMENT]; STORE a; 结束:`
                Statement::NullCoalesceAssign {
                    name,
                    name_span,
                    value,
                    span,
                } => {
                    let end = self.new_label();
                    self.emit_expression(&Expression::Name(name.clone(), *name_span))?;
                    self.emit_jump(
                        *span,
                        opcode::opcode("POP_JUMP_IF_NOT_NONE").expect("条件跳转在表里"),
                        end,
                    );
                    self.emit_expression(value)?;
                    self.epilogue_span = *name_span;
                    self.emit_assignment_check(*name_span, name);
                    self.emit_store_name(*name_span, name);
                    self.mark_label(end);
                    Ok(())
                }
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
                            match (
                                self.plain_local_slot(value_name),
                                self.plain_local_slot(object_name),
                            ) {
                                // **前提：两个槽号都要装得进 4 位** ✓（第 308 轮修 ✗）：融合形式把两个槽号各塞进
                                // 半个字节 ✓（`oparg >> 4` / `oparg & 0x0F` ✓）⇒ 槽号 ≥16 时装不下 ✗ ⇒ 会读到**别的槽**
                                // （实测：`method` 写进槽 22、融合加载却读槽 7 ✗ ⇒ `enum.py` 一族 118 个模块卡住 ✓）。
                                // 装不下就返回 `None` ✓，交给调用方的**非融合回退**发两条独立加载 ✓（参照同样只在 <16 时融合 ✓）。
                                (Some(first), Some(second)) if first <= 0x0F && second <= 0x0F => {
                                    Some((first, second))
                                }
                                (Some(_), Some(_)) => None,
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
            _ => unreachable!("emit_statement_assign 只接收 stmt_assign 族的 Statement"),
        }
    }
}
