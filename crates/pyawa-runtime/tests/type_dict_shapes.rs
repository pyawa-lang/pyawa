//! **常量表越界（task-20 第 2 轮真 bug）＋ `type.__dict__` 描述符 ＋ `__mro_entries__` ＋ `sys.modules["__main__"]`** ✓。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（脚本见各 `const` ✓）。
//!
//! 被这组抽例钉住的真 bug：
//! ① **`LOAD_CONST` 常量表下标 ≥ 256 时截断** ✗：延迟入池那条路（折叠元组／`frozenset`／关键字元组 ✓）
//!    在 `flush_pending` 里只写 `index as u8` ✗ ⇒ 高字节丢 ✓（实测 `Lib/inspect.py:1616` 的
//!    `('frame',) + Traceback._fields` 取到了**别的常量** ✓）；修法：`emit_constant` 里
//!    上界超过 191 就**当场入池**并走带 `EXTENDED_ARG` 的那条 ✓（小模块仍按参照顺序延迟 ⇒ 夹具逐字节不动 ✓）；
//! ② **`type.__dict__["__mro__"]`／`["__dict__"]`** ✗ 先前 `KeyError` ✓（`Lib/inspect.py:1673/1674`
//!    的 `_static_getmro`／`_get_dunder_dict_of_class` 要它们 ✓）⇒ 与 `__annotations__` 同一套
//!    getset 描述符（按 `_kind` 分派 ✓）；
//! ③ **`__mro_entries__`（PEP 560）** ✗：`class T(Mapping[str, str])` 的下标结果是 `GenericAlias` ✗
//!    ⇒ 参照先调 `__mro_entries__` 换出真正的基类 ✓（先前直接报 `bases must be types` ✓，
//!    `Lib/_colorize.py:115` 正是这个形状 ✓）；
//! ④ **`sys.modules["__main__"]` 缺** ✗ ⇒ 脚本模块对象（命名空间**就是**脚本 globals ✓）在
//!    `pa_exec_string`／`pa_exec_bytecode` 两条路开跑前登记 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-typedict-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 常量表 > 255 项时 `LOAD_CONST` 的**高位不丢** ✓（`(i,)` 折叠元组的那条路 ✓）
const CONST_INDEX_SCRIPT: &str = r#"c1 = (1,)
c2 = (2,)
c3 = (3,)
c4 = (4,)
c5 = (5,)
c6 = (6,)
c7 = (7,)
c8 = (8,)
c9 = (9,)
c10 = (10,)
c11 = (11,)
c12 = (12,)
c13 = (13,)
c14 = (14,)
c15 = (15,)
c16 = (16,)
c17 = (17,)
c18 = (18,)
c19 = (19,)
c20 = (20,)
c21 = (21,)
c22 = (22,)
c23 = (23,)
c24 = (24,)
c25 = (25,)
c26 = (26,)
c27 = (27,)
c28 = (28,)
c29 = (29,)
c30 = (30,)
c31 = (31,)
c32 = (32,)
c33 = (33,)
c34 = (34,)
c35 = (35,)
c36 = (36,)
c37 = (37,)
c38 = (38,)
c39 = (39,)
c40 = (40,)
c41 = (41,)
c42 = (42,)
c43 = (43,)
c44 = (44,)
c45 = (45,)
c46 = (46,)
c47 = (47,)
c48 = (48,)
c49 = (49,)
c50 = (50,)
c51 = (51,)
c52 = (52,)
c53 = (53,)
c54 = (54,)
c55 = (55,)
c56 = (56,)
c57 = (57,)
c58 = (58,)
c59 = (59,)
c60 = (60,)
c61 = (61,)
c62 = (62,)
c63 = (63,)
c64 = (64,)
c65 = (65,)
c66 = (66,)
c67 = (67,)
c68 = (68,)
c69 = (69,)
c70 = (70,)
c71 = (71,)
c72 = (72,)
c73 = (73,)
c74 = (74,)
c75 = (75,)
c76 = (76,)
c77 = (77,)
c78 = (78,)
c79 = (79,)
c80 = (80,)
c81 = (81,)
c82 = (82,)
c83 = (83,)
c84 = (84,)
c85 = (85,)
c86 = (86,)
c87 = (87,)
c88 = (88,)
c89 = (89,)
c90 = (90,)
c91 = (91,)
c92 = (92,)
c93 = (93,)
c94 = (94,)
c95 = (95,)
c96 = (96,)
c97 = (97,)
c98 = (98,)
c99 = (99,)
c100 = (100,)
c101 = (101,)
c102 = (102,)
c103 = (103,)
c104 = (104,)
c105 = (105,)
c106 = (106,)
c107 = (107,)
c108 = (108,)
c109 = (109,)
c110 = (110,)
c111 = (111,)
c112 = (112,)
c113 = (113,)
c114 = (114,)
c115 = (115,)
c116 = (116,)
c117 = (117,)
c118 = (118,)
c119 = (119,)
c120 = (120,)
c121 = (121,)
c122 = (122,)
c123 = (123,)
c124 = (124,)
c125 = (125,)
c126 = (126,)
c127 = (127,)
c128 = (128,)
c129 = (129,)
c130 = (130,)
c131 = (131,)
c132 = (132,)
c133 = (133,)
c134 = (134,)
c135 = (135,)
c136 = (136,)
c137 = (137,)
c138 = (138,)
c139 = (139,)
c140 = (140,)
c141 = (141,)
c142 = (142,)
c143 = (143,)
c144 = (144,)
c145 = (145,)
c146 = (146,)
c147 = (147,)
c148 = (148,)
c149 = (149,)
c150 = (150,)
c151 = (151,)
c152 = (152,)
c153 = (153,)
c154 = (154,)
c155 = (155,)
c156 = (156,)
c157 = (157,)
c158 = (158,)
c159 = (159,)
c160 = (160,)
c161 = (161,)
c162 = (162,)
c163 = (163,)
c164 = (164,)
c165 = (165,)
c166 = (166,)
c167 = (167,)
c168 = (168,)
c169 = (169,)
c170 = (170,)
c171 = (171,)
c172 = (172,)
c173 = (173,)
c174 = (174,)
c175 = (175,)
c176 = (176,)
c177 = (177,)
c178 = (178,)
c179 = (179,)
c180 = (180,)
c181 = (181,)
c182 = (182,)
c183 = (183,)
c184 = (184,)
c185 = (185,)
c186 = (186,)
c187 = (187,)
c188 = (188,)
c189 = (189,)
c190 = (190,)
c191 = (191,)
c192 = (192,)
c193 = (193,)
c194 = (194,)
c195 = (195,)
c196 = (196,)
c197 = (197,)
c198 = (198,)
c199 = (199,)
c200 = (200,)
c201 = (201,)
c202 = (202,)
c203 = (203,)
c204 = (204,)
c205 = (205,)
c206 = (206,)
c207 = (207,)
c208 = (208,)
c209 = (209,)
c210 = (210,)
c211 = (211,)
c212 = (212,)
c213 = (213,)
c214 = (214,)
c215 = (215,)
c216 = (216,)
c217 = (217,)
c218 = (218,)
c219 = (219,)
c220 = (220,)
c221 = (221,)
c222 = (222,)
c223 = (223,)
c224 = (224,)
c225 = (225,)
c226 = (226,)
c227 = (227,)
c228 = (228,)
c229 = (229,)
c230 = (230,)
c231 = (231,)
c232 = (232,)
c233 = (233,)
c234 = (234,)
c235 = (235,)
c236 = (236,)
c237 = (237,)
c238 = (238,)
c239 = (239,)
c240 = (240,)
c241 = (241,)
c242 = (242,)
c243 = (243,)
c244 = (244,)
c245 = (245,)
c246 = (246,)
c247 = (247,)
c248 = (248,)
c249 = (249,)
c250 = (250,)
c251 = (251,)
c252 = (252,)
c253 = (253,)
c254 = (254,)
c255 = (255,)
c256 = (256,)
c257 = (257,)
c258 = (258,)
c259 = (259,)
c260 = (260,)
c261 = (261,)
c262 = (262,)
c263 = (263,)
c264 = (264,)
c265 = (265,)
c266 = (266,)
c267 = (267,)
c268 = (268,)
c269 = (269,)
c270 = (270,)
c271 = (271,)
c272 = (272,)
c273 = (273,)
c274 = (274,)
c275 = (275,)
c276 = (276,)
c277 = (277,)
c278 = (278,)
c279 = (279,)
c280 = (280,)
c281 = (281,)
c282 = (282,)
c283 = (283,)
c284 = (284,)
c285 = (285,)
c286 = (286,)
c287 = (287,)
c288 = (288,)
c289 = (289,)
c290 = (290,)
c291 = (291,)
c292 = (292,)
c293 = (293,)
c294 = (294,)
c295 = (295,)
c296 = (296,)
c297 = (297,)
c298 = (298,)
c299 = (299,)
c300 = (300,)
class T:
    _fields = ('a', 'b')
