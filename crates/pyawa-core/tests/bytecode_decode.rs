//! 码元解码的可测性质（`docs/SPEC-bytecode.md` §8.2／§8.3：**BC-32**…**BC-36**）。
//!
//! 端到端 `dis` 反汇编（`T-BC-12` 的后半）要等 M2 的 import 系统；这里钉的是**偏移算术**
//! 与**发射方体检**——`T-BC-12` 的硬杠（cache 偏移对齐）由本文件的用例覆盖。

mod common;

use pyawa_core::decode::{parse_exception_table, validate, DecodeError, Decoder, ExceptionEntry};
use pyawa_core::opcode::{self, inline_cache_entries};
use pyawa_core::opcode_metadata::{MIN_INSTRUMENTED_OPCODE, OPMAP};

fn op(name: &str) -> u8 {
    opcode::opcode(name).unwrap_or_else(|| panic!("opmap 缺 {name}")) as u8
}

#[test]
fn code_units_are_two_bytes() {
    let bytes = vec![op("LOAD_CONST"), 3, op("RESUME"), 0];
    let mut decoder = Decoder::new(&bytes);

    let first = decoder.next_instruction().unwrap().unwrap();
    assert_eq!((first.offset, first.opcode, first.oparg, first.size), (0, op("LOAD_CONST"), 3, 1));
    let second = decoder.next_instruction().unwrap().unwrap();
    assert_eq!((second.offset, second.opcode, second.size), (1, op("RESUME"), 1));
    assert_eq!(decoder.next_instruction().unwrap(), None, "BC-33：两个码元读完就结束");
    assert_eq!(validate(&bytes), Ok(()));

    // 半截码元必须报错，而不是装作读到了
    assert_eq!(
        validate(&[op("NOP")]),
        Err(DecodeError::TruncatedCodeUnit { offset: 0 })
    );
}

#[test]
fn extended_arg_folds_big_endian() {
    // BC-34：先读到的 EXTENDED_ARG 在高位 ⇒ 0x01_02 << 8 | 0x03
    let bytes = vec![
        op("EXTENDED_ARG"), 0x01,
        op("EXTENDED_ARG"), 0x02,
        op("LOAD_CONST"), 0x03,
    ];
    let mut decoder = Decoder::new(&bytes);
    let instruction = decoder.next_instruction().unwrap().unwrap();

    assert_eq!(instruction.offset, 0);
    assert_eq!(instruction.opcode, op("LOAD_CONST"));
    assert_eq!(instruction.oparg, 0x0001_0203, "BC-34：大端拼接");
    assert_eq!(instruction.size, 3, "两个前缀 ＋ 指令本体");
    assert_eq!(validate(&bytes), Ok(()));

    // 前缀后面没有真正的指令
    assert_eq!(
        validate(&[op("EXTENDED_ARG"), 1]),
        Err(DecodeError::DanglingExtendedArg { offset: 0 })
    );

    // 拼出 u32 装不下的值
    let mut too_long = Vec::new();
    for _ in 0..5 {
        too_long.extend([op("EXTENDED_ARG"), 0xFF]);
    }
    too_long.extend([op("LOAD_CONST"), 0x00]);
    assert_eq!(
        validate(&too_long),
        Err(DecodeError::OpargOverflow { offset: 0 })
    );
}

#[test]
fn cache_slots_are_skipped_and_offsets_stay_aligned() {
    // T-BC-12 的硬杠：带 cache 的指令之后，下一条指令的偏移必须落在 cache 之后
    for (name, expected_cache) in [
        ("LOAD_ATTR", 9u32),
        ("BINARY_OP", 5),
        ("CALL", 3),
        ("COMPARE_OP", 1),
    ] {
        let mut bytes = vec![op(name), 1];
        bytes.extend(std::iter::repeat_n(0u8, expected_cache as usize * 2));
        bytes.extend([op("NOP"), 0]);

        let mut decoder = Decoder::new(&bytes);
        let instruction = decoder.next_instruction().unwrap().unwrap();
        assert_eq!(instruction.opcode, op(name));
        assert_eq!(instruction.size, 1 + expected_cache as usize, "BC-35：{name}");

        let following = decoder.next_instruction().unwrap().unwrap();
        assert_eq!(
            following.offset,
            1 + expected_cache as usize,
            "BC-35：{name} 之后的偏移必须跳过等宽 cache 槽"
        );
        assert_eq!(validate(&bytes), Ok(()));
    }
}

