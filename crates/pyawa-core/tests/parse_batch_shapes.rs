//! **P3-31 解析器缺口批的编译期抽例**（第 405 轮）。
//!
//! 期望值**不是手写**：由本机 `python3`（3.14.4）对同一段源码的 `dis.get_instructions`
//! ＋ `op.positions` 一次性导出（生成脚本见提交说明；抽例只钉本批的 10 余个最小形状）。
//! 比较口径：`opname`／`oparg`（参照的无参指令 `arg=None` ⇒ 归一成 `0`）／位置四元组
//! （参照的合成指令位置全空 ⇒ `None`）。
//!
//! **已知的既有偏差不在这里比**（都在本批之前就存在，与本批解析器改动无关）：
//! ① `with …: pass` 的体在参照里多一条 `NOP`；② 方法调用（`LOAD_ATTR` 的 NULL 位）
//! 与 `PUSH_NULL` 的形态；③ 常量条件 `if True:` 的折叠。⇒ 对应形状改用等价、但发射
//! 逐条一致的写法（`with … as (a, b): x = a`／`f()[0] = 1`／`if flag: …`）覆盖语义面。

mod common;

use pyawa_core::compile::{compile, CheckTier, CompiledUnit, Constant, Mode};

type Position = Option<(u32, u32, u32, u32)>;

/// 一段抽例：源码 ＋ 参照导出的指令流（`--CODE--` 分隔嵌套 code object）。
struct Case {
    name: &'static str,
    source: &'static str,
    expected: &'static [(&'static str, i64, Position)],
}

fn instruction_stream(unit: &CompiledUnit, out: &mut Vec<(String, i64, Position)>) {
    pyawa_core::decode::validate(&unit.code).expect("产物必须过 validate");
    let mut offset = 0usize;
    let mut index = 0usize;
    let mut extended: u32 = 0;
    while offset + 1 < unit.code.len() {
        let opcode = u16::from(unit.code[offset]);
        let arg = u32::from(unit.code[offset + 1]);
        let opname = pyawa_core::opcode::opname(opcode).expect("编号在表里").to_owned();
        let merged = arg | (extended << 8);
        let position = unit.positions[index];
        let position = match (position.0, position.1) {
            (Some(start), Some(end)) => Some((start, end, position.2.unwrap_or(0), position.3.unwrap_or(0))),
            _ => None,
        };
        out.push((opname.clone(), i64::from(merged), position));
        if opname != "EXTENDED_ARG" {
            extended = 0;
        } else {
            extended = merged;
        }
        offset += 2 + 2 * pyawa_core::opcode::inline_cache_entries(opcode) as usize;
        index += 1;
    }
    for constant in &unit.constants {
        if let Constant::Code(inner) = constant {
            out.push(("--CODE--".to_owned(), 0, None));
            instruction_stream(inner, out);
        }
    }
}

fn norm(expected: &[(&str, i64, Position)]) -> Vec<(String, i64, Position)> {
    expected
        .iter()
        // 参照的无参指令 `arg=None` 记成 `-1`；本层发 `0` ⇒ 归一 ✓
        .map(|(name, arg, position)| ((*name).to_owned(), if *arg < 0 { 0 } else { *arg }, *position))
        .collect()
}