def f(a, b):
    return (a, b)
print(f('_FrameInfo', ('frame',) + T._fields))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const CONST_INDEX_EXPECTED: &str = "('_FrameInfo', ('frame', 'a', 'b'))\n";

#[test]
fn large_constant_pool_indices_agree_with_the_reference() {
    let stdout = run("large_constant_pool_indices_agree", CONST_INDEX_SCRIPT);
    assert_eq!(stdout, CONST_INDEX_EXPECTED, "{stdout}");
}

/// `type.__dict__` 的 `__mro__`／`__dict__`／`__annotations__` 描述符 ＋ `__mro_entries__`（泛型基类 ✓）＋ `import inspect`／`dataclasses` ✓
const TYPE_DICT_SCRIPT: &str = r#"import inspect
import dataclasses
from collections.abc import Mapping


class T(Mapping):
    def __getitem__(self, key):
        raise KeyError(key)

    def __iter__(self):
        return iter(())

    def __len__(self):
        return 0


print(inspect.__name__, dataclasses.__name__)
print([cls.__name__ for cls in type.__dict__["__mro__"].__get__(T)])
print("__module__" in type.__dict__["__dict__"].__get__(T))
print(type.__dict__["__dict__"].__get__(T).get("__module__"))
print(hasattr(T, "__mro_entries__"))


