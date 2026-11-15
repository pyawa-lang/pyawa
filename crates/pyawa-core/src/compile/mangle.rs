//! **类体私有名改写**（CPython 的 private name mangling；`task-5`／缺口 1）。
//!
//! 动因：`Lib/asyncio/futures.py` 的类体写 `__asyncio_awaited_by = None`、方法里写
//! `self.__asyncio_awaited_by`；CPython 在**编译期**把两者都文本改写成
//! `_Future__asyncio_awaited_by` ⇒ 两边对得上。本层先前没实现 ⇒ 方法里读的是另一个名字 ⇒
//! `AttributeError: 'Task' object has no attribute '_Future__asyncio_awaited_by'`。
//!
//! # 规则（**全部用本机 `python3` 3.14.4 实测校准**，不凭记忆）
//!
//! 在**类定义内**（类体 ＋ 其方法体／嵌套函数体／lambda／推导式；嵌套 `class` 用**它自己的**
//! 类名）：形如 `__spam`（**至少两个前导下划线、不以两个下划线结尾**）的**标识符**文本替换为
//! `_<类名去前导下划线>__spam`（去掉 `__spam` 的两个前导下划线再接上）。
//!
//! 实测要点（`dis`／运行期逐例核对）：
//! - `__x_` **改写**（`class C: __x_ = 1` ⇒ `_C__x_`）；`__x__`／`__f___`（≥2 尾随）**不改写**；
//!   `_x`／`___x`（3 前导）按同一把尺子：`___a` ⇒ `_C___a`（只去两个前导）；
//! - 类名前导下划线**全剥掉**：`class _Foo: __x` ⇒ `_Foo__x`；`class __A_: __w` ⇒ `_A___w`；
//!   类名全是下划线（`__`／`___`）⇒ 该类体**不改写**；
//! - 改写范围：变量名（load/store/del）、**属性名**（`self.__x` 的 `LOAD_ATTR`／`STORE_ATTR`）、
//!   形参／`*args`／`**kw` 名、`global`／`nonlocal` 列表、`import`／`from … import` 的模块名与
//!   成员名（实测 CPython 连 `IMPORT_NAME`／`IMPORT_FROM` 的 name 都改写）、match 捕获名；
//! - **不改**：关键字实参名（`f(__x=1)`）、match 类模式的 keyword 名（`case C(__x=1)`）、
//!   字符串内容、`__dunder__`、def／class 的 **code 名／qualname**（实测 `def __m` 的
//!   `co_name` 仍是 `__m`、绑定名才是 `_C__m`）。
//!
//! # 实现位置
//!
//! 在 **parse 之后、typecheck／符号分析／发射之前**对 AST 做**一次**文本改写：
//!
//! - 下游（`symbols` 的探针编译／闭包分析、`co_names`／`co_varnames`／`co_cellvars`／
//!   `co_freevars`、注解单元）都只看到改写后的名字 ⇒ **一处改写、处处一致** ✓；
//! - 不允许运行期特判（本模块不产出任何执行期指令）。
//!
//! 唯一保留源码原名的地方：
//! - `def`／`class`／`type` 的 `name`（code 名／qualname／类名常量）——改写后的绑定名单独放
//!   `binding` 字段（发射器存名字时用它）；
//! - `__static_attributes__`：CPython 收集的是**源码原名**（实测
//!   `class C: def m(self): self.__p = 1` ⇒ `C.__static_attributes__ == ('__p',)`），
//!   所以 `symbols::collect_static_attributes` 用类前缀把 `_C__p` 还原回 `__p`；
//! - PEP 695 的**类型参数名**：CPython 的 `__type_params__` 显示名是源码名（实测
//!   `class G[__T]` 的 `G.__type_params__[0].__name__ == '__T'`），而它内部的 cell 名是改写的
//!   `_G__T`。本层类型参数在内部沿用源码名 ⇒ 用一个「免改写名单」把该作用域里对这些名字的
//!   引用也保持源码名（内部自洽；与参照的差别只在 `co_cellvars` 里的内部名，行为一致）。

use super::*;

/// 一个改写上下文：当前类前缀 ＋ 免改写名单（PEP 695 类型参数名）。
#[derive(Clone)]
struct Context {
    /// 类前缀（`"_C__"` 这种，**不含**被改写的名字后缀）；`None` ＝ 不在类定义里。
    prefix: Option<String>,
    /// 免改写名字（源码名精确匹配）：类型参数名。见模块注释。
    exempt: Vec<String>,
}

