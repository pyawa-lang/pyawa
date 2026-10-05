//! **`OM-40`／`OM-20` ② 的机械化核对**：`py_object!` 里**持引用**的字段，必须出现在该类型的
//! `traverse`／`clear` 里。
//!
//! 由来：第 55 轮给 `FunctionObject` 加 `annotate` 字段时漏了这两处 ⇒ GC 既看不到也不释放它
//! （一处真实的泄漏，第 63 轮才发现）。这类错误靠人眼看不住，故扫源码把它钉死。
//!
//! **扫描范围**：`crates/pyawa-core/src/builtin_objects.rs`（`py_object!` 的集中地）。
//! 判据刻意保守：只看**声明里就写得出引用**的字段类型（`NonNull<Header>`／
//! `Option<NonNull<Header>>`／`Vec<NonNull<Header>>`／`RefCell<Option<NonNull<Header>>>`）；
//! 藏在**枚举**里的引用（如 `ItStateObject` 的 `kind`）静态看不出来 ⇒ 不在这里管。

/// 被扫描的源码：**编译期逐个 `include_str!`** ✓（路径错就在编译期报 ✓）。
///
/// **拆分后要从这里加一行** ✓（第 126 轮 `gc_field_coverage` 曾因 `deque_traverse` 搬到新文件而红 ✗）：
/// `builtin/` 目录下**每新增一族**都要加进来 ✓，否则那一族的 `traverse`／`clear` 就**不在扫描面上** ✗。
const SOURCE: &str = concat!(
    include_str!("../src/builtin_objects.rs"),
    include_str!("../src/builtin/str.rs"),
    include_str!("../src/builtin/bytes.rs"),
    include_str!("../src/builtin/dict.rs"),
    include_str!("../src/builtin/deque.rs"),
    include_str!("../src/builtin/list.rs"),
    include_str!("../src/builtin/object.rs"),
    include_str!("../src/builtin/context.rs"),
    include_str!("../src/builtin/generator.rs"),
    include_str!("../src/builtin/property.rs"),
    include_str!("../src/builtin/set.rs"),
);

/// 一个 `py_object!` 结构体：名字 ＋ `(字段名, 声明里的类型文本)`。
struct Object {
    name: String,
    fields: Vec<(String, String)>,
}

/// 把源码里所有 `py_object! { … }` 块里的 `pub struct X { … }` 解析出来。
fn objects() -> Vec<Object> {
    let mut found = Vec::new();
    let mut rest = SOURCE;
    while let Some(start) = rest.find("py_object! {") {
        rest = &rest[start + "py_object! {".len()..];
        let Some(end) = rest.find("\n}") else { break };
        let block = &rest[..end];
        rest = &rest[end..];
        // 结构体名：`pub struct X {`
        let Some(struct_at) = block.find("pub struct ") else { continue };
        let after = &block[struct_at + "pub struct ".len()..];
        let Some(brace) = after.find('{') else { continue };
        let name = after[..brace].trim().to_owned();
        let Some(close) = after.find("\n    }") else { continue };
        let body = &after[brace + 1..close];
        let fields = body
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.starts_with("///") || line.starts_with("//") || !line.ends_with(',') {
                    return None;
                }
                let (name, kind) = line.split_once(':')?;
                Some((name.trim().to_owned(), kind.trim().trim_end_matches(',').to_owned()))
            })
            .collect();
        found.push(Object { name, fields });
    }
    found
}

/// 这个字段类型"声明里就写得出的引用"？（只看**声明里的类型文本**，不全文搜）
fn holds_reference(kind: &str) -> bool {
    kind.contains("NonNull<Header>")
}

/// 取 `impl <Name> { … }` 块的正文（找不到给空串）。
fn impl_body(name: &str) -> String {
    let needle = format!("impl {name} {{");
    let Some(at) = SOURCE.find(&needle) else {
        return String::new();
    };
    let rest = &SOURCE[at..];
    let end = rest[1..].find("\n}\n").map(|offset| offset + 1).unwrap_or(rest.len());
    rest[..end].to_owned()
}

/// 去掉行注释（`//…`）——不然"注释里提一句"会被当成覆盖。
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 取一个自由函数（`unsafe fn f(` 或 `fn f(`）的正文（找不到给空串）。
fn function_body(name: &str) -> String {
    for prefix in ["unsafe fn ", "fn "] {
        let needle = format!("{prefix}{name}(");
        if let Some(at) = SOURCE.find(&needle) {
            let rest = &SOURCE[at..];
            let end = rest[1..].find("\n}\n").map(|offset| offset + 1).unwrap_or(rest.len());
            return rest[..end].to_owned();
        }
    }
    String::new()
}

