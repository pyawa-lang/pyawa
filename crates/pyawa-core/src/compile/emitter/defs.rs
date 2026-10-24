//! `emitter` 的 `defs` 域：`def`／`class`／`type` 别名与注解单元的发射。

use super::*;

/// 注解常量里引用了哪些名字（递归；`out` 去重、保持首次出现序 ✓）。
pub(crate) fn collect_constant_names(annotation: &Constant, out: &mut Vec<String>) {
    let push = |name: &String, out: &mut Vec<String>| {
        if !out.iter().any(|item| item == name) {
            out.push(name.clone());
        }
    };
    match annotation {
        Constant::Type(name) | Constant::Str(name) => push(name, out),
        Constant::AnnSubscript { base, arguments } => {
            collect_constant_names(base, out);
            for argument in arguments {
                collect_constant_names(argument, out);
            }
        }
        Constant::AnnUnion { left, right } => {
            collect_constant_names(left, out);
            collect_constant_names(right, out);
        }
        Constant::AnnList(items) => {
            for item in items {
                collect_constant_names(item, out);
            }
        }
        Constant::AnnAttribute { base, .. } => collect_constant_names(base, out),
        Constant::Tuple(items) => {
            for item in items {
                collect_constant_names(item, out);
            }
        }
        _ => {}
    }
}

/// 类体里的**注解声明**引用了哪些名字（递归进块体 ✓，**不进**嵌套 `class`／`def` ✓ ——
/// 那两层各自有自己的注解单元 ✓）。
pub(crate) fn collect_class_annotation_names(body: &[Statement], out: &mut Vec<String>) {
    for statement in body {
        match statement {
            Statement::Annotation { annotation, .. } => match annotation {
                AnnotationSource::Expression(expression) => {
                    super::symbols::collect_names_in_expression(expression, out)
                }
                // `TS-47`：`Type` 源是编译期标签 —— 名字在 `Constant` 里（没有表达式可走）。
                AnnotationSource::Type(label) => collect_constant_names(label, out),
            },
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_class_annotation_names(then_body, out);
                collect_class_annotation_names(else_body, out);
            }
            Statement::While { body, else_body, .. } => {
                collect_class_annotation_names(body, out);
                collect_class_annotation_names(else_body, out);
            }
            Statement::For { body, else_body, .. } => {
                collect_class_annotation_names(body, out);
                collect_class_annotation_names(else_body, out);
            }
            Statement::With { body, .. } => collect_class_annotation_names(body, out),
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_class_annotation_names(body, out);
                for handler in handlers {
                    collect_class_annotation_names(&handler.body, out);
                }
                collect_class_annotation_names(else_body, out);
                collect_class_annotation_names(finally_body, out);
            }
            Statement::Match { cases, .. } => {
                for case in cases {
                    collect_class_annotation_names(&case.body, out);
                }
            }
            _ => {}
        }
    }
}

/// **一个单元里"按全局／命名空间读"的名字** ✓（`LOAD_GLOBAL`／`LOAD_NAME` 的 oparg ✓）——
/// 闭包分析只能用它，**不能**用 `unit.names` ✗：`co_names` 把**属性名**（`LOAD_ATTR`）也收在同表 ✓
/// ⇒ `f.name` 里的 `name` 会被误当"自由变量" ✓（task-25 ④ 实测：`Lib/dataclasses.py` 的
/// `_process_class` 有局部 `name` ＋ 生成器表达式里的 `field.name` ⇒ 误判 ⇒ panic ✓）。
pub(crate) fn global_load_names(unit: &CompiledUnit) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut offset = 0usize;
    let mut extended: u32 = 0;
    let push = |index: usize, out: &mut Vec<String>| {
        if let Some(text) = unit.names.get(index) {
            if !out.iter().any(|item| item == text) {
                out.push(text.clone());
            }
        }
    };
    while offset + 1 < unit.code.len() {
        let opcode = u16::from(unit.code[offset]);
        let arg = u32::from(unit.code[offset + 1]);
        let merged = arg | (extended << 8);
        match opcode::opname(opcode) {
            Some("LOAD_GLOBAL") => push((merged >> 1) as usize, &mut out),
            Some("LOAD_NAME") => push(merged as usize, &mut out),
            _ => {}
        }
        extended = if opcode::opname(opcode) == Some("EXTENDED_ARG") {
            merged
        } else {
            0
        };
        offset += 2 + 2 * opcode::inline_cache_entries(opcode) as usize;
    }
    out
}

/// 类体里嵌套 `def` 的**注解**引用了这些名字（PEP 695 的两层闭包 ⇒ 这些类型参数要成为**两层**
/// 自由变量 ✓：类体捕获它们、方法注解单元再从类体捕获 ✓）。
///
/// 只扫 `def` 的**注解** ✓ —— 方法**体**里的引用由 `nested_unit_names` 那一遍收 ✓
/// （方法体的 `LOAD_GLOBAL` 落在方法单元自己的 `co_names` 里 ✗，类体的 `co_names` 看不到 ✗）。
pub(crate) fn class_body_def_references_params(body: &[Statement], params: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    collect_class_body_def_param_refs(body, params, &mut out);
    out
}

pub(crate) fn collect_class_body_def_param_refs(body: &[Statement], params: &[String], out: &mut Vec<String>) {
    let push = |name: &String, out: &mut Vec<String>| {
        if params.iter().any(|param| param == name) && !out.iter().any(|item| item == name) {
            out.push(name.clone());
        }
    };
    for statement in body {
        match statement {
            Statement::Def {
                parameters,
                kwonly,
                returns,
                body,
                ..
            } => {
                let mut names: Vec<String> = Vec::new();
                for parameter in parameters.iter().chain(kwonly.iter()) {
                    if let Some(annotation) = parameter.annotation.as_ref() {
                        collect_constant_names(annotation, &mut names);
                    }
                }
                if let Some(annotation) = returns.as_ref() {
                    collect_constant_names(annotation, &mut names);
                }
                for name in &names {
                    push(name, out);
                }
                collect_class_body_def_param_refs(body, params, out);
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_class_body_def_param_refs(then_body, params, out);
                collect_class_body_def_param_refs(else_body, params, out);
            }
            Statement::While { body, else_body, .. }
            | Statement::For { body, else_body, .. } => {
                collect_class_body_def_param_refs(body, params, out);
                collect_class_body_def_param_refs(else_body, params, out);
            }
            Statement::With { body, .. } => {
                collect_class_body_def_param_refs(body, params, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_class_body_def_param_refs(body, params, out);
                collect_class_body_def_param_refs(else_body, params, out);
                collect_class_body_def_param_refs(finally_body, params, out);
                for handler in handlers {
                    collect_class_body_def_param_refs(&handler.body, params, out);
                }
            }
            Statement::Match { cases, .. } => {
                for case in cases {
                    collect_class_body_def_param_refs(&case.body, params, out);
                }
            }
            _ => {}
        }
    }
}

/// 一个单元里**所有嵌套 code 常量**的 `co_names`（含自身 ✓）—— 用来发现"方法体里引用的类型参数"
/// （那句名字落在方法单元自己的 `co_names` 里 ✗，类体的 `co_names` 看不到 ✗）。
pub(crate) fn nested_unit_names(unit: &CompiledUnit) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    collect_nested_unit_names(unit, &mut out);
    out
}

pub(crate) fn collect_nested_unit_names(unit: &CompiledUnit, out: &mut Vec<String>) {
    for name in &unit.names {
        if !out.iter().any(|item| item == name) {
            out.push(name.clone());
        }
    }
    for constant in &unit.constants {
        if let Constant::Code(inner) = constant {
            collect_nested_unit_names(inner, out);
        }
    }
}

/// 外壳里某个类型参数的 **localsplus 槽**（cell ⇒ `nlocals ＋ cellvars 序号` ✓；否则 ＝ varnames 序号 ✓）。
pub(crate) fn shell_type_param_slot(unit: &CompiledUnit, param: &str, referenced: &[String]) -> usize {
    if referenced.iter().any(|item| item == param) {
        // 类外壳的 `cellvars[0]` 是 `.type_params` ⇒ 序号要按 **cellvars 里的真实位置**算 ✓
        //（先前按 `referenced` 序号算 ⇒ 泛型类的 cell 类型参数把值存进 `.type_params` 那一格 ✗）。
        let index = unit
            .cellvars
            .iter()
            .position(|item| item == param)
            .expect("cell 类型参数必在 cellvars 里");
        unit.nlocals + index
    } else {
        unit.varnames
            .iter()
            .position(|item| item == param)
            .expect("类型参数必在 varnames 或 cellvars 里")
    }
}

/// 外壳里某个 cell 名的槽（`.type_params` 或类型参数 ⇒ `nlocals ＋ cellvars 序号` ✓）。
pub(crate) fn shell_cell_slot(unit: &CompiledUnit, name: &str) -> usize {
    let index = unit
        .cellvars
        .iter()
        .position(|item| item == name)
        .expect("闭包名必在 cellvars 里");
    unit.nlocals + index
}

/// 把类体单元里那个 `__annotate__` 嵌套单元标成 **`CO_NESTED`**（泛型类的外壳是函数 ✓；
/// 参照实测 `class C[T]: x: T` 的注解单元 flags ＝ 0x13 ✓，非泛型类仍是 0x3 ✓）。
pub(crate) fn force_nested_annotate(unit: &mut CompiledUnit) {
    for constant in unit.constants.iter_mut() {
        if let Constant::Code(inner) = constant {
            if inner.name == "__annotate__" {
                inner.flags |= 0x10;
            }
        }
    }
}


/// 发一个**类型参数的构造** ✓（task-24 收口 ①，`dis` 逐条实测）：
/// 无 bound/default ⇒ `LOAD_CONST name; CALL_INTRINSIC_1 7` ✓；
/// 有 `bound` ⇒ `<惰性 thunk>; CALL_INTRINSIC_2 2 (TYPEVAR_WITH_BOUND)` ✓；
/// 有 `default` ⇒ `<惰性 thunk>; CALL_INTRINSIC_2 5 (SET_TYPEPARAM_DEFAULT)` ✓；
/// 最后统一 `COPY 1; STORE_DEREF/STORE_FAST` ✓（位点取**整条参数**的跨度 ✓）。
pub(crate) fn emit_type_param(
    shell: &mut Emitter,
    param: &TypeParam,
    referenced: &[String],
) -> Result<(), CompileError> {
    let whole = param.span;
    let slot = shell_type_param_slot(&shell.unit, &param.name, referenced);
    let constant = shell.intern_constant(Constant::Str(param.name.clone()));
    shell.emit_indexed(whole, "LOAD_CONST", constant);
    if let Some(bound) = param.bound.as_ref() {
        emit_param_thunk(shell, &param.name, bound.span(), bound)?;
        shell.emit_named(whole, "CALL_INTRINSIC_2", 2);
    } else {
        shell.emit_named(whole, "CALL_INTRINSIC_1", 7);
    }
    if let Some(default) = param.default.as_ref() {
        emit_param_thunk(shell, &param.name, default.span(), default)?;
        shell.emit_named(whole, "CALL_INTRINSIC_2", 5);
    }
    shell.emit_named(whole, "COPY", 1);
    if referenced.iter().any(|item| item == &param.name) {
        shell.emit_indexed(whole, "STORE_DEREF", slot);
    } else {
        shell.emit_indexed(whole, "STORE_FAST", slot);
    }
    Ok(())
}