#[test]
fn every_cache_carrying_opcode_round_trips() {
    // 逐个带上 cache 的指令造一段、解一段：跨度必须等于 1 ＋ 实测宽度
    for (name, opcode_number) in OPMAP {
        let cache = inline_cache_entries(*opcode_number);
        if cache == 0 {
            continue;
        }
        let mut bytes = vec![*opcode_number as u8, 0];
        bytes.extend(std::iter::repeat_n(0u8, cache as usize * 2));
        bytes.extend([op("NOP"), 0]);

        let mut decoder = Decoder::new(&bytes);
        let instruction = decoder.next_instruction().unwrap().unwrap();
        assert_eq!(instruction.size, 1 + cache as usize, "{name}");
        assert_eq!(decoder.next_instruction().unwrap().unwrap().offset, 1 + cache as usize);
        assert_eq!(validate(&bytes), Ok(()), "{name} 的 cache 槽应合法");
    }
}

#[test]
fn cache_slots_must_be_zero_filled() {
    let mut bytes = vec![op("COMPARE_OP"), 2, 0, 0];
    assert_eq!(validate(&bytes), Ok(()), "零填充应当通过");

    bytes[3] = 7; // 第二个字节属于 cache 槽
    assert_eq!(
        validate(&bytes),
        Err(DecodeError::NonZeroCacheSlot { offset: 1 }),
        "BC-36：cache 槽不得用于自己的优化"
    );
}

#[test]
fn instrumented_and_unknown_opcodes_are_rejected() {
    // BC-32：instrumented 一族（≥ MIN_INSTRUMENTED_OPCODE）禁止发射。
    // 注意只挑**装得进一个码元**的编号：≥ 256 的那些（如 `ANNOTATIONS_PLACEHOLDER` = 256）
    // 在 `BC-33` 的 `opcode: u8` 里根本表示不出来，属于另一个话题。
    let instrumented = OPMAP
        .iter()
        .find(|(_, number)| *number >= MIN_INSTRUMENTED_OPCODE && *number <= u16::from(u8::MAX))
        .map(|(name, number)| (*name, *number))
        .expect("基线里应当有可编码的 instrumented 编号");
    let bytes = vec![instrumented.1 as u8, 0];
    assert_eq!(
        validate(&bytes),
        Err(DecodeError::InstrumentedOpcode {
            offset: 0,
            opcode: instrumented.1 as u8
        }),
        "{name} 必须被拒", name = instrumented.0
    );
    // 解码器本身仍能识谱（它只是执行器的游标，体检在 validate）
    assert_eq!(
        Decoder::new(&bytes).next_instruction().unwrap().unwrap().opcode,
        instrumented.1 as u8
    );

    // 空隙编号（不在 opmap 里）
    let hole = (0u16..MIN_INSTRUMENTED_OPCODE)
        .find(|number| opcode::opname(*number).is_none())
        .expect("基线里应当有空隙编号");
    assert_eq!(
        validate(&[hole as u8, 0]),
        Err(DecodeError::UnknownOpcode { offset: 0, opcode: hole as u8 })
    );
}

#[test]
fn no_argument_instructions_must_zero_their_oparg() {
    // BC-33：无参指令的 oparg 必须为 0
    assert_eq!(
        validate(&[op("NOP"), 1]),
        Err(DecodeError::UnexpectedArgument {
            offset: 0,
            opcode: op("NOP"),
            oparg: 1
        })
    );
    // 也不得用 EXTENDED_ARG 给它凑前缀
    assert_eq!(
        validate(&[op("EXTENDED_ARG"), 0, op("NOP"), 0]),
        Err(DecodeError::UnexpectedArgument {
            offset: 0,
            opcode: op("NOP"),
            oparg: 0
        })
    );
}

