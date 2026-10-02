//! **编译器**（`P1-10`）的对拍：产物必须与参照实现**逐条指令**一致（`BC-16` 的纯函数性
//! 另有用例），并且过 `validate`（`BC-36` 的缓存槽零填充也照做）。
//!
//! 期望值来自 `tools/gen_compile_fixture.py` 从本机 CPython 导出的夹具（**禁止手写**）。
//! 夹具里带 `covered: false` 的那几段是**证据**（记着未接线的构造与原因），测试按原因跳过。

mod common;

use pyawa_core::compile::{compile, CompileError, Constant, Mode};

/// 把编译产物里的指令解出来（复用解码器 ⇒ 顺带验了 `BC-35`／`BC-36` 的缓存槽）。
fn instruction_stream(unit: &pyawa_core::compile::CompiledUnit) -> Vec<(usize, u8, String, u32)> {
    pyawa_core::decode::validate(&unit.code).expect("产物必须过 validate");
    let mut decoder = pyawa_core::decode::Decoder::new(&unit.code);
    let mut out = Vec::new();
    while let Some(instruction) = decoder.next_instruction().expect("应当解得动") {
        let opname = pyawa_core::opcode::opname(u16::from(instruction.opcode))
            .expect("编号应当在表里")
            .to_owned();
        out.push((
            instruction.offset,
            instruction.opcode,
            opname,
            instruction.oparg,
        ));
    }
    out
}

fn render_constant(constant: &Constant) -> String {
    match constant {
        Constant::None => "none".to_owned(),
        Constant::Int(value) => format!("int:{value}"),
        Constant::Str(text) => format!("str:{text}"),
        // 嵌套 code object 只比名字（`repr` 带地址，逐字比不了也用不着）
        Constant::Code(unit) => format!("code:{}", unit.name),
    }
}

/// 递归比对一份产物与夹具里的一节（嵌套 code object 一并比）。
fn check_unit(unit: &pyawa_core::compile::CompiledUnit, entry: &common::Json, where_: &str) {
    assert_eq!(unit.argcount as i64, entry.key("argcount").as_i64(), "{where_} argcount");
    assert_eq!(unit.nlocals as i64, entry.key("nlocals").as_i64(), "{where_} nlocals");
    assert_eq!(unit.flags as i64, entry.key("flags").as_i64(), "{where_} flags");
    assert_eq!(
        unit.names,
        entry.key("names").as_arr().iter().map(|item| item.as_str().to_owned()).collect::<Vec<_>>(),
        "{where_} names"
    );
    assert_eq!(
        unit.varnames,
        entry.key("varnames").as_arr().iter().map(|item| item.as_str().to_owned()).collect::<Vec<_>>(),
        "{where_} varnames"
    );
    assert_eq!(
        unit.constants.iter().map(render_constant).collect::<Vec<_>>(),
        entry.key("consts").as_arr().iter().map(|item| item.as_str().to_owned()).collect::<Vec<_>>(),
        "{where_} consts"
    );

    // 指令流：偏移、名字、oparg。偏移能逐字对上，说明**缓存槽补得对**
    // （`BC-35`／`BC-36`：带缓存的指令后必须留等宽零填充）。
    // 本层的 `Instruction::offset` 单位是**码元**（`BC-42`），`dis` 用**字节** ⇒ 乘 2。
    let observed: Vec<(i64, String, Option<i64>)> = instruction_stream(unit)
        .into_iter()
        .map(|(offset, _, name, arg)| {
            let has_arg = pyawa_core::opcode::has_arg(
                pyawa_core::opcode::opcode(&name).expect("刚解出来的名字"),
            );
            (offset as i64 * 2, name, has_arg.then_some(i64::from(arg)))
        })
        .collect();
    let expected: Vec<(i64, String, Option<i64>)> = entry
        .key("instructions")
        .as_arr()
        .iter()
        .map(|item| {
            let arg = match item.get("arg") {
                Some(common::Json::Num(number)) => Some(*number),
                _ => None,
            };
            (
                item.key("offset").as_i64(),
                item.key("opname").as_str().to_owned(),
                arg,
            )
        })
        .collect();
    assert_eq!(observed, expected, "{where_} 的指令流");

    // 嵌套（按常量表里出现的顺序）
    let nested: Vec<&pyawa_core::compile::CompiledUnit> = unit
        .constants
        .iter()
        .filter_map(|constant| match constant {
            Constant::Code(inner) => Some(&**inner),
            _ => None,
        })
        .collect();
    let expected_nested = entry.key("nested").as_arr();
    assert_eq!(nested.len(), expected_nested.len(), "{where_} 的嵌套单元个数");
    for (index, (inner, expected)) in nested.iter().zip(expected_nested.iter()).enumerate() {
        check_unit(inner, expected, &format!("{where_} / nested[{index}]"));
    }
}

#[test]
fn the_emitter_matches_the_reference_instruction_by_instruction() {
    let fixture = common::parse(include_str!("fixture-compile-3.14.json"));
    let mut checked = 0usize;
    let mut skipped = 0usize;
    for (_, entry) in fixture.key("cases").as_obj() {
        let source = entry.key("source").as_str();
        if !entry.key("covered").as_bool() {
            // 未接线的构造：**必须**写明理由（`BC-59` 的口径）
            assert!(
                !entry.key("uncovered_because").as_str().is_empty(),
                "跳过的样本必须写明理由：{source:?}"
            );
            skipped += 1;
            continue;
        }
        let unit = compile(source, "<t>", Mode::PurePython)
            .unwrap_or_else(|error| panic!("{source:?} 应当编得过，却报了 {error:?}"));

        check_unit(&unit, entry, &format!("{source:?}"));
        checked += 1;
    }
    assert!(checked >= 8, "对拍的源码要够多，实际 {checked} 段");
    assert_eq!(skipped, 2, "跳过的应当是那两段常量折叠的证据");
}

#[test]
fn compiling_is_a_pure_function() {
    // `BC-16`：相同（源码、文件名、模式）⇒ 相同字节码
    let first = compile("x = 1; y = x", "<t>", Mode::PurePython).expect("编得过");
    let second = compile("x = 1; y = x", "<t>", Mode::PurePython).expect("编得过");
    assert_eq!(first, second);
    // `BC-14`：模式是显式入参；`§13-12` 已决"扩展特性清单为空" ⇒ 此刻两种模式产物相同
    let extended = compile("x = 1; y = x", "<t>", Mode::Extension).expect("编得过");
    assert_eq!(first, extended);
}

#[test]
fn unsupported_and_bad_sources_are_reported_not_guessed() {
    // 字符串转义未接线 ⇒ 如实报（常量折叠已接线，大整数相加溢出仍未接）
    assert!(matches!(
        compile("x = 9223372036854775807 + 1", "<t>", Mode::PurePython),
        Err(CompileError::Unsupported(_))
    ));
    // 负数常量未接线 ⇒ 词法就不认（报 Syntax，不猜）
    assert!(matches!(
        compile("x = -3", "<t>", Mode::PurePython),
        Err(CompileError::Syntax(_))
    ));
    // 不支持的语句形态
    assert!(matches!(
        compile("x", "<t>", Mode::PurePython),
        Err(CompileError::Unsupported(_))
    ));
    // 字符串转义未接线
    assert!(matches!(
        compile("x = 'a\\n'", "<t>", Mode::PurePython),
        Err(CompileError::Unsupported(_))
    ));
}
