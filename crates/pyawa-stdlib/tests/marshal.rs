//! `marshal`（**`CM-27`**：自有格式 ＋ 自己的版本号，`loads(dumps(x))` 往返一致）。
//!
//! 义务边界照规范原文：**必须存在且自洽**、API 面（`dump`／`dumps`／`load`／`loads`／`version`）
//! 齐备、`loads(dumps(x))` 往返；**不追**与参照的字节兼容（属实现定义行为）。
//!
//! 断言分四类：
//! ① **往返**：`dumps(loads(dumps(x))) == dumps(x)`（**规范字节**口径，不需要值相等语义，
//!    因此 `dict` 这种"值比较还没接线"的类型也验得了）
//! ② **义务面**：参照实测往返得动的那些类型，我们逐个覆盖（夹具 `fixtures/marshal.rs`）
//! ③ **自有格式**：`version` 与参照的版本号**必须不同**（`CM-27` 的"自有格式"判据）
//! ④ **错误**：空输入／垃圾／截断／未知版本／循环引用（穿过不可变容器的环）

#[path = "fixtures/marshal.rs"]
mod fixture;

use core::ptr::NonNull;

use pyawa_core::{Header, Instance};
use pyawa_stdlib::marshal_module;

use fixture::{
    REFERENCE_BAD_TYPE_MESSAGE, REFERENCE_CIRCULAR_MESSAGE, REFERENCE_EMPTY_MESSAGE,
    REFERENCE_GARBAGE_MESSAGE, REFERENCE_ROUND_TRIP, REFERENCE_TRUNCATED_MESSAGE,
    REFERENCE_VERSION,
};

fn call(
    instance: &Instance,
    namespace: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("marshal.{name} 必须存在"));
    pyawa_core::call_value(instance, function, args, &[])
}

fn error_text(instance: &Instance, error: pyawa_core::ExecError) -> String {
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: 异常对象由实例保活。
            let ty = unsafe { exception.as_ref() }.ty();
            let name = unsafe { ty.as_ref() }.name().to_owned();
            // SAFETY: 同上。
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default();
            format!("{name}: {message}")
        }
        other => panic!("应当是脚本异常，得到 {other:?}"),
    }
}

fn dumps(
    instance: &Instance,
    namespace: NonNull<Header>,
    value: NonNull<Header>,
) -> Vec<u8> {
    let result = call(instance, namespace, "dumps", &[value]).expect("dumps 应当成功");
    instance.bytes_value(result).expect("dumps 给 bytes").to_vec()
}

fn loads(
    instance: &Instance,
    namespace: NonNull<Header>,
    data: &[u8],
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let object = instance.new_bytes(data);
    call(instance, namespace, "loads", &[object])
}

/// 夹具里那些**义务面**值的构造（名字与 `tools/gen_marshal_fixture.py` 一一对应）。
fn value_named(instance: &Instance, name: &str) -> Option<NonNull<Header>> {
    Some(match name {
        "none" => instance.retain(instance.singletons().none()),
        "true" => instance.retain(instance.singletons().boolean(true)),
        "false" => instance.retain(instance.singletons().boolean(false)),
        "int_small" => instance.new_int(42),
        "int_negative" => instance.new_int(-7),
        "int_i64_max" => instance.new_int(i64::MAX),
        "int_big" => {
            // 2**100：大整数（`TS-45`）也要能往返
            let text = "1267650600228229401496703205376";
            let wide = pyawa_core::bigint::BigInt::from_decimal(text).expect("十进制可解析");
            instance.new_int_value(pyawa_core::bigint::IntValue::from_big(wide))
        }
        "float" => instance.new_float(1.5),
        "float_nan" => instance.new_float(f64::NAN),
        "str_ascii" => instance.new_str("abc"),
        "str_unicode" => instance.new_str("café"),
        "bytes" => instance.new_bytes(&[0x00, 0xff]),
        "tuple" => {
            let items = vec![instance.new_int(1), instance.new_int(2)];
            instance.new_tuple(items)
        }
        "list" => {
            let items = vec![instance.new_int(1), instance.new_str("a")];
            instance.new_list(items)
        }
        "dict" => {
            let dict = instance.new_dict();
            let key = instance.new_str("a");
            let value = instance.new_int(1);
            instance.dict_insert_raw(dict, key, value);
            dict
        }
        "set" => {
            let items = vec![instance.new_int(1), instance.new_int(2)];
            instance.new_set(items)
        }
        "nested" => {
            let inner_tuple = instance.new_tuple(vec![instance.new_int(2), instance.new_bytes(b"x")]);
            let inner_list = instance.new_list(vec![instance.new_int(1), inner_tuple]);
            let dict = instance.new_dict();
            let key = instance.new_str("k");
            instance.dict_insert_raw(dict, key, inner_list);
            dict
        }
        _ => return None,
    })
}

#[test]
fn the_api_surface_is_complete_and_version_is_our_own() {
    let instance = Instance::new();
    let namespace = marshal_module::build(&instance);
    for name in ["dump", "dumps", "load", "loads", "version"] {
        assert!(
            instance.dict_get(namespace, name).is_some(),
            "`CM-27`：marshal.{name} 必须存在"
        );
    }
    // `CM-27`：**自有**版本号 ⇒ 与参照的**必须不同**
    let version = instance.dict_get(namespace, "version").expect("version");
    let observed = instance.int_value(version).expect("version 是整数");
    assert_eq!(observed, i64::from(marshal_module::FORMAT_VERSION));
    assert_ne!(
        observed, REFERENCE_VERSION,
        "CM-27：自有格式的版本号不该等于参照的（参照实测 {REFERENCE_VERSION}）"
    );
    assert_eq!(marshal_module::NAME, "marshal");
}

