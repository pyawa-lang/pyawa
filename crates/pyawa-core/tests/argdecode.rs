//! `BC-59` 的 oracle 对拍：语料里出现的**每一条**指令，`argval`／`argrepr` 都要与 `dis` 逐条一致
//! （`T-BC-19`／`T-BC-20`／`T-BC-21`）。
//!
//! 期望值来自 `tests/fixture-argval-3.14.json`（`tools/gen_argval_fixture.py` 从参照实现导出）。
//! 夹具里 `comparable = false` 的样本是"常量里含 code object"的那些——它们的 `repr` 带地址与
//! 文件名，要等 `BC-4` 的 `co_filename`／`co_firstlineno`／`co_qualname` 落地；**这件事在夹具里
//! 显式标出**，测试据此计数并断言，而不是悄悄跳过（`BC-59`）。

mod common;

use core::ptr::NonNull;

use pyawa_core::argdecode::decode_all;
use pyawa_core::{
    FloatObject, Header, IntObject, StrObject, TupleObject,
};

use common::{emit, op, Json, Vm};

/// 按夹具里的结构描述造一个常量对象（**新引用**）。
fn build_constant(vm: &Vm, description: &Json) -> Option<NonNull<Header>> {
    let instance = &vm.instance;
    match description.key("kind").as_str() {
        "none" => Some(instance.own(instance.singletons().none()).into_raw()),
        "bool" => {
            let value = description.key("value").as_bool();
            Some(instance.own(instance.singletons().boolean(value)).into_raw())
        }
        "int" => {
            let value = description.key("value").as_i64();
            Some(
                instance
                    .alloc(IntObject::new(instance.singletons().int_type(), value))
                    .into_raw()
                    .cast::<Header>(),
            )
        }
        "float" => {
            let value: f64 = description.key("value").as_str().parse().expect("夹具里的浮点");
            Some(
                instance
                    .alloc(FloatObject::new(instance.type_named("float").unwrap(), value))
                    .into_raw()
                    .cast::<Header>(),
            )
        }
        "str" => {
            let value = description.key("value").as_str().to_owned();
            Some(
                instance
                    .alloc(StrObject::new(instance.singletons().str_type(), value))
                    .into_raw()
                    .cast::<Header>(),
            )
        }
        "tuple" => {
            let mut items = Vec::new();
            for item in description.key("items").as_arr() {
                items.push(build_constant(vm, item)?);
            }
            Some(
                instance
                    .alloc(TupleObject::new(instance.type_named("tuple").unwrap(), items))
                    .into_raw()
                    .cast::<Header>(),
            )
        }
        // code object 常量：见文件头（样本会被标为不可对拍）
        _ => None,
    }
}

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("夹具 hex"))
        .collect()
}

#[test]
fn every_instruction_decodes_like_dis() {
    let fixture = common::parse(include_str!("fixture-argval-3.14.json"));
    let vm = Vm::new();
    let mut checked = 0usize;
    let mut skipped = 0usize;

    for sample in fixture.key("samples").as_arr() {
        if !sample.key("comparable").as_bool() {
            skipped += 1;
            assert!(
                !sample.key("skipped_because").as_str().is_empty(),
                "跳过的样本必须写明理由（BC-59）"
            );
            continue;
        }

        let mut consts = Vec::new();
        for description in sample.key("co_consts").as_arr() {
            consts.push(build_constant(&vm, description));
        }
        let names: Vec<String> = sample
            .key("co_names")
            .as_arr()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        let varnames: Vec<String> = sample
            .key("co_varnames")
            .as_arr()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect();

        let code = vm.code_with_names(
            32,
            0,
            0,
            varnames,
            names,
            hex_bytes(sample.key("co_code").as_str()),
            consts,
        );

        let decoded = decode_all(&vm.instance, code.get()).unwrap_or_else(|error| {
            panic!(
                "{}（{}）解码失败：{error:?}",
                sample.key("snippet").as_str(),
                sample.key("name").as_str()
            )
        });
        let expected = sample.key("instructions").as_arr();
        assert_eq!(
            decoded.len(),
            expected.len(),
            "{}：指令条数",
            sample.key("snippet").as_str()
        );

        for (index, wanted) in expected.iter().enumerate() {
            let got = &decoded[index];
            let where_ = format!(
                "{} @{}",
                sample.key("snippet").as_str(),
                wanted.key("offset").as_i64()
            );
            assert_eq!(got.offset, wanted.key("offset").as_i64() as usize, "偏移 {where_}");
            assert_eq!(got.opname, wanted.key("opname").as_str(), "指令名 {where_}");
            assert_eq!(
                got.argval,
                wanted.key("argval").as_str(),
                "argval {where_}（BC-59）"
            );
            assert_eq!(
                got.argrepr,
                wanted.key("argrepr").as_str(),
                "argrepr {where_}（BC-59）"
            );
            checked += 1;
        }
    }

    assert!(
        checked >= 100,
        "BC-59：语料要足够大，实际对拍 {checked} 条"
    );
    // `BC-59`：跳过的样本要**少**且**写明理由**。判据用比例而不是写死的数字——
    // 语料每加一段带嵌套 `def` 的片段，模块级样本就会多一个（它含 code object 常量，
    // 而那种常量的 `repr` 带**地址**，两边对不齐）。写死的阈值迟早会失守，比例不会。
    let total = fixture.key("samples").as_arr().len();
    assert!(
        skipped * 2 < total,
        "BC-59：跳过的样本要少（{skipped}/{total}），且每个都要写明理由"
    );
}

#[test]
fn unsupported_forms_are_reported_not_skipped() {
    // BC-59 的另一面：没覆盖的形态必须**报错**，而不是给一个看似合理的值。
    // 这里是一条名字下标越界的 `LOAD_GLOBAL`。
    let vm = Vm::new();
    let bytes = emit(&[(op("LOAD_GLOBAL"), 0), (op("RETURN_VALUE"), 0)]);
    let code = vm.code_with_names(8, 0, 0, Vec::new(), Vec::new(), bytes, Vec::new());
    assert!(matches!(
        decode_all(&vm.instance, code.get()),
        Err(pyawa_core::argdecode::ArgDecodeError::Unsupported { .. })
    ));
}
