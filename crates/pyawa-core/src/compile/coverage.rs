//! **标注覆盖率与严格档位**（`SPEC-type-system.md` §10 的 `TS-23`／`TS-24`／`TS-25`；
//! M5，2026-11-14）。
//!
//! 报告**只由编译期信息算出**（`TS-24`：禁止依赖运行时统计）：遍历解析后的 AST，按
//! `TS-39` 的分列口径统计「位置参数／仅关键字／`*args`／`**kwargs`／返回／变量」的
//! **已标注／总数**，并带上**严格档位**（`CheckTier`）与**模式**（`IM-1`）。
//!
//! 格式与档位命名属**实现细节**（`TS-25`；刻意不进规格）：本实现给 [`AnnotationCoverage::to_report`]
//! 的稳定文本形态，宿主可按行解析，也可据此决定是否加载脚本（`DESIGN.md` §6 的产品含义）。
//!
//! **产物内嵌／独立导出**（`TS-38` 的"权威数据在 `.pyac` 内"、`TS-39` 的"可导出为独立文件"）
//! 是**工具链步骤**：本模块先给出可计算的权威数据来源；把它写进 `.pyac` 需改 `IM-18`／`IM-19`
//! 的产物要素与黄金字节（见 `PLAN` §12.9 的 M5 边界登记），运行期**不读**任何伴随文件
//! （`TS-38` 的"沙箱下可能只读"约束保持）。

use super::*;

/// 一类标注位的计数（`TS-39` 的"分列"）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnnotationTally {
    /// 该类的总位数（例如形参个数）。
    pub total: usize,
    /// 其中**写了注解**的个数。
    pub annotated: usize,
}

impl AnnotationTally {
    /// 覆盖率（`total == 0` ⇒ `None`：没有该类别，不是 0%）。
    pub fn ratio(&self) -> Option<f64> {
        if self.total == 0 {
            None
        } else {
            Some(self.annotated as f64 / self.total as f64)
        }
    }

    /// **乘 100 的整数百分比**（`total == 0` ⇒ `None`；报告文本用，避免浮点格式漂移）。
    pub fn percent(&self) -> Option<usize> {
        self.ratio().map(|value| (value * 100.0).round() as usize)
    }
}

/// 一次编译的**标注覆盖率报告**（`TS-23`；编译期算出，`TS-24`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnnotationCoverage {
    /// 编译模式（`IM-1`；`TS-39` 要求报告含模式）。
    pub mode: Mode,
    /// 严格档位（`TS-31`：浅层／深层）。
    pub tier: CheckTier,
    /// 位置参数（含仅位置参数）。
    pub positional: AnnotationTally,
    /// 仅关键字参数。
    pub kwonly: AnnotationTally,
    /// `*args`（有则 1 位）。
    pub varargs: AnnotationTally,
    /// `**kwargs`（有则 1 位）。
    pub varkw: AnnotationTally,
    /// 返回注解（每个有返回值的 `def` 记 1 位）。
    pub returns: AnnotationTally,
    /// 变量标注（`x: T` 声明记"已标注"；对名字的普通赋值 `x = …` 记"未标注"）。
    pub variables: AnnotationTally,
}

impl AnnotationCoverage {
    /// **总覆盖率**（所有类别合并；空模块 ⇒ `None`）。
    pub fn overall(&self) -> AnnotationTally {
        let mut total = AnnotationTally::default();
        for tally in [
            self.positional,
            self.kwonly,
            self.varargs,
            self.varkw,
            self.returns,
            self.variables,
        ] {
            total.total += tally.total;
            total.annotated += tally.annotated;
        }
        total
    }

    /// **报告文本**（实现细节格式，`TS-25`；宿主可逐行解析）。
    pub fn to_report(&self) -> String {
        let mode = match self.mode {
            Mode::PurePython => "python",
            Mode::Extension => "pyawa",
        };
        let tier = match self.tier {
            CheckTier::Shallow => "shallow",
            CheckTier::Deep => "deep",
        };
        let line = |name: &str, tally: AnnotationTally| {
            match tally.percent() {
                Some(percent) => format!(
                    "{name}: {}/{} ({percent}%)",
                    tally.annotated, tally.total
                ),
                None => format!("{name}: -/- (-)"),
            }
        };
        let mut out = String::new();
        out.push_str("annotation-coverage v1\n");
        out.push_str(&format!("mode: {mode}\n"));
        out.push_str(&format!("tier: {tier}\n"));
        out.push_str(&line("positional", self.positional));
        out.push('\n');
        out.push_str(&line("kwonly", self.kwonly));
        out.push('\n');
        out.push_str(&line("varargs", self.varargs));
        out.push('\n');
        out.push_str(&line("varkw", self.varkw));
        out.push('\n');
        out.push_str(&line("returns", self.returns));
        out.push('\n');
        out.push_str(&line("variables", self.variables));
        out.push('\n');
        out.push_str(&line("overall", self.overall()));
        out.push('\n');
        out
    }
}