/// 发一个 `bound`／`default` 的**惰性 thunk** ✓（`.format` 形参 ＋ 守卫 ＋ 值 ＋ `RETURN_VALUE` ✓，
/// 与别名 thunk 同形 ✓）＋ 它前面的 `LOAD_CONST (1,)`／`MAKE_FUNCTION`／`SET_FUNCTION_ATTRIBUTE 1` ✓。
/// 位点：`RESUME` 取合成 ✓，守卫／收尾取 **bound／default 表达式**的跨度 ✓（实测 `[T: int = str]`）。
pub(crate) fn emit_param_thunk(
    shell: &mut Emitter,
    name: &str,
    guard: Span,
    expression: &Expression,
) -> Result<(), CompileError> {
    let unit = CompiledUnit {
        name: name.to_owned(),
        qualname: name.to_owned(),
        argcount: 1,
        posonlyargcount: 1,
        kwonlyargcount: 0,
        nlocals: 1,
        // thunk 嵌在外壳函数里 ⇒ `CO_NESTED`（参照实测 0x13 ✓）。
        flags: 0x13,
        names: Vec::new(),
        varnames: vec![".format".to_owned()],
        cellvars: Vec::new(),
        freevars: Vec::new(),
        demanded: Vec::new(),
        constants: Vec::new(),
        code: Vec::new(),
        positions: Vec::new(),
        exceptiontable: Vec::new(),
    };
    let mut thunk = Emitter::blank(shell.mode, shell.tier, name, ScopeKind::Function, unit, guard);
    thunk.emit_named(
        Span::new(guard.line_start, guard.line_start, 0, 0),
        "RESUME",
        0,
    );
    thunk.emit_named(guard, "LOAD_FAST_BORROW", 0);
    thunk.intern_constant(Constant::Int(2));
    thunk.emit_named(guard, "LOAD_SMALL_INT", 2);
    thunk.emit_named(guard, "COMPARE_OP", 132);
    let end = thunk.new_label();
    thunk.emit_jump(
        guard,
        opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
        end,
    );
    thunk.emit_named(guard, "NOT_TAKEN", 0);
    thunk.emit_named(guard, "LOAD_COMMON_CONSTANT", 1);
    thunk.emit_named(guard, "RAISE_VARARGS", 1);
    thunk.mark_label(end);
    thunk.emit_expression(expression)?;
    thunk.emit_named(guard, "RETURN_VALUE", 0);
    let thunk = thunk.finish_shell()?;
    let defaults = shell.intern_constant(Constant::Tuple(vec![Constant::Int(1)]));
    shell.emit_indexed(guard, "LOAD_CONST", defaults);
    let index = shell.intern_constant(Constant::Code(Box::new(thunk)));
    shell.emit_indexed(guard, "LOAD_CONST", index);
    shell.emit_named(guard, "MAKE_FUNCTION", 0);
    shell.emit_named(guard, "SET_FUNCTION_ATTRIBUTE", 1);
    Ok(())
}