impl Context {
    fn root() -> Context {
        Context {
            prefix: None,
            exempt: Vec::new(),
        }
    }

    /// 文本改写一个**标识符**（非标识符／不该改写的形态原样返回）。
    fn mangle(&self, name: &str) -> String {
        if self.exempt.iter().any(|item| item == name) {
            return name.to_owned();
        }
        let Some(prefix) = self.prefix.as_deref() else {
            return name.to_owned();
        };
        if !needs_mangling(name) {
            return name.to_owned();
        }
        format!("{prefix}{}", &name[2..])
    }

    fn with_exempt(&self, names: impl IntoIterator<Item = String>) -> Context {
        let mut next = self.clone();
        next.exempt.extend(names);
        next
    }
}

/// **入口**：改写一个模块（parse 的产物）里的所有私有名。
pub(super) fn mangle_module(statements: &mut [Statement]) {
    statements_in(&Context::root(), statements);
}

/// 类名前导下划线全剥掉后的前缀（`class __A_` ⇒ `"_A___"`）；类名全是下划线 ⇒ `None`。
///
/// 结果直接拼在 `name[2..]` 前面：`_` ＋ 去前导的类名 ＋ `__`。
pub(super) fn class_prefix(class_name: &str) -> Option<String> {
    let stripped = class_name.trim_start_matches('_');
    if stripped.is_empty() {
        None
    } else {
        Some(format!("_{stripped}__"))
    }
}

/// 需不需要改写（`__spam`：≥2 前导下划线、**不以两个下划线结尾**）。
fn needs_mangling(name: &str) -> bool {
    name.starts_with("__") && !name.ends_with("__")
}

/// 一条语句块（同一作用域内的所有语句用同一个上下文）。
fn statements_in(context: &Context, statements: &mut [Statement]) {
    for statement in statements {
        statement_in(context, statement);
    }
}