class G(Mapping[str, str]):
    def __getitem__(self, key):
        raise KeyError(key)

    def __iter__(self):
        return iter(())

    def __len__(self):
        return 0


print([cls.__name__ for cls in G.__mro__])
print(type.__dict__["__annotations__"].__get__(G))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TYPE_DICT_EXPECTED: &str = "inspect dataclasses\n['T', 'Mapping', 'Collection', 'Sized', 'Iterable', 'Container', 'object']\nTrue\n__main__\nFalse\n['G', 'Mapping', 'Collection', 'Sized', 'Iterable', 'Container', 'object']\n{}\n";

#[test]
fn type_dict_descriptors_and_mro_entries_agree_with_the_reference() {
    let stdout = run("type_dict_descriptors_and_mro_entries_agree", TYPE_DICT_SCRIPT);
    assert_eq!(stdout, TYPE_DICT_EXPECTED, "{stdout}");
}

/// `sys.modules["__main__"]`（模块对象与脚本 globals **同一份** ✓）
const MAIN_MODULE_SCRIPT: &str = r#"import sys
m = sys.modules["__main__"]
print(type(m).__name__, m.__name__)
print("dict-is-globals", m.__dict__ is globals() if hasattr(m, "__dict__") else "n/a")
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const MAIN_MODULE_EXPECTED: &str = "module __main__\ndict-is-globals True\n";

#[test]
fn sys_modules_main_agrees_with_the_reference() {
    let stdout = run("sys_modules_main_agrees", MAIN_MODULE_SCRIPT);
    assert_eq!(stdout, MAIN_MODULE_EXPECTED, "{stdout}");
}

