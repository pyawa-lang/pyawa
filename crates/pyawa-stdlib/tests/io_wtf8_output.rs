//! `_io` 文本流输出面（P3-38 S3b-1，task-61）的契约测试。
//!
//! 写端字节面（`print`／`sys.stdout.write` 写 WTF-8）由 CLI 探针做 **xxd 逐字节**验收；
//! 这里钉"对象面"：`sys.stdout`／`sys.stderr` 的文本层属性照参照 3.14.4 补上，且
//! `write`／`writeln`／`flush` 三个方法是逐对象可调用。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};
use pyawa_stdlib::sys_module;

fn attribute(instance: &Instance, object: NonNull<Header>, name: &str) -> NonNull<Header> {
    instance
        .attribute_optional_of(object, name)
        .unwrap_or_else(|_| panic!("读 `{name}` 时抛异常"))
        .unwrap_or_else(|| panic!("`{name}` 必须存在"))
}

fn text(instance: &Instance, object: NonNull<Header>, name: &str) -> String {
    let value = attribute(instance, object, name);
    instance
        .text_of(value)
        .unwrap_or_else(|| panic!("`{name}` 必须是 str"))
        .to_owned()
}

#[test]
fn stdout_and_stderr_text_attributes_match_the_reference() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let stdout = instance.dict_get(namespace, "stdout").expect("sys.stdout");
    let stderr = instance.dict_get(namespace, "stderr").expect("sys.stderr");
    for (stream, expected_name) in [(stdout, "<stdout>"), (stderr, "<stderr>")] {
        assert_eq!(text(&instance, stream, "encoding"), "utf-8");
        // 本层写端总是写 WTF-8 原始字节（不 lossy）⇒ 如实报 surrogateescape
        //（参照**管道**下是 stdout=strict／stderr=backslashreplace，见 task-61 报告）。
        assert_eq!(text(&instance, stream, "errors"), "surrogateescape");
        assert_eq!(text(&instance, stream, "name"), expected_name);
        assert_eq!(text(&instance, stream, "mode"), "w");
        let line_buffering = attribute(&instance, stream, "line_buffering");
        assert_eq!(instance.bool_value(line_buffering), Some(false));
        // 三个方法逐对象可调用（属性通道里都是绑定方法对象）。
        for method in ["write", "writeln", "flush"] {
            let _found = instance
                .attribute_optional_of(stream, method)
                .unwrap_or_else(|_| panic!("读 `{method}` 时抛异常"))
                .unwrap_or_else(|| panic!("文本流必须有 `{method}`"));
        }
        // **`sys.stdout.buffer`／`sys.stderr.buffer`**（P3-38 S3b-3 最小面）：存在且带
        // `write`／`flush`；类型名 `_io.BufferedWriter`（参照是 `BufferedWriter`，如实偏差）。
        let buffer = instance
            .attribute_optional_of(stream, "buffer")
            .unwrap_or_else(|_| panic!("读 `buffer` 时抛异常"))
            .expect("文本流必须有 `buffer`");
        assert_eq!(instance.type_name(instance.type_of(buffer)), "_io.BufferedWriter");
        for method in ["write", "flush"] {
            let _found = instance
                .attribute_optional_of(buffer, method)
                .unwrap_or_else(|_| panic!("读 buffer.`{method}` 时抛异常"))
                .unwrap_or_else(|| panic!("buffer 必须有 `{method}`"));
        }
    }
}
