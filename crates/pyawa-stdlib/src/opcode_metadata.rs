//! 指令表与元数据——**数值的唯一出处**（`BC-38`）。
//!
//! 由 `tools/gen_opcode_tables.py` 从本机 CPython 运行时探测生成：**禁止手改**；
//! 重生成：`python3 tools/gen_opcode_tables.py`（本文件生成时的基线：CPython 3.14.4）。
//!
//! 基线＝CPython 3.14 的指令名与编号（`BC-30`），此处**不发明指令、不改名**。

/// `BC-40`：指令集版本常量——增删指令、语义变化、cache 宽度变化**必须**递增；
/// `BC-41` 要求它随 `.pyac` 头部一起判陈旧。取值由实现维护。
pub const INSTRUCTION_SET_VERSION: u32 = 1;

/// `BC-30`（实测）：有参指令的编号下界。
pub const HAVE_ARGUMENT: u16 = 43;

/// `BC-30`／`BC-32`：instrumented 区段的下界；Pyawa **禁止发射**该区段（`BC-32`）。
pub const MIN_INSTRUMENTED_OPCODE: u16 = 234;

/// 名字 → 编号，按名字升序（`BC-30`：与 CPython 3.14 的 `opmap` 全等）。
pub static OPMAP: &[(&str, u16)] = &[
    ("ANNOTATIONS_PLACEHOLDER", 256), ("BINARY_OP", 44), ("BINARY_SLICE", 1),
    ("BUILD_INTERPOLATION", 45), ("BUILD_LIST", 46), ("BUILD_MAP", 47),
    ("BUILD_SET", 48), ("BUILD_SLICE", 49), ("BUILD_STRING", 50),
    ("BUILD_TEMPLATE", 2), ("BUILD_TUPLE", 51), ("CACHE", 0),
    ("CALL", 52), ("CALL_FUNCTION_EX", 4), ("CALL_INTRINSIC_1", 53),
    ("CALL_INTRINSIC_2", 54), ("CALL_KW", 55), ("CHECK_EG_MATCH", 5),
    ("CHECK_EXC_MATCH", 6), ("CLEANUP_THROW", 7), ("COMPARE_OP", 56),
    ("CONTAINS_OP", 57), ("CONVERT_VALUE", 58), ("COPY", 59),
    ("COPY_FREE_VARS", 60), ("DELETE_ATTR", 61), ("DELETE_DEREF", 62),
    ("DELETE_FAST", 63), ("DELETE_GLOBAL", 64), ("DELETE_NAME", 65),
    ("DELETE_SUBSCR", 8), ("DICT_MERGE", 66), ("DICT_UPDATE", 67),
    ("END_ASYNC_FOR", 68), ("END_FOR", 9), ("END_SEND", 10),
    ("ENTER_EXECUTOR", 255), ("EXIT_INIT_CHECK", 11), ("EXTENDED_ARG", 69),
    ("FORMAT_SIMPLE", 12), ("FORMAT_WITH_SPEC", 13), ("FOR_ITER", 70),
    ("GET_AITER", 14), ("GET_ANEXT", 15), ("GET_AWAITABLE", 71),
    ("GET_ITER", 16), ("GET_LEN", 18), ("GET_YIELD_FROM_ITER", 19),
    ("IMPORT_FROM", 72), ("IMPORT_NAME", 73), ("INSTRUMENTED_CALL", 250),
    ("INSTRUMENTED_CALL_FUNCTION_EX", 252), ("INSTRUMENTED_CALL_KW", 251), ("INSTRUMENTED_END_ASYNC_FOR", 248),
    ("INSTRUMENTED_END_FOR", 234), ("INSTRUMENTED_END_SEND", 236), ("INSTRUMENTED_FOR_ITER", 237),
    ("INSTRUMENTED_INSTRUCTION", 238), ("INSTRUMENTED_JUMP_BACKWARD", 253), ("INSTRUMENTED_JUMP_FORWARD", 239),
    ("INSTRUMENTED_LINE", 254), ("INSTRUMENTED_LOAD_SUPER_ATTR", 249), ("INSTRUMENTED_NOT_TAKEN", 240),
    ("INSTRUMENTED_POP_ITER", 235), ("INSTRUMENTED_POP_JUMP_IF_FALSE", 242), ("INSTRUMENTED_POP_JUMP_IF_NONE", 243),
    ("INSTRUMENTED_POP_JUMP_IF_NOT_NONE", 244), ("INSTRUMENTED_POP_JUMP_IF_TRUE", 241), ("INSTRUMENTED_RESUME", 245),
    ("INSTRUMENTED_RETURN_VALUE", 246), ("INSTRUMENTED_YIELD_VALUE", 247), ("INTERPRETER_EXIT", 20),
    ("IS_OP", 74), ("JUMP", 257), ("JUMP_BACKWARD", 75),
    ("JUMP_BACKWARD_NO_INTERRUPT", 76), ("JUMP_FORWARD", 77), ("JUMP_IF_FALSE", 258),
    ("JUMP_IF_TRUE", 259), ("JUMP_NO_INTERRUPT", 260), ("LIST_APPEND", 78),
    ("LIST_EXTEND", 79), ("LOAD_ATTR", 80), ("LOAD_BUILD_CLASS", 21),
    ("LOAD_CLOSURE", 261), ("LOAD_COMMON_CONSTANT", 81), ("LOAD_CONST", 82),
    ("LOAD_DEREF", 83), ("LOAD_FAST", 84), ("LOAD_FAST_AND_CLEAR", 85),
    ("LOAD_FAST_BORROW", 86), ("LOAD_FAST_BORROW_LOAD_FAST_BORROW", 87), ("LOAD_FAST_CHECK", 88),
    ("LOAD_FAST_LOAD_FAST", 89), ("LOAD_FROM_DICT_OR_DEREF", 90), ("LOAD_FROM_DICT_OR_GLOBALS", 91),
    ("LOAD_GLOBAL", 92), ("LOAD_LOCALS", 22), ("LOAD_NAME", 93),
    ("LOAD_SMALL_INT", 94), ("LOAD_SPECIAL", 95), ("LOAD_SUPER_ATTR", 96),
    ("MAKE_CELL", 97), ("MAKE_FUNCTION", 23), ("MAP_ADD", 98),
    ("MATCH_CLASS", 99), ("MATCH_KEYS", 24), ("MATCH_MAPPING", 25),
    ("MATCH_SEQUENCE", 26), ("NOP", 27), ("NOT_TAKEN", 28),
    ("POP_BLOCK", 262), ("POP_EXCEPT", 29), ("POP_ITER", 30),
    ("POP_JUMP_IF_FALSE", 100), ("POP_JUMP_IF_NONE", 101), ("POP_JUMP_IF_NOT_NONE", 102),
    ("POP_JUMP_IF_TRUE", 103), ("POP_TOP", 31), ("PUSH_EXC_INFO", 32),
    ("PUSH_NULL", 33), ("RAISE_VARARGS", 104), ("RERAISE", 105),
    ("RESERVED", 17), ("RESUME", 128), ("RETURN_GENERATOR", 34),
    ("RETURN_VALUE", 35), ("SEND", 106), ("SETUP_ANNOTATIONS", 36),
    ("SETUP_CLEANUP", 263), ("SETUP_FINALLY", 264), ("SETUP_WITH", 265),
    ("SET_ADD", 107), ("SET_FUNCTION_ATTRIBUTE", 108), ("SET_UPDATE", 109),
    ("STORE_ATTR", 110), ("STORE_DEREF", 111), ("STORE_FAST", 112),
    ("STORE_FAST_LOAD_FAST", 113), ("STORE_FAST_MAYBE_NULL", 266), ("STORE_FAST_STORE_FAST", 114),
    ("STORE_GLOBAL", 115), ("STORE_NAME", 116), ("STORE_SLICE", 37),
    ("STORE_SUBSCR", 38), ("SWAP", 117), ("TO_BOOL", 39),
    ("UNARY_INVERT", 40), ("UNARY_NEGATIVE", 41), ("UNARY_NOT", 42),
    ("UNPACK_EX", 118), ("UNPACK_SEQUENCE", 119), ("WITH_EXCEPT_START", 43),
    ("YIELD_VALUE", 120),
];

