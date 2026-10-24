//! **`TS-47` 的编译期一半**：静态赋值检查 ＋ `TS-48`（平台类型）＋ `TS-50`（类型收束）。
//!
//! 只在**扩展模式**（`.pyawa`，[`Mode::Extension`]）下由 [`compile`](super::compile) 调用；
//! 纯 Python 模式**根本不会进来**（`TS-5`：`.py` 的标注不产生任何检查、产物逐字节不变）。
//! 这里只做**静态视图**：不改任何发射，也不改运行期行为（运行期那半是 round 5 的
//! `CHECK_ASSIGNMENT`／`BC-65`）。
//!
//! # 口径（一处真相：`docs/SPEC-type-system.md`）
//!
//! - **`TS-47`**：变量被标注之后，赋给它的值**必须与标注相容**。本模块负责**编译期那半**——
//!   只报**能静态判定**的不符；判不出的**留给运行期**（`TS-47` ②，禁止静默通过）。
//! - **`TS-48`**：**未标注**的名字／形参、以及判不出的表达式 ⇒ **平台类型（Any）**，与一切
//!   相容；**禁止**为它们推断类型（`TS-2`）。`Any` 的来源只有"未标注"与"显式 `Any`"（`TS-16`）。
//! - **`TS-29`／`TS-40`**：非 `Any` 之间按**内建类型层次**判子类型（`bool ⊂ int`）；不做结构
//!   类型（协议）。层次表取 [`crate::builtin_types`]（`TS-41` 的同一份真相）。
//! - **`TS-50`**：按控制流收窄**已确知**的可空类型；只对名字（属性／下标不窄化），`assert` 不作
//!   依据；收窄只影响本模块的检查视图，**不改变**声明类型、发射与运行期行为。
//!
//! # 如实边界（本轮不做，别当成已支持）
//!
//! - **类体**不查（任务书：类体单独一轮）——`class C:` 里的标注与方法体整个跳过。
//! - 嵌套函数读**外层函数**的局部／形参不建模（只回退到模块表）⇒ 判不出的按 `Any` 放行。
//! - 函数**返回值**与形参的边界检查不在本模块（那是 `BC-23`…`BC-25` 的边界族，运行期／边界层）。
//! - RHS 只推断"一眼能判定"的字面量与**已标注名字**（`TS-47` ②：其余交给运行期）。
//! - 收窄只有 `TS-50` 明列的两种：`if x is None:`（该支必然走掉 ⇒ 其后收窄）与
//!   `if x is not None:`（分支内收窄；`else` 同理）。`isinstance`／`match`／`assert` 一律不窄化。

use super::symbols::block_terminates;
use super::*;
use crate::builtin_types::builtin_type;
use std::collections::{HashMap, HashSet};

/// 静态类型（`TS-47` 的域）：`Any` ＝ **平台类型**（`TS-48`），其余都是 `BC-24` 的编译期标签。
#[derive(Clone, Debug, PartialEq, Eq)]
enum StaticType {
    /// 平台类型：未标注、判不出的表达式、判不出的注解形状。与一切相容。
    Any,
    /// 已确知的标签（`Type`／`Tuple`／`AnnUnion`…；`"Any"` 标签已归一到 [`StaticType::Any`]）。
    Label(Constant),
}

/// 一个**作用域**的静态信息（模块或函数）。
///
/// 声明表在**整个作用域**内共享（含分支体）：标注是**编译期**登记的事实，分支没执行也照登记
/// —— 与发射器的 `assignment_labels`（按发射顺序登记）同口径。**收窄表**则是流敏感的，
/// 由调用方按控制流分支单独持有（见 [`Checker::check_block`] 的返回值）。
struct Scope {
    /// 本作用域**已确知**的声明类型（形参注解、变量标注）。
    declared: HashMap<String, StaticType>,
    /// 本作用域里被赋过值的名字（用来区分"未标注的局部"与"读模块表"）。
    assigned: HashSet<String>,
    /// `global` 声明的名字（读／写都走模块表）。
    globals: HashSet<String>,
    /// 模块作用域在**这个作用域入口**时的声明表快照（函数体读模块名用）。
    module_snapshot: HashMap<String, StaticType>,
    /// 是不是模块作用域。
    is_module: bool,
}

/// 检查器（除了文件名，还带 `TS-49` 要用的**类属性标注表**）。
struct Checker<'a> {
    filename: &'a str,
    /// **`TS-49` 的已知类型来源**：模块级 `class C:` 体里 `属性: 类型` 的注解。
    /// `A?` 上的 `a?.b`（`b: B`）⇒ `B?` 要靠它；查不到（内建类型／未标注属性）⇒ 平台类型（`TS-48`）。
    /// 类体本身仍**不做赋值检查**（与 M4 的口径一致），这里只读标注。
    class_attributes: HashMap<String, HashMap<String, StaticType>>,
}

