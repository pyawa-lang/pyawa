//! `compile` 的子模块（拆分自单文件时期，见 `AGENTS.md`）。

use super::*;

/// **`__static_attributes__` 的静态收集**（实测 3.14 的规则）：
///
/// - 只收**赋值**形态 `self.名字 = …`；只读 `self.名字` 不算
/// - **按字母序输出**且**去重**（同一条里先写 `self.b` 再写 `self.a` ⇒ `('a', 'b')`）
/// - 类体层的普通赋值（`x = 1`）不算
/// - **嵌套函数里也算**（方法里的 `def inner(): self.z = 1` ⇒ 收到的 `z`）
///
/// - `if`／`while`／`for` 的体（含各自的 `else` 体）也走进去（实测：`if x: self.a = 1` ⇒ `('a',)`）
pub(super) fn collect_static_attributes(statements: &[Statement], out: &mut Vec<String>) {
    for statement in statements {
        match statement {
            Statement::AssignAttr { object, name, .. } => {
                if matches!(object, Expression::Name(base, _) if base == "self") {
                    out.push(name.clone());
                }
            }
            // 方法与嵌套函数
            Statement::Def { body, .. } => collect_static_attributes(body, out),
            // 复合语句的体（实测：`if`／`while`／`for` 体里写 `self.X` 同样会收）
            Statement::For {
                body, else_body, ..
            }
            | Statement::While {
                body, else_body, ..
            } => {
                collect_static_attributes(body, out);
                collect_static_attributes(else_body, out);
            }
            Statement::If {
                then_body, else_body, ..
            } => {
                collect_static_attributes(then_body, out);
                collect_static_attributes(else_body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                // 与**发射顺序**一致：套体 → `else` → `finally` → 各处理块
                collect_static_attributes(body, out);
                collect_static_attributes(else_body, out);
                collect_static_attributes(finally_body, out);
                for handler in handlers {
                    collect_static_attributes(&handler.body, out);
                }
            }
            Statement::With { body, .. } => collect_static_attributes(body, out),
            // `import` 不改属性；`from … import *` 也别去猜绑定了什么
            Statement::Import { .. } | Statement::ImportFrom { .. } => {}
            _ => {}
        }
    }
}

/// **源码序预登记**（第 229 轮）：参照的 `co_names` 按**编译（源码）顺序**登记，而块结构模型会
/// **复制**退出路径（先发复制件、后发正常路径）⇒ 不预登记就会错位（实测
/// `for i in s:\n    if i:\n        break\n    x = i\ny = 2\n` 的 `co_names` 是 `["s","i","x","y"]`，
/// 按发射顺序会得到 `["s","i","y","x"]`）。
///
/// 只处理**本作用域**：嵌套 `def`／`class` 的**体**归各自作用域（跳过），但它们的名字与
/// 默认值表达式仍在**本作用域**求值 ⇒ 照走。函数作用域里被赋名的目标是**局部**（进 `varnames`）。
pub(super) fn pre_intern(emitter: &mut Emitter, statements: &[Statement]) {
    if emitter.kind == ScopeKind::Function {
        collect_locals(emitter, statements);
    }
    for statement in statements {
        match statement {
            Statement::NonLocal(..) => {}
            Statement::Yield(value, _) => {
                if let Some(value) = value {
                    pre_intern_expression(emitter, value);
                }
            }
            Statement::Assign { target, value, .. } => {
                pre_intern_expression(emitter, value);
                // **cell／自由变量的名字不进 `co_names`**（实测 `nonlocal x` 的内层 `co_names=()`；
                // 它只该出现在 `co_freevars` 里 ✓）
                if emitter.deref_slot(target).is_none() {
                    pre_intern_target(emitter, target);
                }
            }
            Statement::Return(value, _) | Statement::Expression(value, _) => {
                pre_intern_expression(emitter, value);
            }
            Statement::For {
                target,
                iterable,
                body,
                else_body,
                tuple_targets,
                ..
            } => {
                pre_intern_expression(emitter, iterable);
                pre_intern_target(emitter, target);
                // **元组目标**（第 118 轮）：顺序照实测 `co_names = ('x','n','line')` ✓
                //（可迭代表达式的名字在前、目标按源码序在后 ✓）；函数里同时声明为局部 ✓。
                for (name, _) in tuple_targets {
                    pre_intern_target(emitter, name);
                }
                pre_intern(emitter, body);
                pre_intern(emitter, else_body);
            }
            Statement::While {
                condition,
                body,
                else_body,
                ..
            } => {
                pre_intern_expression(emitter, condition);
                pre_intern(emitter, body);
                pre_intern(emitter, else_body);
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                pre_intern_expression(emitter, condition);
                pre_intern(emitter, then_body);
                pre_intern(emitter, else_body);
            }
            Statement::Raise { value, cause, .. } => {
                if let Some(value) = value {
                    pre_intern_expression(emitter, value);
                }
                if let Some(cause) = cause {
                    pre_intern_expression(emitter, cause);
                }
            }
            Statement::With { items, body, .. } => {
                for (context, target) in items {
                    pre_intern_expression(emitter, context);
                    if let Some((target, _)) = target {
                        pre_intern_target(emitter, target);
                    }
                }
                pre_intern(emitter, body);
            }
            // `import a.b as c` ⇒ 名字顺序 `('a.b', 'b', 'c')`（实测）
            Statement::Import { items, .. } => {
                for (module, alias) in items {
                    emitter.intern_name(module);
                    match alias {
                        Some(alias) => {
                            let last = module.rsplit('.').next().unwrap_or(module);
                            emitter.intern_name(last);
                            emitter.intern_name(alias);
                        }
                        None => {
                            let top = module.split('.').next().unwrap_or(module);
                            emitter.intern_name(top);
                        }
                    }
                }
            }
            // `from a import b as c, d` ⇒ 名字顺序 `('a', 'b', 'c', 'd')`（实测）
            Statement::ImportFrom {
                module,
                names,
                star,
                ..
            } => {
                emitter.intern_name(module);
                if !*star {
                    for (name, alias) in names {
                        emitter.intern_name(name);
                        if let Some(alias) = alias {
                            emitter.intern_name(alias);
                        }
                    }
                }
            }
            // **按发射顺序**登记：套体 → `else` → `finally` → 各处理块（实测 `co_names` 就是这个次序）
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                // 次序＝CPython 的**编译顺序**（`try/except/finally` 脱糖成"内层 try/except 先、
                // finally 后"）⇒ body → `else` → 各处理块 → `finally`（实测 `co_names`）
                pre_intern(emitter, body);
                pre_intern(emitter, else_body);
                for handler in handlers {
                    if let Some(type_) = &handler.type_ {
                        pre_intern_expression(emitter, type_);
                    }
                    if let Some(name) = &handler.name {
                        // **按作用域** ✓（第 202 轮真 bug 修复 ✗：先前一律 `intern_name` ⇒
                        // 函数里 `except … as 名字` 会多出一个 `co_names` 项 ⇒ 与参照对不上 ✓）。
                        pre_intern_target(emitter, name);
                    }
                    pre_intern(emitter, &handler.body);
                }
                pre_intern(emitter, finally_body);
            }
            Statement::AugAssign { target, value, .. } => {
                match target {
                    AugTarget::Name(name, _) => pre_intern_target(emitter, name),
                    AugTarget::Attribute { object, name, .. } => {
                        pre_intern_expression(emitter, object);
                        emitter.intern_name(name);
                    }
                    AugTarget::Subscript { container, key, .. } => {
                        pre_intern_expression(emitter, container);
                        pre_intern_expression(emitter, key);
                    }
                }
                pre_intern_expression(emitter, value);
            }
            Statement::AssignAttr {
                object, name, value, ..
            } => {
                pre_intern_expression(emitter, value);
                pre_intern_expression(emitter, object);
                emitter.intern_name(name);
            }
            Statement::AssignSubscript {
                container, key, value, ..
            } => {
                pre_intern_expression(emitter, value);
                pre_intern_expression(emitter, container);
                pre_intern_expression(emitter, key);
            }
            Statement::Def {
                name,
                parameters,
                kwonly,
                decorators,
                body: _,
                ..
            } => {
                // **装饰器先于 `def` 自己的名字**：实测 `@a.b` 的 `co_names` 是 `["a","b","f"]` ✓
                // （装饰器在源码里先被求值 ⇒ 名字表顺序照它 ✓；第一版把 `f` 排在最前 ✗，夹具抓住 ✓）
                for decorator in decorators {
                    pre_intern_expression(emitter, decorator);
                }
                for parameter in parameters.iter().chain(kwonly.iter()) {
                    if let Some(default) = &parameter.default {
                        pre_intern_expression(emitter, default);
                    }
                }
                // **函数里嵌套的 `def`**：名字是**局部**（进 `varnames`，不进 `co_names`）⇒ 不 intern。
                // 实测 `def outer(): def inner(): …` 的内层单元 `co_names` 是**空**的。
                if emitter.kind != ScopeKind::Function {
                    emitter.intern_name(name);
                }
            }
            Statement::Class {
                name, bases, body: _, ..
            } => {
                for base in bases {
                    pre_intern_expression(emitter, base);
                }
                // **按作用域** ✓（第 202 轮真 bug 修复 ✗：先前一律 `intern_name` ⇒
                // 函数里定义类会多出一个 `co_names` 项 ⇒ 与参照对不上 ✓；函数里该进 `varnames` ✓）。
                pre_intern_target(emitter, name);
            }
            Statement::AssignChained { targets, value, .. } => {
                // 实测 `a = b = x` 的 `co_names` 是 `('x','a','b')` ✓ ⇒ **值先、目标后** ✓
                pre_intern_expression(emitter, value);
                for target in targets {
                    pre_intern_expression(emitter, target);
                }
            }
            Statement::AssignTuple { targets, value, .. } => {
                // 实测 `a, b = x` 的 `co_names` 是 `('x','a','b')` ✓ ⇒ **值先、目标后** ✓
                pre_intern_expression(emitter, value);
                for (target, _) in targets {
                    pre_intern_expression(emitter, target);
                }
            }
            Statement::Delete { targets, .. } => {
                // `DELETE_NAME` 的 oparg 是**名字下标** ⇒ 名字必须先登记 ✓（顺序＝源码序 ✓）
                for target in targets {
                    pre_intern_expression(emitter, target);
                }
            }
            Statement::Global(_, _) => {}
            Statement::Pass(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Assert { .. } => {}
        }
    }
}

/// **收集局部名**（函数作用域）：赋名的目标按源码顺序进 `varnames`。
/// 收本作用域的局部名。**必须在发射任何指令之前调用一次**：`slot_of` 的索引取决于
/// `varnames` 的最终内容（闭包分析还要把 cell 名移出去，第 291 轮）。
/// **`global` 声明的名字**：本作用域的语句树里（`if`／`while`／`for`／`try`／`with` 体内也算 ✓，
/// **不**下探内层 `def` ✓）声明的都收 ✓。
pub(super) fn collect_scope_globals(statements: &[Statement], out: &mut Vec<String>) {
    for statement in statements {
        match statement {
            Statement::Global(names, _) => {
                for name in names {
                    if !out.iter().any(|item| item == name) {
                        out.push(name.clone());
                    }
                }
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_scope_globals(then_body, out);
                collect_scope_globals(else_body, out);
            }
            Statement::While { body, else_body, .. }
            | Statement::For { body, else_body, .. } => {
                collect_scope_globals(body, out);
                collect_scope_globals(else_body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_scope_globals(body, out);
                for handler in handlers {
                    collect_scope_globals(&handler.body, out);
                }
                collect_scope_globals(else_body, out);
                collect_scope_globals(finally_body, out);
            }
            Statement::With { body, .. } => collect_scope_globals(body, out),
            _ => {}
        }
    }
}

/// **声明一个局部**（第 114 轮）：`global` 声明的名字**不进 `varnames`** ✓。
fn declare_local(emitter: &mut Emitter, name: &str) {
    if emitter.global_names.iter().any(|item| item == name) {
        return;
    }
    emitter.slot_of(name);
}

pub(super) fn collect_scope_locals(emitter: &mut Emitter, statements: &[Statement]) {
    // **先收 `global` 声明**（第 114 轮）：本作用域里这些名字不进 `varnames` ✓
    let mut globals = Vec::new();
    collect_scope_globals(statements, &mut globals);
    for name in globals {
        if !emitter.global_names.iter().any(|item| item == &name) {
            emitter.global_names.push(name);
        }
    }
    collect_locals(emitter, statements);
}

/// **闭包分析**：本作用域里被**内层 `def`** 引用到的局部 ⇒ `co_cellvars`（并从 `varnames` 移出，
/// 它在 localsplus 里排在 varnames 之后）。判定用**探针编译**：把每个内层 `def` 按下层作用域编一遍，
/// 看它把哪些名字当成了**全局**（`co_names`）—— 那些正是它引用的外层局部 ✓。
pub(super) fn analyze_cells(
    emitter: &mut Emitter,
    statements: &[Statement],
    qualname: &str,
    mode: Mode,
    tier: CheckTier,
) {
    if !emitter.unit.cellvars.is_empty() {
        return; // 已经算过（幂等）
    }
    let mut nested_defs: Vec<&Statement> = Vec::new();
    collect_nested_defs(statements, &mut nested_defs);
    let mut cells: Vec<String> = Vec::new();
    let mut demanded: Vec<String> = Vec::new();
    for def in nested_defs {
        let Statement::Def {
            name,
            parameters,
            kwonly,
            returns,
            varargs,
            varkw,
            body,
            first_line,
            ..
        } = def
        else {
            continue;
        };
        let probe_qualname = format!("{qualname}.<locals>.{name}");
        let probe = compile_scope(
            name,
            &probe_qualname,
            parameters,
            kwonly,
            returns.as_ref(),
            varargs.as_deref(),
            varkw.as_deref(),
            mode,
            tier,
            body,
            ScopeKind::Function,
            false,
            &[],
            Span::new(*first_line, *first_line, 0, 0),
        );
        // **`nonlocal` 声明的名字**：内层声明 `nonlocal x` ⇒ 本层的 `x` 必须是 **cell**
        // （实测 `def outer(): x = 0; def inner(): nonlocal x; x = 1` ⇒ 外层 `cellvars=('x',)`、
        // 内层 `freevars=('x',)`；内层的 `co_names` 里**没有** `x`，光看 `names` 会漏掉 ✓）
        // 每个嵌套 `def` 的体里声明的 `nonlocal` 与其中的 lambda 需求（第 295／297 轮）
        collect_nonlocals(body, &mut demanded);
        collect_lambda_demands(body, &mut demanded);
        if let Ok(probe) = probe {
            // **内层的需求逐层上浮**（第 298 轮）：探针编出的内层单元带着它向外索取的名字 ✓
            for wanted in &probe.demanded {
                if !demanded.iter().any(|item| item == wanted) {
                    demanded.push(wanted.clone());
                }
            }
            for referenced in &probe.names {
                if emitter.unit.varnames.iter().any(|local| local == referenced) {
                    // 本层有这个名字 ⇒ 本层把它变 cell 就够了，需求到此为止 ✓
                    if !cells.iter().any(|cell| cell == referenced) {
                        cells.push(referenced.clone());
                    }
                } else if !demanded.iter().any(|item| item == referenced) {
                    // 本层没有 ⇒ **继续向外层索取** ✓（两层闭包的关键一环）
                    demanded.push(referenced.clone());
                }
            }
        }
    }
    // **本层的 lambda 需求**（不在任何嵌套 `def` 里的那些）—— 第 297 轮踩过的坑 ✗：
    // 需求的收集原先写在 `for def in nested_defs` 循环体里 ⇒ **没有嵌套 `def` 的作用域整段不执行**
    // ⇒ `return lambda: x` 这种（只有 lambda、没有 def）就漏了 `x` ✗。
    collect_lambda_demands(statements, &mut demanded);
    for name in &demanded {
        if emitter.unit.varnames.iter().any(|local| local == name)
            && !cells.iter().any(|cell| cell == name)
        {
            cells.push(name.clone());
        }
    }
    // **向外索取的名字** ＝ 需求集合减去本层自己解决的（cell）—— 剩下的留给更外层 ✓
    emitter.unit.demanded = demanded
        .iter()
        .filter(|name| !cells.iter().any(|cell| cell == *name))
        .cloned()
        .collect();
    if !cells.is_empty() {
        cells.sort_by_key(|cell| {
            emitter
                .unit
                .varnames
                .iter()
                .position(|local| local == cell)
                .unwrap_or(usize::MAX)
        });
        // **形参 cell 保留在 `varnames`**（实参槽就是它的 cell 槽；实测 `def outer(x): …` ⇒
        // `varnames=('x','inner')`、`nlocals=2`），只把**局部** cell 移出去 ✓。
        let parameters = emitter.parameter_count();
        let kept: Vec<String> = emitter
            .unit
            .varnames
            .iter()
            .enumerate()
            .filter(|(index, local)| {
                *index < parameters || !cells.iter().any(|cell| cell == *local)
            })
            .map(|(_, local)| local.clone())
            .collect();
        emitter.unit.varnames = kept;
        emitter.unit.nlocals = emitter.unit.varnames.len();
        emitter.unit.cellvars = cells;
    }
}

/// 走一个**表达式**，收集其中的 `Name`（`lambda` 只收它**体外**所需的名字：体内引用减去自己的形参）。
/// 给 lambda 的闭包分析用（第 297 轮）。
pub(super) fn collect_names_in_expression(expression: &Expression, out: &mut Vec<String>) {
    let push = |name: &String, out: &mut Vec<String>| {
        if !out.iter().any(|item| item == name) {
            out.push(name.clone());
        }
    };
    match expression {
        Expression::Name(name, _) => push(name, out),
        Expression::Walrus { target, value, .. } => {
            // 值先、目标后（实测 `if (o := f()) is None:` 的 `co_names` 是 `('f','o','x')` ✓）
            collect_names_in_expression(value, out);
            push(target, out);
        }
        Expression::List(items, _)
        | Expression::SetLiteral(items, _)
        | Expression::TupleLiteral(items, _) => {
            for item in items {
                collect_names_in_expression(item, out);
            }
        }
        Expression::Map(items, _) => {
            for (key, value) in items {
                collect_names_in_expression(key, out);
                collect_names_in_expression(value, out);
            }
        }
        Expression::Attribute(target, _, _)
        | Expression::Not(target, _)
        | Expression::Unary(_, target, _) => collect_names_in_expression(target, out),
        Expression::Binary(_, left, right, _)
        | Expression::Compare(left, _, right, _)
        | Expression::Subscript(left, right, _) => {
            collect_names_in_expression(left, out);
            collect_names_in_expression(right, out);
        }
        Expression::BoolOp { values, .. } | Expression::ChainedCompare { operands: values, .. } => {
            for value in values {
                collect_names_in_expression(value, out);
            }
        }
        Expression::Conditional {
            condition,
            then_value,
            else_value,
            ..
        } => {
            collect_names_in_expression(condition, out);
            collect_names_in_expression(then_value, out);
            collect_names_in_expression(else_value, out);
        }
        Expression::FString { parts, .. } => {
            for part in parts {
                if let FStringPart::Formatted { expression, spec, .. } = part {
                    collect_names_in_expression(expression, out);
                    if let Some(spec) = spec {
                        for item in spec {
                            if let FStringPart::Formatted { expression, .. } = item {
                                collect_names_in_expression(expression, out);
                            }
                        }
                    }
                }
            }
        }
        Expression::Lambda {
            parameters,
            kwonly,
            varargs,
            varkw,
            body,
            ..
        } => {
            // lambda 体里引用的名字，扣掉它自己的形参（那些是它自己的局部）
            let mut inner = Vec::new();
            collect_names_in_expression(body, &mut inner);
            for name in &inner {
                let is_parameter = parameters.iter().any(|item| item.name == *name)
                    || kwonly.iter().any(|item| item.name == *name)
                    || varargs.as_deref() == Some(name.as_str())
                    || varkw.as_deref() == Some(name.as_str());
                if !is_parameter {
                    push(name, out);
                }
            }
        }
        Expression::Call {
            function,
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            ..
        } => {
            collect_names_in_expression(function, out);
            for argument in arguments.iter().chain(star_arguments) {
                collect_names_in_expression(argument, out);
            }
            for (_, value) in keywords {
                collect_names_in_expression(value, out);
            }
            for value in dict_arguments {
                collect_names_in_expression(value, out);
            }
        }
        Expression::Comprehension {
            element,
            value,
            generators,
            ..
        } => {
            collect_names_in_expression(element, out);
            if let Some(value) = value {
                collect_names_in_expression(value, out);
            }
            // 生成器的**目标名**是推导式自己的局部，不算需求 ⇒ 只走可迭代对象与过滤条件 ✓
            for generator in generators {
                collect_names_in_expression(&generator.iterable, out);
                for condition in &generator.conditions {
                    collect_names_in_expression(condition, out);
                }
            }
        }
        Expression::SliceLiteral {
            lower,
            upper,
            step,
            ..
        } => {
            for part in [lower, upper, step].into_iter().flatten() {
                collect_names_in_expression(part, out);
            }
        }
        _ => {}
    }
}

/// 走一批**语句**（含 `if`／循环／`try`／`with` 的体，**不进**内层 `def`／`class`），收集其中
/// **所有 lambda** 需要从本作用域拿的名字（第 297 轮）。
pub(super) fn collect_lambda_demands(statements: &[Statement], out: &mut Vec<String>) {
    for statement in statements {
        let walk = |value: &Expression, out: &mut Vec<String>| find_lambda_demands(value, out);
        match statement {
            Statement::Return(value, _) | Statement::Expression(value, _) => walk(value, out),
            Statement::Assign { value, .. } | Statement::AugAssign { value, .. } => {
                walk(value, out);
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                walk(condition, out);
                collect_lambda_demands(then_body, out);
                collect_lambda_demands(else_body, out);
            }
            Statement::While {
                condition,
                body,
                else_body,
                ..
            } => {
                walk(condition, out);
                collect_lambda_demands(body, out);
                collect_lambda_demands(else_body, out);
            }
            Statement::For {
                iterable,
                body,
                else_body,
                ..
            } => {
                walk(iterable, out);
                collect_lambda_demands(body, out);
                collect_lambda_demands(else_body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_lambda_demands(body, out);
                for handler in handlers {
                    collect_lambda_demands(&handler.body, out);
                }
                collect_lambda_demands(else_body, out);
                collect_lambda_demands(finally_body, out);
            }
            Statement::With { body, .. } => collect_lambda_demands(body, out),
            _ => {}
        }
    }
}

/// **收集本作用域（含 `if`／循环／`try`／`with` 体，但**不进**内层 `def`／`class`）里声明的
/// `nonlocal` 名字** —— 给闭包分析用（第 295 轮）。
pub(super) fn collect_nonlocals(statements: &[Statement], out: &mut Vec<String>) {
    for statement in statements {
        match statement {
            Statement::NonLocal(names, _) => {
                for name in names {
                    if !out.iter().any(|item| item == name) {
                        out.push(name.clone());
                    }
                }
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_nonlocals(then_body, out);
                collect_nonlocals(else_body, out);
            }
            Statement::While { body, else_body, .. }
            | Statement::For { body, else_body, .. } => {
                collect_nonlocals(body, out);
                collect_nonlocals(else_body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_nonlocals(body, out);
                for handler in handlers {
                    collect_nonlocals(&handler.body, out);
                }
                collect_nonlocals(else_body, out);
                collect_nonlocals(finally_body, out);
            }
            Statement::With { body, .. } => collect_nonlocals(body, out),
            _ => {}
        }
    }
}

/// **收集本作用域里的内层 `def`**（递归进 `if`／循环／`try`／`with` 的体；**不进** `def`／`class`
/// 的体——那是下一层作用域的事）。给闭包分析用（第 290 轮）。
pub(super) fn collect_nested_defs<'a>(statements: &'a [Statement], out: &mut Vec<&'a Statement>) {
    for statement in statements {
        match statement {
            Statement::Def { .. } => out.push(statement),
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_nested_defs(then_body, out);
                collect_nested_defs(else_body, out);
            }
            Statement::While { body, else_body, .. }
            | Statement::For { body, else_body, .. } => {
                collect_nested_defs(body, out);
                collect_nested_defs(else_body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_nested_defs(body, out);
                for handler in handlers {
                    collect_nested_defs(&handler.body, out);
                }
                collect_nested_defs(else_body, out);
                collect_nested_defs(finally_body, out);
            }
            Statement::With { body, .. } => collect_nested_defs(body, out),
            _ => {}
        }
    }
}

/// **声明一个赋值目标里的所有名字** ✓（第 262 轮真 bug ✗）：**递归**处理
/// `Name`／`TupleLiteral`／`Starred` 三种形态 ✓ —— `a, b = x` 与 `for a, b in …` 一类
/// 目标是**元组** ✗ ⇒ 先前只认单个 `Name` ✗ ⇒ 那些名字要**等到发射期**才被追加 ✗，
/// 而**序言**里的 `MAKE_CELL` 槽号是按**当时**的 `varnames` 算的 ✗ ⇒ cell 槽**整体错位** ✗
///（实测 `Lib/os.py`：`_create_environ_mapping` 少了 `value` ✗、`makedirs` 少了 `head`／`tail` ✗）。
pub(super) fn declare_target(emitter: &mut Emitter, target: &Expression) {
    match target {
        Expression::Name(name, _) => declare_local(emitter, name),
        Expression::TupleLiteral(items, _) | Expression::List(items, _) => {
            for item in items {
                declare_target(emitter, item);
            }
        }
        Expression::Starred(inner, _) => declare_target(emitter, inner),
        _ => {}
    }
}

/// **推导式目标** ✓（第 264 轮）：3.12+ 把**列表／集合／字典**推导式**内联** ⇒ 它的目标是**外层局部** ✓
/// （`Lib/site.py` 的 `sys.path = [p for p in original_path if p != '']` 就是这样 ✓）。
/// **生成器表达式仍是独立作用域** ✗（3.12 只内联了前三者 ✓）⇒ 它的目标**不算**外层局部 ✓。
/// **推导式的目标** ✓（`ComprehensionTarget`：名字或名字元组 ✓）。
fn declare_comprehension_target(emitter: &mut Emitter, target: &ComprehensionTarget) {
    match target {
        ComprehensionTarget::Name(name, _) => declare_local(emitter, name),
        ComprehensionTarget::Tuple(items) => {
            // 元组里的元素是**（名字, 跨度）** ✓ ⇒ 逐个声明 ✓。
            for (name, _) in items {
                declare_local(emitter, name);
            }
        }
    }
}

/// **推导式目标／`:=` 目标的预扫** ✓（第 264 轮起；第 277 轮改成**递归**并把缺口补全）。
///
/// 3.12+ 把**列表／集合／字典**推导式**内联** ⇒ 它的目标是**外层局部** ✓（生成器表达式仍是
/// 独立作用域 ✗ ⇒ 目标不算外层局部 ✓）；`:=` 的目标同样是外层局部 ✓。
///
/// **必须递归** ✗：推导式可以嵌在**任何**表达式里（`sys.path = [p for p in …]` ✓、
/// `f([x for x in y])` ✓）—— 先前只认"值恰好就是推导式" ✗ ⇒ `Lib/site.py` 的
/// `register_readline` 漏了 `p` ✗ ⇒ **序言**里的 `MAKE_CELL` 槽号错位 ✗（与 `DIV-9` 同一族 ✓）。
/// `lambda` 的体**不进** ✗：那是**另一个作用域**（由它自己的 `collect_scope_locals` 收 ✓）。
pub(super) fn collect_comprehension_locals(emitter: &mut Emitter, expression: &Expression) {
    match expression {
        Expression::Comprehension {
            kind,
            element,
            value,
            generators,
            ..
        } => {
            if !matches!(kind, ComprehensionKind::Generator) {
                for generator in generators {
                    declare_comprehension_target(emitter, &generator.target);
                }
            }
            collect_comprehension_locals(emitter, element);
            if let Some(value) = value {
                collect_comprehension_locals(emitter, value);
            }
            for generator in generators {
                collect_comprehension_locals(emitter, &generator.iterable);
                for condition in &generator.conditions {
                    collect_comprehension_locals(emitter, condition);
                }
            }
        }
        // `:=` 的目标是本作用域的局部 ✓（值先、目标后 ✓ 与 `co_names` 的实测次序一致 ✓）
        Expression::Walrus { target, value, .. } => {
            collect_comprehension_locals(emitter, value);
            declare_local(emitter, target);
        }
        Expression::List(items, _)
        | Expression::SetLiteral(items, _)
        | Expression::TupleLiteral(items, _) => {
            for item in items {
                collect_comprehension_locals(emitter, item);
            }
        }
        Expression::Map(items, _) => {
            for (key, value) in items {
                collect_comprehension_locals(emitter, key);
                collect_comprehension_locals(emitter, value);
            }
        }
        Expression::Attribute(target, _, _)
        | Expression::Not(target, _)
        | Expression::Unary(_, target, _)
        | Expression::Starred(target, _) => collect_comprehension_locals(emitter, target),
        Expression::Binary(_, left, right, _)
        | Expression::Compare(left, _, right, _)
        | Expression::Subscript(left, right, _) => {
            collect_comprehension_locals(emitter, left);
            collect_comprehension_locals(emitter, right);
        }
        Expression::BoolOp { values, .. } | Expression::ChainedCompare { operands: values, .. } => {
            for value in values {
                collect_comprehension_locals(emitter, value);
            }
        }
        Expression::Conditional {
            condition,
            then_value,
            else_value,
            ..
        } => {
            collect_comprehension_locals(emitter, condition);
            collect_comprehension_locals(emitter, then_value);
            collect_comprehension_locals(emitter, else_value);
        }
        Expression::FString { parts, .. } => {
            for part in parts {
                if let FStringPart::Formatted { expression, spec, .. } = part {
                    collect_comprehension_locals(emitter, expression);
                    if let Some(spec) = spec {
                        for item in spec {
                            if let FStringPart::Formatted { expression, .. } = item {
                                collect_comprehension_locals(emitter, expression);
                            }
                        }
                    }
                }
            }
        }
        Expression::SliceLiteral {
            lower,
            upper,
            step,
            ..
        } => {
            for part in [lower, upper, step].into_iter().flatten() {
                collect_comprehension_locals(emitter, part);
            }
        }
        Expression::Call {
            function,
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            ..
        } => {
            collect_comprehension_locals(emitter, function);
            for argument in arguments.iter().chain(star_arguments) {
                collect_comprehension_locals(emitter, argument);
            }
            for (_, value) in keywords {
                collect_comprehension_locals(emitter, value);
            }
            for value in dict_arguments {
                collect_comprehension_locals(emitter, value);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_locals(emitter: &mut Emitter, statements: &[Statement]) {
    // **先收本作用域的 `global` 声明**（第 115 轮）：这些名字不进 `varnames` ✓。
    // 放在这里（而不是只在 `collect_scope_locals` 里）是因为**嵌套作用域**的局部收集点不止一处 ✓。
    let mut globals = Vec::new();
    collect_scope_globals(statements, &mut globals);
    for name in globals {
        if !emitter.global_names.iter().any(|item| item == &name) {
            emitter.global_names.push(name);
        }
    }
    for statement in statements {
        match statement {
            Statement::Delete { targets, .. } => {
                // `del x` 在函数里让 `x` 成为**局部**（参照：`co_varnames=('x',)` ＋ `DELETE_FAST` ✓）
                // 第 108 轮实测：漏了这一条 ⇒ 函数里 `del x` 的 `nlocals` 少 1 ✗（夹具当场抓到 ✓）
                for target in targets {
                    declare_target(emitter, target);
                }
            }
            Statement::AssignChained { targets, value, .. } => {
                // **右值里的推导式／`:=` 目标** ✓（第 277 轮：递归预扫）
                collect_comprehension_locals(emitter, value);
                for target in targets {
                    declare_target(emitter, target);
                }
            }
            Statement::AssignTuple { targets, value, .. } => {
                collect_comprehension_locals(emitter, value);
                // **只有名字目标**声明局部（`a, b = x` ✓；`a[0], b = x` 只声明 `b` ✓）
                // 第 108 轮实测：漏了这一条 ⇒ 函数里 `a, b = x` 的 `nlocals` 少 2 ✗（夹具当场抓到 ✓）
                for (target, _) in targets {
                    declare_target(emitter, target);
                }
            }
            // **属性／下标目标的赋值** ✓（第 277 轮真 bug ✗）：`sys.path = [p for p in …]` 走的是
            // `AssignAttr` ✓，而 `collect_locals` 先前**没有**这两条臂 ✗（落进 `_ => {}` ✓）⇒ 右值里的
            // 推导式目标**漏收** ✗ ⇒ 它晚到发射期才被追加 ✗ ⇒ **序言** `MAKE_CELL` 的槽号错位 ✗
            //（实测 `Lib/site.py` 的 `register_readline`：`MAKE_CELL` 要槽 10、最终布局是 11 ✓）。
            Statement::AssignAttr { value, .. } | Statement::AssignSubscript { value, .. } => {
                collect_comprehension_locals(emitter, value);
            }
            // 返回值／表达式语句里的推导式目标 ✓（第 277 轮）
            Statement::Return(value, _) | Statement::Expression(value, _) => {
                collect_comprehension_locals(emitter, value);
            }
            Statement::Assign { target, value, .. } => {
                // **右值里的推导式目标** ✓（第 264 轮；内联 ⇒ 目标是外层局部 ✓）
                collect_comprehension_locals(emitter, value);
                declare_local(emitter, target);
            }
            // **函数里嵌套的 `def`**：名字是局部（实测 `def outer(): def inner(): …` ⇒
            // `co_varnames = ('inner',)`）——少了这条，收尾重算 `varnames` 时会把名字丢掉 ✗
            // **类的名字也是本作用域的局部** ✓（第 202 轮真 bug ✗：先前漏了这一支 ⇒
            // "函数里定义类"会把类名按模块级 `STORE_NAME` 发 ✗ ⇒ 撞"需要命名空间帧" ✓）。
            // **不递归进类体** ✗：那是**另一个作用域** ✓（它的 `STORE_NAME` 落在类字典上 ✓）。
            Statement::Class { name, .. } => {
                declare_local(emitter, name);
            }
            Statement::Def { name, .. } => {
                declare_local(emitter, name);
            }
            // `import a` 在函数里存的是**局部**（实测 `def f(): import a` ⇒ `STORE_FAST a`）
            Statement::Import { items, .. } => {
                for (module, alias) in items {
                    let dest = alias.clone().unwrap_or_else(|| {
                        module.split('.').next().unwrap_or(module).to_owned()
                    });
                    emitter.slot_of(&dest);
                }
            }
            Statement::ImportFrom { names, .. } => {
                for (name, alias) in names {
                    emitter.slot_of(alias.as_deref().unwrap_or(name));
                }
            }
            Statement::AugAssign {
                target: AugTarget::Name(name, _),
                value,
                ..
            } => {
                collect_comprehension_locals(emitter, value);
                declare_local(emitter, name);
            }
            // 属性／下标目标的增强赋值：目标不是新局部 ✓，但右值里的推导式目标要收 ✓
            Statement::AugAssign { value, .. } => collect_comprehension_locals(emitter, value),
            Statement::For {
                target,
                tuple_targets,
                iterable,
                body,
                else_body,
                ..
            } => {
                // 可迭代对象**先求值** ✓ ⇒ 其中的推导式目标先成为局部 ✓
                collect_comprehension_locals(emitter, iterable);
                emitter.slot_of(target);
                // **元组目标** ✓（第 262 轮真 bug ✗）：`for key, value in …` 的**每个**名字都是本作用域的局部 ✓
                // ⇒ 先前只声明了单个 `target` ✗ ⇒ 其余名字要**等到发射期**才被追加 ✗ ⇒ 而**序言**里的
                // `MAKE_CELL` 槽号是按**当时**的 `varnames` 算的 ✗ ⇒ cell 槽**整体错位一格** ✗
                //（实测 `Lib/os.py` 的 `_create_environ_mapping`：序言时 varnames 只有 4 个、
                //  少了 `value` ✗ ⇒ `MAKE_CELL encode` 发成 **4**、而最终布局是 **5** ✗）。
                for (name, _) in tuple_targets {
                    emitter.slot_of(name);
                }
                collect_locals(emitter, body);
                collect_locals(emitter, else_body);
            }
            Statement::While {
                condition,
                body,
                else_body,
                ..
            } => {
                collect_comprehension_locals(emitter, condition);
                collect_locals(emitter, body);
                collect_locals(emitter, else_body);
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_comprehension_locals(emitter, condition);
                collect_locals(emitter, then_body);
                collect_locals(emitter, else_body);
            }
            Statement::With { items, body, .. } => {
                // 上下文表达式**先求值** ✓ ⇒ 其中的推导式目标先成为局部 ✓
                for (context, _) in items {
                    collect_comprehension_locals(emitter, context);
                }
                for (_, target) in items {
                    if let Some((target, _)) = target {
                        emitter.slot_of(target);
                    }
                }
                collect_locals(emitter, body);
            }
            // 同样按**发射顺序**（局部槽位的次序要跟 `STORE_FAST` 的出现次序一致）
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_locals(emitter, body);
                collect_locals(emitter, else_body);
                for handler in handlers {
                    if let Some(name) = &handler.name {
                        declare_local(emitter, name);
                    }
                    collect_locals(emitter, &handler.body);
                }
                collect_locals(emitter, finally_body);
            }
            _ => {}
        }
    }
}

/// 赋值类目标的预登记：**函数作用域里是局部**（进 `varnames`），其余作用域进 `names`。
pub(super) fn pre_intern_target(emitter: &mut Emitter, name: &str) {
    match emitter.kind {
        ScopeKind::Function => {
            declare_local(emitter, name);
        }
        _ => {
            emitter.intern_name(name);
        }
    }
}

/// 表达式的预登记（**只登记名字**；常量不预登记，理由见 `pre_intern`）。
pub(super) fn pre_intern_expression(emitter: &mut Emitter, expression: &Expression) {
    match expression {
        // **`yield` 当表达式** ✓（第 219 轮）：值那一半照样要**预登记**名字 ✓。
        Expression::Yield(value, _) => {
            if let Some(value) = value {
                pre_intern_expression(emitter, value);
            }
        }
        Expression::Int(_, _)
        | Expression::BigInt(_, _)
        | Expression::Float(_, _)
        | Expression::Str(_, _)
        | Expression::Bytes(_, _)
        | Expression::Constant(_, _) => {}
        Expression::Starred(value, _) => pre_intern_expression(emitter, value),
        Expression::Walrus { target, value, .. } => {
            // 值先、目标后（实测 `if (o := f()) is None:` 的 `co_names` 是 `('f','o','x')` ✓）
            pre_intern_expression(emitter, value);
            if emitter.comprehension_locals.iter().any(|item| item == target) {
                return;
            }
            // **函数里要声明为局部**（第 119 轮）：此前这里显式跳过函数作用域 ✗ ⇒ 海象目标
            // 既不在 `varnames` 也不是 cell／free ⇒ 发射期只能报未接线 ✗（`_bootstrap.py:206` 就是它 ✓）。
            // `pre_intern_target` 正好两件事都做：函数里 `declare_local` ✓、其余作用域登记名字 ✓。
            pre_intern_target(emitter, target);
        }
        Expression::Name(name, _) => {
            // **正在发射的推导式目标**当局部（不进 `co_names`）；函数作用域里被赋名的局部同样跳过
            if emitter.comprehension_locals.iter().any(|item| item == name) {
                return;
            }
            if emitter.kind == ScopeKind::Function
                && emitter.unit.varnames.iter().any(|item| item == name)
            {
                return;
            }
            // **cell／自由变量同样不进 `co_names`**（实测 `nonlocal x` 的内层 `co_names=()` ✓
            // —— `x` 只该在 `co_freevars` 里；这条以前只挡了 `varnames`，自由变量会漏过去 ✗）
            if emitter.deref_slot(name).is_some() {
                return;
            }
            emitter.intern_name(name);
        }
        // f-string：各插值里的表达式在本作用域求值（按源序登记名字）；字面段是常量，无需登记
        Expression::FString { parts, .. } => pre_intern_fstring(emitter, parts),
        // 集合字面量：逐元素在本作用域求值
        Expression::SetLiteral(items, _) => {
            for item in items {
                pre_intern_expression(emitter, item);
            }
        }
        // 推导式：元素表达式与各生成器的可迭代表达式在本作用域求值；**目标名进局部槽**
        // （实测模块级 `[x for x in s]` 的 `co_varnames` 就是 `('x',)`）——但**只在推导式内部**
        // 把目标名当局部（模块级同名变量的其它用处仍进 `co_names`）
        Expression::Comprehension {
            element,
            generators,
            kind,
            ..
        } => {
            // **生成器表达式**（第 125 轮）：它的目标是**内层 code object** 的局部 ✓，
            // 绝不进外层 `varnames` / `co_names` ✗（实测外层 `def f(g)` 的 `nlocals` 仍是 1 ✓）；
            // 只有**内联**的列表／集合／字典推导式才把目标当外层局部（PEP 709 ✓）。
            let inline = !matches!(kind, ComprehensionKind::Generator);
            let saved = emitter.comprehension_locals.len();
            for generator in generators {
                pre_intern_expression(emitter, &generator.iterable);
                if inline {
                    for name in generator.target.names() {
                        declare_local(emitter, name);
                        emitter.comprehension_locals.push(name.to_owned());
                    }
                    // **条件也在本作用域求值**这句同样只对内联成立 ✓；生成器表达式的条件属于
                    // **内层** code object ✗（外层登记它会把条件里的名字塞进外层 `co_names` ✗，
                    // 夹具当场抓到：`sum(i for i in g if i > 0)` 的外层多出一个 `i` ✗）。
                    for condition in &generator.conditions {
                        pre_intern_expression(emitter, condition);
                    }
                }
            }
            // **元素在本作用域求值**这句只对**内联**推导式成立 ✓；生成器表达式的元素属于
            // **内层** code object ✗（外层登记它会把 `i` 塞进外层 `co_names` ✗，夹具当场抓到 ✓）。
            if inline {
                pre_intern_expression(emitter, element);
            }
            // 字典的值那一半也按同样规则预登记（同样只对**内联**推导式 ✓）
            if inline {
            if let Expression::Comprehension { value, .. } = expression {
                if let Some(value) = value {
                    pre_intern_expression(emitter, value);
                }
            }
            }
            emitter.comprehension_locals.truncate(saved);
        }
        // `lambda`：**默认值**在本作用域求值；参数与体属嵌套作用域（各自登记）
        Expression::Lambda { parameters, kwonly, .. } => {
            for parameter in parameters.iter().chain(kwonly.iter()) {
                if let Some(default) = &parameter.default {
                    pre_intern_expression(emitter, default);
                }
            }
        }
        Expression::Attribute(target, name, _) => {
            pre_intern_expression(emitter, target);
            emitter.intern_name(name);
        }
        Expression::Binary(_, left, right, _) => {
            pre_intern_expression(emitter, left);
            pre_intern_expression(emitter, right);
        }
        Expression::Compare(left, _, right, _) => {
            pre_intern_expression(emitter, left);
            pre_intern_expression(emitter, right);
        }
        Expression::ChainedCompare { operands, .. } => {
            for operand in operands {
                pre_intern_expression(emitter, operand);
            }
        }
        Expression::Conditional {
            condition,
            then_value,
            else_value,
            ..
        } => {
            pre_intern_expression(emitter, condition);
            pre_intern_expression(emitter, then_value);
            pre_intern_expression(emitter, else_value);
        }
        Expression::Unary(_, operand, _) | Expression::Not(operand, _) => {
            pre_intern_expression(emitter, operand);
        }
        Expression::BoolOp { values, .. } | Expression::TupleLiteral(values, _) => {
            for value in values {
                pre_intern_expression(emitter, value);
            }
        }
        Expression::Subscript(container, key, _) => {
            pre_intern_expression(emitter, container);
            pre_intern_expression(emitter, key);
        }
        Expression::SliceLiteral {
            lower, upper, step, ..
        } => {
            for part in [lower, upper, step].into_iter().flatten() {
                pre_intern_expression(emitter, part);
            }
        }
        Expression::List(items, _) => {
            for item in items {
                pre_intern_expression(emitter, item);
            }
        }
        Expression::Map(pairs, _) => {
            for (key, value) in pairs {
                pre_intern_expression(emitter, key);
                pre_intern_expression(emitter, value);
            }
        }
        Expression::Call {
            function,
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            ..
        } => {
            pre_intern_expression(emitter, function);
            for argument in arguments {
                pre_intern_expression(emitter, argument);
            }
            for argument in star_arguments {
                pre_intern_expression(emitter, argument);
            }
            for (_, value) in keywords {
                pre_intern_expression(emitter, value);
            }
            for argument in dict_arguments {
                pre_intern_expression(emitter, argument);
            }
        }
    }
}

/// 一个语句块是否**必然终止**（`break`／`continue`／`return`／`raise`，或 `if/else` 两边都终止）。
///
/// 参照据此**丢掉不可达的循环回跳**（实测：`for i in s:\n    continue\n` 只有 `continue` 那条
/// `JUMP_BACKWARD`，循环尾那条不发）。
/// 体是否以**函数级终止**收尾（`return`／`raise`）。
///
/// 与 [`block_terminates`] 的区别很要紧：`break`／`continue` 只终止**本轮迭代**，作用域**仍可能**
/// 落到底（收尾还得发）。实测 `def f(x):\n    while x:\n        try:\n            continue\n        finally:\n            y = 1\n`
/// 的参照 `co_consts` 是 `["int:1", "none"]` —— 那个 `none` 就是收尾；按 `block_terminates` 一刀切会把它砍掉。
pub(super) fn block_returns_or_raises(statements: &[Statement]) -> bool {
    match statements.last() {
        Some(Statement::Return(_, _) | Statement::Raise { .. }) => true,
        Some(Statement::If {
            then_body,
            else_body,
            ..
        }) => {
            !else_body.is_empty()
                && block_returns_or_raises(then_body)
                && block_returns_or_raises(else_body)
        }
        _ => false,
    }
}

/// 循环体是否**必然落不到回边**（⇒ 回跳指令不可达、参照不发）。
///
/// 除 [`block_terminates`] 认的那几种终止语句，还认"体末是**纯 `try/finally`** 且其体终止"——
/// 实测 `def f(x):\n    while x:\n        try:\n            continue\n        finally:\n            y = 1\n`
/// 的参照产物里**没有**循环尾那条 `JUMP_BACKWARD`。
pub(super) fn loop_body_terminates(statements: &[Statement]) -> bool {
    if block_terminates(statements) {
        return true;
    }
    match statements.last() {
        Some(Statement::Try {
            body,
            handlers,
            finally_body,
            ..
        }) => handlers.is_empty() && block_terminates(body) && !block_terminates(finally_body),
        _ => false,
    }
}

pub(super) fn block_terminates(statements: &[Statement]) -> bool {
    match statements.last() {
        Some(
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Return(_, _)
            | Statement::Raise { .. },
        ) => true,
        Some(Statement::If {
            then_body,
            else_body,
            ..
        }) => !else_body.is_empty() && block_terminates(then_body) && block_terminates(else_body),
        // **`while True:`（体内没有 `break`）也落不下去**（第 121 轮）：实测 `while True:\n    pass\n`
        // 的参照产物**不发收尾两条、也不登记 `None`** ✓ ⇒ 这样模块尾／`if` 体的隐式 return 都能正确省掉 ✓。
        // 判定**保守**：体内任何地方出现 `break` 就不算终止 ✓（漏判只多发一条尾，不会错 ✓）。
        Some(Statement::While { condition, body, .. }) => {
            matches!(condition, Expression::Constant(Constant::Bool(true), _)) && !contains_break(body)
        }
        _ => false,
    }
}

/// **这个作用域里有没有 `yield`**（第 124 轮）：有就是**生成器** ✓（`flags |= 0x20` ✓、
/// 前言要补 `RETURN_GENERATOR; POP_TOP` ✓、收尾要补 `CALL_INTRINSIC_1 3; RERAISE 1` ✓）。
///
/// **不下探**内层 `def`／`class` 与 `lambda` ✓（它们的 `yield` 属于它们自己 ✓）。
pub(super) fn statements_have_yield(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::Yield(_, _) => true,
        Statement::If {
            then_body,
            else_body,
            ..
        } => statements_have_yield(then_body) || statements_have_yield(else_body),
        Statement::While { body, else_body, .. }
        | Statement::For { body, else_body, .. } => {
            statements_have_yield(body) || statements_have_yield(else_body)
        }
        Statement::Try {
            body,
            handlers,
            else_body,
            finally_body,
            ..
        } => {
            statements_have_yield(body)
                || handlers.iter().any(|handler| statements_have_yield(&handler.body))
                || statements_have_yield(else_body)
                || statements_have_yield(finally_body)
        }
        Statement::With { body, .. } => statements_have_yield(body),
        // **语句里的表达式** ✓（第 219 轮）：lambda 的体会被合成成 `return <体>` ✓ ⇒ 必须看到这一层 ✓
        //（否则含 `yield` 的 lambda **不会**被标成生成器 ✗ ⇒ `type((lambda: (yield))())` 就不是 `generator` ✗）。
        // **如实说** ✗：目前只覆盖 `return` 与裸表达式两块 ✓；赋值那一族的表达式随后补 ✓。
        Statement::Return(value, _) | Statement::Expression(value, _) => {
            crate::compile::expressions_have_yield(value)
        }
        _ => false,
    })
}

/// 递归找 `break`（保守：只要出现就当"可能跳出" ✓）。
fn contains_break(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::Break(_) => true,
        Statement::If {
            then_body,
            else_body,
            ..
        } => contains_break(then_body) || contains_break(else_body),
        Statement::While { body, else_body, .. }
        | Statement::For { body, else_body, .. } => {
            contains_break(body) || contains_break(else_body)
        }
        Statement::Try {
            body,
            handlers,
            else_body,
            finally_body,
            ..
        } => {
            contains_break(body)
                || handlers.iter().any(|handler| contains_break(&handler.body))
                || contains_break(else_body)
                || contains_break(finally_body)
        }
        Statement::With { body, .. } => contains_break(body),
        _ => false,
    })
}

/// 预登记 f-string 各插值表达式里的名字（字面段不登记）。
pub(super) fn pre_intern_fstring(emitter: &mut Emitter, parts: &[FStringPart]) {
    for part in parts {
        if let FStringPart::Formatted {
            expression, spec, ..
        } = part
        {
            pre_intern_expression(emitter, expression);
            if let Some(spec) = spec {
                pre_intern_fstring(emitter, spec);
            }
        }
    }
}