impl Emitter {
    /// **造一个函数对象**（`def` 与 `lambda` 共用）：默认值元组／仅关键字默认值映射 →
    /// （有注解时先造 `__annotate__` 单元）→ `LOAD_CONST <code>` → `MAKE_FUNCTION` →
    /// `SET_FUNCTION_ATTRIBUTE`（实测挂载次序 **16 → 2 → 1**）。
    /// **不做 `STORE_*`**：`def` 之后自己存；`lambda` 把它当作表达式的值留在栈上。
    pub(crate) fn emit_function_object(
        &mut self,
        nested: CompiledUnit,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        returns: Option<&Constant>,
        returns_span: Option<Span>,
        // **`__annotate__` 单元的自由变量**（task-24 泛型函数外壳 ✓）：类型参数名 ✓（普通函数空表 ✓）。
        annotate_freevars: &[String],
        span: Span,
    ) -> Result<(), CompileError> {
                // **默认值**（实测）：`def f(a, b=x)` ⇒ 逐个求值默认值再 `BUILD_TUPLE n`，
        // 排在 `LOAD_CONST <code>` **之前**；挂载在 `MAKE_FUNCTION` 之后
        // （有注解时次序是 `SET_FUNCTION_ATTRIBUTE 16` 再 `1`）。
        let defaults: Vec<&Expression> = parameters
            .iter()
            .filter_map(|parameter| parameter.default.as_ref())
            .collect();
        if !defaults.is_empty() {
            // **字面量默认值折叠**（实测：`def f(a, b=2)` ⇒ 一条 `LOAD_CONST (2,)`，
            // 常量表里那个元组排在**最后**，位点取**体末句**）
            let literals: Option<Vec<Constant>> =
                defaults.iter().map(|expression| constant_expression(expression)).collect();
            match literals {
                Some(constants) => {
                    // 实测：参照折叠时**先把字面量本身入池**（这些槽没人引用，是折叠的
                    // 痕渍）⇒ 要照做，否则常量表对不上
                    for constant in &constants {
                        self.intern_constant(constant.clone());
                    }
                    let offset = self.unit.code.len() + 1;
                    self.emit_named(span, "LOAD_CONST", 0);
                    self.deferred.push((offset, Constant::Tuple(constants)));
                }
                None => {
                    for expression in &defaults {
                        self.emit_expression(expression)?;
                    }
                    self.emit_indexed(span, "BUILD_TUPLE", defaults.len());
                }
            }
        }
        // **仅关键字默认值**（实测）：`LOAD_CONST 'c'; <值>; …; BUILD_MAP n`，
        // 排在位置默认值元组之后、code 之前；**不折叠**（字面量也走 `LOAD_SMALL_INT`）
        let kwdefaults: Vec<&Parameter> = kwonly
            .iter()
            .filter(|parameter| parameter.default.is_some())
            .collect();
        if !kwdefaults.is_empty() {
            for parameter in &kwdefaults {
                let key =
                    self.intern_constant(Constant::Str(parameter.name.clone()));
                self.emit_indexed(span, "LOAD_CONST", key);
                if let Some(default) = parameter.default.as_ref() {
                    self.emit_expression(default)?;
                }
            }
            self.emit_indexed(span, "BUILD_MAP", kwdefaults.len());
        }
        // **PEP 649**：带注解的 `def` 先造 `__annotate__` 单元（实测：它在常量表里
        // 排在函数 code **之前**，随即 `MAKE_FUNCTION` ＋ `SET_FUNCTION_ATTRIBUTE 16`）
        let annotated = parameters
            .iter()
            .chain(kwonly.iter())
            .any(|parameter| parameter.annotation.is_some())
            || returns.is_some();
        if annotated {
            // **`__annotate__` 单元的 qualname** ✓（参照实测）：模块级 ⇒ `__annotate__` ✓；
            // **类体里定义**的方法 ⇒ `<类 qualname>.__annotate__` ✓（**不是** `.<locals>.` 那一段 ✗
            // —— task-24 收口 ② 实测 `class C[T]: def m(self) -> T` 的注解单元 qualname ＝ `C.__annotate__` ✓）；
            // 函数里 ⇒ `<函数 qualname>.<locals>.__annotate__` ✓。
            let annotate_qualname = match self.kind {
                ScopeKind::Module => "__annotate__".to_owned(),
                ScopeKind::Class => format!("{}.__annotate__", self.qualname),
                ScopeKind::Function => format!("{}.<locals>.__annotate__", self.qualname),
            };
            // **方法注解单元也是闭包** ✓（task-24 收口 ②，两层闭包的第二层 ✓）：类体先把自由变量
            // 压成闭包元组 ✓、`MAKE_FUNCTION` 之后再挂 `SET_FUNCTION_ATTRIBUTE 8` ✓
            // （与泛型函数外壳里那条**同形** ✓ —— 参照实测 `class C[T]: def m(self) -> T` ✓）。
            if !annotate_freevars.is_empty() {
                for free in annotate_freevars {
                    let slot = self
                        .deref_slot(free)
                        .expect("注解自由变量必在本层 cell／free 表里");
                    self.emit_indexed(span, "LOAD_FAST_BORROW", slot);
                }
                self.emit_indexed(span, "BUILD_TUPLE", annotate_freevars.len());
            }
            let mut unit = self.annotate_unit(
                &annotate_qualname,
                parameters,
                kwonly,
                returns,
                returns_span,
                annotate_freevars,
                // 类体里的 `def` ⇒ 注解表达式按**类字典优先**求值 ✓（参照实测 ✓）；
                // 模块／函数里照旧 ✓。
                self.kind == ScopeKind::Class,
                span,
            );
            // **`CO_NESTED`（0x10）** ✓：注解单元定义在**嵌套作用域**里就要置 ✓
            //（参照实测 `def outer(): def g(x: int)` 的注解单元 flags ＝ 0x13 ✓；模块级仍是 0x3 ✓）。
            if annotate_qualname.contains(".<locals>.") {
                unit.flags |= 0x10;
            }
            let annotate_index = self.intern_constant(Constant::Code(Box::new(unit)));
            self.emit_indexed(span, "LOAD_CONST", annotate_index);
            self.emit_named(span, "MAKE_FUNCTION", 0);
            if !annotate_freevars.is_empty() {
                self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 8);
            }
        }
        let index = self.intern_constant(Constant::Code(Box::new(nested)));
        // 实测：`def` 的三条指令（＋收尾）位置都是**整个 `def` 语句**
        self.emit_indexed(span, "LOAD_CONST", index);
        // 3.14 的 `MAKE_FUNCTION` **没有 oparg**（`dis` 显示 `arg=None`）
        self.emit_named(span, "MAKE_FUNCTION", 0);
        if annotated {
            // bit4 `annotate`（`SPEC-bytecode.md` 的属性位表）
            self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 16);
        }
        if !kwdefaults.is_empty() {
            // bit1 `kwdefaults`；实测的挂载次序是 **16 → 2 → 1**
            self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 2);
        }
        if !defaults.is_empty() {
            // bit0 `defaults`（同一张位表）
            self.emit_named(span, "SET_FUNCTION_ATTRIBUTE", 1);
        }
        Ok(())
    }

    /// 造一个 **`__annotate__` 单元**（PEP 649 的 3.14 形态，逐条实测）。
    ///
    /// 形态：`format` 参数的守卫（`format > 2` ⇒ `NotImplementedError`）＋ 注解字典
    /// （键＝形参名，最后 `'return'`；值＝注解表达式）＋ `RETURN_VALUE`；
    /// `argcount = 1`、`varnames = ('format',)`、`flags = 0x3`。
    /// **`class` 语句的发射** ✓（task-24 轮 2 的**纯搬运** ✗→✓：从 `emit_statement` 那个巨型
    /// match 里整段搬来 ✓，**一行逻辑都没改** ✓ —— 逐字节夹具（`--test compile`）就是搬运没搬坏的证明 ✓。
    /// 抽出它是为了让 PEP 695 的**外壳**能"原样重放"这条构造 ✓（外壳只在外层加序言／收尾 ✓）。
    pub(crate) fn emit_class_statement(
        &mut self,
        name: &String,
        span: &Span,
        first_line: &u32,
        decorators: &[Expression],
        keywords: &[(String, Expression)],
        bases: &[Expression],
        body: &[Statement],
        type_params: &[TypeParam],
    ) -> Result<(), CompileError> {
                // **PEP 695**：带类型参数 ⇒ 整条类构造包进 `<generic parameters of C>` 外壳 ✓
                //（普通类照旧走下面这条"逐字节搬运"的老路 ✓）。
                if !type_params.is_empty() {
                    return self.emit_generic_class_statement(
                        name,
                        span,
                        first_line,
                        decorators,
                        keywords,
                        bases,
                        body,
                        type_params,
                    );
                }
                // **装饰器求值**（源码序 ✓，与 `Def` 同口径 ✓）：先算出来 ✓，类建好之后再逆序套 ✓。
                let decorator_spans: Vec<Span> =
                    decorators.iter().map(|decorator| decorator.span()).collect();
                for decorator in decorators {
                    self.emit_expression(decorator)?;
                }
                // 实测：基类是用 **`LOAD_NAME`** 压栈的（不是 `LOAD_CONST`）
                // **`BC-4` 的 qualname 规则** ✓（第 202 轮真 bug 修复 ✗：先前一律把**裸名字**当
                // `co_qualname` ⇒ "函数里定义类"与参照对不上 ✓）：模块级 ⇒ `C` ✓；
                // 类体里 ⇒ `外.类` ✓；函数里 ⇒ `外.<locals>.C` ✓（**与 `Def` 同一口径** ✓）。
                let class_qualname = match self.kind {
                    ScopeKind::Module => name.clone(),
                    ScopeKind::Class => format!("{}.{name}", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
                };
                let mut nested = compile_class_scope(
                    name,
                    &class_qualname,
                    self.mode,
                    self.tier,
                    body,
                    *first_line,
                    &[],
                    &[],
                )?;
                // **class 体的闭包分析** ✓（task-24 轮 2 步① ✓）：与 `Def` 那条**同款探针** ——
                // 探针编出的类体若把本作用域的局部／cell／自由名当"名字"引用过 ✓，那它就是
                // **本层的自由变量** ✓ ⇒ 带 `freevars` **重编一次** ✓（`COPY_FREE_VARS`／`LOAD_DEREF`
                // 才发得出 ✓），并在 `LOAD_CONST <code>` 之前压**闭包元组** ✓。
                let mut closure_freevars: Vec<String> = Vec::new();
                // **模块作用域没有闭包来源** ✓：模块的 `varnames` 只是**内联推导式目标**（3.12+），
                // 类体／方法看不到它们（参照里这类名字在类体里是全局读，不是自由变量）⇒ 不收
                // （收了也发不出 `LOAD_FAST_BORROW`：模块没有 cell／free 槽。`Lib/` 全量编译实测到
                // `varnames=["name"]` 的 panic ✓）。
                if self.kind != ScopeKind::Module {
                    for referenced in global_load_names(&nested)
                        .into_iter()
                        .chain(nested.demanded.clone())
                        // **方法单元**里的名字也要算候选 ✓（task-12 ✓）：类体**自己**不读、但方法体里
                        // 引用外层局部时（`def make(): base = 10; class K: def plain(self): return base`），
                        // 那句 `LOAD_GLOBAL base` 落在**方法的 code 常量**里 ✗ —— 只看类体的 `co_names`
                        // （`global_load_names`）看不到 ✗ ⇒ 类体不会被当闭包体重编 ⇒ 方法退化成全局读
                        // （运行期 `NameError`）。`nested_unit_names` 递归收嵌套 code 单元的 `co_names` ✓
                        // （真 `functools.cmp_to_key` 的 `K.__lt__` 引用闭包 `mycmp` 正是这一格 ✓）。
                        .chain(nested_unit_names(&nested))
                    {
                        // **只认本层真能 `LOAD_FAST_BORROW` 的槽**（cell／free）✓：类体自己的
                        // `varnames`（内联推导式目标）不是闭包来源（类作用域不是闭包边界）✗。
                        let from_here = self.unit.cellvars.iter().any(|cell| cell == &referenced)
                            || self.unit.freevars.iter().any(|free| free == &referenced);
                        if from_here && !closure_freevars.iter().any(|item| item == &referenced) {
                            closure_freevars.push(referenced);
                        }
                    }
                }
                if !closure_freevars.is_empty() {
                    nested = compile_class_scope(
                        name,
                        &class_qualname,
                        self.mode,
                        self.tier,
                        body,
                        *first_line,
                        &closure_freevars,
                        &[],
                    )?;
                }
                let index = self.intern_constant(Constant::Code(Box::new(nested)));
                self.emit_named(*span, "LOAD_BUILD_CLASS", 0);
                self.emit_named(*span, "PUSH_NULL", 0);
                if !closure_freevars.is_empty() {
                    for free in &closure_freevars {
                        // 元组元素是**本层 cell 槽的 `LOAD_FAST_BORROW`** ✓（与 `Def` 同口径 ✓）。
                        let slot = self
                            .deref_slot(free)
                            .expect("自由变量必在本层 cell／free 表里");
                        self.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
                    }
                    self.emit_indexed(*span, "BUILD_TUPLE", closure_freevars.len());
                }
                self.emit_indexed(*span, "LOAD_CONST", index);
                self.emit_named(*span, "MAKE_FUNCTION", 0);
                if !closure_freevars.is_empty() {
                    // bit3 `closure`（`SPEC-bytecode.md` 的属性位表 ✓；与 `Def` 同款 ✓）。
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
                }
                // **类名常量** ✓：参照**当场登记** ✓（`co_consts` 顺序实测 `[code C, 'C', <关键字值…>, names]` ✓）
                // —— 与有无类关键字无关 ✓。
                //
                // **task-24 尾件真 bug 修 ✗**：有关键字时原先走 `pending` **延迟回填** ✗（理由是"类名最后登记"，
                // 那其实是 `co_names` 的 `[B, M, C]` 顺序 ✓，与常量无关 ✗）⇒ `flush_pending` 只写 `index as u8` ✗
                // ⇒ **大模块下标 > 255 时高字节丢** ✓：实测 `Lib/typing.py`（模板块 335 个常量）的
                // `class _SpecialForm(…)` 类名常量真下标 315 ⇒ 低字节 59 ⇒ 运行时把 `'DefaultDict'` 当类名 ✗
                //（`class _SpecialForm(_Final, _NotIterable, _root=True)` 建成了别的名字的类 ⇒ `super()`
                // 找不到定义类 ⇒ `import typing` 卡死 ✓）。
                let name_const = self.intern_constant(Constant::Str(name.clone()));
                self.emit_indexed(*span, "LOAD_CONST", name_const);
                for base in bases {
                    self.emit_expression(base)?;
                }
                if keywords.is_empty() {
                    self.emit_indexed(*span, "CALL", 2 + bases.len());
                // **套上装饰器**（逆序 ✓，与 `Def` 同口径 ✓）：此时类对象在 TOS ✓。
                // **纠错** ✓（第 193 轮）：我一度把它改成 `CALL 1` ✗ —— 参照实测（`dis` ✓）就是
                // **`CALL arg=0`** ✓（`… CALL arg=2 ／ CALL arg=0 ／ STORE_NAME` ✓）⇒ 发射器**原本是对的** ✓，
                // 病根在**运行期**（原生没收到那个实参 ✗）⇒ 已改回 ✓。
                for decorator_span in decorator_spans.iter().rev() {
                    self.emit_named(*decorator_span, "CALL", 0);
                }
                } else {
                    // **类关键字**（第 157 轮，实测形状）：
                    //   `LOAD_CONST 'C'; <基类…>; <关键字值…>; LOAD_CONST <名字元组>; CALL_KW <位置＋关键字>`
                    //   ⇒ 名字元组常量**最后**登记 ✓（实测 consts ＝ `[code, 'C', names]` ✓）。
                    for (_, value) in keywords {
                        self.emit_expression(value)?;
                    }
                    let names: Vec<Constant> = keywords
                        .iter()
                        .map(|(key, _)| Constant::Str(key.clone()))
                        .collect();
                    let names_index = self.intern_constant(Constant::Tuple(names));
                    self.emit_indexed(*span, "LOAD_CONST", names_index);
                    self.emit_indexed(*span, "CALL_KW", 2 + bases.len() + keywords.len());
                }
                // **走统一入口** ✓（第 202 轮真 bug 修复 ✗：先前写死 `STORE_NAME` ⇒
                // "函数里定义类"当场撞"需要命名空间帧" ✓ —— 与处理器那两处同族 ✓）。
                self.emit_store_name(*span, name);
                // 收尾两条跟整段（与 `def` 同规则，实测）
                self.epilogue_span = *span;
                Ok(())
    }

    /// **`def` 语句的发射** ✓（task-24 轮 2 的**纯搬运** ✗→✓，同上）。
    pub(crate) fn emit_def_statement(
        &mut self,
        name: &String,
        decorators: &[Expression],
        span: &Span,
        first_line: &u32,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        varargs: &Option<String>,
        varkw: &Option<String>,
        returns: &Option<Constant>,
        returns_span: &Option<Span>,
        body: &[Statement],
        type_params: &[TypeParam],
    ) -> Result<(), CompileError> {
                // **PEP 695**：带类型参数 ⇒ 整条 `def` 包进 `<generic parameters of f>` 外壳 ✓
                //（普通函数照旧走下面那条老路 ✓）。
                if !type_params.is_empty() {
                    return self.emit_generic_def_statement(
                        name,
                        decorators,
                        span,
                        first_line,
                        parameters,
                        kwonly,
                        varargs,
                        varkw,
                        returns,
                        returns_span,
                        body,
                        type_params,
                    );
                }
                // **装饰器**：参照实测 ⇒ 先按**源码序**把各装饰器求值压栈 ✓
                // （`@dec` ⇒ `LOAD_NAME dec` 位点＝`dec` ✓；`@a.b` ⇒ `LOAD_NAME a; LOAD_ATTR b` ✓；
                //   `@dec(1)` ⇒ `LOAD_NAME dec; PUSH_NULL; …; CALL 1` 位点＝整个装饰器 ✓）
                let decorator_spans: Vec<Span> =
                    decorators.iter().map(|decorator| decorator.span()).collect();
                for decorator in decorators {
                    self.emit_expression(decorator)?;
                }
                // **函数里嵌套 `def`**（第 278 轮接线）：无闭包时与模块／类体同一套形态——
                // `LOAD_CONST <code>; MAKE_FUNCTION; STORE_FAST`（`co_flags` 的 `CO_NESTED` 由
                // 限定名里的 `.<locals>.` 自动置位，实测 `def outer(): def inner(): …` ⇒ flags 19）。
                // **闭包**（内层引用外层局部 ⇒ 要 `cellvars`／`freevars`／`MAKE_CELL`）**尚未接线**，
                // 而且**没有拦截**：那种名字会按全局发（运行期 `NameError`，不是静默改语义，
                // 但仍是错的）——要闭包得另做一层，见待接线清单。
                // `BC-4` 的 qualname 规则（实测）：模块级 `def f` ⇒ `f`；函数**里**的定义
                // 走 `<locals>` 段（`f.<locals>.g`）；类体（编译器尚未接线）则是 `C.m`。
                // 实测：类体里的 `def m` ⇒ `C.m`；函数里的 `def g` ⇒ `f.<locals>.g`
                let nested_qualname = match self.kind {
                    ScopeKind::Module => name.clone(),
                    ScopeKind::Class => format!("{}.{name}", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
                };
                let nested = compile_scope(
                    name,
                    &nested_qualname,
                    parameters,
                    kwonly,
                    returns.as_ref(),
                    varargs.as_deref(),
                    varkw.as_deref(),
                    self.mode,
                    self.tier,
                    body,
                    ScopeKind::Function,
                    // **类体里定义** ⇒ 方法（多置 `0x8000000`）
                    self.kind == ScopeKind::Class,
                    &[],
                    Span::new(*first_line, *first_line, 0, 0),
                )?;
                // **闭包（第 292 轮）**：内层若把本作用域的局部名当成了**全局**（`co_names` 里出现
                // 本作用域的 `varnames`／`cellvars`），那它就是本作用域的自由变量 ⇒ 用带 `freevars`
                // 的参数**重编一次**内层单元（`COPY_FREE_VARS`／`LOAD_DEREF` 才发得出 ✓）。
                // （顺序取内层 `co_names` 的出现序 —— 实测参照的自由变量表就是引用序）
                let mut nested = nested;
                let mut closure_freevars: Vec<String> = Vec::new();
                // **类体也是闭包体** ✓（task-24 收口 ②）：泛型类的类体把类型参数当自由变量 ✓ ⇒
                // 方法体里引用 `T` 时，方法单元要走**同一条**闭包分析 ✓（`LOAD_DEREF` 才发得出 ✓）。
                if matches!(self.kind, ScopeKind::Function | ScopeKind::Class) {
                    // **`nonlocal` 声明的名字**也是内层的自由变量（它的 `co_names` 里没有，
                    // 光靠名字交集会漏 ✓）—— 只在函数体里收 ✓（类作用域不是 `nonlocal` 的目标 ✗）。
                    if self.kind == ScopeKind::Function {
                        collect_nonlocals(body, &mut closure_freevars);
                    }
                    // **内层向外索取的名字**（含隔着若干层的需求 ✓）
                    for wanted in nested.demanded.clone() {
                        let from_here = self.unit.varnames.iter().any(|local| local == &wanted)
                            || self.unit.cellvars.iter().any(|cell| cell == &wanted)
                            || self.unit.freevars.iter().any(|free| free == &wanted);
                        if from_here && !closure_freevars.iter().any(|item| item == &wanted) {
                            closure_freevars.push(wanted);
                        }
                    }
                    for referenced in global_load_names(&nested) {
                        let from_here = self.unit.varnames.iter().any(|local| local == &referenced)
                            || self.unit.cellvars.iter().any(|cell| cell == &referenced)
                            || self.unit.freevars.iter().any(|free| free == &referenced);
                        if from_here && !closure_freevars.iter().any(|item| item == &referenced) {
                            closure_freevars.push(referenced);
                        }
                    }
                    if !closure_freevars.is_empty() {
                        nested = compile_scope(
                            name,
                            &nested_qualname,
                            parameters,
                            kwonly,
                            returns.as_ref(),
                            varargs.as_deref(),
                            varkw.as_deref(),
                            self.mode,
                            self.tier,
                            body,
                            ScopeKind::Function,
                            // 类体里定义 ⇒ 方法（多置 `0x8000000` ✓）
                            self.kind == ScopeKind::Class,
                            &closure_freevars,
                            Span::new(*first_line, *first_line, 0, 0),
                        )?;
                    }
                }
                // **外壳里的类体所定义的方法要置 `CO_NESTED`（0x10）** ✓（参照实测
                // `class C[T]: def m(self) -> T` 的 `m` flags ＝ `0x8000013` ✓）：类体本身跑在
                // `<generic parameters of C>` 这个函数里 ✓ ⇒ 它的方法在"函数里定义" ✓
                // （`.type_params` 是**壳类体**的标志 ✓；普通类体照旧按 qualname 判 ✓）。
                if self.kind == ScopeKind::Class
                    && self.unit.freevars.iter().any(|free| free == ".type_params")
                {
                    nested.flags |= 0x10;
                }
                // **方法注解单元的自由变量** ✓（task-24 收口 ② 第二层）：注解里引用的名字若在
                // 本层 cell／free 表 ⇒ 它是注解单元的自由变量 ✓（参照实测
                // `class C[T]: def m(self) -> T` ⇒ 注解单元 freevars ＝ `('T', '__classdict__')` ✓；
                // 顺序：**先类型参数**（类体 freevars 序 ✓）、**再 `__classdict__`**、其余按 localsplus 序 ✓）。
                let mut annotate_freevars: Vec<String> = Vec::new();
                if self.kind == ScopeKind::Class {
                    let mut annotation_names: Vec<String> = Vec::new();
                    for parameter in parameters.iter().chain(kwonly.iter()) {
                        if let Some(annotation) = parameter.annotation.as_ref() {
                            collect_constant_names(annotation, &mut annotation_names);
                        }
                    }
                    if let Some(annotation) = returns.as_ref() {
                        collect_constant_names(annotation, &mut annotation_names);
                    }
                    let referenced =
                        |name: &String| annotation_names.iter().any(|item| item == name);
                    // ① 类型参数（类体 freevars 里 `.type_params` 之后那一段 ✓）
                    if self.unit.freevars.first().map(String::as_str) == Some(".type_params") {
                        for free in &self.unit.freevars[1..] {
                            if referenced(free) {
                                annotate_freevars.push(free.clone());
                            }
                        }
                    }
                    // ② `__classdict__`（类字典优先的底座 ✓；参照里**恒在** ✓，即使没被用到 ✓）
                    for cell in self.unit.cellvars.iter() {
                        if (cell == "__classdict__" || referenced(cell))
                            && !annotate_freevars.iter().any(|item| item == cell)
                        {
                            annotate_freevars.push(cell.clone());
                        }
                    }
                    for free in self.unit.freevars.iter() {
                        if free == ".type_params" {
                            continue;
                        }
                        if referenced(free) && !annotate_freevars.iter().any(|item| item == free) {
                            annotate_freevars.push(free.clone());
                        }
                    }
                }
                // **闭包元组**（实测顺序：`LOAD_FAST_BORROW <cell slot>; BUILD_TUPLE n;` 排在
                // `LOAD_CONST <code>`／`MAKE_FUNCTION` **之前**，`SET_FUNCTION_ATTRIBUTE 8` 在其**之后**）
                if !closure_freevars.is_empty() {
                    for free in &closure_freevars {
                        // 元组元素一律是 **`LOAD_FAST_BORROW <localsplus 槽>`**（本层 cell 用它的
                        // cell 槽；若来自更外层则是它自己的自由槽 = `varnames.len()+cellvars.len()+序号` ✓）。
                        // **两层闭包尚未接线** ✗：探针分析只能看到**直接**内层 `def` 的需求，
                        // 传不到隔一层的外层（实测 `def a(): x=1; def b(): def c(): return x` —— `b`
                        // 自己不引用 `x` ⇒ `a` 的探针看不到需求 ⇒ `x` 不会被移出 `varnames`、
                        // `nlocals` 差 1 ✗）。那需要真正的**符号表前向分析**（自由名逐层上浮 ✓），
                        // 不是探针能顶的 ⇒ 如实报错，不静默发错代码 ✗。
                        let slot = self
                            .deref_slot(free)
                            .expect("自由变量必在本层 cell／free 表里");
                        self.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
                    }
                    self.emit_indexed(*span, "BUILD_TUPLE", closure_freevars.len());
                }
                self.emit_function_object(nested, parameters, kwonly, returns.as_ref(), *returns_span, &annotate_freevars, *span)?;
                if !closure_freevars.is_empty() {
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
                }
                // **逆序裹上装饰器**（`CALL 0` 的位点＝各自装饰器的跨度 ✓）：最近的那条先裹 ✓；
                // 模块体与函数体**都要**裹（第一版只写在函数分支里 ✗ ⇒ 模块级的 `def` 漏裹 ✓）
                for decorator_span in decorator_spans.iter().rev() {
                    self.emit_named(*decorator_span, "CALL", 0);
                }
                if self.kind == ScopeKind::Function {
                    // **名字是 cell／自由变量时走 `STORE_DEREF`** ✓（第 267 轮真 bug ✗）：先前这里
                    // 直接 `slot_of` ＋ `STORE_FAST` ✗ ⇒ **嵌在函数里的 `def`** 若其名字是 cell ✓
                    // （会被内层捕获 ✓），就**从没写进 cell** ✗ ⇒ 后面 `LOAD_DEREF` 读到**空 cell** ✗
                    //（实测 `Lib/os.py` 的 `_create_environ_mapping`：`def encode(...)` 是 cell ✓ ⇒
                    //  读到 `NameError: cannot access free variable 'encode'` ✗）。
                    if let Some(slot) = self.deref_slot(name) {
                        self.emit_indexed(*span, "STORE_DEREF", slot);
                    } else {
                        // 函数里嵌的函数存**局部**（实测 `STORE_FAST inner`）
                        let slot = self.slot_of(name);
                        self.emit_indexed(*span, "STORE_FAST", slot);
                    }
                } else {
                    let name_index = self.intern_name(name);
                    self.emit_indexed(*span, "STORE_NAME", name_index);
                }
                // 收尾两条跟 `def` 的整段（实测：`def f(): return 1` 的五条位置都是它）
                self.epilogue_span = *span;
                Ok(())
    }

    /// 造一个**空白作用域发射器**（只给 PEP 695 外壳单元用 ✓）：其余字段取"干净作用域"的默认值 ✓。
    pub(crate) fn blank(
        mode: Mode,
        tier: CheckTier,
        qualname: &str,
        kind: ScopeKind,
        unit: CompiledUnit,
        span: Span,
    ) -> Emitter {
        Emitter {
            unit,
            kind,
            mode,
            tier,
            qualname: qualname.to_owned(),
            boundary_out: None,
            deferred: Vec::new(),
            last_span: span,
            epilogue_span: span,
            global_names: Vec::new(),
            pending: Vec::new(),
            jumps: Vec::new(),
            labels: Vec::new(),
            label_origins: Vec::new(),
            epilogue_needed: false,
            class_annotations: Vec::new(),
            assignment_labels: Vec::new(),
            annotate_freevars: Vec::new(),
            in_condition: false,
            suppress_chain_tail: false,
            loops: Vec::new(),
            block_end_labels: Vec::new(),
            wide_jumps: Vec::new(),
            resolved_jumps: Vec::new(),
            widen_high: Vec::new(),
            widen_shift: Vec::new(),
            exception_entries: Vec::new(),
            comprehension_locals: Vec::new(),
            pending_cleanups: Vec::new(),
            pending_fused_load: None,
            handler_segments: Vec::new(),
            clause_condition_tail: Span::synthetic(),
            clause_had_else: false,
            boolop_scaffold_span: None,
            if_implicit_return: false,
            in_loop_body: false,
            loop_last_if: false,
            in_epilogue_body: false,
            defer_return_literal: false,
            with_return_span: None,
            with_exit_stack: Vec::new(),
            with_body_end: None,
            finally_stack: Vec::new(),
            handler_stack: Vec::new(),
            block_depth: 0,
            block_tail: false,
            handler_depth: 0,
            with_depth: 0,
            condition_landings: Vec::new(),
            collect_condition_exits: false,
            pending_condition_copies: Vec::new(),
            pending_chain_copies: Vec::new(),
            chain_takeover: None,
            suppress_chain_takeover: true,
        }
    }

    /// 收尾一个外壳单元（冲刷待发队列 ＋ 加宽 ＋ 异常表 ＋ 编译期不变量）——与 `compile_scope` 同口径 ✓。
    pub(crate) fn finish_shell(mut self) -> Result<CompiledUnit, CompileError> {
        self.flush_pending_cleanups()?;
        self.flush_pending();
        self.flush_deferred();
        self.flush_condition_copies()?;
        self.flush_jumps();
        self.widen_extended_args();
        self.unit.exceptiontable = self.encode_exceptiontable();
        super::verify::check(&self.unit)?;
        Ok(self.unit)
    }

    /// **`type X = <值>`**（PEP 695 别名语句 ✓，task-24 收口 ③ ✓，`dis` 逐条实测）：
    ///
    /// 模块层：`LOAD_CONST 'X'; LOAD_CONST None; LOAD_CONST (1,); LOAD_CONST <thunk>;
    /// MAKE_FUNCTION; SET_FUNCTION_ATTRIBUTE 1; BUILD_TUPLE 3; CALL_INTRINSIC_1 11 (TYPEALIAS)` ✓。
    /// thunk ＝ `.format` 形参（`argcount=1`／`posonly=1`）＋ 守卫（`format > 2` ⇒ `NotImplementedError` ✓）
    /// ＋ **值表达式** ＋ `RETURN_VALUE` ✓ —— 值**惰性**求值（运行期读 `__value__` 才调它 ✓）。
    /// **如实范围** ✗：泛型别名 `type Y[T] = …`（外壳那条 ✓）尚未接线 ⇒ 当场报未接线 ✓。
    pub(crate) fn emit_type_alias_statement(
        &mut self,
        name: &String,
        _name_span: &Span,
        type_params: &[TypeParam],
        value: &Expression,
        span: &Span,
    ) -> Result<(), CompileError> {
        if !type_params.is_empty() {
            return self.emit_generic_type_alias_statement(name, type_params, value, span);
        }
        let qualname = match self.kind {
            ScopeKind::Module => name.clone(),
            ScopeKind::Class => format!("{}.{name}", self.qualname),
            ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
        };
        let unit = CompiledUnit {
            name: name.clone(),
            qualname: qualname.clone(),
            argcount: 1,
            posonlyargcount: 1,
            kwonlyargcount: 0,
            nlocals: 1,
            // **在函数里 ⇒ `CO_NESTED`（0x13）** ✓（参照实测 `def outer(): type Y = int` ✓）。
            flags: if self.kind == ScopeKind::Function { 0x13 } else { 0x3 },
            names: Vec::new(),
            varnames: vec![".format".to_owned()],
            cellvars: Vec::new(),
            freevars: Vec::new(),
            demanded: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
            exceptiontable: Vec::new(),
        };
        let mut thunk = Emitter::blank(
            self.mode,
            self.tier,
            &qualname,
            ScopeKind::Function,
            unit,
            *span,
        );
        // 与注解单元同一条守卫（`RESUME` 取合成位点 ✓、守卫取整条别名的跨度 ✓）。
        thunk.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        thunk.emit_named(*span, "LOAD_FAST_BORROW", 0);
        thunk.intern_constant(Constant::Int(2));
        thunk.emit_named(*span, "LOAD_SMALL_INT", 2);
        thunk.emit_named(*span, "COMPARE_OP", 132);
        let end = thunk.new_label();
        thunk.emit_jump(
            *span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            end,
        );
        thunk.emit_named(*span, "NOT_TAKEN", 0);
        thunk.emit_named(*span, "LOAD_COMMON_CONSTANT", 1);
        thunk.emit_named(*span, "RAISE_VARARGS", 1);
        thunk.mark_label(end);
        // **值表达式**（位点取自己的跨度 ✓）
        thunk.emit_expression(value)?;
        thunk.emit_named(*span, "RETURN_VALUE", 0);
        let thunk = thunk.finish_shell()?;
        let name_index = self.intern_constant(Constant::Str(name.clone()));
        self.emit_indexed(*span, "LOAD_CONST", name_index);
        let none_index = self.intern_constant(Constant::None);
        self.emit_indexed(*span, "LOAD_CONST", none_index);
        let defaults_index = self.intern_constant(Constant::Tuple(vec![Constant::Int(1)]));
        self.emit_indexed(*span, "LOAD_CONST", defaults_index);
        let code_index = self.intern_constant(Constant::Code(Box::new(thunk)));
        self.emit_indexed(*span, "LOAD_CONST", code_index);
        self.emit_named(*span, "MAKE_FUNCTION", 0);
        self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 1);
        self.emit_indexed(*span, "BUILD_TUPLE", 3);
        self.emit_named(*span, "CALL_INTRINSIC_1", 11);
        self.emit_store_name(*span, name);
        self.epilogue_span = *span;
        Ok(())
    }

    /// **`type Y[T] = …` 的外壳**（task-24 收口 ②，`dis` 逐条实测）：
    /// 外层 `LOAD_CONST <generic parameters of Y>; MAKE_FUNCTION; PUSH_NULL; CALL 0; STORE_* Y` ✓；
    /// 壳内＝`MAKE_CELL`／`RESUME`／类型参数循环（`TYPEVAR` ⇒ 元组留在栈上 ✓）／
    /// `LOAD_CONST (1,)`（默认值元组 ✓）／thunk 的**闭包元组**＋`MAKE_FUNCTION`＋`SET_FUNCTION_ATTRIBUTE 8`
    /// ＋`SET_FUNCTION_ATTRIBUTE 1`／`BUILD_TUPLE 3`／`CALL_INTRINSIC_1 11` ✓；
    /// thunk 用 `LOAD_DEREF` 读类型参数 ✓（与泛型函数外壳里那条**同一手法** ✓）。
    pub(crate) fn emit_generic_type_alias_statement(
        &mut self,
        name: &String,
        type_params: &[TypeParam],
        value: &Expression,
        span: &Span,
    ) -> Result<(), CompileError> {
        let qualname = match self.kind {
            ScopeKind::Module => name.clone(),
            ScopeKind::Class => format!("{}.{name}", self.qualname),
            ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
        };
        let param_names: Vec<String> = type_params.iter().map(|item| item.name.clone()).collect();
        // 值表达式里引用的类型参数（声明序 ✓）⇒ 它们要成为 thunk 的自由变量 ✓（壳里是 cell ✓）。
        let mut names: Vec<String> = Vec::new();
        super::symbols::collect_names_in_expression(value, &mut names);
        let referenced: Vec<String> = param_names
            .iter()
            .filter(|item| names.iter().any(|name| name == *item))
            .cloned()
            .collect();
        // **thunk 单元**（与别名的非泛型版同形 ✓）：`.format` 形参 ＋ 守卫 ＋ 值 ＋ `RETURN_VALUE` ✓；
        // 自由变量 ＝ 值里引用的类型参数 ✓ ⇒ 值里的 `T` 发 `LOAD_DEREF` ✓（参照实测 ✓）。
        let thunk_unit = CompiledUnit {
            name: name.clone(),
            qualname: qualname.clone(),
            argcount: 1,
            posonlyargcount: 1,
            kwonlyargcount: 0,
            nlocals: 1,
            // thunk 定义在外壳**函数**里 ⇒ `CO_NESTED`（0x13 ✓）。
            flags: 0x13,
            names: Vec::new(),
            varnames: vec![".format".to_owned()],
            cellvars: Vec::new(),
            freevars: referenced.clone(),
            demanded: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
            exceptiontable: Vec::new(),
        };
        let mut thunk = Emitter::blank(
            self.mode,
            self.tier,
            &qualname,
            ScopeKind::Function,
            thunk_unit,
            *span,
        );
        if !referenced.is_empty() {
            thunk.emit_none(
                opcode::opcode("COPY_FREE_VARS").expect("COPY_FREE_VARS 在表里"),
                referenced.len() as u8,
            );
        }
        thunk.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        thunk.emit_named(*span, "LOAD_FAST_BORROW", 0);
        thunk.intern_constant(Constant::Int(2));
        thunk.emit_named(*span, "LOAD_SMALL_INT", 2);
        thunk.emit_named(*span, "COMPARE_OP", 132);
        let end = thunk.new_label();
        thunk.emit_jump(
            *span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            end,
        );
        thunk.emit_named(*span, "NOT_TAKEN", 0);
        thunk.emit_named(*span, "LOAD_COMMON_CONSTANT", 1);
        thunk.emit_named(*span, "RAISE_VARARGS", 1);
        thunk.mark_label(end);
        thunk.emit_expression(value)?;
        thunk.emit_named(*span, "RETURN_VALUE", 0);
        let thunk = thunk.finish_shell()?;
        // **外壳单元**：`nlocals = 0`、`cellvars ＝ referenced`（cell 槽 ＝ 序号 ✓）
        let mut unit = CompiledUnit {
            name: format!("<generic parameters of {name}>"),
            qualname: format!("<generic parameters of {name}>"),
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            nlocals: 0,
            // 壳定义在**函数里**（或泛型外壳的类体里 ✓）⇒ `CO_NESTED`（0x13 ✓）；模块级 0x3 ✓。
            flags: if self.kind == ScopeKind::Function
                || self.qualname.contains(".<locals>.")
                || self.unit.freevars.iter().any(|free| free == ".type_params")
            {
                0x13
            } else {
                0x3
            },
            names: Vec::new(),
            varnames: Vec::new(),
            cellvars: Vec::new(),
            freevars: Vec::new(),
            demanded: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
            exceptiontable: Vec::new(),
        };
        // **未引用的类型参数走 varnames**（`STORE_FAST` ✓）、引用的走 cellvars（`STORE_DEREF` ✓）
        // —— 与 def／class 外壳同一口径 ✓（参照实测 `type Z[T] = int` ⇒ `varnames=('T',)` ✓）。
        for param in &param_names {
            if !referenced.iter().any(|item| item == param) {
                unit.varnames.push(param.clone());
            }
        }
        unit.cellvars = referenced.clone();
        unit.nlocals = unit.varnames.len();
        let shell_qualname = unit.qualname.clone();
        let mut shell = Emitter::blank(
            self.mode,
            self.tier,
            &shell_qualname,
            ScopeKind::Function,
            unit,
            *span,
        );
        for index in 0..shell.unit.cellvars.len() {
            let slot = shell.unit.nlocals + index;
            shell.emit_named_none("MAKE_CELL", slot as u8);
        }
        shell.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        // **别名名字**（`TYPEALIAS` 三元组的第一个元素 ✓；位点取整条别名跨度 ✓）
        let name_constant = shell.intern_constant(Constant::Str(name.clone()));
        shell.emit_indexed(*span, "LOAD_CONST", name_constant);
        for param in type_params {
            emit_type_param(&mut shell, param, &referenced)?;
        }
        let first_param_span = type_params[0].span;
        shell.emit_indexed(first_param_span, "BUILD_TUPLE", type_params.len());
        // **默认值元组**（thunk 的 `.format` 默认值 `(1,)` ✓，排在闭包元组**之前** ✓）
        let defaults_index = shell.intern_constant(Constant::Tuple(vec![Constant::Int(1)]));
        shell.emit_indexed(*span, "LOAD_CONST", defaults_index);
        if !referenced.is_empty() {
            for free in &referenced {
                let slot = shell_cell_slot(&shell.unit, free);
                shell.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
            }
            shell.emit_indexed(*span, "BUILD_TUPLE", referenced.len());
        }
        let code_index = shell.intern_constant(Constant::Code(Box::new(thunk)));
        shell.emit_indexed(*span, "LOAD_CONST", code_index);
        shell.emit_named(*span, "MAKE_FUNCTION", 0);
        if !referenced.is_empty() {
            shell.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
        }
        shell.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 1);
        shell.emit_indexed(*span, "BUILD_TUPLE", 3);
        shell.emit_named(*span, "CALL_INTRINSIC_1", 11);
        shell.emit_named(*span, "RETURN_VALUE", 0);
        let wrapper = shell.finish_shell()?;
        let wrapper_index = self.intern_constant(Constant::Code(Box::new(wrapper)));
        self.emit_indexed(*span, "LOAD_CONST", wrapper_index);
        self.emit_named(*span, "MAKE_FUNCTION", 0);
        self.emit_named(*span, "PUSH_NULL", 0);
        self.emit_named(*span, "CALL", 0);
        if self.kind == ScopeKind::Function {
            if let Some(slot) = self.deref_slot(name) {
                self.emit_indexed(*span, "STORE_DEREF", slot);
            } else {
                let slot = self.slot_of(name);
                self.emit_indexed(*span, "STORE_FAST", slot);
            }
        } else {
            self.emit_store_name(*span, name);
        }
        self.epilogue_span = *span;
        Ok(())
    }

    /// **`class C[T]` 的外壳**（task-24 步②/④，`dis` 逐条实测）：
    ///
    /// 外层发 `LOAD_CONST <generic parameters of C>; MAKE_FUNCTION; PUSH_NULL; CALL 0; STORE_NAME C` ✓
    /// （与普通类那条同形 ✓）；外壳单元内部＝序言（`MAKE_CELL`／`RESUME`／`TYPEVAR` 循环 ⇒
    /// `.type_params` 元组）＋**原样重放**类构造（`LOAD_BUILD_CLASS`…，类体带 `.type_params`
    /// 闭包 ✓）＋收尾（`SUBSCRIPT_GENERIC` ⇒ `.generic_base` ＋ `CALL 3`）✓。
    pub(crate) fn emit_generic_class_statement(
        &mut self,
        name: &String,
        span: &Span,
        first_line: &u32,
        decorators: &[Expression],
        keywords: &[(String, Expression)],
        bases: &[Expression],
        body: &[Statement],
        type_params: &[TypeParam],
    ) -> Result<(), CompileError> {
        if !keywords.is_empty() {
            return Err(CompileError::Unsupported(
                "PEP 695 泛型类的类关键字（`metaclass=` 一族）尚未接线".to_owned(),
            ));
        }
        // 装饰器先按**源码序**求值（与普通类同口径 ✓）
        let decorator_spans: Vec<Span> =
            decorators.iter().map(|decorator| decorator.span()).collect();
        for decorator in decorators {
            self.emit_expression(decorator)?;
        }
        let class_qualname = match self.kind {
            ScopeKind::Module => name.clone(),
            ScopeKind::Class => format!("{}.{name}", self.qualname),
            ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
        };
        let param_names: Vec<String> = type_params.iter().map(|item| item.name.clone()).collect();
        // 注解里引用的类型参数（声明序 ✓）——它们要成为类体／注解单元的自由变量 ✓
        let mut annotation_names: Vec<String> = Vec::new();
        collect_class_annotation_names(body, &mut annotation_names);
        let annotate_freevars: Vec<String> = param_names
            .iter()
            .filter(|item| annotation_names.iter().any(|name| name == *item))
            .cloned()
            .collect();
        // **方法注解**引用类型参数（task-24 收口 ② ✓）：这些类型参数要成为**两层闭包**的自由变量
        // —— 类体捕获它们（`.type_params` 那一族 ✓）、方法注解单元再从类体捕获 ✓。
        // 收集成名字表并按**声明序**并进类体自由变量 ✓（参照的类体 freevars 就是这个序 ✓）。
        let method_freevars: Vec<String> = class_body_def_references_params(body, &param_names);
        // 基类引用**外层局部** ⇒ 外壳还要闭包 ⇒ 如实未接线 ✗（模块级基类照常 ✓）
        for base in bases {
            let mut names: Vec<String> = Vec::new();
            super::symbols::collect_names_in_expression(base, &mut names);
            for referenced in names {
                if self.local_or_cell_or_free(&referenced) {
                    return Err(CompileError::Unsupported(format!(
                        "PEP 695 泛型类的基类引用外层局部 `{referenced}` 尚未接线"
                    )));
                }
            }
        }
        // **类体自由变量**：`.type_params` ＋ 体里引用到的类型参数 ✓（类体注解 ＋ 方法注解 ✓，
        // 一律按**声明序** ✓）
        let mut base_referenced: Vec<String> = Vec::new();
        for param in &param_names {
            if annotate_freevars.iter().any(|name| name == param)
                || method_freevars.iter().any(|name| name == param)
            {
                base_referenced.push(param.clone());
            }
        }
        let mut base_freevars: Vec<String> = vec![".type_params".to_owned()];
        base_freevars.extend(base_referenced.iter().cloned());
        let probe = compile_class_scope(
            name,
            &class_qualname,
            self.mode,
            self.tier,
            body,
            *first_line,
            &base_freevars,
            &annotate_freevars,
        )?;
        for referenced in probe.names.clone().into_iter().chain(probe.demanded.clone()) {
            if self.local_or_cell_or_free(&referenced) {
                return Err(CompileError::Unsupported(format!(
                    "PEP 695 泛型类体引用外层局部 `{referenced}`（外壳闭包）尚未接线"
                )));
            }
        }
        let mut referenced: Vec<String> = base_referenced;
        for param in &param_names {
            if referenced.iter().any(|name| name == param) {
                continue;
            }
            if probe.names.iter().any(|name| name == param)
                || probe.demanded.iter().any(|name| name == param)
                // **方法体**里引用类型参数（`class C[T]: def m(self): return T` ✓）：那句 `LOAD_GLOBAL T`
                // 落在**方法单元自己的** `co_names` 里 ✗（不在类体的 `co_names` 里 ✗）⇒ 要扫一遍
                // 类体常量里的**嵌套 code 单元** ✓（task-24 收口 ②）。
                || nested_unit_names(&probe).iter().any(|name| name == param)
            {
                referenced.push(param.clone());
            }
        }
        let mut class_freevars: Vec<String> = vec![".type_params".to_owned()];
        class_freevars.extend(referenced.iter().cloned());
        let mut nested = if class_freevars == base_freevars {
            probe
        } else {
            compile_class_scope(
                name,
                &class_qualname,
                self.mode,
                self.tier,
                body,
                *first_line,
                &class_freevars,
                &annotate_freevars,
            )?
        };
        // 类体在**外壳函数**里 ⇒ 它的 `__annotate__` 单元是**嵌套函数**（多置 `CO_NESTED` 0x10 ✓，
        // 参照实测 `class C[T]: x: T` 的注解单元 flags ＝ 0x13 ✓；非泛型类仍是 0x3 ✓）。
        force_nested_annotate(&mut nested);
        // **外壳单元**：varnames ＝ 非 cell 类型参数（声明序）＋ `.generic_base` ✓；
        // cellvars ＝ `.type_params` ＋ cell 类型参数 ✓（cell 槽 ＝ nlocals ＋ 序号 ✓）。
        let mut unit = CompiledUnit {
            name: format!("<generic parameters of {name}>"),
            qualname: format!("<generic parameters of {name}>"),
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            nlocals: 0,
            flags: 0x3,
            names: Vec::new(),
            varnames: Vec::new(),
            cellvars: Vec::new(),
            freevars: Vec::new(),
            demanded: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
            exceptiontable: Vec::new(),
        };
        for param in &param_names {
            if !referenced.iter().any(|item| item == param) {
                unit.varnames.push(param.clone());
            }
        }
        unit.varnames.push(".generic_base".to_owned());
        unit.cellvars = vec![".type_params".to_owned()];
        unit.cellvars.extend(referenced.iter().cloned());
        unit.nlocals = unit.varnames.len();
        let shell_qualname = unit.qualname.clone();
        let mut shell = Emitter::blank(
            self.mode,
            self.tier,
            &shell_qualname,
            ScopeKind::Function,
            unit,
            *span,
        );
        for index in 0..shell.unit.cellvars.len() {
            let slot = shell.unit.nlocals + index;
            shell.emit_named_none("MAKE_CELL", slot as u8);
        }
        shell.emit_named(Span::new(*first_line, *first_line, 0, 0), "RESUME", 0);
        for param in type_params {
            emit_type_param(&mut shell, param, &referenced)?;
        }
        let first_param_span = type_params[0].span;
        shell.emit_indexed(first_param_span, "BUILD_TUPLE", type_params.len());
        // `.type_params` cell ＝ cellvars[0] ⇒ 槽 ＝ nlocals ✓
        shell.emit_indexed(*span, "STORE_DEREF", shell.unit.nlocals);
        // **原样重放类构造**（与普通类同形，只是类体带闭包 ＋ 基类多一条泛型基 ✓）
        shell.emit_named(*span, "LOAD_BUILD_CLASS", 0);
        shell.emit_named(*span, "PUSH_NULL", 0);
        for free in &class_freevars {
            let slot = shell_cell_slot(&shell.unit, free);
            shell.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
        }
        shell.emit_indexed(*span, "BUILD_TUPLE", class_freevars.len());
        let code_index = shell.intern_constant(Constant::Code(Box::new(nested)));
        shell.emit_indexed(*span, "LOAD_CONST", code_index);
        shell.emit_named(*span, "MAKE_FUNCTION", 0);
        shell.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
        let name_constant = shell.intern_constant(Constant::Str(name.clone()));
        shell.emit_indexed(*span, "LOAD_CONST", name_constant);
        // 泛型基：`LOAD_DEREF .type_params; CALL_INTRINSIC_1 10`
        shell.emit_indexed(*span, "LOAD_DEREF", shell.unit.nlocals);
        shell.emit_named(*span, "CALL_INTRINSIC_1", 10);
        let generic_base = shell.unit.varnames.len() - 1;
        if bases.is_empty() {
            // 参照的窥孔：无显式基类时打成 `STORE_FAST_LOAD_FAST(.generic_base,.generic_base)` ✓
            if generic_base < 16 {
                shell.emit_named(
                    *span,
                    "STORE_FAST_LOAD_FAST",
                    ((generic_base << 4) | generic_base) as u8,
                );
            } else {
                shell.emit_indexed(*span, "STORE_FAST", generic_base);
                shell.emit_indexed(*span, "LOAD_FAST_BORROW", generic_base);
            }
        } else {
            shell.emit_indexed(*span, "STORE_FAST", generic_base);
            for base in bases {
                shell.emit_expression(base)?;
            }
            shell.emit_indexed(*span, "LOAD_FAST_BORROW", generic_base);
        }
        shell.emit_indexed(*span, "CALL", 2 + bases.len() + 1);
        shell.emit_named(*span, "RETURN_VALUE", 0);
        let wrapper = shell.finish_shell()?;
        let wrapper_index = self.intern_constant(Constant::Code(Box::new(wrapper)));
        self.emit_indexed(*span, "LOAD_CONST", wrapper_index);
        self.emit_named(*span, "MAKE_FUNCTION", 0);
        self.emit_named(*span, "PUSH_NULL", 0);
        self.emit_named(*span, "CALL", 0);
        for decorator_span in decorator_spans.iter().rev() {
            self.emit_named(*decorator_span, "CALL", 0);
        }
        self.emit_store_name(*span, name);
        self.epilogue_span = *span;
        Ok(())
    }

    /// **`def f[T](…)` 的外壳**（task-24 步④，`dis` 逐条实测）：外层
    /// `LOAD_CONST <generic parameters of f>; MAKE_FUNCTION; PUSH_NULL; CALL 0; STORE_NAME f` ✓；
    /// 内部＝序言（`MAKE_CELL`／`TYPEVAR` 循环 ⇒ 类型参数元组留在栈上 ✓）＋注解单元的闭包 ✓
    /// ＋内层函数对象 ＋ 收尾 `SWAP 2; CALL_INTRINSIC_2 4`（挂 `__type_params__`）✓。
    pub(crate) fn emit_generic_def_statement(
        &mut self,
        name: &String,
        decorators: &[Expression],
        span: &Span,
        first_line: &u32,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        varargs: &Option<String>,
        varkw: &Option<String>,
        returns: &Option<Constant>,
        returns_span: &Option<Span>,
        body: &[Statement],
        type_params: &[TypeParam],
    ) -> Result<(), CompileError> {
        // **默认值参数**要先经"外壳的参数"传进来（`.defaults` 槽 ✓）⇒ 通道未接线 ✗
        if parameters
            .iter()
            .chain(kwonly.iter())
            .any(|parameter| parameter.default.is_some())
        {
            return Err(CompileError::Unsupported(
                "PEP 695 泛型函数的默认值参数（`.defaults` 通道）尚未接线".to_owned(),
            ));
        }
        let decorator_spans: Vec<Span> =
            decorators.iter().map(|decorator| decorator.span()).collect();
        for decorator in decorators {
            self.emit_expression(decorator)?;
        }
        let nested_qualname = match self.kind {
            ScopeKind::Module => name.clone(),
            ScopeKind::Class => format!("{}.{name}", self.qualname),
            ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
        };
        let annotate_qualname = if self.kind == ScopeKind::Module {
            "__annotate__".to_owned()
        } else {
            format!("{}.<locals>.__annotate__", self.qualname)
        };
        let param_names: Vec<String> = type_params.iter().map(|item| item.name.clone()).collect();
        // 注解里引用的类型参数（声明序 ✓）
        let mut annotation_names: Vec<String> = Vec::new();
        for parameter in parameters.iter().chain(kwonly.iter()) {
            if let Some(annotation) = parameter.annotation.as_ref() {
                collect_constant_names(annotation, &mut annotation_names);
            }
        }
        if let Some(annotation) = returns.as_ref() {
            collect_constant_names(annotation, &mut annotation_names);
        }
        let annotate_freevars: Vec<String> = param_names
            .iter()
            .filter(|item| annotation_names.iter().any(|name| name == *item))
            .cloned()
            .collect();
        let annotated = parameters
            .iter()
            .chain(kwonly.iter())
            .any(|parameter| parameter.annotation.is_some())
            || returns.is_some();
        // **内层函数**：先探针一遍 ⇒ 体内引用类型参数 ⇒ 成为内层的自由变量（⇒ 类型参数成 cell ✓）
        let mut nested = compile_scope(
            name,
            &nested_qualname,
            parameters,
            kwonly,
            returns.as_ref(),
            varargs.as_deref(),
            varkw.as_deref(),
            self.mode,
            self.tier,
            body,
            ScopeKind::Function,
            self.kind == ScopeKind::Class,
            &[],
            Span::new(*first_line, *first_line, 0, 0),
        )?;
        for referenced in nested.names.clone().into_iter().chain(nested.demanded.clone()) {
            if self.local_or_cell_or_free(&referenced) {
                return Err(CompileError::Unsupported(format!(
                    "PEP 695 泛型函数体引用外层局部 `{referenced}`（外壳闭包）尚未接线"
                )));
            }
        }
        let mut body_freevars: Vec<String> = Vec::new();
        for wanted in nested.demanded.clone() {
            if param_names.iter().any(|item| item == &wanted)
                && !body_freevars.iter().any(|item| item == &wanted)
            {
                body_freevars.push(wanted);
            }
        }
        for referenced in nested.names.clone() {
            if param_names.iter().any(|item| item == &referenced)
                && !body_freevars.iter().any(|item| item == &referenced)
            {
                body_freevars.push(referenced);
            }
        }
        if !body_freevars.is_empty() {
            nested = compile_scope(
                name,
                &nested_qualname,
                parameters,
                kwonly,
                returns.as_ref(),
                varargs.as_deref(),
                varkw.as_deref(),
                self.mode,
                self.tier,
                body,
                ScopeKind::Function,
                false,
                &body_freevars,
                Span::new(*first_line, *first_line, 0, 0),
            )?;
        }
        let referenced: Vec<String> = param_names
            .iter()
            .filter(|item| {
                annotate_freevars.iter().any(|name| name == *item)
                    || body_freevars.iter().any(|name| name == *item)
            })
            .cloned()
            .collect();
        // 内层函数在**外壳函数**里 ⇒ `CO_NESTED`（0x10；参照实测 flags ＝ 0x13 ✓）
        nested.flags |= 0x10;
        // 外壳单元：varnames ＝ `.defaults` ＋ 非 cell 类型参数 ✓；cellvars ＝ cell 类型参数 ✓
        let mut unit = CompiledUnit {
            name: format!("<generic parameters of {name}>"),
            qualname: format!("<generic parameters of {name}>"),
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            nlocals: 0,
            flags: 0x3,
            names: Vec::new(),
            varnames: Vec::new(),
            cellvars: Vec::new(),
            freevars: Vec::new(),
            demanded: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
            exceptiontable: Vec::new(),
        };
        unit.varnames.push(".defaults".to_owned());
        for param in &param_names {
            if !referenced.iter().any(|item| item == param) {
                unit.varnames.push(param.clone());
            }
        }
        unit.cellvars = referenced.clone();
        unit.nlocals = unit.varnames.len();
        let shell_qualname = unit.qualname.clone();
        let mut shell = Emitter::blank(
            self.mode,
            self.tier,
            &shell_qualname,
            ScopeKind::Function,
            unit,
            *span,
        );
        for index in 0..shell.unit.cellvars.len() {
            let slot = shell.unit.nlocals + index;
            shell.emit_named_none("MAKE_CELL", slot as u8);
        }
        shell.emit_named(Span::new(*first_line, *first_line, 0, 0), "RESUME", 0);
        for param in type_params {
            emit_type_param(&mut shell, param, &referenced)?;
        }
        let first_param_span = type_params[0].span;
        shell.emit_indexed(first_param_span, "BUILD_TUPLE", type_params.len());
        if annotated {
            if !annotate_freevars.is_empty() {
                for free in &annotate_freevars {
                    let slot = shell_cell_slot(&shell.unit, free);
                    shell.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
                }
                shell.emit_indexed(*span, "BUILD_TUPLE", annotate_freevars.len());
            }
            let mut annotate = shell.annotate_unit(
                &annotate_qualname,
                parameters,
                kwonly,
                returns.as_ref(),
                *returns_span,
                &annotate_freevars,
                // 泛型函数的注解单元**没有**类字典（不在类体里 ✓）。
                false,
                *span,
            );
            // 注解单元也在外壳函数里 ⇒ `CO_NESTED`（参照实测 flags ＝ 0x13 ✓）
            annotate.flags |= 0x10;
            let index = shell.intern_constant(Constant::Code(Box::new(annotate)));
            shell.emit_indexed(*span, "LOAD_CONST", index);
            shell.emit_named(*span, "MAKE_FUNCTION", 0);
            if !annotate_freevars.is_empty() {
                shell.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
            }
        }
        if !body_freevars.is_empty() {
            for free in &body_freevars {
                let slot = shell_cell_slot(&shell.unit, free);
                shell.emit_indexed(*span, "LOAD_FAST_BORROW", slot);
            }
            shell.emit_indexed(*span, "BUILD_TUPLE", body_freevars.len());
        }
        let code_index = shell.intern_constant(Constant::Code(Box::new(nested)));
        shell.emit_indexed(*span, "LOAD_CONST", code_index);
        shell.emit_named(*span, "MAKE_FUNCTION", 0);
        if annotated {
            shell.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 16);
        }
        if !body_freevars.is_empty() {
            shell.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 8);
        }
        shell.emit_named(*span, "SWAP", 2);
        shell.emit_named(*span, "CALL_INTRINSIC_2", 4);
        shell.emit_named(*span, "RETURN_VALUE", 0);
        let wrapper = shell.finish_shell()?;
        let wrapper_index = self.intern_constant(Constant::Code(Box::new(wrapper)));
        self.emit_indexed(*span, "LOAD_CONST", wrapper_index);
        self.emit_named(*span, "MAKE_FUNCTION", 0);
        self.emit_named(*span, "PUSH_NULL", 0);
        self.emit_named(*span, "CALL", 0);
        for decorator_span in decorator_spans.iter().rev() {
            self.emit_named(*decorator_span, "CALL", 0);
        }
        if self.kind == ScopeKind::Function {
            if let Some(slot) = self.deref_slot(name) {
                self.emit_indexed(*span, "STORE_DEREF", slot);
            } else {
                let slot = self.slot_of(name);
                self.emit_indexed(*span, "STORE_FAST", slot);
            }
        } else {
            let name_index = self.intern_name(name);
            self.emit_indexed(*span, "STORE_NAME", name_index);
        }
        self.epilogue_span = *span;
        Ok(())
    }

    pub(crate) fn annotate_unit(
        &self,
        qualname: &str,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        returns: Option<&Constant>,
        returns_span: Option<Span>,
        // **自由变量**（task-24 泛型函数 ✓）：注解里引用的类型参数名 ✓（声明序 ✓）。普通函数空表 ✓。
        freevars: &[String],
        // **类字典优先**（task-24 收口 ② ✓）：类体里定义的方法 ⇒ 注解表达式里每个名字先读
        // `__classdict__`（类命名空间 ✓）再退全局／cell ✓（参照实测
        // `class C[T]: def m(self) -> T` ⇒ `LOAD_DEREF <__classdict__>; LOAD_FROM_DICT_OR_DEREF <T>` ✓）。
        classdict: bool,
        span: Span,
    ) -> CompiledUnit {
        let mut emitter = Emitter {
            block_depth: 0,
            handler_depth: 0,
            with_depth: 0,
        comprehension_locals: Vec::new(),
        pending_cleanups: Vec::new(),
        pending_fused_load: None,
            mode: self.mode,
            tier: self.tier,
            qualname: qualname.to_owned(),
            global_names: Vec::new(),
            wide_jumps: Vec::new(),
            resolved_jumps: Vec::new(),
            widen_high: Vec::new(),
            widen_shift: Vec::new(),
            boundary_out: None,
            deferred: Vec::new(),
            pending: Vec::new(),
            jumps: Vec::new(),
            labels: Vec::new(),
            label_origins: Vec::new(),
            if_implicit_return: false,
            block_tail: false,
            in_loop_body: false,
            loop_last_if: false,
            with_return_span: None,
            in_epilogue_body: false,
            defer_return_literal: false,
            with_exit_stack: Vec::new(),
            with_body_end: None,
            finally_stack: Vec::new(),
            handler_stack: Vec::new(),
            condition_landings: Vec::new(),
            collect_condition_exits: false,
            pending_condition_copies: Vec::new(),
            pending_chain_copies: Vec::new(),
            chain_takeover: None,
            suppress_chain_takeover: true,
            suppress_chain_tail: false,
            loops: Vec::new(),
            block_end_labels: Vec::new(),
            exception_entries: Vec::new(),
            handler_segments: Vec::new(),
            clause_condition_tail: Span::synthetic(),
            clause_had_else: false,
            boolop_scaffold_span: None,
            in_condition: false,
            epilogue_needed: false,
            class_annotations: Vec::new(),
            assignment_labels: Vec::new(),
        annotate_freevars: Vec::new(),
            epilogue_span: span,
            last_span: span,
            kind: ScopeKind::Function,
            unit: CompiledUnit {
                name: "__annotate__".to_owned(),
                qualname: qualname.to_owned(),
                argcount: 1,
                posonlyargcount: 0,
                kwonlyargcount: 0,
                nlocals: 1,
                flags: 0x3,
                names: Vec::new(),
                varnames: vec!["format".to_owned()],
                cellvars: Vec::new(),
                freevars: freevars.to_vec(),
                demanded: Vec::new(),
                constants: Vec::new(),
                code: Vec::new(),
                positions: Vec::new(),
                exceptiontable: Vec::new(),
            },
        };
        // **闭包体**：`COPY_FREE_VARS` 排在 `RESUME` **之前**（泛型函数的注解单元 ✓，实测）。
        if !freevars.is_empty() {
            emitter.emit_none(
                opcode::opcode("COPY_FREE_VARS").expect("COPY_FREE_VARS 在表里"),
                freevars.len() as u8,
            );
        }
        // `__classdict__` 在 localsplus 里的槽（`varnames = ('format',)` ⇒ 起点 1 ✓）。
        let classdict_slot: Option<u8> = if classdict {
            freevars
                .iter()
                .position(|name| name == "__classdict__")
                .map(|index| (1 + index) as u8)
        } else {
            None
        };
        // 实测：`__annotate__` 的 `RESUME` 取**合成**位点（`(def 行, def 行, 0, 0)`）
        emitter.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        emitter.emit_named(span, "LOAD_FAST_BORROW", 0);
        // 实测：守卫的 `2` **也**在常量表里（下标 0），尽管指令用的是 `LOAD_SMALL_INT`
        emitter.intern_constant(Constant::Int(2));
        emitter.emit_named(span, "LOAD_SMALL_INT", 2);
        // 实测：`COMPARE_OP 132` 就是 `>`（比较下标 << 5 ｜ 提示位）
        emitter.emit_named(span, "COMPARE_OP", 132);
        let end = emitter.new_label();
        emitter.emit_jump(
            span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            end,
        );
        emitter.emit_named(span, "NOT_TAKEN", 0);
        emitter.emit_named(span, "LOAD_COMMON_CONSTANT", 1);
        emitter.emit_named(span, "RAISE_VARARGS", 1);
        emitter.mark_label(end);
        let mut count = 0usize;
        // 注解字典按**声明次序**：位置参数 → 仅关键字 → `'return'`（实测）
        for parameter in parameters.iter().chain(kwonly.iter()) {
            let Some(annotation) = parameter.annotation.as_ref() else {
                continue;
            };
            // **注解单元不走 `pending`** ✗（那一边没有 `flush_pending` ✓）⇒ 当场入池、走带
            // `EXTENDED_ARG` 的那条 ✓（`as u8` 会截断 ✗）。
            let key = emitter.intern_constant(Constant::Str(parameter.name.clone()));
            emitter.emit_indexed(span, "LOAD_CONST", key);
            // 注解表达式取**注解自身**的跨度（实测 `def f(a: int):` 的 `LOAD_GLOBAL` 是 `(1,1,9,12)`）
            let annotation_span = parameter.annotation_span.unwrap_or(span);
            emitter.emit_annotation_expression(annotation, annotation_span, classdict_slot);
            count += 1;
        }
        if let Some(annotation) = returns {
            let key = emitter.intern_constant(Constant::Str("return".to_owned()));
            emitter.emit_indexed(span, "LOAD_CONST", key);
            // 同上：返回注解取注解自身的跨度（实测 `def f() -> int:` 的 `LOAD_GLOBAL` 是 `(1,1,11,14)`）
            emitter.emit_annotation_expression(annotation, returns_span.unwrap_or(span), classdict_slot);
            count += 1;
        }
        emitter.emit_indexed(span, "BUILD_MAP", count);
        emitter.emit_named(span, "RETURN_VALUE", 0);
        // __annotate__ 单元不会有待发的条件副本
        emitter.flush_jumps();
        emitter.unit
    }

    /// **类体的 `__annotate__` 单元** ✓（task-21 ✓，PEP 649）：1 个 `format` 形参 ✓、**无闭包** ✓
    /// （注解表达式按**函数作用域**求值 ⇒ 名字走 `LOAD_GLOBAL` ✓ —— 参照另有
    /// `LOAD_FROM_DICT_OR_GLOBALS`（类字典优先 ✓），本层**如实简化** ✓：类的名字空间遮蔽全局名时
    /// 的差别记在未修清单里 ✓）。体＝守卫（`format > 2` ⇒ `NotImplementedError` ✓）＋
    /// `BUILD_MAP 0` ＋ 每条注解「值 ⇒ `COPY 2` ⇒ 键 ⇒ `STORE_SUBSCR`」✓ ＋ `RETURN_VALUE` ✓。
    pub(crate) fn class_annotate_unit(
        &self,
        qualname: &str,
        annotations: &[(String, AnnotationSource, Span)],
        // **自由变量**（task-24 泛型类 ✓）：注解里引用的类型参数名 ✓（声明序 ✓）。普通类空表 ✓。
        freevars: &[String],
        span: Span,
    ) -> Result<CompiledUnit, CompileError> {
        let mut emitter = Emitter {
            block_depth: 0,
            handler_depth: 0,
            with_depth: 0,
        comprehension_locals: Vec::new(),
        pending_cleanups: Vec::new(),
        pending_fused_load: None,
            mode: self.mode,
            tier: self.tier,
            qualname: qualname.to_owned(),
            global_names: Vec::new(),
            wide_jumps: Vec::new(),
            resolved_jumps: Vec::new(),
            widen_high: Vec::new(),
            widen_shift: Vec::new(),
            boundary_out: None,
            deferred: Vec::new(),
            pending: Vec::new(),
            jumps: Vec::new(),
            labels: Vec::new(),
            label_origins: Vec::new(),
            if_implicit_return: false,
            block_tail: false,
            in_loop_body: false,
            loop_last_if: false,
            with_return_span: None,
            in_epilogue_body: false,
            defer_return_literal: false,
            with_exit_stack: Vec::new(),
            with_body_end: None,
            finally_stack: Vec::new(),
            handler_stack: Vec::new(),
            condition_landings: Vec::new(),
            collect_condition_exits: false,
            pending_condition_copies: Vec::new(),
            pending_chain_copies: Vec::new(),
            chain_takeover: None,
            suppress_chain_takeover: true,
            suppress_chain_tail: false,
            loops: Vec::new(),
            block_end_labels: Vec::new(),
            exception_entries: Vec::new(),
            handler_segments: Vec::new(),
            clause_condition_tail: Span::synthetic(),
            clause_had_else: false,
            boolop_scaffold_span: None,
            in_condition: false,
            epilogue_needed: false,
            class_annotations: Vec::new(),
            assignment_labels: Vec::new(),
        annotate_freevars: Vec::new(),
            epilogue_span: span,
            last_span: span,
            kind: ScopeKind::Function,
            unit: CompiledUnit {
                name: "__annotate__".to_owned(),
                qualname: qualname.to_owned(),
                argcount: 1,
                posonlyargcount: 0,
                kwonlyargcount: 0,
                nlocals: 1,
                flags: 0x3,
                names: Vec::new(),
                varnames: vec!["format".to_owned()],
                cellvars: Vec::new(),
                freevars: freevars.to_vec(),
                demanded: Vec::new(),
                constants: Vec::new(),
                code: Vec::new(),
                positions: Vec::new(),
                exceptiontable: Vec::new(),
            },
        };
        // **闭包体**：`COPY_FREE_VARS` 排在 `RESUME` **之前**（与函数／类体同款 ✓）。
        if !freevars.is_empty() {
            emitter.emit_none(
                opcode::opcode("COPY_FREE_VARS").expect("COPY_FREE_VARS 在表里"),
                freevars.len() as u8,
            );
        }
        // 与函数级 `__annotate__` 同一条守卫（实测位点：`RESUME` 取合成 ✓、守卫取 `def` 段 ✓）。
        emitter.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        emitter.emit_named(span, "LOAD_FAST_BORROW", 0);
        emitter.intern_constant(Constant::Int(2));
        emitter.emit_named(span, "LOAD_SMALL_INT", 2);
        emitter.emit_named(span, "COMPARE_OP", 132);
        let end = emitter.new_label();
        emitter.emit_jump(
            span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            end,
        );
        emitter.emit_named(span, "NOT_TAKEN", 0);
        emitter.emit_named(span, "LOAD_COMMON_CONSTANT", 1);
        emitter.emit_named(span, "RAISE_VARARGS", 1);
        emitter.mark_label(end);
        emitter.emit_named(span, "BUILD_MAP", 0);
        for (name, annotation, annotation_span) in annotations {
            // 值先压 ✓；键取**注解声明**那一段的位点 ✓。
            // `TS-47`：两种来源各自一条发射通道（`Type` 源是编译期标签 ⇒ `emit_annotation_expression`；
            // `Expression` 源是普通表达式 ⇒ `emit_expression`）——不合并、不错位。
            match annotation {
                AnnotationSource::Expression(expression) => emitter.emit_expression(expression)?,
                AnnotationSource::Type(label) => {
                    emitter.emit_annotation_expression(label, *annotation_span, None)
                }
            }
            emitter.emit_named(*annotation_span, "COPY", 2);
            let key = emitter.intern_constant(Constant::Str(name.clone()));
            emitter.emit_indexed(*annotation_span, "LOAD_CONST", key);
            emitter.emit_named(*annotation_span, "STORE_SUBSCR", 0);
        }
        emitter.emit_named(span, "RETURN_VALUE", 0);
        emitter.flush_jumps();
        Ok(emitter.unit)
    }

    /// 注解**表达式**的发射（实测）：类型名走 `LOAD_GLOBAL`（oparg ＝ `名字下标 << 1`）；
    /// `None` ⇒ `LOAD_CONST None`；`X[...]` ⇒ 先外后内再 `BINARY_OP 26`（`[]`）。
    pub(crate) fn emit_annotation_expression(
        &mut self,
        annotation: &Constant,
        span: Span,
        // **类字典优先**（task-24 收口 ② ✓）：`Some(<__classdict__ 槽>)` ⇒ 每个名字先读类命名空间 ✓
        //（`LOAD_DEREF <槽>; LOAD_FROM_DICT_OR_{GLOBALS,DEREF} …` ✓，参照实测 ✓）；否则照旧 ✓。
        classdict: Option<u8>,
    ) {
        match annotation {
            Constant::Type(name) if name == "NoneType" => {
                let index = self.intern_constant(Constant::None);
                self.emit_indexed(span, "LOAD_CONST", index);
            }
            Constant::Type(name) | Constant::Str(name) => {
                // **类型参数**（task-24 泛型函数 ✓）：注解里引用的类型参数是 `__annotate__` 单元的
                // **自由变量** ⇒ 走 `LOAD_DEREF` ✓（参照实测：`def f[T](…) -> T` 的返回注解
                // 是 `LOAD_DEREF <T 槽>` ✓）；其余名字照旧 `LOAD_GLOBAL` ✓。
                if let Some(slot) = self.deref_slot(name) {
                    if let Some(classdict_slot) = classdict {
                        self.emit_indexed(span, "LOAD_DEREF", usize::from(classdict_slot));
                        self.emit_indexed(span, "LOAD_FROM_DICT_OR_DEREF", slot);
                    } else {
                        self.emit_indexed(span, "LOAD_DEREF", slot);
                    }
                    return;
                }
                let index = self.intern_name(name);
                if let Some(classdict_slot) = classdict {
                    self.emit_indexed(span, "LOAD_DEREF", usize::from(classdict_slot));
                    // `LOAD_FROM_DICT_OR_GLOBALS` 的 oparg 是**名字下标原值**（不左移 ✓，参照实测）。
                    self.emit_indexed(span, "LOAD_FROM_DICT_OR_GLOBALS", index);
                } else {
                    self.emit_shifted(span, "LOAD_GLOBAL", index << 1);
                }
            }
            Constant::Tuple(parts) => {
                for part in parts {
                    self.emit_annotation_expression(part, span, classdict);
                }
                // 实测：`BINARY_OP 26` 的 argrepr 是 `[]`
                self.emit_named(span, "BINARY_OP", 26);
            }
            // **`名字[实参…]`** ✓（第 287 轮，`dis` 逐条实测）：一个实参**不**发 `BUILD_TUPLE` ✓
            //（`list[int]` ⇒ `LOAD_GLOBAL list; LOAD_GLOBAL int; BINARY_OP 26` ✓）、
            // 两个及以上 ⇒ `BUILD_TUPLE n` ✓（`dict[str, object]` ✓）。
            Constant::AnnSubscript { base, arguments } => {
                self.emit_annotation_expression(base, span, classdict);
                for argument in arguments {
                    self.emit_annotation_expression(argument, span, classdict);
                }
                if arguments.len() > 1 {
                    self.emit_indexed(span, "BUILD_TUPLE", arguments.len());
                }
                self.emit_named(span, "BINARY_OP", 26);
            }
            // **注解里的 `|`** ✓（实测 `int | None` ⇒ `…; BINARY_OP 7` ✓）
            Constant::AnnUnion { left, right } => {
                self.emit_annotation_expression(left, span, classdict);
                self.emit_annotation_expression(right, span, classdict);
                self.emit_named(span, "BINARY_OP", 7);
            }
            // **注解里的列表** ✓（`Callable[[int, str], None]` 的头一个实参 ⇒ `BUILD_LIST n` ✓）
            Constant::AnnList(items) => {
                for item in items {
                    self.emit_annotation_expression(item, span, classdict);
                }
                self.emit_indexed(span, "BUILD_LIST", items.len());
            }
            // **点号** ✓（参照发 `LOAD_ATTR`，oparg ＝ 名字下标 `<< 1` ✓，`dis` 实测 ✓）
            Constant::AnnAttribute { base, name } => {
                self.emit_annotation_expression(base, span, classdict);
                let index = self.intern_name(name);
                self.emit_shifted(span, "LOAD_ATTR", index << 1);
            }
            // **前向引用的字符串** ✓（参照发 `LOAD_CONST 'X'` ✓）
            Constant::AnnString(text) => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_indexed(span, "LOAD_CONST", index);
            }
            // **`...`** ✓（参照发 `LOAD_CONST Ellipsis` ✓）
            Constant::Ellipsis => {
                let index = self.intern_constant(Constant::Ellipsis);
                self.emit_indexed(span, "LOAD_CONST", index);
            }
            _ => {}
        }
    }

}