/// 检查一段源码（模块体）。只在扩展模式调用。
pub(super) fn check(statements: &[Statement], filename: &str) -> Result<(), CompileError> {
    let mut class_attributes = HashMap::new();
    collect_class_attributes(statements, &mut class_attributes);
    let mut checker = Checker {
        filename,
        class_attributes,
    };
    let mut scope = Scope {
        declared: HashMap::new(),
        assigned: HashSet::new(),
        globals: HashSet::new(),
        module_snapshot: HashMap::new(),
        is_module: true,
    };
    checker.check_block(statements, &mut scope, &HashMap::new())?;
    Ok(())
}

/// 收**类属性标注表**（`TS-49`；`class C:` 体里的 `属性: 类型` 与嵌套类）。
fn collect_class_attributes(
    statements: &[Statement],
    out: &mut HashMap<String, HashMap<String, StaticType>>,
) {
    for statement in statements {
        match statement {
            Statement::Class { name, body, .. } => {
                let mut attributes = HashMap::new();
                for item in body {
                    if let Statement::Annotation {
                        name: attribute,
                        annotation,
                        ..
                    } = item
                    {
                        attributes.insert(attribute.clone(), static_from_annotation(annotation));
                    }
                }
                if !attributes.is_empty() {
                    out.insert(name.clone(), attributes);
                }
                // 嵌套类照收（`class Outer: class Inner:` —— 名字按**内层类名**登记）
                collect_class_attributes(body, out);
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_class_attributes(then_body, out);
                collect_class_attributes(else_body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_class_attributes(body, out);
                for handler in handlers {
                    collect_class_attributes(&handler.body, out);
                }
                collect_class_attributes(else_body, out);
                collect_class_attributes(finally_body, out);
            }
            _ => {}
        }
    }
}

impl Checker<'_> {
    /// 顺序检查一个块，返回**出块时**的收窄表（分支体用调用方给的副本 ⇒ 收窄不外泄）。
    fn check_block(
        &mut self,
        statements: &[Statement],
        scope: &mut Scope,
        narrow_in: &HashMap<String, StaticType>,
    ) -> Result<HashMap<String, StaticType>, CompileError> {
        let mut narrow = narrow_in.clone();
        for statement in statements {
            narrow = self.check_statement(statement, scope, &narrow)?;
        }
        Ok(narrow)
    }

    /// 检查一条语句，返回**执行完之后**的收窄表。
    fn check_statement(
        &mut self,
        statement: &Statement,
        scope: &mut Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> Result<HashMap<String, StaticType>, CompileError> {
        let mut narrow = narrow.clone();
        match statement {
            Statement::Annotation {
                name, annotation, ..
            } => {
                scope
                    .declared
                    .insert(name.clone(), static_from_annotation(annotation));
                narrow.remove(name);
            }
            Statement::Assign {
                target,
                target_span,
                value,
                ..
            } => {
                self.check_assignment(target, *target_span, value, scope, &mut narrow)?;
            }
            Statement::AssignChained { targets, value, .. } => {
                // 只查**名字**目标（属性／下标目标不在 `TS-47` 这一格；`TS-50` 也不窄化它们）。
                for target in targets {
                    if let Expression::Name(name, name_span) = target {
                        self.check_assignment(name, *name_span, value, scope, &mut narrow)?;
                    }
                }
            }
            // **`BC-63` 的 `a ??= b`**（`TS-49`）：**不改变 `a` 的标注类型** —— 但赋值本身仍按
            // `TS-47` 查（`b` 与标注相容才算数；`a` 是未标注 ⇒ 平台类型，放行）。
            Statement::NullCoalesceAssign {
                name,
                name_span,
                value,
                ..
            } => {
                self.check_assignment(name, *name_span, value, scope, &mut narrow)?;
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                return self.check_if(condition, then_body, else_body, scope, &narrow);
            }
            Statement::Def {
                parameters,
                kwonly,
                varargs,
                varkw,
                body,
                ..
            } => {
                self.check_function(parameters, kwonly, varargs, varkw, body, scope)?;
            }
            // 复合语句：体内照查（声明照常登记；收窄只在体内局部有效）。
            Statement::For {
                target,
                tuple_targets,
                body,
                else_body,
                ..
            } => {
                self.check_branch(body, scope, &narrow)?;
                self.check_branch(else_body, scope, &narrow)?;
                narrow.remove(target);
                for item in tuple_targets {
                    clear_for_target(item, &mut narrow);
                }
                clear_assigned(&mut narrow, body);
                clear_assigned(&mut narrow, else_body);
            }
            Statement::While {
                body, else_body, ..
            } => {
                self.check_branch(body, scope, &narrow)?;
                self.check_branch(else_body, scope, &narrow)?;
                clear_assigned(&mut narrow, body);
                clear_assigned(&mut narrow, else_body);
            }
            Statement::With { items, body, .. } => {
                for (_, target) in items {
                    if let Some(target) = target {
                        clear_for_target(target, &mut narrow);
                    }
                }
                self.check_branch(body, scope, &narrow)?;
                clear_assigned(&mut narrow, body);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                self.check_branch(body, scope, &narrow)?;
                for handler in handlers {
                    self.check_branch(&handler.body, scope, &narrow)?;
                }
                self.check_branch(else_body, scope, &narrow)?;
                self.check_branch(finally_body, scope, &narrow)?;
                clear_assigned(&mut narrow, body);
                for handler in handlers {
                    clear_assigned(&mut narrow, &handler.body);
                }
                clear_assigned(&mut narrow, else_body);
                clear_assigned(&mut narrow, finally_body);
            }
            Statement::Match { cases, .. } => {
                for case in cases {
                    self.check_branch(&case.body, scope, &narrow)?;
                    clear_assigned(&mut narrow, &case.body);
                }
            }
            // **类体不查**（本轮边界；见模块文档）。
            Statement::Class { .. } => {}
            // 其余语句（表达式／return／raise／import／del …）不改静态类型视图。
            _ => {}
        }
        Ok(narrow)
    }

    /// 分支体：**声明**照常登记进共享作用域，**收窄**用副本（出分支即丢）。
    fn check_branch(
        &mut self,
        body: &[Statement],
        scope: &mut Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> Result<(), CompileError> {
        self.check_block(body, scope, narrow)?;
        Ok(())
    }

    /// **`if`**：先按两种 `is None` 形态算分支收窄，再查两个体，最后算出后续收窄。
    fn check_if(
        &mut self,
        condition: &Expression,
        then_body: &[Statement],
        else_body: &[Statement],
        scope: &mut Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> Result<HashMap<String, StaticType>, CompileError> {
        let info = self.is_none_condition(condition, scope, narrow);
        let mut then_narrow = narrow.clone();
        let mut else_narrow = narrow.clone();
        let mut then_type: Option<StaticType> = None;
        let mut else_type: Option<StaticType> = None;
        let mut name: Option<String> = None;
        if let Some((kind, target, base)) = info {
            let (when_none, when_not_none) = match base {
                StaticType::Any => (None, None),
                StaticType::Label(label) => (
                    narrowed_to_none(&label).map(StaticType::Label),
                    narrowed_non_none(&label).map(StaticType::Label),
                ),
            };
            let (body_type, other_type) = match kind {
                // `if x is None:` 的体里 x 是 None；`else`／其后是"非 None"。
                IsNoneKind::IsNone => (when_none, when_not_none),
                // `if x is not None:` 的体里 x 非 None；`else`／其后是 None。
                IsNoneKind::IsNotNone => (when_not_none, when_none),
            };
            if let Some(ty) = body_type.clone() {
                then_narrow.insert(target.clone(), ty);
            }
            if let Some(ty) = other_type.clone() {
                else_narrow.insert(target.clone(), ty);
            }
            then_type = body_type;
            else_type = other_type;
            name = Some(target);
        }
        self.check_block(then_body, scope, &then_narrow)?;
        self.check_block(else_body, scope, &else_narrow)?;
        // **后续收窄**：哪一支必然走掉，后续就只剩另一支的类型。
        let mut after = narrow.clone();
        clear_assigned(&mut after, then_body);
        clear_assigned(&mut after, else_body);
        if let Some(name) = name {
            let then_terminates = block_terminates(then_body);
            let else_terminates = !else_body.is_empty() && block_terminates(else_body);
            // 两支都终止 ⇒ 后续不可达，收窄无关；一支终止 ⇒ 走另一支的收窄。
            let continuation = if then_terminates && else_terminates {
                None
            } else if then_terminates {
                else_type
            } else if else_terminates {
                then_type
            } else {
                None
            };
            if let Some(ty) = continuation {
                // 分支里又给这个名字赋过值 ⇒ 收窄前提被破坏（保守地不窄化）。
                if !block_assigns(then_body, &name) && !block_assigns(else_body, &name) {
                    after.insert(name, ty);
                }
            }
        }
        Ok(after)
    }

    /// 认出 `if` 条件里的 `x is None`／`x is not None`，并给出该名字**当前**的静态类型。
    ///
    /// 只对**已确知**的名字（已标注的局部／形参／模块级标注名）报收窄；`Any`（平台类型）不窄化
    /// （`TS-48`：禁止为平台类型推断）。`global` 名不窄化（`TS-50`：只对局部名与形参）。
    fn is_none_condition(
        &self,
        condition: &Expression,
        scope: &Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> Option<(IsNoneKind, String, StaticType)> {
        let Expression::Compare(left, operator, right, _) = condition else {
            return None;
        };
        let kind = match operator {
            CompareOperator::Is => IsNoneKind::IsNone,
            CompareOperator::IsNot => IsNoneKind::IsNotNone,
            _ => return None,
        };
        let (Expression::Name(name, _), Expression::Constant(Constant::None, _)) =
            (left.as_ref(), right.as_ref())
        else {
            return None;
        };
        if scope.globals.contains(name) || !scope.declared.contains_key(name) {
            return None;
        }
        Some((kind, name.clone(), self.read_type(scope, narrow, name)))
    }

    /// 进入一个**函数作用域**：形参注解是声明，体里的标注也是声明；模块表用入口快照。
    fn check_function(
        &mut self,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        varargs: &Option<String>,
        varkw: &Option<String>,
        body: &[Statement],
        outer: &Scope,
    ) -> Result<(), CompileError> {
        let module_snapshot = if outer.is_module {
            outer.declared.clone()
        } else {
            outer.module_snapshot.clone()
        };
        let mut scope = Scope {
            declared: HashMap::new(),
            assigned: assigned_names(body),
            globals: global_names(body),
            module_snapshot,
            is_module: false,
        };
        for parameter in parameters.iter().chain(kwonly.iter()) {
            scope.assigned.insert(parameter.name.clone());
            if let Some(annotation) = &parameter.annotation {
                scope
                    .declared
                    .insert(parameter.name.clone(), static_from_label(annotation));
            }
        }
        for name in [varargs, varkw].into_iter().flatten() {
            scope.assigned.insert(name.clone());
        }
        self.check_block(body, &mut scope, &HashMap::new())?;
        Ok(())
    }

    /// **赋值检查**（`TS-47`）：只看名字目标、只看**本作用域已确知**的声明。
    fn check_assignment(
        &mut self,
        name: &str,
        target_span: Span,
        value: &Expression,
        scope: &Scope,
        narrow: &mut HashMap<String, StaticType>,
    ) -> Result<(), CompileError> {
        let expected = if scope.globals.contains(name) {
            scope.module_snapshot.get(name).cloned()
        } else {
            scope.declared.get(name).cloned()
        };
        if let Some(expected) = expected {
            let actual = self.infer(value, scope, narrow);
            if !compatible(&expected, &actual) {
                return Err(CompileError::TypeAssignment(format!(
                    "assignment type check failed: `{name}` expects {}, got {} \
                     (TS-47; {}: line {})",
                    render_static(&expected),
                    render_static(&actual),
                    self.filename,
                    target_span.line_start
                )));
            }
        }
        narrow.remove(name);
        Ok(())
    }

    /// **RHS 静态类型"尽力"推断**（`TS-47` ②）：只认一眼能判定的字面量与已标注名字，其余 `Any`。
    fn infer(
        &self,
        expression: &Expression,
        scope: &Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> StaticType {
        match expression {
            Expression::Int(_, _) | Expression::BigInt(_, _) => StaticType::Label(type_label("int")),
            Expression::Float(_, _) => StaticType::Label(type_label("float")),
            Expression::Str(_, _) => StaticType::Label(type_label("str")),
            Expression::Bytes(_, _) => StaticType::Label(type_label("bytes")),
            Expression::Constant(Constant::None, _) => StaticType::Label(type_label("NoneType")),
            Expression::Constant(Constant::Bool(_), _) => StaticType::Label(type_label("bool")),
            Expression::Name(name, _) => self.read_type(scope, narrow, name),
            // **`BC-63`／`TS-49`**：空安全操作符的类型规则（见 `infer_null_safe`）。
            Expression::NullCoalesce(left, right, _) => {
                self.infer_null_coalesce(left, right, scope, narrow)
            }
            Expression::SafeChain { receiver, rest, .. } => {
                let receiver_type = self.infer(receiver, scope, narrow);
                self.infer_safe_chain(&receiver_type, rest, scope, narrow)
            }
            // 空安全链的基底占位只在 `SafeChain::rest` 里出现（那里由 `infer_safe_chain` 直接带
            // 类型走，不经过 `infer`）⇒ 走到这里说明判不出，返回平台类型。
            Expression::ChainSubject(_) => StaticType::Any,
            // 调用／属性／下标／容器字面量／运算… ⇒ 判不出，交运行期（`TS-47` ②）。
            _ => StaticType::Any,
        }
    }

    /// **`a ?? b` 的结果类型**（`TS-49`）：
    ///
    /// - 任一侧是**平台类型** ⇒ **平台类型**（`TS-48`：静态判不了，交运行期兜）；
    /// - 左侧**非可空**（已知、不含 `None`）⇒ 结果就是左侧类型（`TS-49`／`TS-50`：
    ///   "收窄为 `T` 之后 `x ?? d` 在编译期即可判恒取左值"）；
    /// - 否则两侧都已知 ⇒ 取**相容上界**：把两边的 union 成员并起来去重（`int? ?? int ⇒ int?`、
    ///   `int ?? int ⇒ int`、`str ?? int ⇒ str | int`）。§13-9 的相容关系下并集是上界，
  ///   这是实现层能给出的**保守但可静态判定**的写法（不推断平台类型，`TS-16`）。
    fn infer_null_coalesce(
        &self,
        left: &Expression,
        right: &Expression,
        scope: &Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> StaticType {
        let left_type = self.infer(left, scope, narrow);
        let right_type = self.infer(right, scope, narrow);
        let (StaticType::Label(left_label), StaticType::Label(right_label)) =
            (&left_type, &right_type)
        else {
            return StaticType::Any;
        };
        if label_has_any(left_label) || label_has_any(right_label) {
            return StaticType::Any;
        }
        // 左侧非可空 ⇒ 恒取左值（结果沿用左侧的已知类型）
        if !label_has_none(left_label) {
            return StaticType::Label(left_label.clone());
        }
        StaticType::Label(join_labels(left_label, right_label))
    }

    /// **`receiver?.…` 的结果类型**（`TS-49`）：`receiver: A?` ⇒ 结果在"`A` 上的链类型"之上
    /// 加回可空（`B?`）；`receiver: A`（非可空）⇒ 结果**不可空**（`B`，静态即可断言不是 `None`）。
    fn infer_safe_chain(
        &self,
        receiver: &StaticType,
        rest: &Expression,
        scope: &Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> StaticType {
        let StaticType::Label(label) = receiver else {
            return StaticType::Any;
        };
        if label_has_any(label) {
            return StaticType::Any;
        }
        let Some(base) = narrowed_non_none(label) else {
            // 接收者只可能是 `None` ⇒ 结果就是 `None`（链上什么都不做）
            return StaticType::Label(type_label("NoneType"));
        };
        let inner = self.infer_chain_subject(rest, StaticType::Label(base), scope, narrow);
        if label_has_none(label) {
            make_nullable(inner)
        } else {
            inner
        }
    }

    /// 走 `SafeChain::rest`：基底类型由 `subject` 提供（[`Expression::ChainSubject`] 就是它）。
    fn infer_chain_subject(
        &self,
        expression: &Expression,
        subject: StaticType,
        scope: &Scope,
        narrow: &HashMap<String, StaticType>,
    ) -> StaticType {
        match expression {
            Expression::ChainSubject(_) => subject,
            Expression::Attribute(target, name, _) => {
                let target_type = self.infer_chain_subject(target, subject, scope, narrow);
                self.attribute_type(&target_type, name)
            }
            Expression::Subscript(container, _, _) => {
                let container_type = self.infer_chain_subject(container, subject, scope, narrow);
                element_type(&container_type)
            }
            // 链上再出现安全点（`a?.b?.c`）：参数是**内层接收者**的类型，结果照 `infer_safe_chain`
            Expression::SafeChain { receiver, rest, .. } => {
                let receiver_type = self.infer_chain_subject(receiver, subject, scope, narrow);
                self.infer_safe_chain(&receiver_type, rest, scope, narrow)
            }
            // 调用结果判不出（属性类型那一格只覆盖 `.名字`）
            Expression::Call { .. } => StaticType::Any,
            // 理论上不会出现（`rest` 只由后缀构成）；保守交运行期
            other => self.infer(other, scope, narrow),
        }
    }

    /// **属性访问的类型**（`TS-49` 的"已知类型"来源）：接收者的类名 ⇒ 类属性标注表里查。
    /// 内建类型／未标注属性／判不出的接收者 ⇒ 平台类型（`TS-48`）。
    fn attribute_type(&self, receiver: &StaticType, name: &str) -> StaticType {
        let StaticType::Label(label) = receiver else {
            return StaticType::Any;
        };
        let Some(base) = narrowed_non_none(label) else {
            return StaticType::Any;
        };
        let Some(class_name) = simple_type_name(&base) else {
            return StaticType::Any;
        };
        match self
            .class_attributes
            .get(class_name)
            .and_then(|attributes| attributes.get(name))
        {
            Some(ty) => ty.clone(),
            None => StaticType::Any,
        }
    }

    /// 读一个名字的静态类型：收窄优先，其次本作用域声明，最后模块表；都没有 ⇒ `Any`（`TS-48`）。
    fn read_type(
        &self,
        scope: &Scope,
        narrow: &HashMap<String, StaticType>,
        name: &str,
    ) -> StaticType {
        if scope.globals.contains(name) {
            return scope
                .module_snapshot
                .get(name)
                .cloned()
                .unwrap_or(StaticType::Any);
        }
        if let Some(ty) = narrow.get(name) {
            return ty.clone();
        }
        if let Some(ty) = scope.declared.get(name) {
            return ty.clone();
        }
        if scope.is_module {
            return StaticType::Any;
        }
        // 函数里赋过值但没标注 ⇒ 未标注局部（`TS-2`：禁止推断）⇒ 平台类型。
        if scope.assigned.contains(name) {
            return StaticType::Any;
        }
        scope
            .module_snapshot
            .get(name)
            .cloned()
            .unwrap_or(StaticType::Any)
    }
}

/// `if` 条件里的两种形态（`TS-50` 只认这两种）。
#[derive(Clone, Copy)]
enum IsNoneKind {
    /// `x is None`
    IsNone,
    /// `x is not None`
    IsNotNone,
}

fn type_label(name: &str) -> Constant {
    Constant::Type(name.to_owned())
}

/// 注解标签（`Type`／`Tuple`／`AnnUnion`…）⇒ 静态类型；`"Any"` 与判不出的形状 ⇒ 平台类型。
fn static_from_label(label: &Constant) -> StaticType {
    match label {
        Constant::Str(text) if text == "Any" => StaticType::Any,
        Constant::Type(_)
        | Constant::Tuple(_)
        | Constant::AnnUnion { .. }
        | Constant::AnnSubscript { .. } => StaticType::Label(label.clone()),
        // 前向引用（`AnnString`）、`...`、`AnnList`、`AnnAttribute` 等判不出 ⇒ 平台类型（`TS-48`）。
        _ => StaticType::Any,
    }
}

/// 变量／类属性标注的来源 ⇒ 静态类型：表达式通道（`.py` 与 `.pyawa` 的兼容注解）＝ 平台类型。
fn static_from_annotation(annotation: &AnnotationSource) -> StaticType {
    match annotation {
        AnnotationSource::Type(label) => static_from_label(label),
        AnnotationSource::Expression(_) => StaticType::Any,
    }
}

/// 标签（摊平 union 之后）里有没有 `"Any"` 成员 ⇒ 平台类型参与（`TS-48`／`TS-49`）。
fn label_has_any(label: &Constant) -> bool {
    let mut members = Vec::new();
    flatten_union(label, &mut members);
    members.iter().any(is_any)
}

/// 标签（摊平 union 之后）里有没有 `NoneType` ⇒ 可空。
fn label_has_none(label: &Constant) -> bool {
    let mut members = Vec::new();
    flatten_union(label, &mut members);
    members.iter().any(is_none_type)
}

/// 把类型**加上可空**（`TS-49`：`A?` 上的安全链结果 ⇒ `B?`）。平台类型保持平台类型（`TS-48`）。
fn make_nullable(ty: StaticType) -> StaticType {
    match ty {
        StaticType::Any => StaticType::Any,
        StaticType::Label(label) => {
            if label_has_none(&label) || label_has_any(&label) {
                StaticType::Label(label)
            } else {
                StaticType::Label(Constant::AnnUnion {
                    left: Box::new(label),
                    right: Box::new(type_label("NoneType")),
                })
            }
        }
    }
}

/// **`TS-49` 的"相容上界"**（实现口径）：把两边的 union 成员摊平、去重、按左侧顺序折回 union。
fn join_labels(left: &Constant, right: &Constant) -> Constant {
    let mut members = Vec::new();
    flatten_union(left, &mut members);
    flatten_union(right, &mut members);
    let mut unique: Vec<Constant> = Vec::new();
    for member in members {
        if !unique.contains(&member) {
            unique.push(member);
        }
    }
    debug_assert!(!unique.is_empty(), "标签至少有一个成员");
    build_union(unique)
}

/// **`?[i]`／`[i]` 的元素类型**（`TS-49` 的已知类型来源）：`list[T]?`／`tuple[T]` 的 `T`、
/// `str` 的元素是 `str`、`bytes` 的元素是 `int`；判不出 ⇒ 平台类型（`TS-48`）。
fn element_type(container: &StaticType) -> StaticType {
    let StaticType::Label(label) = container else {
        return StaticType::Any;
    };
    let Some(base) = narrowed_non_none(label) else {
        return StaticType::Any;
    };
    if label_has_any(&base) {
        return StaticType::Any;
    }
    match &base {
        // `list[int]` 在标签里是**二元组** `(list, int)`（`BC-24` 的浅层形态）
        Constant::Tuple(items) if items.len() == 2 => match simple_type_name(&items[0]) {
            Some("list") | Some("tuple") => static_from_label(&items[1]),
            _ => StaticType::Any,
        },
        Constant::AnnSubscript { base, arguments } => match simple_type_name(base) {
            Some("list") | Some("tuple") => arguments
                .first()
                .map(static_from_label)
                .unwrap_or(StaticType::Any),
            _ => StaticType::Any,
        },
        Constant::Type(name) => match name.as_str() {
            "str" => StaticType::Label(type_label("str")),
            "bytes" => StaticType::Label(type_label("int")),
            _ => StaticType::Any,
        },
        _ => StaticType::Any,
    }
}

fn is_any(label: &Constant) -> bool {
    matches!(label, Constant::Str(text) if text == "Any")
}

fn is_none_type(label: &Constant) -> bool {
    matches!(label, Constant::Type(name) if name == "NoneType")
}

/// 把 `AnnUnion` 摊平（保持书写顺序）。
fn flatten_union(label: &Constant, out: &mut Vec<Constant>) {
    match label {
        Constant::AnnUnion { left, right } => {
            flatten_union(left, out);
            flatten_union(right, out);
        }
        other => out.push(other.clone()),
    }
}

fn union_members(label: &Constant) -> Option<Vec<Constant>> {
    match label {
        Constant::AnnUnion { .. } => {
            let mut members = Vec::new();
            flatten_union(label, &mut members);
            Some(members)
        }
        _ => None,
    }
}

/// 把成员列表折回 `AnnUnion`（左结合）；单个成员就是它自己。
fn build_union(mut members: Vec<Constant>) -> Constant {
    let mut value = members.remove(0);
    for member in members {
        value = Constant::AnnUnion {
            left: Box::new(value),
            right: Box::new(member),
        };
    }
    value
}

/// 去掉 `NoneType` 后的标签；本来就不含 `None` ⇒ 原样；只有 `NoneType` ⇒ 不窄化（`None`）。
fn narrowed_non_none(label: &Constant) -> Option<Constant> {
    let mut members = Vec::new();
    flatten_union(label, &mut members);
    if members.iter().any(is_any) {
        return None;
    }
    let before = members.len();
    members.retain(|member| !is_none_type(member));
    if members.len() == before {
        return Some(label.clone());
    }
    if members.is_empty() {
        return None;
    }
    Some(build_union(members))
}

/// 收窄成 `NoneType`；标签本来就不含 `NoneType` ⇒ 不窄化（`None`）。
fn narrowed_to_none(label: &Constant) -> Option<Constant> {
    let mut members = Vec::new();
    flatten_union(label, &mut members);
    if members.iter().any(is_any) {
        return None;
    }
    members
        .iter()
        .any(is_none_type)
        .then(|| type_label("NoneType"))
}

/// 标签的**外层类型名**（`list[int]` ⇒ `list`）；判不出（前向引用／`Any`／`...`）⇒ `None`。
fn simple_type_name(label: &Constant) -> Option<&str> {
    match label {
        Constant::Type(name) => Some(name.as_str()),
        Constant::Tuple(items) => items.first().and_then(simple_type_name),
        Constant::AnnSubscript { base, .. } => simple_type_name(base),
        _ => None,
    }
}

/// **相容判定**（`§13-9` 的落地）：`Any` 双向相容；union 按成员；其余按内建子类型（`TS-29`／`TS-40`）。
fn compatible(expected: &StaticType, actual: &StaticType) -> bool {
    match (expected, actual) {
        (StaticType::Any, _) | (_, StaticType::Any) => true,
        (StaticType::Label(expected), StaticType::Label(actual)) => {
            label_compatible(expected, actual)
        }
    }
}

fn label_compatible(expected: &Constant, actual: &Constant) -> bool {
    if is_any(expected) || is_any(actual) {
        return true;
    }
    // 实际是 union：**每个**成员都要被接受（静态上不知道运行期是哪一个）——先摊平实际那一侧，
    // 再交给"期望 union 任一成员相容"的规则 ⇒ `int?` 赋给 `int?` 也必须通过（`TS-46`／`TS-49`
    // 的结果类型就是 union，这要求 union-vs-union 是对称可接受的）。
    if let Some(members) = union_members(actual) {
        return members
            .iter()
            .all(|member| label_compatible(expected, member));
    }
    // 期望是 union：任一成员相容即接受（`TS-46` 的 `T?` 就是这条）。
    if let Some(members) = union_members(expected) {
        return members
            .iter()
            .any(|member| label_compatible(member, actual));
    }
    let (Some(expected_name), Some(actual_name)) =
        (simple_type_name(expected), simple_type_name(actual))
    else {
        // 判不出的标签形状（前向引用等）⇒ 放行（`TS-48`）。
        return true;
    };
    if actual_name == expected_name {
        return true;
    }
    // 内建类型层次（`TS-41` 的同一份表）：`bool ⊂ int` 等。
    let (Some(actual_type), Some(_expected_type)) =
        (builtin_type(actual_name), builtin_type(expected_name))
    else {
        // 至少有一侧不是内建类型（用户类／前向引用）⇒ 静态判不出父子关系 ⇒ 放行（`TS-29`：不做结构类型）。
        return true;
    };
    actual_type.mro.contains(&expected_name)
}

fn render_static(ty: &StaticType) -> String {
    match ty {
        StaticType::Any => "Any".to_owned(),
        StaticType::Label(label) => render_label(label),
    }
}

fn render_label(label: &Constant) -> String {
    if is_any(label) {
        return "Any".to_owned();
    }
    if let Some(members) = union_members(label) {
        return members
            .iter()
            .map(render_label)
            .collect::<Vec<_>>()
            .join(" | ");
    }
    match label {
        // 报错消息里 `NoneType` 按源码写法渲染成 `None`（与 `TS-46` 的示例一致，更好读）。
        Constant::Type(name) if name == "NoneType" => "None".to_owned(),
        Constant::Type(name) => name.clone(),
        Constant::Tuple(items) => {
            let head = items
                .first()
                .map(render_label)
                .unwrap_or_else(|| "?".to_owned());
            let tail: Vec<String> = items[1..].iter().map(render_label).collect();
            format!("{head}[{}]", tail.join(", "))
        }
        Constant::AnnSubscript { base, arguments } => format!(
            "{}[{}]",
            render_label(base),
            arguments
                .iter()
                .map(render_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Constant::AnnAttribute { base, name } => format!("{}.{}", render_label(base), name),
        Constant::AnnString(text) => format!("\"{text}\""),
        Constant::Ellipsis => "...".to_owned(),
        other => format!("{other:?}"),
    }
}

/// 一个块里**被赋值**的名字（含分支体；不进嵌套 `def`／`class` 的体）。
fn assigned_names(statements: &[Statement]) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_assigned(statements, &mut out);
    out
}

fn collect_assigned(statements: &[Statement], out: &mut HashSet<String>) {
    for statement in statements {
        match statement {
            Statement::Assign { target, .. } | Statement::Annotation { name: target, .. } => {
                out.insert(target.clone());
            }
            Statement::NullCoalesceAssign { name, .. } => {
                out.insert(name.clone());
            }
            Statement::AssignChained { targets, .. } => {
                for target in targets {
                    collect_target_names(target, out);
                }
            }
            Statement::AssignTuple { targets, chain, .. } => {
                for (target, _) in targets {
                    collect_target_names(target, out);
                }
                for target in chain {
                    collect_target_names(target, out);
                }
            }
            Statement::AugAssign { target, .. } => {
                if let AugTarget::Name(name, _) = target {
                    out.insert(name.clone());
                }
            }
            Statement::For {
                target,
                tuple_targets,
                body,
                else_body,
                ..
            } => {
                out.insert(target.clone());
                for item in tuple_targets {
                    collect_for_target_names(item, out);
                }
                collect_assigned(body, out);
                collect_assigned(else_body, out);
            }
            Statement::While {
                body, else_body, ..
            } => {
                collect_assigned(body, out);
                collect_assigned(else_body, out);
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_assigned(then_body, out);
                collect_assigned(else_body, out);
            }
            Statement::With { items, body, .. } => {
                for (_, target) in items {
                    if let Some(target) = target {
                        collect_for_target_names(target, out);
                    }
                }
                collect_assigned(body, out);
            }
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_assigned(body, out);
                for handler in handlers {
                    if let Some(name) = &handler.name {
                        out.insert(name.clone());
                    }
                    collect_assigned(&handler.body, out);
                }
                collect_assigned(else_body, out);
                collect_assigned(finally_body, out);
            }
            Statement::Match { cases, .. } => {
                for case in cases {
                    collect_assigned(&case.body, out);
                }
            }
            Statement::Def { name, .. } | Statement::Class { name, .. } => {
                out.insert(name.clone());
            }
            Statement::Import { items, .. } => {
                for (module, alias) in items {
                    let bound = alias
                        .clone()
                        .unwrap_or_else(|| module.split('.').next().unwrap_or(module).to_owned());
                    out.insert(bound);
                }
            }
            Statement::ImportFrom { names, .. } => {
                for (name, alias) in names {
                    out.insert(alias.clone().unwrap_or_else(|| name.clone()));
                }
            }
            _ => {}
        }
    }
}

fn collect_target_names(target: &Expression, out: &mut HashSet<String>) {
    match target {
        Expression::Name(name, _) => {
            out.insert(name.clone());
        }
        Expression::TupleLiteral(items, _) | Expression::List(items, _) => {
            for item in items {
                collect_target_names(item, out);
            }
        }
        Expression::Starred(inner, _) => collect_target_names(inner, out),
        _ => {}
    }
}

fn collect_for_target_names(target: &ForTarget, out: &mut HashSet<String>) {
    match target {
        ForTarget::Name(name, _) => {
            out.insert(name.clone());
        }
        ForTarget::Group(items, _) => {
            for item in items {
                collect_for_target_names(item, out);
            }
        }
        ForTarget::Starred(inner, _) => collect_for_target_names(inner, out),
        ForTarget::Attribute(..) | ForTarget::Subscript(..) => {}
    }
}

/// 一个块里**被 `global` 声明的名字**（`global` 在分支里也照样对整函数生效 ⇒ 递归；不进嵌套
/// `def`／`class` 的体——那里的 `global` 属于它们自己）。
fn global_names(statements: &[Statement]) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_globals(statements, &mut out);
    out
}

fn collect_globals(statements: &[Statement], out: &mut HashSet<String>) {
    for statement in statements {
        match statement {
            Statement::Global(names, _) => {
                for name in names {
                    out.insert(name.clone());
                }
            }
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                collect_globals(then_body, out);
                collect_globals(else_body, out);
            }
            Statement::While {
                body, else_body, ..
            }
            | Statement::For {
                body, else_body, ..
            } => {
                collect_globals(body, out);
                collect_globals(else_body, out);
            }
            Statement::With { body, .. } => collect_globals(body, out),
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                collect_globals(body, out);
                for handler in handlers {
                    collect_globals(&handler.body, out);
                }
                collect_globals(else_body, out);
                collect_globals(finally_body, out);
            }
            Statement::Match { cases, .. } => {
                for case in cases {
                    collect_globals(&case.body, out);
                }
            }
            _ => {}
        }
    }
}

/// 清掉块里被赋值过的名字的收窄（块可能没执行／执行后值已变 ⇒ 收窄前提不再成立）。
fn clear_assigned(narrow: &mut HashMap<String, StaticType>, body: &[Statement]) {
    for name in assigned_names(body) {
        narrow.remove(&name);
    }
}

/// 清掉 `for`／`with` 目标名字的收窄（目标被重新绑定 ⇒ 旧收窄失效）。
fn clear_for_target(target: &ForTarget, narrow: &mut HashMap<String, StaticType>) {
    match target {
        ForTarget::Name(name, _) => {
            narrow.remove(name);
        }
        ForTarget::Group(items, _) => {
            for item in items {
                clear_for_target(item, narrow);
            }
        }
        ForTarget::Starred(inner, _) => clear_for_target(inner, narrow),
        ForTarget::Attribute(..) | ForTarget::Subscript(..) => {}
    }
}

/// 一个块里有没有给 `name` 赋值（收窄延续用；与 [`assigned_names`] 同口径）。
fn block_assigns(body: &[Statement], name: &str) -> bool {
    assigned_names(body).contains(name)
}