fn statement_in(context: &Context, statement: &mut Statement) {
    match statement {
        Statement::AssignChained { targets, value, .. } => {
            for target in targets {
                expression_in(context, target);
            }
            expression_in(context, value);
        }
        Statement::AssignTuple {
            targets,
            value,
            chain,
            ..
        } => {
            for (target, _) in targets {
                expression_in(context, target);
            }
            for target in chain {
                expression_in(context, target);
            }
            expression_in(context, value);
        }
        Statement::TypeAlias {
            name,
            binding,
            type_params,
            value,
            ..
        } => {
            *binding = context.mangle(name);
            // 别名值的 thunk 是**嵌套作用域**：继承外层类前缀；它自己的类型参数免改写。
            let alias_context = context.with_exempt(type_params.iter().map(|item| item.name.clone()));
            type_params_in(&alias_context, type_params);
            expression_in(&alias_context, value);
        }
        Statement::Delete { targets, .. } => {
            for target in targets {
                expression_in(context, target);
            }
        }
        Statement::Assign {
            target,
            value,
            ..
        } => {
            *target = context.mangle(target);
            expression_in(context, value);
        }
        Statement::Annotation {
            name, annotation, ..
        } => {
            *name = context.mangle(name);
            // 注解在**另一个 code object**（`__annotate__`）里求值：那个单元嵌套在本作用域里 ⇒
            // 继承同一个类前缀（实测 `class C: __T = int; def m(self, x: __T)` ⇒ 注解里是 `_C__T`）。
            annotation_source_in(context, annotation);
        }
        Statement::Return(value, _) | Statement::Expression(value, _) => {
            expression_in(context, value);
        }
        Statement::Assert { test, message, .. } => {
            expression_in(context, test);
            if let Some(message) = message {
                expression_in(context, message);
            }
        }
        Statement::NonLocal(names, _) | Statement::Global(names, _) => {
            // 实测 CPython 连声明列表也改写（`nonlocal __x` 报错信息里就是 `_C__x`）。
            for name in names {
                *name = context.mangle(name);
            }
        }
        Statement::Yield(value, _) => {
            if let Some(value) = value {
                expression_in(context, value);
            }
        }
        Statement::YieldFrom(value, _) => expression_in(context, value),
        Statement::For {
            target,
            target_span: _,
            tuple_targets,
            iterable,
            body,
            else_body,
            ..
        } => {
            *target = context.mangle(target);
            for target in tuple_targets {
                for_target_in(context, target);
            }
            expression_in(context, iterable);
            statements_in(context, body);
            statements_in(context, else_body);
        }
        Statement::Match {
            subject, cases, ..
        } => {
            expression_in(context, subject);
            for case in cases {
                pattern_in(context, &mut case.pattern);
                if let Some(guard) = &mut case.guard {
                    expression_in(context, guard);
                }
                statements_in(context, &mut case.body);
            }
        }
        Statement::While {
            condition,
            body,
            else_body,
            ..
        } => {
            expression_in(context, condition);
            statements_in(context, body);
            statements_in(context, else_body);
        }
        Statement::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            expression_in(context, condition);
            statements_in(context, then_body);
            statements_in(context, else_body);
        }
        Statement::Raise { value, cause, .. } => {
            if let Some(value) = value {
                expression_in(context, value);
            }
            if let Some(cause) = cause {
                expression_in(context, cause);
            }
        }
        Statement::Pass(_) | Statement::Break(_) | Statement::Continue(_) => {}
        Statement::Import { items, .. } => {
            // 实测 CPython 把 `IMPORT_NAME` 的模块名也改写（`class C: import __m` 去 import
            // `_C__m`）；`as` 别名同尺子。
            for (module, alias) in items {
                *module = context.mangle(module);
                if let Some(alias) = alias {
                    *alias = context.mangle(alias);
                }
            }
        }
        Statement::ImportFrom { module, names, .. } => {
            *module = context.mangle(module);
            for (name, alias) in names {
                *name = context.mangle(name);
                if let Some(alias) = alias {
                    *alias = context.mangle(alias);
                }
            }
        }
        Statement::With {
            items, body, ..
        } => {
            for (expression, target) in items {
                expression_in(context, expression);
                if let Some(target) = target {
                    for_target_in(context, target);
                }
            }
            statements_in(context, body);
        }
        Statement::Try {
            body,
            handlers,
            else_body,
            finally_body,
            ..
        } => {
            statements_in(context, body);
            for handler in handlers {
                if let Some(type_) = &mut handler.type_ {
                    expression_in(context, type_);
                }
                if let Some(name) = &mut handler.name {
                    *name = context.mangle(name);
                }
                statements_in(context, &mut handler.body);
            }
            statements_in(context, else_body);
            statements_in(context, finally_body);
        }
        Statement::AugAssign { target, value, .. } => {
            aug_target_in(context, target);
            expression_in(context, value);
        }
        Statement::AssignSubscript {
            container,
            key,
            value,
            ..
        } => {
            expression_in(context, container);
            expression_in(context, key);
            expression_in(context, value);
        }
        Statement::AssignAttr {
            object, name, value, ..
        } => {
            expression_in(context, object);
            *name = context.mangle(name);
            expression_in(context, value);
        }
        Statement::Class {
            name,
            binding,
            decorators,
            keywords,
            bases,
            body,
            type_params,
            ..
        } => {
            *binding = context.mangle(name);
            // 装饰器／基类／类关键字在**外层作用域**求值 ⇒ 用外层上下文。
            for decorator in decorators {
                expression_in(context, decorator);
            }
            for base in bases {
                expression_in(context, base);
            }
            // 类关键字名（`metaclass=`）是字符串，不改写；值在外层求值。
            for (_, value) in keywords {
                expression_in(context, value);
            }
            // 类型参数的 bound／default 在「泛型参数」外壳里求值：实测**模块级**类的外部
            // 私有名**不改写**（`class G[__T: __b]` 的 bound 里是 `__b`）⇒ 用**外层**上下文；
            // 它自己的类型参数名免改写（显示名保持源码名）。
            let params_context =
                context.with_exempt(type_params.iter().map(|item| item.name.clone()));
            type_params_in(&params_context, type_params);
            // 类体：**前缀重置成本类**（嵌套 class 用嵌套类自己的名字）；类型参数名免改写。
            let body_context = Context {
                prefix: class_prefix(name),
                exempt: params_context.exempt,
            };
            statements_in(&body_context, body);
        }
        Statement::Def {
            name,
            binding,
            decorators,
            parameters,
            kwonly,
            varargs,
            varkw,
            varargs_annotation,
            varkw_annotation,
            returns,
            body,
            type_params,
            ..
        } => {
            *binding = context.mangle(name);
            // 装饰器／默认值在**外层作用域**求值 ⇒ 外层上下文。
            for decorator in decorators {
                expression_in(context, decorator);
            }
            for parameter in parameters.iter_mut().chain(kwonly.iter_mut()) {
                parameter_in(context, parameter);
            }
            if let Some(varargs) = varargs {
                *varargs = context.mangle(varargs);
            }
            if let Some(varkw) = varkw {
                *varkw = context.mangle(varkw);
            }
            // 注解在 `__annotate__` 单元里求值：那个单元嵌套在本作用域里 ⇒ 继承同一个类前缀；
            // 本函数自己的类型参数名免改写。
            let annotate_context =
                context.with_exempt(type_params.iter().map(|item| item.name.clone()));
            if let Some((annotation, _)) = varargs_annotation {
                annotation_source_in(&annotate_context, annotation);
            }
            if let Some((annotation, _)) = varkw_annotation {
                annotation_source_in(&annotate_context, annotation);
            }
            if let Some(returns) = returns {
                annotation_source_in(&annotate_context, returns);
            }
            // 类型参数的 bound／default：在「泛型参数」外壳里求值（实测类里的
            // `def m[__T: __b]` 里是 `_C__b` ⇒ 继承类前缀）；自己的类型参数名免改写。
            type_params_in(&annotate_context, type_params);
            // 函数体：**继承**外层类前缀（方法／嵌套 def／lambda 都用外层类名，实测）。
            statements_in(&annotate_context, body);
        }
        Statement::NullCoalesceAssign { name, value, .. } => {
            *name = context.mangle(name);
            expression_in(context, value);
        }
    }
}

