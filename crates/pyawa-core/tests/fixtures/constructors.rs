//! 构造器失败消息夹具 —— 由 `tools/gen_constructors_fixture.py` 从**参照实测**导出。
//!
//! 供 `OM-11` 扩（`NewFn` 槽必须能表达失败）的实现用来逐条对拍：
//! 槽位报的 `Err` 转成 Python 异常后，类名与消息都要与这里一致。

/// `(槽（哪个 `*_new`）, 调用写法, 期望异常类, 期望消息)`。
pub static CONSTRUCTOR_FAILURES: &[(&str, &str, &str, &str)] = &[
    ("object()", "object(1)", "TypeError", "object() takes no arguments"),
    ("int()", "int('a')", "ValueError", "invalid literal for int() with base 10: 'a'"),
    ("int()", "int([])", "TypeError", "int() argument must be a string, a bytes-like object or a real number, not 'list'"),
    ("bool()", "bool(1, 2)", "TypeError", "bool expected at most 1 argument, got 2"),
    ("float()", "float('x')", "ValueError", "could not convert string to float: 'x'"),
    ("float()", "float([])", "TypeError", "float() argument must be a string or a real number, not 'list'"),
    ("str()", "str(1, 2, 3)", "TypeError", "str() argument 'encoding' must be str, not int"),
    ("list()", "list(5)", "TypeError", "'int' object is not iterable"),
    ("tuple()", "tuple(5)", "TypeError", "'int' object is not iterable"),
    ("dict()", "dict(5)", "TypeError", "'int' object is not iterable"),
    ("set()", "set(5)", "TypeError", "'int' object is not iterable"),
    ("exception()", "ValueError(a=1)", "TypeError", "ValueError() takes no keyword arguments"),
];
