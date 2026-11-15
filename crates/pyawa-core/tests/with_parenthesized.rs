//! **`with (` 的两种形态**（round 268；`MS-19`：合法 Python 被拒必须修 ✗）：
//!
//! * **括号包裹的 with 项列表**（3.10+ 语法）：`with (a as x, b):` ✓；
//! * **普通括号表达式**再接属性／调用：`with (path / "x").open("wb") as f:` ✓
//!   —— 先前一律当项列表 ⇒ 报"这里要冒号，实际 Some(Dot)" ✗（`pathlib` 的日常写法）。
//!
//! 本文件只钉**解析**（可编过）；运行期行为在 `crates/pyawa-runtime/tests/io_pathlib_face.rs`
//! 的对拍与独立用例里（`with` 进入顺序／`as` 绑定）。

use pyawa_core::compile::{compile, CheckTier, Mode};

fn parses(source: &str) -> bool {
    compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0).is_ok()
}

#[test]
fn parenthesized_expression_followed_by_attribute_parses() {
    assert!(parses("with (a / b).open(\"wb\") as f:\n    pass\n"));
    assert!(parses("with (a).open() as f:\n    pass\n"));
    assert!(parses("with (a / b).open() as f, c:\n    pass\n"));
}

#[test]
fn parenthesized_with_item_list_parses() {
    assert!(parses("with (a as x, b):\n    pass\n"));
    assert!(parses("with (\n    a as x,\n    b as y,\n):\n    pass\n"));
    // 括号里只有逗号（无 `as`）⇒ 项列表（参照会**依次进入**两个上下文 ✓）。
    assert!(parses("with (a, b):\n    pass\n"));
}

#[test]
fn async_with_uses_the_same_disambiguation() {
    assert!(parses(
        "async def f():\n    async with (a / b).open() as g:\n        pass\n"
    ));
    assert!(parses(
        "async def f():\n    async with (a as x, b):\n        pass\n"
    ));
}

#[test]
fn tuple_expression_item_still_parses() {
    // `(a, b), c:` ⇒ 第一项是**元组表达式**（参照解析如此；运行期元组没有 `__enter__` ⇒
    // AttributeError ✓ —— 这里只钉"解析不吃错"）。
    assert!(parses("with (a, b), c:\n    pass\n"));
}