/// 一个形参：名字改写、注解用「注解上下文」、默认值用「外层上下文」。
fn parameter_in(context: &Context, parameter: &mut Parameter) {
    parameter.name = context.mangle(&parameter.name);
    if let Some(annotation) = &mut parameter.annotation {
        annotation_source_in(context, annotation);
    }
    if let Some(default) = &mut parameter.default {
        expression_in(context, default);
    }
}

/// PEP 695 类型参数：名字**不改写**（显示名；内部引用靠免改写名单），bound／default 走表达式。
fn type_params_in(context: &Context, type_params: &mut [TypeParam]) {
    for type_param in type_params {
        if let Some(bound) = &mut type_param.bound {
            expression_in(context, bound);
        }
        if let Some(default) = &mut type_param.default {
            expression_in(context, default);
        }
    }
}

fn annotation_source_in(context: &Context, annotation: &mut AnnotationSource) {
    if let AnnotationSource::Expression(expression) = annotation {
        expression_in(context, expression);
    }
}

fn for_target_in(context: &Context, target: &mut ForTarget) {
    match target {
        ForTarget::Name(name, _) => *name = context.mangle(name),
        ForTarget::Group(items, _) => {
            for item in items {
                for_target_in(context, item);
            }
        }
        ForTarget::Starred(inner, _) => for_target_in(context, inner),
        ForTarget::Attribute(object, name, _) => {
            expression_in(context, object);
            *name = context.mangle(name);
        }
        ForTarget::Subscript(container, key, _) => {
            expression_in(context, container);
            expression_in(context, key);
        }
    }
}

fn comprehension_target_in(context: &Context, target: &mut ComprehensionTarget) {
    match target {
        ComprehensionTarget::Name(name, _) => *name = context.mangle(name),
        ComprehensionTarget::Group(items, _) => {
            for item in items {
                comprehension_target_in(context, item);
            }
        }
        ComprehensionTarget::Attribute(object, name, _) => {
            expression_in(context, object);
            *name = context.mangle(name);
        }
        ComprehensionTarget::Subscript(container, key, _) => {
            expression_in(context, container);
            expression_in(context, key);
        }
        ComprehensionTarget::Starred(inner, _) => comprehension_target_in(context, inner),
    }
}

fn aug_target_in(context: &Context, target: &mut AugTarget) {
    match target {
        AugTarget::Name(name, _) => *name = context.mangle(name),
        AugTarget::Attribute { object, name, .. } => {
            expression_in(context, object);
            *name = context.mangle(name);
        }
        AugTarget::Subscript {
            container, key, ..
        } => {
            expression_in(context, container);
            expression_in(context, key);
        }
    }
}