/// 编号 → 名字，按编号升序。
pub static OPNAME: &[(u16, &str)] = &[
    (0, "CACHE"), (1, "BINARY_SLICE"), (2, "BUILD_TEMPLATE"),
    (4, "CALL_FUNCTION_EX"), (5, "CHECK_EG_MATCH"), (6, "CHECK_EXC_MATCH"),
    (7, "CLEANUP_THROW"), (8, "DELETE_SUBSCR"), (9, "END_FOR"),
    (10, "END_SEND"), (11, "EXIT_INIT_CHECK"), (12, "FORMAT_SIMPLE"),
    (13, "FORMAT_WITH_SPEC"), (14, "GET_AITER"), (15, "GET_ANEXT"),
    (16, "GET_ITER"), (17, "RESERVED"), (18, "GET_LEN"),
    (19, "GET_YIELD_FROM_ITER"), (20, "INTERPRETER_EXIT"), (21, "LOAD_BUILD_CLASS"),
    (22, "LOAD_LOCALS"), (23, "MAKE_FUNCTION"), (24, "MATCH_KEYS"),
    (25, "MATCH_MAPPING"), (26, "MATCH_SEQUENCE"), (27, "NOP"),
    (28, "NOT_TAKEN"), (29, "POP_EXCEPT"), (30, "POP_ITER"),
    (31, "POP_TOP"), (32, "PUSH_EXC_INFO"), (33, "PUSH_NULL"),
    (34, "RETURN_GENERATOR"), (35, "RETURN_VALUE"), (36, "SETUP_ANNOTATIONS"),
    (37, "STORE_SLICE"), (38, "STORE_SUBSCR"), (39, "TO_BOOL"),
    (40, "UNARY_INVERT"), (41, "UNARY_NEGATIVE"), (42, "UNARY_NOT"),
    (43, "WITH_EXCEPT_START"), (44, "BINARY_OP"), (45, "BUILD_INTERPOLATION"),
    (46, "BUILD_LIST"), (47, "BUILD_MAP"), (48, "BUILD_SET"),
    (49, "BUILD_SLICE"), (50, "BUILD_STRING"), (51, "BUILD_TUPLE"),
    (52, "CALL"), (53, "CALL_INTRINSIC_1"), (54, "CALL_INTRINSIC_2"),
    (55, "CALL_KW"), (56, "COMPARE_OP"), (57, "CONTAINS_OP"),
    (58, "CONVERT_VALUE"), (59, "COPY"), (60, "COPY_FREE_VARS"),
    (61, "DELETE_ATTR"), (62, "DELETE_DEREF"), (63, "DELETE_FAST"),
    (64, "DELETE_GLOBAL"), (65, "DELETE_NAME"), (66, "DICT_MERGE"),
    (67, "DICT_UPDATE"), (68, "END_ASYNC_FOR"), (69, "EXTENDED_ARG"),
    (70, "FOR_ITER"), (71, "GET_AWAITABLE"), (72, "IMPORT_FROM"),
    (73, "IMPORT_NAME"), (74, "IS_OP"), (75, "JUMP_BACKWARD"),
    (76, "JUMP_BACKWARD_NO_INTERRUPT"), (77, "JUMP_FORWARD"), (78, "LIST_APPEND"),
    (79, "LIST_EXTEND"), (80, "LOAD_ATTR"), (81, "LOAD_COMMON_CONSTANT"),
    (82, "LOAD_CONST"), (83, "LOAD_DEREF"), (84, "LOAD_FAST"),
    (85, "LOAD_FAST_AND_CLEAR"), (86, "LOAD_FAST_BORROW"), (87, "LOAD_FAST_BORROW_LOAD_FAST_BORROW"),
    (88, "LOAD_FAST_CHECK"), (89, "LOAD_FAST_LOAD_FAST"), (90, "LOAD_FROM_DICT_OR_DEREF"),
    (91, "LOAD_FROM_DICT_OR_GLOBALS"), (92, "LOAD_GLOBAL"), (93, "LOAD_NAME"),
    (94, "LOAD_SMALL_INT"), (95, "LOAD_SPECIAL"), (96, "LOAD_SUPER_ATTR"),
    (97, "MAKE_CELL"), (98, "MAP_ADD"), (99, "MATCH_CLASS"),
    (100, "POP_JUMP_IF_FALSE"), (101, "POP_JUMP_IF_NONE"), (102, "POP_JUMP_IF_NOT_NONE"),
    (103, "POP_JUMP_IF_TRUE"), (104, "RAISE_VARARGS"), (105, "RERAISE"),
    (106, "SEND"), (107, "SET_ADD"), (108, "SET_FUNCTION_ATTRIBUTE"),
    (109, "SET_UPDATE"), (110, "STORE_ATTR"), (111, "STORE_DEREF"),
    (112, "STORE_FAST"), (113, "STORE_FAST_LOAD_FAST"), (114, "STORE_FAST_STORE_FAST"),
    (115, "STORE_GLOBAL"), (116, "STORE_NAME"), (117, "SWAP"),
    (118, "UNPACK_EX"), (119, "UNPACK_SEQUENCE"), (120, "YIELD_VALUE"),
    (128, "RESUME"), (234, "INSTRUMENTED_END_FOR"), (235, "INSTRUMENTED_POP_ITER"),
    (236, "INSTRUMENTED_END_SEND"), (237, "INSTRUMENTED_FOR_ITER"), (238, "INSTRUMENTED_INSTRUCTION"),
    (239, "INSTRUMENTED_JUMP_FORWARD"), (240, "INSTRUMENTED_NOT_TAKEN"), (241, "INSTRUMENTED_POP_JUMP_IF_TRUE"),
    (242, "INSTRUMENTED_POP_JUMP_IF_FALSE"), (243, "INSTRUMENTED_POP_JUMP_IF_NONE"), (244, "INSTRUMENTED_POP_JUMP_IF_NOT_NONE"),
    (245, "INSTRUMENTED_RESUME"), (246, "INSTRUMENTED_RETURN_VALUE"), (247, "INSTRUMENTED_YIELD_VALUE"),
    (248, "INSTRUMENTED_END_ASYNC_FOR"), (249, "INSTRUMENTED_LOAD_SUPER_ATTR"), (250, "INSTRUMENTED_CALL"),
    (251, "INSTRUMENTED_CALL_KW"), (252, "INSTRUMENTED_CALL_FUNCTION_EX"), (253, "INSTRUMENTED_JUMP_BACKWARD"),
    (254, "INSTRUMENTED_LINE"), (255, "ENTER_EXECUTOR"), (256, "ANNOTATIONS_PLACEHOLDER"),
    (257, "JUMP"), (258, "JUMP_IF_FALSE"), (259, "JUMP_IF_TRUE"),
    (260, "JUMP_NO_INTERRUPT"), (261, "LOAD_CLOSURE"), (262, "POP_BLOCK"),
    (263, "SETUP_CLEANUP"), (264, "SETUP_FINALLY"), (265, "SETUP_WITH"),
    (266, "STORE_FAST_MAYBE_NULL"),
];

