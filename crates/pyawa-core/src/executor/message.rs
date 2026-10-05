//! **`message` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `message_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。



pub(crate) fn message_too_many(name: &str, accepted: usize, required: usize, given: usize) -> String {
    // 动词也随**实参个数**变：`… but 1 was given`（实测；夹具 `fixture-argbind-3.14.json`
    // 的 `none_positional` 用例抓出来的）。名词则随**形参个数**变（`1 positional argument`）。
    let verb = if given == 1 { "was" } else { "were" };
    let noun = if accepted == 1 {
        "argument"
    } else {
        "arguments"
    };
    if required < accepted {
        format!(
            "{name}() takes from {required} to {accepted} positional {noun} but {given} {verb} given"
        )
    } else if accepted == 1 {
        format!("{name}() takes 1 positional argument but {given} {verb} given")
    } else {
        format!("{name}() takes {accepted} positional {noun} but {given} {verb} given")
    }
}

pub(crate) fn message_missing(name: &str, missing: &[String], keyword_only: bool) -> String {
    let kind = if keyword_only {
        "keyword-only"
    } else {
        "positional"
    };
    if missing.len() == 1 {
        return format!(
            "{name}() missing 1 required {kind} argument: '{}'",
            missing[0]
        );
    }
    let quoted: Vec<String> = missing.iter().map(|item| format!("'{item}'")).collect();
    let head = quoted[..quoted.len() - 1].join(", ");
    let last = quoted.last().cloned().unwrap_or_default();
    // 实测：两个是 `'a' and 'b'`（无逗号），三个及以上是 `'a', 'b', and 'c'`（有逗号）
    let conjunction = if quoted.len() == 2 { " and " } else { ", and " };
    format!(
        "{name}() missing {} required {kind} arguments: {head}{conjunction}{last}",
        missing.len()
    )
}

pub(crate) fn message_duplicate(name: &str, argument: &str) -> String {
    format!("{name}() got multiple values for argument '{argument}'")
}

pub(crate) fn message_unexpected_keyword(name: &str, argument: &str) -> String {
    format!("{name}() got an unexpected keyword argument '{argument}'")
}

pub(crate) fn message_positional_only(name: &str, arguments: &[String]) -> String {
    format!(
        "{name}() got some positional-only arguments passed as keyword arguments: '{}'",
        arguments.join(", ")
    )
}
