//! **标注覆盖率与严格档位**（`SPEC-type-system.md` §10 的 `TS-23`／`TS-24`／`TS-25`；M5）。
//!
//! 钉住：①编译期算出（不跑脚本）②`TS-39` 的分列口径（位置参数／仅关键字／`*args`／
//! `**kwargs`／返回／变量）③模式与档位进报告④递归覆盖所有语句块⑤报告文本可导出。

use pyawa_core::compile::{annotation_coverage, AnnotationCoverage, CheckTier, Mode};

fn coverage(source: &str, mode: Mode) -> AnnotationCoverage {
    annotation_coverage(source, "probe.py", mode, CheckTier::Deep)
        .unwrap_or_else(|error| panic!("覆盖率报告应当可算：{error:?}"))
}

#[test]
fn counts_annotations_by_category() {
    let report = coverage(
        r#"
def f(a: int, b, *args: str, c: float, **kw: bytes) -> int:
    x: int = 1
    y = 2
    return a

class C:
    z: int

    def m(self, value):
        pass
"#,
        Mode::Extension,
    );
    // 位置参数：`f` 的 `a: int`+`b` 与 `C.m` 的 `self`+`value` ⇒ 1/4；
    // `*args`/`**kw` 各 1/1；返回 1/1（`m` 没有返回注解也计入分母）。
    assert_eq!((report.positional.annotated, report.positional.total), (1, 4));
    assert_eq!((report.kwonly.annotated, report.kwonly.total), (1, 1));
    assert_eq!((report.varargs.annotated, report.varargs.total), (1, 1));
    assert_eq!((report.varkw.annotated, report.varkw.total), (1, 1));
    // `f` 与 `C.m` 各一个返回位；`f` 有注解。
    assert_eq!((report.returns.annotated, report.returns.total), (1, 2));
    // 变量：`x: int = 1`（一格已标注）、`y = 2`（未标注）、类体 `z: int`（已标注）⇒ 2/3。
    assert_eq!((report.variables.annotated, report.variables.total), (2, 3));
    // 模式/档位进报告（`TS-39`）。
    assert_eq!(report.mode, Mode::Extension);
    assert_eq!(report.tier, CheckTier::Deep);
    // 总覆盖率：全部合并（2+1+1+1+2+2 +1+1+1+1+1+3 → 见各类别）。
    let overall = report.overall();
    // 1+1+1+1+1+2 = 7；4+1+1+1+2+3 = 12。
    assert_eq!(overall.annotated, 7);
    assert_eq!(overall.total, 12);
}

#[test]
fn counts_annotations_in_pure_python_mode_too() {
    // `TS-5`：标注在两种模式下都照常保留；报告反映**写了多少**，模式由宿主自行判断。
    let report = coverage("def g(x: int) -> int:\n    return x\n", Mode::PurePython);
    assert_eq!(report.mode, Mode::PurePython);
    assert_eq!((report.positional.annotated, report.positional.total), (1, 1));
    assert_eq!((report.returns.annotated, report.returns.total), (1, 1));
}

#[test]
fn report_text_is_parseable_and_exportable() {
    let report = coverage("def h(x: int, y) -> int:\n    return x\n", Mode::Extension);
    let text = report.to_report();
    for line in [
        "annotation-coverage v1",
        "mode: pyawa",
        "tier: deep",
        "positional: 1/2 (50%)",
        "returns: 1/1 (100%)",
        "overall:",
    ] {
        assert!(text.contains(line), "报告缺行 {line:?}：\n{text}");
    }
    // 空类别给 `-/- (-)`，不是 0%。
    let empty = coverage("pass\n", Mode::PurePython);
    assert!(empty.to_report().contains("positional: -/- (-)"));
    assert_eq!(empty.overall().ratio(), None);
}

#[test]
fn syntax_errors_are_reported_as_compile_errors() {
    assert!(matches!(
        annotation_coverage("def f(x: int:\n", "probe.py", Mode::Extension, CheckTier::Shallow),
        Err(pyawa_core::compile::CompileError::Syntax(_))
    ));
}