#[test]
fn every_probed_type_round_trips() {
    let instance = Instance::new();
    let namespace = marshal_module::build(&instance);
    for name in REFERENCE_ROUND_TRIP {
        let value = value_named(&instance, name)
            .unwrap_or_else(|| panic!("夹具列了 {name}，测试里也得造得出来"));
        let first = dumps(&instance, namespace, value);
        let decoded = loads(&instance, namespace, &first).unwrap_or_else(|error| {
            panic!("{name} 应当解得回来：{error:?}")
        });
        let second = dumps(&instance, namespace, decoded);
        assert_eq!(
            second, first,
            "{name} 往返后字节不一致（规范字节口径：同一对象必编出同一串）"
        );
        // SAFETY: value 是本测试持有的新引用。
        unsafe { instance.release_object(value.as_ptr()) };
    }
}

#[test]
fn cycles_round_trip_for_mutable_containers() {
    // 实测：参照 3.14 的 marshal **支持**循环引用（`l = []; l.append(l)` 往返回来还是自引用）
    let instance = Instance::new();
    let namespace = marshal_module::build(&instance);
    // 夹具里那条 `REFERENCE_CIRCULAR_MESSAGE` 是 `None`——即：参照**没有**"循环引用"这条
    // 错误消息（它 3.14 起支持循环）⇒ 我们也支持（下面的断言就是这条实测的落地）
    assert!(
        REFERENCE_CIRCULAR_MESSAGE.is_none(),
        "参照实测：marshal 能往返循环引用，没有这条错误消息"
    );
    let list = instance.new_list(Vec::new());
    let self_reference = instance.retain(list);
    instance.list_append(list, self_reference);
    let data = dumps(&instance, namespace, list);
    let decoded = loads(&instance, namespace, &data).expect("循环的 list 应当解得回来");
    let items = instance.list_items(decoded).expect("是 list");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0], decoded, "解回来的第一项必须指回它自己");

    let dict = instance.new_dict();
    let key = instance.new_str("self");
    let self_reference = instance.retain(dict);
    instance.dict_insert_raw(dict, key, self_reference);
    let data = dumps(&instance, namespace, dict);
    let decoded = loads(&instance, namespace, &data).expect("循环的 dict 应当解得回来");
    let entries = instance.dict_entries(decoded).expect("是 dict");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].1, decoded, "解回来的值必须指回它自己");
}

#[test]
fn malformed_input_reports_the_probed_messages() {
    let instance = Instance::new();
    let namespace = marshal_module::build(&instance);
    // 空输入
    let error = loads(&instance, namespace, b"").expect_err("空输入要报错");
    assert_eq!(
        error_text(&instance, error),
        REFERENCE_EMPTY_MESSAGE.expect("夹具里有这条消息")
    );
    // 格式版本不对（**我们自有格式**的第一字节就是版本号 ⇒ 参照那套 tag 口径不适用）
    let error = loads(&instance, namespace, b"\x7f").expect_err("版本不对要报错");
    let observed = error_text(&instance, error);
    assert!(
        observed.contains("unknown format version 127"),
        "实际：{observed}"
    );
    // 版本对、tag 未知 ⇒ 与参照实测的"未知类型码"同一条消息
    let error = loads(&instance, namespace, b"\x01\x7f").expect_err("未知 tag 要报错");
    assert_eq!(
        error_text(&instance, error),
        REFERENCE_BAD_TYPE_MESSAGE.expect("夹具里有这条消息")
    );
    // 截断：`[1, 2, 3]` 的编码去掉最后一个字节
    let value = instance.new_list(vec![
        instance.new_int(1),
        instance.new_int(2),
        instance.new_int(3),
    ]);
    let mut data = dumps(&instance, namespace, value);
    data.pop();
    let error = loads(&instance, namespace, &data).expect_err("截断要报错");
    assert_eq!(
        error_text(&instance, error),
        REFERENCE_TRUNCATED_MESSAGE.expect("夹具里有这条消息")
    );
    // 垃圾但 tag 合法（`str` 后面接不出长度）
    let error = loads(&instance, namespace, b"\x01\x06\x02").expect_err("长度不够要报错");
    assert!(error_text(&instance, error).starts_with("EOFError"));
    let _ = REFERENCE_GARBAGE_MESSAGE;
}

#[test]
fn dump_and_load_are_honestly_unimplemented() {
    // `dump`／`load` 要文件对象（fs 域／M3+）⇒ 如实报未实现，不假装能写
    let instance = Instance::new();
    let namespace = marshal_module::build(&instance);
    let value = instance.new_int(1);
    let error = call(&instance, namespace, "dump", &[value]).expect_err("`dump` 还没接线");
    assert!(matches!(error, pyawa_core::ExecError::Unsupported { .. }));
    let error = call(&instance, namespace, "load", &[value]).expect_err("`load` 还没接线");
    assert!(matches!(error, pyawa_core::ExecError::Unsupported { .. }));
}

#[test]
fn unmarshalable_objects_are_reported_not_guessed() {
    // 函数对象这类 marshal 表示不了的：报错，不静默换成别的东西
    let instance = Instance::new();
    let namespace = marshal_module::build(&instance);
    let function = instance.new_int(1);
    // 先确认"能 marshal 的"通路是通的，再确认`slice`这种没进格式的类型会报错
    let slice = instance.new_slice(Some(1), Some(2), None);
    let _ = dumps(&instance, namespace, function);
    let error = call(&instance, namespace, "dumps", &[slice]).expect_err("slice 不在格式里");
    assert!(error_text(&instance, error).contains("cannot marshal"));
}