/// `BC-31`：Pyawa 专有指令取**空闲编号**（不进 `OPMAP`——那是 CPython 基线）；
/// 编号在「未占用且低于 instrumented 区段」的空隙里取，生成时校验。
pub static PYAWA_SPECIFIC: &[(&str, u16)] = &[
    ("CHECK_BOUNDARY_IN", 233),
    ("CHECK_BOUNDARY_OUT", 232),
];

/// `BC-35`：inline cache 宽度（只列非零项；发射方**必须**留等宽零填充槽）。
pub static INLINE_CACHE_ENTRIES: &[(u16, u32)] = &[
    (38, 1), (39, 3), (44, 5), (52, 3), (55, 3), (56, 1),
    (57, 1), (70, 1), (75, 1), (80, 9), (92, 4), (96, 1),
    (100, 1), (101, 1), (102, 1), (103, 1), (106, 1), (110, 4),
    (119, 1),
];

/// `BC-37`：`has_arg` 为真的指令编号，升序。
pub static HAS_ARG: &[u16] = &[
    44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59,
    60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75,
    76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91,
    92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107,
    108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 128, 237, 239,
    241, 242, 243, 244, 245, 247, 248, 249, 250, 251, 253, 255, 257, 258, 259, 260,
    261, 263, 264, 265, 266,
];