fn pattern_in(context: &Context, pattern: &mut Pattern) {
    match pattern {
        Pattern::Literal(..) | Pattern::Wildcard(..) => {}
        Pattern::Value(expression, _) => expression_in(context, expression),
        Pattern::Class {
            class,
            positional,
            keywords,
            ..
        } => {
            expression_in(context, class);
            for item in positional {
                pattern_in(context, item);
            }
            // 类模式的关键字名是**字符串**（实测 `case C(__kw=1)` 的常量元组是 `('__kw',)`）⇒ 不改写。
            for (_, item) in keywords {
                pattern_in(context, item);
            }
        }
        Pattern::Capture(name, _) => *name = context.mangle(name),
        Pattern::Or(items, _) => {
            for item in items {
                pattern_in(context, item);
            }
        }
        Pattern::As(inner, name, _) => {
            pattern_in(context, inner);
            *name = context.mangle(name);
        }
        Pattern::Sequence { items, .. } => {
            for item in items {
                pattern_in(context, item);
            }
        }
        Pattern::Mapping { items, .. } => {
            for (key, value) in items {
                pattern_in(context, key);
                pattern_in(context, value);
            }
        }
        Pattern::Star(inner, _) => pattern_in(context, inner),
    }
}

fn fstring_parts_in(context: &Context, parts: &mut [FStringPart]) {
    for part in parts {
        if let FStringPart::Formatted {
            expression, spec, ..
        } = part
        {
            expression_in(context, expression);
            if let Some(spec) = spec {
                fstring_parts_in(context, spec);
            }
        }
    }
}

fn lambda_body_in(context: &Context, body: &mut LambdaBody) {
    match body {
        LambdaBody::Expression(expression) => expression_in(context, expression),
        LambdaBody::Statements(statements) => statements_in(context, statements),
    }
}