const CASES: &[Case] = &[
    Case {
        name: "nested_tuple_module",
        source: "(a, b), (c, d) = (1, 2), (3, 4)\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 2, Some((1, 1, 17, 23))),
            ("LOAD_CONST", 3, Some((1, 1, 25, 31))),
            ("SWAP", 2, Some((1, 1, 0, 14))),
            ("UNPACK_SEQUENCE", 2, Some((1, 1, 0, 6))),
            ("STORE_NAME", 0, Some((1, 1, 1, 2))),
            ("STORE_NAME", 1, Some((1, 1, 4, 5))),
            ("UNPACK_SEQUENCE", 2, Some((1, 1, 8, 14))),
            ("STORE_NAME", 2, Some((1, 1, 9, 10))),
            ("STORE_NAME", 3, Some((1, 1, 12, 13))),
            ("LOAD_CONST", 1, Some((1, 1, 12, 13))),
            ("RETURN_VALUE", -1, Some((1, 1, 12, 13))),
        ],
    },
    Case {
        name: "nested_tuple_function",
        source: "def f():\n    a, (b, c) = x\n    return a\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 3, 0, 12))),
            ("MAKE_FUNCTION", -1, Some((1, 3, 0, 12))),
            ("STORE_NAME", 0, Some((1, 3, 0, 12))),
            ("LOAD_CONST", 1, Some((1, 3, 0, 12))),
            ("RETURN_VALUE", -1, Some((1, 3, 0, 12))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_GLOBAL", 0, Some((2, 2, 16, 17))),
            ("UNPACK_SEQUENCE", 2, Some((2, 2, 4, 13))),
            ("STORE_FAST", 0, Some((2, 2, 4, 5))),
            ("UNPACK_SEQUENCE", 2, Some((2, 2, 7, 13))),
            ("STORE_FAST_STORE_FAST", 18, Some((2, 2, 8, 9))),
            ("LOAD_FAST_BORROW", 0, Some((3, 3, 11, 12))),
            ("RETURN_VALUE", -1, Some((3, 3, 4, 12))),
        ],
    },
    Case {
        name: "nested_tuple_both_groups",
        source: "def f():\n    (a, b), (c, d) = x, y\n    return a\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 3, 0, 12))),
            ("MAKE_FUNCTION", -1, Some((1, 3, 0, 12))),
            ("STORE_NAME", 0, Some((1, 3, 0, 12))),
            ("LOAD_CONST", 1, Some((1, 3, 0, 12))),
            ("RETURN_VALUE", -1, Some((1, 3, 0, 12))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_GLOBAL", 0, Some((2, 2, 21, 22))),
            ("LOAD_GLOBAL", 2, Some((2, 2, 24, 25))),
            ("SWAP", 2, Some((2, 2, 4, 18))),
            ("UNPACK_SEQUENCE", 2, Some((2, 2, 4, 10))),
            ("STORE_FAST_STORE_FAST", 1, Some((2, 2, 5, 6))),
            ("UNPACK_SEQUENCE", 2, Some((2, 2, 12, 18))),
            ("STORE_FAST_STORE_FAST", 35, Some((2, 2, 13, 14))),
            ("LOAD_FAST_BORROW", 0, Some((3, 3, 11, 12))),
            ("RETURN_VALUE", -1, Some((3, 3, 4, 12))),
        ],
    },
    Case {
        name: "nested_tuple_starred",
        source: "def f():\n    a, (b, *c) = x\n    return a\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 3, 0, 12))),
            ("MAKE_FUNCTION", -1, Some((1, 3, 0, 12))),
            ("STORE_NAME", 0, Some((1, 3, 0, 12))),
            ("LOAD_CONST", 1, Some((1, 3, 0, 12))),
            ("RETURN_VALUE", -1, Some((1, 3, 0, 12))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_GLOBAL", 0, Some((2, 2, 17, 18))),
            ("UNPACK_SEQUENCE", 2, Some((2, 2, 4, 14))),
            ("STORE_FAST", 0, Some((2, 2, 4, 5))),
            ("UNPACK_EX", 1, Some((2, 2, 7, 14))),
            ("STORE_FAST_STORE_FAST", 18, Some((2, 2, 8, 9))),
            ("LOAD_FAST_BORROW", 0, Some((3, 3, 11, 12))),
            ("RETURN_VALUE", -1, Some((3, 3, 4, 12))),
        ],
    },
    Case {
        name: "with_tuple_target",
        source: "with m as (a, b):\n    x = a\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 5, 6))),
            ("COPY", 1, Some((1, 1, 5, 6))),
            ("LOAD_SPECIAL", 1, Some((1, 1, 5, 6))),
            ("SWAP", 2, Some((1, 1, 5, 6))),
            ("SWAP", 3, Some((1, 1, 5, 6))),
            ("LOAD_SPECIAL", 0, Some((1, 1, 5, 6))),
            ("CALL", 0, Some((1, 1, 5, 6))),
            ("UNPACK_SEQUENCE", 2, Some((1, 1, 10, 16))),
            ("STORE_NAME", 1, Some((1, 1, 11, 12))),
            ("STORE_NAME", 2, Some((1, 1, 14, 15))),
            ("LOAD_NAME", 1, Some((2, 2, 8, 9))),
            ("STORE_NAME", 3, Some((2, 2, 4, 5))),
            ("LOAD_CONST", 0, Some((1, 1, 5, 6))),
            ("LOAD_CONST", 0, Some((1, 1, 5, 6))),
            ("LOAD_CONST", 0, Some((1, 1, 5, 6))),
            ("CALL", 3, Some((1, 1, 5, 6))),
            ("POP_TOP", -1, Some((1, 1, 5, 6))),
            ("LOAD_CONST", 0, Some((1, 1, 5, 6))),
            ("RETURN_VALUE", -1, Some((1, 1, 5, 6))),
            ("PUSH_EXC_INFO", -1, Some((1, 1, 5, 6))),
            ("WITH_EXCEPT_START", -1, Some((1, 1, 5, 6))),
            ("TO_BOOL", -1, Some((1, 1, 5, 6))),
            ("POP_JUMP_IF_TRUE", 2, Some((1, 1, 5, 6))),
            ("NOT_TAKEN", -1, Some((1, 1, 5, 6))),
            ("RERAISE", 2, Some((1, 1, 5, 6))),
            ("POP_TOP", -1, Some((1, 1, 5, 6))),
            ("POP_EXCEPT", -1, Some((1, 1, 5, 6))),
            ("POP_TOP", -1, Some((1, 1, 5, 6))),
            ("POP_TOP", -1, Some((1, 1, 5, 6))),
            ("POP_TOP", -1, Some((1, 1, 5, 6))),
            ("LOAD_CONST", 0, Some((1, 1, 5, 6))),
            ("RETURN_VALUE", -1, Some((1, 1, 5, 6))),
            ("COPY", 3, None),
            ("POP_EXCEPT", -1, None),
            ("RERAISE", 1, None),
        ],
    },
    Case {
        name: "comprehension_bracket_pattern",
        source: "x = [off for [off] in data]\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 22, 26))),
            ("GET_ITER", -1, Some((1, 1, 22, 26))),
            ("LOAD_FAST_AND_CLEAR", 0, Some((1, 1, 4, 27))),
            ("SWAP", 2, Some((1, 1, 4, 27))),
            ("BUILD_LIST", 0, Some((1, 1, 4, 27))),
            ("SWAP", 2, Some((1, 1, 4, 27))),
            ("FOR_ITER", 6, Some((1, 1, 22, 26))),
            ("UNPACK_SEQUENCE", 1, Some((1, 1, 13, 18))),
            ("STORE_FAST_LOAD_FAST", 0, Some((1, 1, 14, 17))),
            ("LIST_APPEND", 2, Some((1, 1, 5, 8))),
            ("JUMP_BACKWARD", 8, Some((1, 1, 5, 8))),
            ("END_FOR", -1, Some((1, 1, 22, 26))),
            ("POP_ITER", -1, Some((1, 1, 22, 26))),
            ("SWAP", 2, Some((1, 1, 4, 27))),
            ("STORE_FAST", 0, Some((1, 1, 4, 27))),
            ("STORE_NAME", 1, Some((1, 1, 0, 1))),
            ("LOAD_CONST", 0, Some((1, 1, 0, 1))),
            ("RETURN_VALUE", -1, Some((1, 1, 0, 1))),
            ("SWAP", 2, None),
            ("POP_TOP", -1, None),
            ("SWAP", 2, Some((1, 1, 4, 27))),
            ("STORE_FAST", 0, Some((1, 1, 4, 27))),
            ("RERAISE", 0, Some((1, 1, 4, 27))),
        ],
    },
    Case {
        name: "comprehension_paren_tuple",
        source: "x = [v for (k, v) in data]\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 21, 25))),
            ("GET_ITER", -1, Some((1, 1, 21, 25))),
            ("LOAD_FAST_AND_CLEAR", 0, Some((1, 1, 4, 26))),
            ("LOAD_FAST_AND_CLEAR", 1, Some((1, 1, 4, 26))),
            ("SWAP", 3, Some((1, 1, 4, 26))),
            ("BUILD_LIST", 0, Some((1, 1, 4, 26))),
            ("SWAP", 2, Some((1, 1, 4, 26))),
            ("FOR_ITER", 7, Some((1, 1, 21, 25))),
            ("UNPACK_SEQUENCE", 2, Some((1, 1, 11, 17))),
            ("STORE_FAST_STORE_FAST", 1, Some((1, 1, 12, 13))),
            ("LOAD_FAST_BORROW", 1, Some((1, 1, 5, 6))),
            ("LIST_APPEND", 2, Some((1, 1, 5, 6))),
            ("JUMP_BACKWARD", 9, Some((1, 1, 5, 6))),
            ("END_FOR", -1, Some((1, 1, 21, 25))),
            ("POP_ITER", -1, Some((1, 1, 21, 25))),
            ("SWAP", 3, Some((1, 1, 4, 26))),
            ("STORE_FAST", 1, Some((1, 1, 4, 26))),
            ("STORE_FAST", 0, Some((1, 1, 4, 26))),
            ("STORE_NAME", 1, Some((1, 1, 0, 1))),
            ("LOAD_CONST", 0, Some((1, 1, 0, 1))),
            ("RETURN_VALUE", -1, Some((1, 1, 0, 1))),
            ("SWAP", 2, None),
            ("POP_TOP", -1, None),
            ("SWAP", 3, Some((1, 1, 4, 26))),
            ("STORE_FAST", 1, Some((1, 1, 4, 26))),
            ("STORE_FAST", 0, Some((1, 1, 4, 26))),
            ("RERAISE", 0, Some((1, 1, 4, 26))),
        ],
    },
    Case {
        name: "list_target_nested",
        source: "[a, (b, c)] = x\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 14, 15))),
            ("UNPACK_SEQUENCE", 2, Some((1, 1, 0, 11))),
            ("STORE_NAME", 1, Some((1, 1, 1, 2))),
            ("UNPACK_SEQUENCE", 2, Some((1, 1, 4, 10))),
            ("STORE_NAME", 2, Some((1, 1, 5, 6))),
            ("STORE_NAME", 3, Some((1, 1, 8, 9))),
            ("LOAD_CONST", 0, Some((1, 1, 8, 9))),
            ("RETURN_VALUE", -1, Some((1, 1, 8, 9))),
        ],
    },
    Case {
        name: "call_base_subscript_assign",
        source: "f()[0] = 1\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((1, 1, 9, 10))),
            ("LOAD_NAME", 0, Some((1, 1, 0, 1))),
            ("PUSH_NULL", -1, Some((1, 1, 0, 1))),
            ("CALL", 0, Some((1, 1, 0, 3))),
            ("LOAD_SMALL_INT", 0, Some((1, 1, 4, 5))),
            ("STORE_SUBSCR", -1, Some((1, 1, 0, 6))),
            ("LOAD_CONST", 1, Some((1, 1, 0, 6))),
            ("RETURN_VALUE", -1, Some((1, 1, 0, 6))),
        ],
    },
    Case {
        name: "leading_dot_float",
        source: "y = .5\nx = min(1, .05)\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 1, 4, 6))),
            ("STORE_NAME", 0, Some((1, 1, 0, 1))),
            ("LOAD_NAME", 1, Some((2, 2, 4, 7))),
            ("PUSH_NULL", -1, Some((2, 2, 4, 7))),
            ("LOAD_SMALL_INT", 1, Some((2, 2, 8, 9))),
            ("LOAD_CONST", 1, Some((2, 2, 11, 14))),
            ("CALL", 2, Some((2, 2, 4, 15))),
            ("STORE_NAME", 2, Some((2, 2, 0, 1))),
            ("LOAD_CONST", 2, Some((2, 2, 0, 1))),
            ("RETURN_VALUE", -1, Some((2, 2, 0, 1))),
        ],
    },
    Case {
        name: "star_in_tuple_display",
        source: "t = 1, *args\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((1, 1, 4, 5))),
            ("BUILD_LIST", 1, Some((1, 1, 4, 12))),
            ("LOAD_NAME", 0, Some((1, 1, 8, 12))),
            ("LIST_EXTEND", 1, Some((1, 1, 4, 12))),
            ("CALL_INTRINSIC_1", 6, Some((1, 1, 4, 12))),
            ("STORE_NAME", 1, Some((1, 1, 0, 1))),
            ("LOAD_CONST", 1, Some((1, 1, 0, 1))),
            ("RETURN_VALUE", -1, Some((1, 1, 0, 1))),
        ],
    },
    Case {
        name: "star_first_tuple_display",
        source: "x = *a,\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("BUILD_LIST", 0, Some((1, 1, 4, 7))),
            ("LOAD_NAME", 0, Some((1, 1, 5, 6))),
            ("LIST_EXTEND", 1, Some((1, 1, 4, 7))),
            ("CALL_INTRINSIC_1", 6, Some((1, 1, 4, 7))),
            ("STORE_NAME", 1, Some((1, 1, 0, 1))),
            ("LOAD_CONST", 0, Some((1, 1, 0, 1))),
            ("RETURN_VALUE", -1, Some((1, 1, 0, 1))),
        ],
    },
    Case {
        name: "star_after_keyword_arg",
        source: "f(obj=1, *[2])\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 0, 1))),
            ("PUSH_NULL", -1, Some((1, 1, 0, 1))),
            ("LOAD_SMALL_INT", 2, Some((1, 1, 11, 12))),
            ("BUILD_LIST", 1, Some((1, 1, 10, 13))),
            ("LOAD_CONST", 1, Some((1, 1, 0, 14))),
            ("LOAD_SMALL_INT", 1, Some((1, 1, 6, 7))),
            ("BUILD_MAP", 1, Some((1, 1, 0, 14))),
            ("CALL_FUNCTION_EX", -1, Some((1, 1, 0, 14))),
            ("POP_TOP", -1, Some((1, 1, 0, 14))),
            ("LOAD_CONST", 2, Some((1, 1, 0, 14))),
            ("RETURN_VALUE", -1, Some((1, 1, 0, 14))),
        ],
    },
    Case {
        name: "inline_if_semicolon_else",
        source: "if flag: x = 1; y = 2\nelse: x = 3; y = 4\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 3, 7))),
            ("TO_BOOL", -1, Some((1, 1, 3, 7))),
            ("POP_JUMP_IF_FALSE", 7, Some((1, 1, 3, 7))),
            ("NOT_TAKEN", -1, Some((1, 1, 3, 7))),
            ("LOAD_SMALL_INT", 1, Some((1, 1, 13, 14))),
            ("STORE_NAME", 1, Some((1, 1, 9, 10))),
            ("LOAD_SMALL_INT", 2, Some((1, 1, 20, 21))),
            ("STORE_NAME", 2, Some((1, 1, 16, 17))),
            ("LOAD_CONST", 1, Some((1, 1, 16, 17))),
            ("RETURN_VALUE", -1, Some((1, 1, 16, 17))),
            ("LOAD_SMALL_INT", 3, Some((2, 2, 10, 11))),
            ("STORE_NAME", 1, Some((2, 2, 6, 7))),
            ("LOAD_SMALL_INT", 4, Some((2, 2, 17, 18))),
            ("STORE_NAME", 2, Some((2, 2, 13, 14))),
            ("LOAD_CONST", 1, Some((2, 2, 13, 14))),
            ("RETURN_VALUE", -1, Some((2, 2, 13, 14))),
        ],
    },
    Case {
        name: "annotated_attr_target",
        source: "class C:\n    def __init__(self):\n        self.x: int = 1\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_BUILD_CLASS", -1, Some((1, 3, 0, 23))),
            ("PUSH_NULL", -1, Some((1, 3, 0, 23))),
            ("LOAD_CONST", 0, Some((1, 3, 0, 23))),
            ("MAKE_FUNCTION", -1, Some((1, 3, 0, 23))),
            ("LOAD_CONST", 1, Some((1, 3, 0, 23))),
            ("CALL", 2, Some((1, 3, 0, 23))),
            ("STORE_NAME", 0, Some((1, 3, 0, 23))),
            ("LOAD_CONST", 2, Some((1, 3, 0, 23))),
            ("RETURN_VALUE", -1, Some((1, 3, 0, 23))),
            ("--CODE--", -1, None),
            ("MAKE_CELL", 0, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 0, 0))),
            ("STORE_NAME", 1, Some((1, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 1, 0, 0))),
            ("STORE_NAME", 2, Some((1, 1, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((1, 1, 0, 0))),
            ("STORE_NAME", 3, Some((1, 1, 0, 0))),
            ("LOAD_LOCALS", -1, Some((1, 1, 0, 0))),
            ("STORE_DEREF", 0, Some((1, 1, 0, 0))),
            ("LOAD_CONST", 1, Some((2, 3, 4, 23))),
            ("MAKE_FUNCTION", -1, Some((2, 3, 4, 23))),
            ("STORE_NAME", 4, Some((2, 3, 4, 23))),
            ("LOAD_CONST", 2, Some((2, 3, 4, 23))),
            ("STORE_NAME", 5, Some((2, 3, 4, 23))),
            ("LOAD_FAST_BORROW", 0, Some((2, 3, 4, 23))),
            ("STORE_NAME", 6, Some((2, 3, 4, 23))),
            ("LOAD_CONST", 3, Some((2, 3, 4, 23))),
            ("RETURN_VALUE", -1, Some((2, 3, 4, 23))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((2, 2, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((3, 3, 22, 23))),
            ("LOAD_FAST_BORROW", 0, Some((3, 3, 8, 12))),
            ("STORE_ATTR", 0, Some((3, 3, 8, 14))),
            ("LOAD_CONST", 1, Some((3, 3, 8, 14))),
            ("RETURN_VALUE", -1, Some((3, 3, 8, 14))),
        ],
    },
    Case {
        name: "annotated_subscript_target",
        source: "class C:\n    def __init__(self):\n        self.a[0]: int = 1\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_BUILD_CLASS", -1, Some((1, 3, 0, 26))),
            ("PUSH_NULL", -1, Some((1, 3, 0, 26))),
            ("LOAD_CONST", 0, Some((1, 3, 0, 26))),
            ("MAKE_FUNCTION", -1, Some((1, 3, 0, 26))),
            ("LOAD_CONST", 1, Some((1, 3, 0, 26))),
            ("CALL", 2, Some((1, 3, 0, 26))),
            ("STORE_NAME", 0, Some((1, 3, 0, 26))),
            ("LOAD_CONST", 2, Some((1, 3, 0, 26))),
            ("RETURN_VALUE", -1, Some((1, 3, 0, 26))),
            ("--CODE--", -1, None),
            ("MAKE_CELL", 0, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_NAME", 0, Some((1, 1, 0, 0))),
            ("STORE_NAME", 1, Some((1, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 1, 0, 0))),
            ("STORE_NAME", 2, Some((1, 1, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((1, 1, 0, 0))),
            ("STORE_NAME", 3, Some((1, 1, 0, 0))),
            ("LOAD_LOCALS", -1, Some((1, 1, 0, 0))),
            ("STORE_DEREF", 0, Some((1, 1, 0, 0))),
            ("LOAD_CONST", 1, Some((2, 3, 4, 26))),
            ("MAKE_FUNCTION", -1, Some((2, 3, 4, 26))),
            ("STORE_NAME", 4, Some((2, 3, 4, 26))),
            ("LOAD_CONST", 2, Some((2, 3, 4, 26))),
            ("STORE_NAME", 5, Some((2, 3, 4, 26))),
            ("LOAD_FAST_BORROW", 0, Some((2, 3, 4, 26))),
            ("STORE_NAME", 6, Some((2, 3, 4, 26))),
            ("LOAD_CONST", 3, Some((2, 3, 4, 26))),
            ("RETURN_VALUE", -1, Some((2, 3, 4, 26))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((2, 2, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((3, 3, 25, 26))),
            ("LOAD_FAST_BORROW", 0, Some((3, 3, 8, 12))),
            ("LOAD_ATTR", 0, Some((3, 3, 8, 14))),
            ("LOAD_SMALL_INT", 0, Some((3, 3, 15, 16))),
            ("STORE_SUBSCR", -1, Some((3, 3, 8, 17))),
            ("LOAD_CONST", 1, Some((3, 3, 8, 17))),
            ("RETURN_VALUE", -1, Some((3, 3, 8, 17))),
        ],
    },
    Case {
        name: "tab_indented_code",
        source: "def f():\n\ta = 1\n\treturn a\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 3, 0, 9))),
            ("MAKE_FUNCTION", -1, Some((1, 3, 0, 9))),
            ("STORE_NAME", 0, Some((1, 3, 0, 9))),
            ("LOAD_CONST", 1, Some((1, 3, 0, 9))),
            ("RETURN_VALUE", -1, Some((1, 3, 0, 9))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((2, 2, 5, 6))),
            ("STORE_FAST", 0, Some((2, 2, 1, 2))),
            ("LOAD_FAST_BORROW", 0, Some((3, 3, 8, 9))),
            ("RETURN_VALUE", -1, Some((3, 3, 1, 9))),
        ],
    },
    Case {
        name: "tab_comment_indented",
        source: "def f():\n    a = 1\n\t# comment\n    return a\n",
        expected: &[
            ("RESUME", 0, Some((0, 1, 0, 0))),
            ("LOAD_CONST", 0, Some((1, 4, 0, 12))),
            ("MAKE_FUNCTION", -1, Some((1, 4, 0, 12))),
            ("STORE_NAME", 0, Some((1, 4, 0, 12))),
            ("LOAD_CONST", 1, Some((1, 4, 0, 12))),
            ("RETURN_VALUE", -1, Some((1, 4, 0, 12))),
            ("--CODE--", -1, None),
            ("RESUME", 0, Some((1, 1, 0, 0))),
            ("LOAD_SMALL_INT", 1, Some((2, 2, 8, 9))),
            ("STORE_FAST", 0, Some((2, 2, 4, 5))),
            ("LOAD_FAST_BORROW", 0, Some((4, 4, 11, 12))),
            ("RETURN_VALUE", -1, Some((4, 4, 4, 12))),
        ],
    },
];

#[test]
fn parser_batch_shapes_match_the_reference_instruction_by_instruction() {
    for case in CASES {
        let unit = compile(case.source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0)
            .unwrap_or_else(|error| panic!("{}{:?}", case.name, error));
        let mut actual = Vec::new();
        instruction_stream(&unit, &mut actual);
        assert_eq!(actual, norm(case.expected), "{} 的指令流", case.name);
    }
}

/// **task-25：P3-35 语法缺口批 A** 的"能编过"回归（不逐条对拍指令流——这些形状是本批
/// 新接的，参照对拍随后补；先钉住"合法 Python 不得被拒"这一条）：
///
/// ① 行内套件体（`try:`／`except:`／`else:`／`finally:`／`with:`，含 `for/while` 的 `else:`）；
/// ② `for` 目标里的 `*`；③ 链式赋值的**元组目标**；④ 带注解的**非名字目标**；
/// ⑤ `with … as` 的**属性／下标目标**（含调用链）。
#[test]
fn task25_syntax_gaps_compile() {
    let sources: &[(&str, &str)] = &[
        (
            "inline_suites",
            "def f(x):\n    try: x = 1\n    except ValueError: pass\n    else: x = 2\n    finally: x = 3\n    return x\n",
        ),
        (
            "inline_with_for_else",
            "def f(xs):\n    for a in xs: pass\n    else: pass\n    return 1\n",
        ),
        (
            "inline_while_else",
            "def f(x):\n    while x: x = x - 1\n    else: x = 2\n    return x\n",
        ),
        (
            "for_star_target",
            "def f(items):\n    for full_name, *_ in items:\n        return full_name\n",
        ),
        (
            "chained_tuple_target",
            "def f(a, b):\n    chan = _, send = a, b\n    return chan, send\n",
        ),
        (
            "annotated_paren_name",
            "(pars): bool = True\n",
        ),
        (
            "annotated_attr_via_call",
            "def f(new):\n    new().attr: object = 1\n",
        ),
        (
            "with_attr_target",
            "def f(cm, holder):\n    with cm as holder.value:\n        pass\n",
        ),
        (
            "with_subscript_call_target",
            "def f(cm, values):\n    with cm as list(values)[0]:\n        pass\n",
        ),
        // **表达式语句的元组显示** ✓（尾逗号；官方 `test_clinic`／`test_cprofile` 一族）
        (
            "tuple_display_statement",
            "def f(g):\n    g(a=0),\n    g(1), g(2),\n",
        ),
        // **字面量表达式语句** ✓（`{1: 2}`／`{1, 2}` 成句；官方 `test_dict`／`test_setcomps`）
        (
            "dict_set_literal_statement",
            "{1: 2}\n{1, 2}\n[1][0]\n",
        ),
        // 增强赋值的右值可以是元组显示
        (
            "augassign_tuple_rhs",
            "def f(x, y):\n    x += y,\n",
        ),
    ];
    for (name, source) in sources {
        compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0)
            .unwrap_or_else(|error| panic!("{name} 编不过：{error:?}"));
    }
}