/// `BC-37`：`has_const` 为真的指令编号，升序。
pub static HAS_CONST: &[u16] = &[
    82,
];

/// `BC-37`：`has_name` 为真的指令编号，升序。
pub static HAS_NAME: &[u16] = &[
    61, 64, 65, 72, 73, 80, 91, 92, 93, 96, 110, 115, 116, 249,
];

/// `BC-37`：`has_jump` 为真的指令编号，升序。
pub static HAS_JUMP: &[u16] = &[
    68, 70, 75, 76, 77, 100, 101, 102, 103, 106, 237, 248, 257, 258, 259, 260,
];

/// `BC-37`：`has_free` 为真的指令编号，升序。
pub static HAS_FREE: &[u16] = &[
    62, 90, 97, 111,
];

/// `BC-37`：`has_local` 为真的指令编号，升序。
pub static HAS_LOCAL: &[u16] = &[
    63, 83, 84, 85, 86, 87, 88, 89, 112, 113, 114, 261, 266,
];

/// `BC-37`：`has_exc` 为真的指令编号，升序。
pub static HAS_EXC: &[u16] = &[
    263, 264, 265,
];

/// `BC-39`：`BINARY_OP` 的 oparg 顺序（名字，运算符）；`NB_SUBSCR` 在最后。
pub static NB_OPS: &[(&str, &str)] = &[
    ("NB_ADD", "+"), ("NB_AND", "&"), ("NB_FLOOR_DIVIDE", "//"),
    ("NB_LSHIFT", "<<"), ("NB_MATRIX_MULTIPLY", "@"), ("NB_MULTIPLY", "*"),
    ("NB_REMAINDER", "%"), ("NB_OR", "|"), ("NB_POWER", "**"),
    ("NB_RSHIFT", ">>"), ("NB_SUBTRACT", "-"), ("NB_TRUE_DIVIDE", "/"),
    ("NB_XOR", "^"), ("NB_INPLACE_ADD", "+="), ("NB_INPLACE_AND", "&="),
    ("NB_INPLACE_FLOOR_DIVIDE", "//="), ("NB_INPLACE_LSHIFT", "<<="), ("NB_INPLACE_MATRIX_MULTIPLY", "@="),
    ("NB_INPLACE_MULTIPLY", "*="), ("NB_INPLACE_REMAINDER", "%="), ("NB_INPLACE_OR", "|="),
    ("NB_INPLACE_POWER", "**="), ("NB_INPLACE_RSHIFT", ">>="), ("NB_INPLACE_SUBTRACT", "-="),
    ("NB_INPLACE_TRUE_DIVIDE", "/="), ("NB_INPLACE_XOR", "^="), ("NB_SUBSCR", "[]"),
];