fn expression_in(context: &Context, expression: &mut Expression) {
    match expression {
        Expression::Name(name, _) => *name = context.mangle(name),
        Expression::Attribute(object, name, _) => {
            expression_in(context, object);
            *name = context.mangle(name);
        }
        Expression::Walrus { target, value, .. } => {
            *target = context.mangle(target);
            expression_in(context, value);
        }
        Expression::Yield(value, _) => {
            if let Some(value) = value {
                expression_in(context, value);
            }
        }
        Expression::YieldFrom(value, _) | Expression::Await(value, _) => {
            expression_in(context, value)
        }
        Expression::List(items, _)
        | Expression::SetLiteral(items, _)
        | Expression::TupleLiteral(items, _) => {
            for item in items {
                expression_in(context, item);
            }
        }
        Expression::Map(items, _) => {
            for item in items {
                match item {
                    MapItem::Pair(key, value) => {
                        expression_in(context, key);
                        expression_in(context, value);
                    }
                    MapItem::Unpack(value) => expression_in(context, value),
                }
            }
        }
        Expression::Starred(value, _) | Expression::Not(value, _) => {
            expression_in(context, value)
        }
        Expression::Binary(_, left, right, _) => {
            expression_in(context, left);
            expression_in(context, right);
        }
        Expression::Unary(_, value, _) => expression_in(context, value),
        Expression::Comprehension {
            element,
            value,
            generators,
            ..
        } => {
            expression_in(context, element);
            if let Some(value) = value {
                expression_in(context, value);
            }
            for generator in generators {
                comprehension_target_in(context, &mut generator.target);
                expression_in(context, &mut generator.iterable);
                for condition in &mut generator.conditions {
                    expression_in(context, condition);
                }
            }
        }
        Expression::FString { parts, .. } => fstring_parts_in(context, parts),
        Expression::Lambda {
            parameters,
            kwonly,
            varargs,
            varkw,
            body,
            ..
        } => {
            for parameter in parameters.iter_mut().chain(kwonly.iter_mut()) {
                parameter_in(context, parameter);
            }
            if let Some(varargs) = varargs {
                *varargs = context.mangle(varargs);
            }
            if let Some(varkw) = varkw {
                *varkw = context.mangle(varkw);
            }
            // lambda 体是嵌套作用域：继承外层类前缀。
            lambda_body_in(context, body);
        }
        Expression::BoolOp { values, .. } => {
            for value in values {
                expression_in(context, value);
            }
        }
        Expression::Subscript(container, key, _) => {
            expression_in(context, container);
            expression_in(context, key);
        }
        Expression::SliceLiteral {
            lower,
            upper,
            step,
            ..
        } => {
            for bound in [lower, upper, step].into_iter().flatten() {
                expression_in(context, bound);
            }
        }
        Expression::Compare(left, _, right, _) => {
            expression_in(context, left);
            expression_in(context, right);
        }
        Expression::ChainedCompare {
            operands,
            operators: _,
            ..
        } => {
            for operand in operands {
                expression_in(context, operand);
            }
        }
        Expression::Conditional {
            condition,
            then_value,
            else_value,
            ..
        } => {
            expression_in(context, condition);
            expression_in(context, then_value);
            expression_in(context, else_value);
        }
        Expression::Call {
            function,
            arguments,
            positional_order,
            star_arguments,
            keywords,
            dict_arguments,
            ..
        } => {
            expression_in(context, function);
            for argument in arguments {
                expression_in(context, argument);
            }
            for argument in positional_order {
                match argument {
                    PositionalArgument::Plain(value) | PositionalArgument::Star(value) => {
                        expression_in(context, value)
                    }
                }
            }
            for argument in star_arguments {
                expression_in(context, argument);
            }
            // **关键字实参名是字符串，不改写**（实测 `f(__x=1)` 的常量元组是 `('__x',)`）。
            for (_, value) in keywords {
                expression_in(context, value);
            }
            for argument in dict_arguments {
                expression_in(context, argument);
            }
        }
        Expression::NullCoalesce(left, right, _) => {
            expression_in(context, left);
            expression_in(context, right);
        }
        Expression::SafeChain { receiver, rest, .. } => {
            expression_in(context, receiver);
            expression_in(context, rest);
        }
        Expression::Int(..)
        | Expression::BigInt(..)
        | Expression::Float(..)
        | Expression::Imaginary(..)
        | Expression::Str(..)
        | Expression::StrWtf8(..)
        | Expression::Bytes(..)
        | Expression::Constant(..)
        | Expression::ChainSubject(..) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{class_prefix, Context};

    #[test]
    fn mangle_rules_match_cpython() {
        let cases = [
            ("C", "__x", Some("_C__x")),
            ("_Foo", "__x", Some("_Foo__x")),
            ("__A_", "__w", Some("_A___w")),
            ("C", "__x_", Some("_C__x_")),
            ("C", "__x__", None),
            ("C", "__f___", None),
            ("C", "___a", Some("_C___a")),
            ("C", "____b", Some("_C____b")),
            ("C", "_x", None),
            ("C", "__", None),
            ("__", "__x", None),
            ("___", "__x", None),
            ("____", "__u", None),
        ];
        for (class, name, expected) in cases {
            let context = Context {
                prefix: class_prefix(class),
                exempt: Vec::new(),
            };
            assert_eq!(
                context.mangle(name),
                expected.map_or_else(|| name.to_owned(), str::to_owned),
                "class={class} name={name}"
            );
        }
    }

    /// **code 名／qualname 保持源码名、绑定名改写**（实测参照：`def __m` 的 `co_name == "__m"`、
    /// `co_qualname == "M.__m"`，而类体 `co_names` 里是 `_M__m`）。
    #[test]
    fn compiled_class_keeps_code_names_raw_and_mangles_bindings() {
        use crate::compile::{self, CheckTier, CompiledUnit, Constant, Mode};

        fn code_constants(unit: &CompiledUnit) -> Vec<&CompiledUnit> {
            unit.constants
                .iter()
                .filter_map(|constant| match constant {
                    Constant::Code(nested) => Some(nested.as_ref()),
                    _ => None,
                })
                .collect()
        }

        let source = r#"class M:
    __x = 1
    def __m(self, __a):
        return self.__x
"#;
        let unit = compile::compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
            .expect("编译");
        let class = code_constants(&unit)
            .into_iter()
            .find(|unit| unit.name == "M")
            .expect("M 的类体单元");
        // 类体 co_names 里的**绑定名**已被改写。
        assert!(class.names.iter().any(|name| name == "_M__x"));
        assert!(class.names.iter().any(|name| name == "_M__m"));
        let method = code_constants(class)
            .into_iter()
            .find(|unit| unit.name == "__m")
            .expect("__m 的方法单元");
        assert_eq!(method.name, "__m", "co_name 保持源码名");
        assert_eq!(method.qualname, "M.__m", "co_qualname 保持源码名");
        // 形参名也是改写后的。
        assert_eq!(method.varnames, vec!["self", "_M__a"]);
        // 方法里的 `self.__x`：LOAD_ATTR 取的是改写后的名字（在名字表里）。
        assert!(method.names.iter().any(|name| name == "_M__x"));
    }
}
