//! **切片**（`P1-12` 点名的"索引／切片"；`slice` 类型 ＋ 四个序列族的边界规则）。
//!
//! 判据全部来自 `tests/fixture-slice-3.14.json`（`tools/gen_slice_fixture.py` 在参照实现上
//! **现场实测**：16 种切法 × `bytes`／`str`／`list`／`tuple`，外加 `slice` 自己的 `repr`
//! 与三条错误消息）。
//!
//! 切片语义**跨类型共用**（`slice.indices()` 那一套）⇒ 夹具与实现都只放**一处**，
//! 不在 `fixture-bytes` 里另抄一份。

mod common;

use core::ptr::NonNull;

use pyawa_core::executor::subscript::subscript_read;
use pyawa_core::Header;

use common::Vm;

fn fixture() -> common::Json {
    common::parse(include_str!("fixture-slice-3.14.json"))
}

fn from_hex(text: &str) -> Vec<u8> {
    if text == "-" {
        return Vec::new();
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("十六进制"))
        .collect()
}

fn to_hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn error_text(instance: &pyawa_core::Instance, error: pyawa_core::ExecError) -> String {
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

/// 参照实测的那个接收者（`b'abcde'`／`"abcde"`／`[10, 20, 30, 40, 50]`）。
fn receiver(vm: &Vm, kind: &str) -> NonNull<Header> {
    let fixture = fixture();
    let value = fixture.key("value");
    match kind {
        "bytes" => {
            let bytes = from_hex(value.key("bytes_hex").as_str());
            vm.instance.new_bytes(&bytes)
        }
        "str" => vm.instance.new_str(value.key("str").as_str()),
        "list" => {
            let items: Vec<NonNull<Header>> = value
                .key("items")
                .as_arr()
                .iter()
                .map(|item| vm.instance.new_int(item.as_i64()))
                .collect();
            vm.instance.new_list(items)
        }
        "tuple" => {
            let items: Vec<NonNull<Header>> = value
                .key("items")
                .as_arr()
                .iter()
                .map(|item| vm.instance.new_int(item.as_i64()))
                .collect();
            vm.instance.new_tuple(items)
        }
        other => panic!("没见过的接收者：{other}"),
    }
}

/// 夹具里的 `null` ↔ `None`。
fn optional(number: &common::Json) -> Option<i64> {
    match number {
        common::Json::Null => None,
        other => Some(other.as_i64()),
    }
}

/// 把切出来的结果渲染成夹具里那种「可比较的形状」。
fn observed(vm: &Vm, kind: &str, value: NonNull<Header>) -> String {
    match kind {
        "bytes" => format!("hex:{}", to_hex(vm.instance.bytes_value(value).expect("bytes"))),
        "str" => format!(
            "text:{}",
            unsafe { &*value.as_ptr().cast::<pyawa_core::StrObject>() }.value()
        ),
        "list" | "tuple" => {
            // SAFETY: 类型身份已确认（`list` 与 `tuple` 的载荷布局不同，分开取）
            let items: Vec<i64> = if Some(vm.instance.type_of(value))
                == vm.instance.type_named("list")
            {
                // SAFETY: 同上。
                let object = unsafe { &*value.as_ptr().cast::<pyawa_core::ListObject>() };
                (0..object.len())
                    .filter_map(|index| object.item(index))
                    .map(|item| vm.instance.int_value(item).expect("整数"))
                    .collect()
            } else {
                // SAFETY: 同上。
                let object = unsafe { &*value.as_ptr().cast::<pyawa_core::TupleObject>() };
                (0..object.len())
                    .filter_map(|index| object.item(index))
                    .map(|item| vm.instance.int_value(item).expect("整数"))
                    .collect()
            };
            format!(
                "items:[{}]",
                items
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        other => panic!("没见过的接收者：{other}"),
    }
}

/// 夹具里那种「可比较的形状」。
fn expected(entry: &common::Json, kind: &str) -> String {
    match kind {
        "bytes" => format!("hex:{}", entry.key("hex").as_str()),
        "str" => format!("text:{}", entry.key("text").as_str()),
        _ => format!(
            "items:[{}]",
            entry
                .key("items")
                .as_arr()
                .iter()
                .map(|item| item.as_i64().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[test]
fn slicing_matches_the_reference() {
    let vm = Vm::new();
    let fixture = fixture();
    let mut checked = 0;
    for row in fixture.key("rows").as_arr() {
        let (start, stop, step) = (
            optional(row.key("start")),
            optional(row.key("stop")),
            optional(row.key("step")),
        );
        let key = vm.instance.new_slice(start, stop, step);
        for kind in ["bytes", "str", "list", "tuple"] {
            let container = receiver(&vm, kind);
            let result = subscript_read(&vm.instance, container, key).unwrap_or_else(|error| {
                panic!("{kind}[{start:?}:{stop:?}:{step:?}] 出了错：{error:?}")
            });
            assert_eq!(
                observed(&vm, kind, result),
                expected(row.key("results").key(kind), kind),
                "{kind}[{start:?}:{stop:?}:{step:?}] 与参照不一致"
            );
            checked += 1;
        }
    }
    assert!(checked >= 64, "切片夹具条目太少（{checked}）");
}

#[test]
fn slice_objects_repr_like_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("slice_reprs").as_arr() {
        let key = vm.instance.new_slice(
            optional(row.key("start")),
            optional(row.key("stop")),
            optional(row.key("step")),
        );
        assert_eq!(
            vm.instance.object_repr(key).expect("slice 的 repr"),
            row.key("repr").as_str()
        );
    }
}

#[test]
fn slice_errors_match_the_reference() {
    let vm = Vm::new();
    let fixture = fixture();
    let errors = fixture.key("errors");
    // 步长为 0：实测在**求值**时报
    let container = receiver(&vm, "bytes");
    let zero_step = vm.instance.new_slice(None, None, Some(0));
    let error = subscript_read(&vm.instance, container, zero_step).expect_err("步长 0 要报错");
    assert_eq!(
        error_text(&vm.instance, error),
        errors.key("step_zero").as_str()
    );
    // `slice` 的字段不是整数 ⇒ `slice()` 构造时就报（实测消息与切片求值那条同形）
    let non_int = vm.instance.new_str("a");
    let callable = vm
        .instance
        .type_value(vm.instance.type_named("slice").expect("slice 在注册表里"));
    let error = pyawa_core::call_value(&vm.instance, callable, &[non_int], &[])
        .expect_err("非整数字段要报错");
    assert_eq!(
        error_text(&vm.instance, error),
        errors.key("slice_non_int").as_str()
    );
}

/// 把一个 list 里的整数读出来（测试用）。
fn list_ints(vm: &Vm, list: NonNull<Header>) -> Vec<i64> {
    vm.instance
        .list_items(list)
        .expect("是列表")
        .into_iter()
        .map(|item| vm.instance.int_value(item).expect("元素是整数"))
        .collect()
}

#[test]
fn slice_assignment_matches_the_reference() {
    let vm = Vm::new();
    let items: Vec<NonNull<Header>> = (1..=5).map(|value| vm.instance.new_int(value)).collect();
    let target = vm.instance.new_list(items);
    // 步长 1：长度可以不同（`l[1:3] = [9, 9, 9]` ⇒ 六个元素）
    let key = vm.instance.new_slice(Some(1), Some(3), None);
    let replacement: Vec<NonNull<Header>> = [9, 9, 9]
        .into_iter()
        .map(|value| vm.instance.new_int(value))
        .collect();
    let value = vm.instance.new_list(replacement);
    pyawa_core::executor::subscript::subscript_write(&vm.instance, target, key, value).expect("切片写");
    assert_eq!(
        list_ints(&vm, target),
        vec![1, 9, 9, 9, 4, 5],
        "`l[1:3] = [9, 9, 9]` 应当替换成三段"
    );

    // 扩展切片（步长 ≠ 1）：长度不等 ⇒ 参照实测的 `ValueError`
    let target = {
        let items: Vec<NonNull<Header>> = (1..=5).map(|value| vm.instance.new_int(value)).collect();
        vm.instance.new_list(items)
    };
    let key = vm.instance.new_slice(None, None, Some(2));
    let value = vm.instance.new_list(vec![vm.instance.new_int(1)]);
    let error = pyawa_core::executor::subscript::subscript_write(&vm.instance, target, key, value)
        .expect_err("长度不等要报错");
    assert_eq!(
        error_text(&vm.instance, error),
        "ValueError: attempt to assign sequence of size 1 to extended slice of size 3"
    );

    // 长度相等 ⇒ 逐个替换
    let target = {
        let items: Vec<NonNull<Header>> = (1..=5).map(|value| vm.instance.new_int(value)).collect();
        vm.instance.new_list(items)
    };
    let key = vm.instance.new_slice(None, None, Some(2));
    let replacement: Vec<NonNull<Header>> = [7, 8, 9]
        .into_iter()
        .map(|value| vm.instance.new_int(value))
        .collect();
    let value = vm.instance.new_list(replacement);
    pyawa_core::executor::subscript::subscript_write(&vm.instance, target, key, value).expect("扩展切片写");
    assert_eq!(list_ints(&vm, target), vec![7, 2, 8, 4, 9]);
}