#[test]
fn pyawa_specific_instructions_decode_with_their_oparg() {
    // BC-24：专有指令带 oparg（签名条目索引）；它们不进 cache 表
    for (name, number) in pyawa_core::opcode_metadata::PYAWA_SPECIFIC {
        let bytes = vec![*number as u8, 5, op("NOP"), 0];
        let instruction = Decoder::new(&bytes).next_instruction().unwrap().unwrap();
        assert_eq!((instruction.opcode, instruction.oparg), (*number as u8, 5), "{name}");
        assert_eq!(instruction.size, 1);
        assert_eq!(validate(&bytes), Ok(()), "{name}");
    }
}

#[test]
fn jump_targets_match_the_oracle() {
    // BC-55／T-BC-17：拿**参照实现产出的字节**验证跳转算术（含前向、后向、带 cache 的跳转）
    let fixture = common::parse(include_str!("fixture-code-3.14.json"));
    let mut checked = 0;

    for sample in fixture.key("samples").as_arr() {
        let bytes: Vec<u8> = hex_bytes(sample.key("co_code").as_str());
        assert_eq!(
            validate(&bytes),
            Ok(()),
            "夹具里的 co_code 必须是合法码元（BC-32…BC-36）：{}",
            sample.key("snippet").as_str()
        );

        let mut decoder = Decoder::new(&bytes);
        let mut actual: Vec<(usize, usize)> = Vec::new();
        while let Some(instruction) = decoder.next_instruction().unwrap() {
            if let Some(target) = instruction.jump_target() {
                actual.push((instruction.offset, target));
            }
        }

        // 参照实现给的是**字节**偏移，本层用**码元**（BC-33：每码元 2 字节）
        let expected: Vec<(usize, usize)> = sample
            .key("jumps")
            .as_arr()
            .iter()
            .map(|jump| {
                (
                    jump.key("offset").as_i64() as usize / 2,
                    jump.key("target").as_i64() as usize / 2,
                )
            })
            .collect();

        assert_eq!(
            actual,
            expected,
            "BC-55：{} 的跳转目标必须与 dis 的 argval 逐条一致",
            sample.key("snippet").as_str()
        );
        checked += actual.len();
    }

    assert!(checked >= 10, "夹具里应当有足够多的跳转，实际 {checked}");
    assert_eq!(
        opcode::has_jump(opcode::opcode("END_ASYNC_FOR").unwrap()),
        true,
        "BC-55：后向判定只按名字，END_ASYNC_FOR 也带目标"
    );
}

/// 十六进制转字节（夹具里 `co_code` 存的是 hex）。
fn hex_bytes(text: &str) -> Vec<u8> {
    assert!(text.len() % 2 == 0, "hex 长度必须是偶数");
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("夹具里的 hex 应当合法")
        })
        .collect()
}

#[test]
fn exception_tables_match_the_oracle() {
    // BC-54：拿参照实现产出的 `co_exceptiontable` 验证解析（含空表）
    let fixture = common::parse(include_str!("fixture-code-3.14.json"));
    let mut checked = 0;

    for sample in fixture.key("samples").as_arr() {
        let table = hex_bytes(sample.key("co_exceptiontable").as_str());
        let actual = parse_exception_table(&table).expect("夹具里的异常表应当合法");
        let expected: Vec<ExceptionEntry> = sample
            .key("exceptions")
            .as_arr()
            .iter()
            .map(|entry| ExceptionEntry {
                start: entry.key("start").as_i64() as usize,
                end: entry.key("end").as_i64() as usize,
                target: entry.key("target").as_i64() as usize,
                depth: entry.key("depth").as_i64() as usize,
                lasti: entry.key("lasti").as_bool(),
            })
            .collect();

        assert_eq!(
            actual,
            expected,
            "BC-54：{} 的异常表必须与 dis._parse_exception_table 一致",
            sample.key("snippet").as_str()
        );
        checked += actual.len();
    }

    assert!(checked >= 2, "夹具里应当有异常表记录，实际 {checked}");
    // 半截 varint 必须报错，而不是装作读到了
    assert_eq!(
        parse_exception_table(&[0x40]),
        Err(DecodeError::TruncatedExceptionTable { offset: 1 })
    );
}
