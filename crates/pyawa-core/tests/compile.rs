//! **编译器**（`P1-10`）的对拍：产物必须与参照实现**逐条指令**一致（`BC-16` 的纯函数性
//! 另有用例），并且过 `validate`（`BC-36` 的缓存槽零填充也照做）。
//!
//! 期望值来自 `tools/gen_compile_fixture.py` 从本机 CPython 导出的夹具（**禁止手写**）。
//! 夹具里带 `covered: false` 的那几段是**证据**（记着未接线的构造与原因），测试按原因跳过。

mod common;

use pyawa_core::compile::{CheckTier, compile, CompileError, Constant, Mode};

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
        // `CALL_KW` 的名元组
        Constant::Names(names) => format!("names:{}", names.join(",")),
        // `TS-31` 的边界标签（只在扩展模式＋深层档位下出现）
        Constant::Type(name) => format!("type:{name}"),
        // 与生成器的规则**逐字对齐**：**全字符串**元组（含空元组）记成 `names:`，
        // 其余（如默认值折叠出来的 `(2,)`）只记类型名 `tuple`
        Constant::Tuple(parts) => {
            if parts.iter().all(|part| matches!(part, Constant::Str(_))) {
                let joined: Vec<String> = parts
                    .iter()
                    .map(|part| match part {
                        Constant::Str(text) => text.clone(),
                        _ => unreachable!("上面刚判断过全是字符串"),
                    })
                    .collect();
                format!("names:{}", joined.join(","))
            } else {
                "tuple".to_owned()
            }
        }
    }
}

/// 一条指令数：位置表与它一一对应（本层的发射器保证两边的条数相同）。
fn unit_code_instruction_count(unit: &pyawa_core::compile::CompiledUnit) -> usize {
    instruction_stream(unit).len()
}

/// 递归比对一份产物与夹具里的一节（嵌套 code object 一并比）。
fn check_unit(unit: &pyawa_core::compile::CompiledUnit, entry: &common::Json, where_: &str) {
    assert_eq!(
        unit.name,
        entry.key("name").as_str(),
        "{where_} co_name"
    );
    assert_eq!(
        unit.qualname,
        entry.key("qualname").as_str(),
        "{where_} co_qualname（BC-4）"
    );
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

    // `BC-18`：位置表与指令**一一对应**，逐条与参照比。
    // 位置表**单独**一个标志：个别构造的指令流对得上、位置传播细节还没对齐（夹具里写明理由）。
    if !entry.key("positions_covered").as_bool() {
        assert!(
            !entry.key("positions_uncovered_because").as_str().is_empty(),
            "{where_} 位置未对齐必须写明理由"
        );
        return;
    }
    assert_eq!(
        unit.positions.len(),
        unit_code_instruction_count(unit),
        "{where_} 位置表条数应当等于指令数"
    );
    let expected_positions: Vec<(u32, u32, u32, u32)> = entry
        .key("instructions")
        .as_arr()
        .iter()
        .map(|item| {
            let position = item.key("position").as_arr();
            (
                position[0].as_i64() as u32,
                position[1].as_i64() as u32,
                position[2].as_i64() as u32,
                position[3].as_i64() as u32,
            )
        })
        .collect();
    assert_eq!(unit.positions, expected_positions, "{where_} 的位置表");

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
        let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow)
            .unwrap_or_else(|error| panic!("{source:?} 应当编得过，却报了 {error:?}"));

        check_unit(&unit, entry, &format!("{source:?}"));
        checked += 1;
    }
    let total = fixture.key("cases").as_obj().len();
    assert!(checked >= 20, "对拍的源码要够多，实际 {checked} 段");
    // 判据用**比例**而不是写死的数字：语料会长，写死的阈值迟早失守（照 `BC-59` 的口径，
    // 跳过的样本要少且**每条都写明理由**——理由断言在循环里）
    assert!(
        skipped * 4 < total,
        "整段跳过的样本要少（{skipped}/{total}）"
    );
}

#[test]
fn compiling_is_a_pure_function() {
    // `BC-16`：相同（源码、文件名、模式）⇒ 相同字节码
    let first = compile("x = 1; y = x", "<t>", Mode::PurePython, CheckTier::Shallow).expect("编得过");
    let second = compile("x = 1; y = x", "<t>", Mode::PurePython, CheckTier::Shallow).expect("编得过");
    assert_eq!(first, second);
    // `BC-14`：模式是显式入参；`§13-12` 已决"扩展特性清单为空" ⇒ 此刻两种模式产物相同
    let extended = compile("x = 1; y = x", "<t>", Mode::Extension, CheckTier::Shallow).expect("编得过");
    assert_eq!(first, extended);
}