/// 收集一段语句序列里的标注计数（递归进所有语句块）。
pub(crate) fn collect(
    statements: &[Statement],
    mode: Mode,
    tier: CheckTier,
) -> AnnotationCoverage {
    let mut coverage = AnnotationCoverage {
        mode,
        tier,
        positional: AnnotationTally::default(),
        kwonly: AnnotationTally::default(),
        varargs: AnnotationTally::default(),
        varkw: AnnotationTally::default(),
        returns: AnnotationTally::default(),
        variables: AnnotationTally::default(),
    };
    walk(statements, &mut coverage);
    coverage
}

fn walk(statements: &[Statement], coverage: &mut AnnotationCoverage) {
    let mut index = 0;
    while index < statements.len() {
        let statement = &statements[index];
        // `x: T = v` 在 AST 里是 **`Annotation` ＋ `Assign` 两条**（注解那半与赋值那半分开）⇒
        // 变量计数不能算两格：只有"紧跟同名赋值"的那种 `Annotation` 吞掉下一条（一格已标注）。
        let annotated_with_value = matches!(
            statement,
            Statement::Annotation { name, .. }
                if matches!(statements.get(index + 1), Some(Statement::Assign { target, .. }) if target == name)
        );
        index += 1;
        match statement {
            Statement::Def {
                parameters,
                kwonly,
                varargs,
                varkw,
                varargs_annotation,
                varkw_annotation,
                returns,
                body,
                ..
            } => {
                coverage.positional.total += parameters.len();
                coverage.positional.annotated +=
                    parameters.iter().filter(|param| param.annotation.is_some()).count();
                coverage.kwonly.total += kwonly.len();
                coverage.kwonly.annotated +=
                    kwonly.iter().filter(|param| param.annotation.is_some()).count();
                if varargs.is_some() {
                    coverage.varargs.total += 1;
                    if varargs_annotation.is_some() {
                        coverage.varargs.annotated += 1;
                    }
                }
                if varkw.is_some() {
                    coverage.varkw.total += 1;
                    if varkw_annotation.is_some() {
                        coverage.varkw.annotated += 1;
                    }
                }
                coverage.returns.total += 1;
                if returns.is_some() {
                    coverage.returns.annotated += 1;
                }
                walk(body, coverage);
            }
            Statement::Class { body, .. } => walk(body, coverage),
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                walk(then_body, coverage);
                walk(else_body, coverage);
            }
            Statement::For { body, else_body, .. } | Statement::While { body, else_body, .. } => {
                walk(body, coverage);
                walk(else_body, coverage);
            }
            Statement::With { body, .. } => walk(body, coverage),
            Statement::Try {
                body,
                handlers,
                else_body,
                finally_body,
                ..
            } => {
                walk(body, coverage);
                for handler in handlers {
                    walk(&handler.body, coverage);
                }
                walk(else_body, coverage);
                walk(finally_body, coverage);
            }
            Statement::Match { cases, .. } => {
                for case in cases {
                    walk(&case.body, coverage);
                }
            }
            Statement::Annotation { .. } => {
                coverage.variables.total += 1;
                coverage.variables.annotated += 1;
                // `x: T = v`：赋值那半已由本格代表（不再算"未标注变量"）。
                if annotated_with_value {
                    index += 1;
                }
            }
            Statement::Assign { .. } => coverage.variables.total += 1,
            Statement::TypeAlias { .. }
            | Statement::AssignChained { .. }
            | Statement::AssignTuple { .. }
            | Statement::Delete { .. }
            | Statement::Return(..)
            | Statement::Assert { .. }
            | Statement::NonLocal(..)
            | Statement::Yield(..)
            | Statement::YieldFrom(..)
            | Statement::Global(..)
            | Statement::Expression(..)
            | Statement::Raise { .. }
            | Statement::Pass(..)
            | Statement::Import { .. }
            | Statement::ImportFrom { .. }
            | Statement::Break(..)
            | Statement::Continue(..)
            | Statement::AugAssign { .. }
            | Statement::AssignSubscript { .. }
            | Statement::AssignAttr { .. }
            | Statement::NullCoalesceAssign { .. } => {}
        }
    }
}