/// `_opcode.get_intrinsic1_descs()` 的返回值。
pub static INTRINSIC1_DESCS: &[&str] = &[
    "INTRINSIC_1_INVALID", "INTRINSIC_PRINT", "INTRINSIC_IMPORT_STAR",
    "INTRINSIC_STOPITERATION_ERROR", "INTRINSIC_ASYNC_GEN_WRAP", "INTRINSIC_UNARY_POSITIVE",
    "INTRINSIC_LIST_TO_TUPLE", "INTRINSIC_TYPEVAR", "INTRINSIC_PARAMSPEC",
    "INTRINSIC_TYPEVARTUPLE", "INTRINSIC_SUBSCRIPT_GENERIC", "INTRINSIC_TYPEALIAS",
];

/// `_opcode.get_intrinsic2_descs()` 的返回值。
pub static INTRINSIC2_DESCS: &[&str] = &[
    "INTRINSIC_2_INVALID", "INTRINSIC_PREP_RERAISE_STAR", "INTRINSIC_TYPEVAR_WITH_BOUND",
    "INTRINSIC_TYPEVAR_WITH_CONSTRAINTS", "INTRINSIC_SET_FUNCTION_TYPE_PARAMS", "INTRINSIC_SET_TYPEPARAM_DEFAULT",
];

