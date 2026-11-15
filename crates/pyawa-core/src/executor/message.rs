//! **`message` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `message_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。



pub(crate) fn message_too_many(
    name: &str,
    accepted: usize,
    required: usize,
    given: usize,
    keyword_only_given: usize,
) -> String {
    // 动词也随**实参个数**变：`… but 1 was given`（实测；夹具 `fixture-argbind-3.14.json`
    // 的 `none_positional` 用例抓出来的）。名词则随**形参个数**变（`1 positional argument`）。
    //
    // **带关键字实参那一档**（round 63，`test_extcall` 实测）：多出来的位置实参之外还给了
    // `n` 个仅关键字实参时，参照的尾巴换成
    // `but 1 positional argument (and 1 keyword-only argument) were given` ✓
    // （名词随**各自个数**变，动词固定 `were` ✓；尾巴里的 `positional` 一词也会重新出现 ✓）。
    let verb = if given == 1 { "was" } else { "were" };
    let noun = if accepted == 1 {
        "argument"
    } else {
        "arguments"
    };
    let expected = if required < accepted {
        format!("takes from {required} to {accepted} positional {noun}")
    } else if accepted == 1 {
        "takes 1 positional argument".to_owned()
    } else {
        format!("takes {accepted} positional {noun}")
    };
    if keyword_only_given > 0 {
        let positional_noun = if given == 1 { "argument" } else { "arguments" };
        let keyword_noun = if keyword_only_given == 1 {
            "argument"
        } else {
            "arguments"
        };
        return format!(
            "{name}() {expected} but {given} positional {positional_noun} (and {keyword_only_given} keyword-only {keyword_noun}) were given"
        );
    }
    format!("{name}() {expected} but {given} {verb} given")
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