/// **`T-CX-12`**（`CX-21`／`OM-12`）的检查主体：扫出"持引用的字段没被
/// `traverse`／`clear` 覆盖"的项。抽成函数是为了让**注入用例**能复用同一份判据
/// （"新增字段漏项必须红"必须证明得了，不能只靠"恰好没漏"）。
fn scan(objects: &[Object]) -> (usize, Vec<String>) {
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for object in objects {
        let reference_fields: Vec<&String> = object
            .fields
            .iter()
            .filter(|(_, kind)| holds_reference(kind))
            .map(|(name, _)| name)
            .collect();
        if reference_fields.is_empty() {
            continue;
        }
        checked += 1;
        let slots = impl_body(&object.name);
        assert!(
            !slots.is_empty(),
            "{}：里持引用的字段，必须实现 `slots()`",
            object.name
        );
        for (slot, keyword) in [("traverse", "with_traverse"), ("clear", "with_clear")] {
            let Some(at) = slots.find(keyword) else {
                failures.push(format!("{}：持引用却没有 `{slot}` 槽", object.name));
                continue;
            };
            // `.with_traverse(foo)` ⇒ 取 foo
            let after = &slots[at + keyword.len()..];
            let function = after
                .trim_start_matches(['(', ' '])
                .split([')', ','])
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            let body = strip_comments(&function_body(&function));
            assert!(
                !body.trim().is_empty(),
                "{}：`{slot}` 指向的 `{function}` 找不到",
                object.name
            );
            for field in &reference_fields {
                // traverse 走**访问器**（`object.<字段>(`）；clear 的写法多样
                // （`set_<字段>(None)`、`mem::take(&mut object.<字段>)`…）⇒ 只要求字段名
                // 以标识符**出现**（注释已去掉）
                let covered = body.contains(&format!("object.{field}("))
                    || (slot == "clear" && body.contains(field.as_str()));
                if !covered {
                    failures.push(format!(
                        "{}：`{function}`（{slot}）没覆盖字段 `{field}`",
                        object.name
                    ));
                }
            }
        }
    }
    (checked, failures)
}

#[test]
fn reference_fields_are_covered_by_traverse_and_clear() {
    let (checked, failures) = scan(&objects());
    assert!(
        checked >= 5,
        "至少该扫到几个持引用的类型，实际 {checked} 个（解析器是不是跟源码格式脱节了？）"
    );
    assert!(
        failures.is_empty(),
        "OM-40／OM-20 ②（`CX-21`／`T-CX-12`）：持引用的字段必须被 traverse／clear 覆盖：\n{}",
        failures.join("\n")
    );
}

/// **注入用例**（`T-CX-12` 的"新增字段漏项必须红"那一半）：把一个**真实存在**的持引用字段
/// 改名成一个源码里不可能出现的名字 ⇒ 扫描必须报出它、且报的是那个名字。
/// 这样"以后有人加了持引用字段却忘了 `traverse`／`clear`"这件事，会在闸门上**红**。
#[path = "fixtures/constructors.rs"]
mod constructors_fixture;

/// `OM-11` 扩的**材料**就位性检查：夹具是从参照实测导出的 12 条（`tools/gen_constructors_fixture.py`）。
/// 实现逐类型落地时，用它逐条对拍（类名 ＋ 消息）。此处只保证"材料齐且形状对"。
#[test]
fn constructor_failure_fixture_is_complete() {
    assert_eq!(constructors_fixture::CONSTRUCTOR_FAILURES.len(), 12, "参照实测是 12 条");
    for (slot, source, kind, message) in constructors_fixture::CONSTRUCTOR_FAILURES {
        assert!(!slot.is_empty() && !source.is_empty(), "槽与写法不能为空：{slot} / {source}");
        assert!(matches!(*kind, "TypeError" | "ValueError"), "异常类只该是这两类：{kind}");
        assert!(!message.is_empty(), "{source} 的消息不能为空");
    }
}

#[test]
fn t_cx_12_injection_makes_the_check_red() {
    let mut injected = objects();
    let victim = injected
        .iter_mut()
        .find(|object| object.fields.iter().any(|(_, kind)| holds_reference(kind)))
        .expect("至少要有一个持引用的类型可供注入");
    let (name, kind) = victim
        .fields
        .iter()
        .find(|(_, kind)| holds_reference(kind))
        .map(|(name, kind)| (name.clone(), kind.clone()))
        .expect("该类型应当有持引用字段");
    let fake = format!("{name}_injected_missing_field");
    for (field, _) in victim.fields.iter_mut() {
        if *field == name {
            *field = fake.clone();
        }
    }
    let _ = kind;
    let (_, failures) = scan(&injected);
    assert!(
        failures.iter().any(|line| line.contains(&fake)),
        "注入 `{fake}` 之后检查**必须**变红并点名它，实际 failures={failures:?}"
    );
}