/// `_opcode.get_special_method_names()` 的返回值。
pub static SPECIAL_METHOD_NAMES: &[&str] = &[
    "__enter__", "__exit__", "__aenter__",
    "__aexit__",
];

/// `BC-32`：Pyawa 不做 CPython 式特化，本表**必须为空**。
/// （CPython 3.14 运行时的同名表非空，那是参照实现的特化，**不照抄**。）
pub static SPECIALIZATIONS: &[(&str, &[&str])] = &[];

/// `BC-32`：同上，**必须为空**（`opcode.py` 的 `opname` 构造对空值安全）。
pub static SPECIALIZED_OPMAP: &[(&str, u16)] = &[];

/// `BC-38`：栈效应的规则形态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackRule {
    /// 与 oparg 无关。
    Const(i32),
    /// `factor * oparg + base`。
    Linear { factor: i32, base: i32 },
    /// 按 oparg 的奇偶取两个值。
    Parity { even: i32, odd: i32 },
    /// `UNPACK_EX`：低 8 位是前置个数、高 8 位是后置个数。
    UnpackEx,
}

/// `BC-38`：编号 → 栈效应规则（升序；`jump` 为 `None`／`True` 时用这一条）。
pub static STACK_EFFECTS: &[(u16, StackRule)] = &[
    (0, StackRule::Const(0)), (1, StackRule::Const(-2)),
    (2, StackRule::Const(-1)), (4, StackRule::Const(-3)),
    (5, StackRule::Const(0)), (6, StackRule::Const(0)),
    (7, StackRule::Const(-1)), (8, StackRule::Const(-2)),
    (9, StackRule::Const(-1)), (10, StackRule::Const(-1)),
    (11, StackRule::Const(-1)), (12, StackRule::Const(0)),
    (13, StackRule::Const(-1)), (14, StackRule::Const(0)),
    (15, StackRule::Const(1)), (16, StackRule::Const(0)),
    (17, StackRule::Const(0)), (18, StackRule::Const(1)),
    (19, StackRule::Const(0)), (20, StackRule::Const(-1)),
    (21, StackRule::Const(1)), (22, StackRule::Const(1)),
    (23, StackRule::Const(0)), (24, StackRule::Const(1)),
    (25, StackRule::Const(1)), (26, StackRule::Const(1)),
    (27, StackRule::Const(0)), (28, StackRule::Const(0)),
    (29, StackRule::Const(-1)), (30, StackRule::Const(-1)),
    (31, StackRule::Const(-1)), (32, StackRule::Const(1)),
    (33, StackRule::Const(1)), (34, StackRule::Const(1)),
    (35, StackRule::Const(0)), (36, StackRule::Const(0)),
    (37, StackRule::Const(-4)), (38, StackRule::Const(-3)),
    (39, StackRule::Const(0)), (40, StackRule::Const(0)),
    (41, StackRule::Const(0)), (42, StackRule::Const(0)),
    (43, StackRule::Const(1)), (44, StackRule::Const(-1)),
    (45, StackRule::Parity { even: -1, odd: -2 }), (46, StackRule::Linear { factor: -1, base: 1 }),
    (47, StackRule::Linear { factor: -2, base: 1 }), (48, StackRule::Linear { factor: -1, base: 1 }),
    (49, StackRule::Linear { factor: -1, base: 1 }), (50, StackRule::Linear { factor: -1, base: 1 }),
    (51, StackRule::Linear { factor: -1, base: 1 }), (52, StackRule::Linear { factor: -1, base: -1 }),
    (53, StackRule::Const(0)), (54, StackRule::Const(-1)),
    (55, StackRule::Linear { factor: -1, base: -2 }), (56, StackRule::Const(-1)),
    (57, StackRule::Const(-1)), (58, StackRule::Const(0)),
    (59, StackRule::Const(1)), (60, StackRule::Const(0)),
    (61, StackRule::Const(-1)), (62, StackRule::Const(0)),
    (63, StackRule::Const(0)), (64, StackRule::Const(0)),
    (65, StackRule::Const(0)), (66, StackRule::Const(-1)),
    (67, StackRule::Const(-1)), (68, StackRule::Const(-2)),
    (69, StackRule::Const(0)), (70, StackRule::Const(1)),
    (71, StackRule::Const(0)), (72, StackRule::Const(1)),
    (73, StackRule::Const(-1)), (74, StackRule::Const(-1)),
    (75, StackRule::Const(0)), (76, StackRule::Const(0)),
    (77, StackRule::Const(0)), (78, StackRule::Const(-1)),
    (79, StackRule::Const(-1)), (80, StackRule::Parity { even: 0, odd: 1 }),
    (81, StackRule::Const(1)), (82, StackRule::Const(1)),
    (83, StackRule::Const(1)), (84, StackRule::Const(1)),
    (85, StackRule::Const(1)), (86, StackRule::Const(1)),
    (87, StackRule::Const(2)), (88, StackRule::Const(1)),
    (89, StackRule::Const(2)), (90, StackRule::Const(0)),
    (91, StackRule::Const(0)), (92, StackRule::Parity { even: 1, odd: 2 }),
    (93, StackRule::Const(1)), (94, StackRule::Const(1)),
    (95, StackRule::Const(1)), (96, StackRule::Parity { even: -2, odd: -1 }),
    (97, StackRule::Const(0)), (98, StackRule::Const(-2)),
    (99, StackRule::Const(-2)), (100, StackRule::Const(-1)),
    (101, StackRule::Const(-1)), (102, StackRule::Const(-1)),
    (103, StackRule::Const(-1)), (104, StackRule::Linear { factor: -1, base: 0 }),
    (105, StackRule::Const(-1)), (106, StackRule::Const(0)),
    (107, StackRule::Const(-1)), (108, StackRule::Const(-1)),
    (109, StackRule::Const(-1)), (110, StackRule::Const(-2)),
    (111, StackRule::Const(-1)), (112, StackRule::Const(-1)),
    (113, StackRule::Const(0)), (114, StackRule::Const(-2)),
    (115, StackRule::Const(-1)), (116, StackRule::Const(-1)),
    (117, StackRule::Const(0)), (118, StackRule::UnpackEx),
    (119, StackRule::Linear { factor: 1, base: -1 }), (120, StackRule::Const(0)),
    (128, StackRule::Const(0)), (232, StackRule::Const(0)),
    (233, StackRule::Const(0)), (234, StackRule::Const(-1)),
    (235, StackRule::Const(-1)), (236, StackRule::Const(-1)),
    (237, StackRule::Const(1)), (238, StackRule::Const(0)),
    (239, StackRule::Const(0)), (240, StackRule::Const(0)),
    (241, StackRule::Const(-1)), (242, StackRule::Const(-1)),
    (243, StackRule::Const(-1)), (244, StackRule::Const(-1)),
    (245, StackRule::Const(0)), (246, StackRule::Const(0)),
    (247, StackRule::Const(0)), (248, StackRule::Const(-2)),
    (249, StackRule::Parity { even: -2, odd: -1 }), (250, StackRule::Linear { factor: -1, base: -1 }),
    (251, StackRule::Linear { factor: -1, base: -2 }), (252, StackRule::Const(-3)),
    (253, StackRule::Const(0)), (254, StackRule::Const(0)),
    (255, StackRule::Const(0)), (256, StackRule::Const(0)),
    (257, StackRule::Const(0)), (258, StackRule::Const(0)),
    (259, StackRule::Const(0)), (260, StackRule::Const(0)),
    (261, StackRule::Const(1)), (262, StackRule::Const(0)),
    (263, StackRule::Const(2)), (264, StackRule::Const(1)),
    (265, StackRule::Const(1)), (266, StackRule::Const(-1)),
];

/// `BC-38`：`jump=False` 时与上面不同的指令（`SETUP_*` 一族）。
pub static STACK_EFFECTS_NOT_JUMP: &[(u16, StackRule)] = &[
    (263, StackRule::Const(0)), (264, StackRule::Const(0)),
    (265, StackRule::Const(0)),
];
