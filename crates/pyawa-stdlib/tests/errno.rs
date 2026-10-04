//! `errno` 模块的契约测试（`CM-4` 的 Python 面 ＋ `CM-19`／`CM-20`）。
//!
//! 这里用**本测试自己的**一张小表当"宿主平台注入值"（不是把某平台的数字写进实现——
//! 实现里一个数字都没有，映射按名字走）。真表由 `pyawa-runtime` 注入，见那边的用例。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, IntObject, StrObject};
use pyawa_stdlib::errno_module::{self, build, oserror_class_for, oserror_class_for_value};

/// 测试用的"注入常量"（只为覆盖形状：别名、普通常量各一）。
const INJECTED: &[(&str, i64)] = &[
    ("EACCES", 13),
    ("EAGAIN", 11),
    ("ENOENT", 2),
    ("EWOULDBLOCK", 11),
];

fn int_of(object: NonNull<Header>) -> i64 {
    // SAFETY: 调用方保证是整数对象。
    unsafe { &*object.as_ptr().cast::<IntObject>() }.value.to_i64().expect("平台常量是小整数")
}

fn text_of(object: NonNull<Header>) -> String {
    // SAFETY: 调用方保证是字符串对象。
    unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned()
}

#[test]
fn the_module_exposes_injected_constants_and_errorcode() {
    let instance = Instance::new();
    let namespace = build(&instance, INJECTED);

    // 常量：按注入值给整数
    let eacces = instance.dict_get(namespace, "EACCES").expect("EACCES 在模块里");
    assert_eq!(int_of(eacces), 13);
    // 别名各自一个名字、同一个数字
    let again = instance.dict_get(namespace, "EAGAIN").expect("EAGAIN 在模块里");
    let would_block = instance
        .dict_get(namespace, "EWOULDBLOCK")
        .expect("EWOULDBLOCK 在模块里");
    assert_eq!(int_of(again), int_of(would_block), "别名指向同一个数字");

    // `__name__`／`__doc__`
    let name = instance.dict_get(namespace, "__name__").expect("__name__");
    assert_eq!(text_of(name), errno_module::NAME);
    let doc = instance.dict_get(namespace, "__doc__").expect("__doc__");
    assert_eq!(text_of(doc), errno_module::DOC);
}

#[test]
fn errorcode_uses_integer_keys() {
    let instance = Instance::new();
    let namespace = build(&instance, INJECTED);
    let errorcode = instance.dict_get(namespace, "errorcode").expect("errorcode");
    // 按整数键取值：`pa` 侧没有"按整数键查 dict"的公开助手，这里直接读 entries
    // SAFETY: errorcode 是本实例里存活的 dict。
    let mapping = unsafe { &*errorcode.as_ptr().cast::<pyawa_core::DictObject>() };
    let entries: Vec<(i64, String)> = mapping
        .entries()
        .into_iter()
        .map(|(key, value)| (int_of(key), text_of(value)))
        .collect();
    assert!(
        entries.contains(&(2, "ENOENT".to_owned())),
        "errorcode 应当有 2 → ENOENT，实际：{entries:?}"
    );
    assert!(
        entries.iter().filter(|(value, _)| *value == 11).count() == 1,
        "别名在 errorcode 里只留一个名字（CM-19 的探测结果）"
    );
}

#[test]
fn the_mapping_is_by_name_and_falls_back_to_oserror() {
    assert_eq!(oserror_class_for("EACCES"), "PermissionError");
    assert_eq!(oserror_class_for("ENOENT"), "FileNotFoundError");
    assert_eq!(oserror_class_for("ENOTTY"), "OSError", "表外一律 OSError");
    let instance = Instance::new();
    assert_eq!(oserror_class_for_value(&instance, 2), "FileNotFoundError");
    assert_eq!(
        oserror_class_for_value(&instance, 999_999),
        "OSError",
        "未知数字落 OSError"
    );
}