#[test]
fn unsupported_and_bad_sources_are_reported_not_guessed() {
    // 字符串转义未接线 ⇒ 如实报（常量折叠已接线，大整数相加溢出仍未接）
    assert!(matches!(
        compile("x = 9223372036854775807 + 1", "<t>", Mode::PurePython, CheckTier::Shallow),
        Err(CompileError::Unsupported(_))
    ));
    // 负号／减法未接线 ⇒ 如实报 `Unsupported`（现在词法认得 `->`，裸 `-` 是"没接"而不是"语法不认"）
    assert!(matches!(
        compile("x = -3", "<t>", Mode::PurePython, CheckTier::Shallow),
        Err(CompileError::Unsupported(_))
    ));
    // 不支持的语句形态
    assert!(matches!(
        compile("x", "<t>", Mode::PurePython, CheckTier::Shallow),
        Err(CompileError::Unsupported(_))
    ));
    // 字符串转义未接线
    assert!(matches!(
        compile("x = 'a\\n'", "<t>", Mode::PurePython, CheckTier::Shallow),
        Err(CompileError::Unsupported(_))
    ));
}

#[test]
fn the_position_table_reaches_the_code_object() {
    // `BC-18` 的下半截：位置表要能过 `OM-11` 的属性通道看到——`co_positions()` 与 `co_lines()`。
    // 期望值同样来自夹具（`tools/gen_compile_fixture.py` 导出的 `co_positions()`／`co_lines()`）。
    let fixture = common::parse(include_str!("fixture-compile-3.14.json"));
    let vm = common::Vm::new();
    let mut checked = 0usize;
    for (_, entry) in fixture.key("cases").as_obj() {
        if !entry.key("covered").as_bool() {
            continue;
        }
        let source = entry.key("source").as_str();
        let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow).expect("编得过");
        let code = pyawa_core::compile::instantiate(&vm.instance, &unit);
        let code_raw = code.as_ptr().cast::<pyawa_core::Header>();
        // 期望值：夹具里那份，展开成 (起始行,结束行,起始列,结束列)
        let expected: Vec<(i64, i64, i64, i64)> = entry
            .key("instructions")
            .as_arr()
            .iter()
            .map(|item| {
                let position = item.key("position").as_arr();
                (
                    position[0].as_i64(),
                    position[1].as_i64(),
                    position[2].as_i64(),
                    position[3].as_i64(),
                )
            })
            .collect();
        if !entry.key("positions_covered").as_bool() {
            continue;
        }
        let observed = call_code_method(&vm, code_raw, "co_positions").expect("co_positions");
        let observed: Vec<(i64, i64, i64, i64)> = tuples_of(&vm, observed)
            .into_iter()
            .map(|numbers| (numbers[0], numbers[1], numbers[2], numbers[3]))
            .collect();
        assert_eq!(observed, expected, "{source:?} 的 co_positions()");

        let expected_lines: Vec<(i64, i64, i64)> = entry
            .key("lines")
            .as_arr()
            .iter()
            .map(|item| {
                let triple = item.as_arr();
                (triple[0].as_i64(), triple[1].as_i64(), triple[2].as_i64())
            })
            .collect();
        let observed_lines = call_code_method(&vm, code_raw, "co_lines").expect("co_lines");
        let observed_lines: Vec<(i64, i64, i64)> = tuples_of(&vm, observed_lines)
            .into_iter()
            .map(|numbers| (numbers[0], numbers[1], numbers[2]))
            .collect();
        assert_eq!(observed_lines, expected_lines, "{source:?} 的 co_lines()");
        checked += 1;
    }
    assert!(checked >= 15, "对拍的源码要够多，实际 {checked} 段");
}

/// 走**属性通道**调 code object 的方法（`LOAD_ATTR` 取方法位 ＋ `CALL`）。
fn call_code_method<'a>(
    vm: &'a common::Vm,
    code: core::ptr::NonNull<pyawa_core::Header>,
    method: &str,
) -> Result<pyawa_core::Value<'a>, pyawa_core::ExecError> {
    // SAFETY: code 由调用方保证存活，常量表要自己那份。
    unsafe { vm.instance.incref_object(code.as_ptr()) };
    let program = common::assemble(&[
        common::Item::Instr(common::op("RESUME"), 0),
        common::Item::Instr(common::op("LOAD_CONST"), 0),
        common::Item::Instr(common::op("LOAD_ATTR"), 0 << 1 | 1),
        common::Item::Instr(common::op("CALL"), 0),
        common::Item::Instr(common::op("RETURN_VALUE"), 0),
    ]);
    let code_object = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec![method.to_owned()],
        program,
        vec![Some(code)],
    );
    vm.run(&code_object)
}

/// 把迭代器里的元组**全取出来**（每项是它的整数列表）。
fn tuples_of(vm: &common::Vm, iterator: pyawa_core::Value<'_>) -> Vec<Vec<i64>> {
    let mut out = Vec::new();
    let iterator = iterator.as_header(&vm.instance).expect("应当是迭代器");
    for _ in 0..64 {
        match pyawa_core::executor::advance(&vm.instance, iterator) {
            Ok(Some(item)) => {
                // SAFETY: item 是 tuple。
                let tuple = unsafe { &*item.as_ptr().cast::<pyawa_core::TupleObject>() };
                let mut numbers = Vec::new();
                for index in 0..tuple.len() {
                    numbers.push(
                        vm.instance
                            .int_value(tuple.item(index).expect("下标在范围内"))
                            .expect("元组里都是整数"),
                    );
                }
                out.push(numbers);
            }
            Ok(None) => break,
            Err(error) => panic!("取迭代器下一项出错：{error:?}"),
        }
    }
    out
}
