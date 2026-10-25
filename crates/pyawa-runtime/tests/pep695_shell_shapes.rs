//! **PEP 695 泛型外壳**（task-24 ✓）：`class C[T]`／`def f[T](…)` 的编译产物与运行期语义。
//!
//! 编译产物由 `crates/pyawa-core/tests/compile.rs`（`--test compile`）**逐字节**对参照
//! （`tools/gen_compile_fixture.py` 的两条新语料 ✓：`<generic parameters of X>` 的序言／重放／收尾、
//! varnames／cellvars 槽位、位置表全部对齐 ✓）。
//!
//! 本文件钉**运行期**那一半（参照值由本机 `python3` 3.14 跑同一段探针取得 ✓）：
//! - `class C[T]` ⇒ 类体序言写 `__type_params__` ✓、`Generic.__init_subclass__` 落 `__parameters__` ✓、
//!   `C.__bases__ == (Generic,)`（`SUBSCRIPT_GENERIC` 的 `__mro_entries__` ✓）、`C[int]` 可下标 ✓；
//! - `def f[T](x: T) -> T` ⇒ `__type_params__` 由 `SET_FUNCTION_TYPE_PARAMS` 挂上 ✓、
//!   注解单元用 `LOAD_DEREF` 读类型参数 ✓、函数体照常可调 ✓；
//! - 类型参数在**函数体**里可用（`def g[T](): return T` ⇒ 内层函数自由变量 ✓）；
//! - **方法注解引用类型参数**（task-24 收口 ②，两层闭包 ✓）：类体捕获 `T`、方法注解单元再从
//!   类体捕获（`T` ＋ `__classdict__` ✓），注解表达式按**类字典优先**求值 ✓；方法体引用 `T` 也通 ✓；
//! - **泛型别名 `type Y[T] = …`**（task-24 收口 ② ✓）：壳 ＋ thunk 闭包（`LOAD_DEREF T` ✓）；
//! - **泛型类的类关键字／`**` 解包**（round 21／task-45 ✓）：`class C[T](metaclass=M)` 等
//!   按参照 `CALL_KW` 形状发射并落地运行期语义（`type(C) is M`／`__bases__`／`__parameters__`／
//!   `__orig_bases__`／`C[int]` 全部对拍 ✓，见 `generic_class_with_metaclass_agrees_with_the_reference`）；
//! - **如实范围** ✗：`*Ts`／`**P`／泛型函数的默认值参数／外壳引用外层局部
//!   **一律当场报未接线** ✓（不静默丢语义 ✓，见末尾用例 ✓）。

use pyawa_core::compile::{compile, CheckTier, CompileError, Mode};

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-pep695-{}-{name}", std::process::id()));
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

/// `class C[T]`：`__type_params__` 落点 ✓／`Generic` 基 ✓／`__parameters__` ✓／`C[int]` ✓。
const CLASS_SHELL_SCRIPT: &str = r#"import _typing

class C[T]:
    pass

print("type_params_in_dict", "__type_params__" in C.__dict__)
print("type_params", len(C.__type_params__), type(C.__type_params__[0]).__name__, C.__type_params__[0].__name__)
print("bases", [b.__name__ for b in C.__bases__])
print("mro", [b.__name__ for b in C.__mro__])
print("parameters", len(C.__parameters__), C.__parameters__[0].__name__)
print("subscript_origin", C[int].__origin__.__name__)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const CLASS_SHELL_EXPECTED: &str = "type_params_in_dict True\n\
type_params 1 TypeVar T\n\
bases ['Generic']\n\
mro ['C', 'Generic', 'object']\n\
parameters 1 T\n\
subscript_origin C\n";

#[test]
fn generic_class_shell_agrees_with_the_reference() {
    let stdout = run("class", CLASS_SHELL_SCRIPT);
    assert_eq!(stdout, CLASS_SHELL_EXPECTED, "{stdout}");
}

/// `def f[T](x: T) -> T`：`__type_params__` 挂载 ✓／注解单元读 `LOAD_DEREF` ✓／调用 ✓。
const DEF_SHELL_SCRIPT: &str = r#"import _typing

def f[T](x: T) -> T:
    return x

print("f_type_params", len(f.__type_params__), f.__type_params__[0].__name__)
print("f_annot_keys", sorted(f.__annotations__.keys()))
print("f_annot_is_param", f.__annotations__["return"] is f.__type_params__[0])
print("f_call", f(3))
print("f_qualname", f.__qualname__)
"#;

const DEF_SHELL_EXPECTED: &str = "f_type_params 1 T\n\
f_annot_keys ['return', 'x']\n\
f_annot_is_param True\n\
f_call 3\n\
f_qualname f\n";

#[test]
fn generic_def_shell_agrees_with_the_reference() {
    let stdout = run("def", DEF_SHELL_SCRIPT);
    assert_eq!(stdout, DEF_SHELL_EXPECTED, "{stdout}");
}

/// 类型参数在**函数体**里可用（内层函数自由变量 ✓）；多参数按声明序 ✓；类体注解引用类型参数 ✓。
const CLOSURE_AND_ANNOTATION_SCRIPT: &str = r#"import _typing

def g[T]():
    return T

print("g_is_param", g() is g.__type_params__[0])

class D[T, U]:
    pass

print("two_params", [p.__name__ for p in D.__type_params__])
print("two_sub", D[int, str].__origin__.__name__)

class H[T]:
    z: T

print("annot_keys", sorted(H.__annotations__.keys()))
print("annot_is_param", H.__annotations__["z"] is H.__type_params__[0])
"#;

const CLOSURE_AND_ANNOTATION_EXPECTED: &str = "g_is_param True\n\
two_params ['T', 'U']\n\
two_sub D\n\
annot_keys ['z']\n\
annot_is_param True\n";

#[test]
fn generic_type_params_in_body_and_class_annotations_agree_with_the_reference() {
    let stdout = run("closure", CLOSURE_AND_ANNOTATION_SCRIPT);
    assert_eq!(stdout, CLOSURE_AND_ANNOTATION_EXPECTED, "{stdout}");
}

/// **未接线的形态必须当场报错** ✗（`BC-59` 的口径：不许静默丢 `[T]` 的语义 ✓）。
/// **泛型类 ＋ 类关键字**（round 21／task-45 ✓）：`class C[T](metaclass=M)` 按参照落地
/// （`CALL_KW` 形状；本轮之前这一档是"当场报未接线"，现在是真能力 ✓）。参照 `python3` 3.14.4
/// 跑同一段脚本逐行取得（`type(C) is M`／基类／类型参数／`__parameters__`／`__orig_bases__`／
/// `C[int]` 的 origin／实例化）。
const GENERIC_CLASS_METACLASS_SCRIPT: &str = r#"import _typing

class M(type):
    pass


class C[T](metaclass=M):
    pass


print("class_is_M", type(C) is M)
print("bases", [b.__name__ for b in C.__bases__])
print("type_params", [p.__name__ for p in C.__type_params__])
print("parameters", [p.__name__ for p in C.__parameters__])
print("orig_bases", [b.__name__ for b in C.__orig_bases__])
print("subscript_origin", C[int].__origin__ is C)
print("instance_is_C", isinstance(C(), C))
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const GENERIC_CLASS_METACLASS_EXPECTED: &str = "class_is_M True\n\
bases ['Generic']\n\
type_params ['T']\n\
parameters ['T']\n\
orig_bases ['Generic']\n\
subscript_origin True\n\
instance_is_C True\n";

#[test]
fn generic_class_with_metaclass_agrees_with_the_reference() {
    let stdout = run("generic-metaclass", GENERIC_CLASS_METACLASS_SCRIPT);
    assert_eq!(stdout, GENERIC_CLASS_METACLASS_EXPECTED, "{stdout}");
}

#[test]
fn unsupported_pep695_shapes_are_reported_not_guessed() {
    // `[T: bound]`／`[T = default]` 已在收口 ① 落地 ✓；泛型类的**类关键字**（`metaclass=`）
    // 在 round 21（task-45）落地 ✓（语义见 `generic_class_with_metaclass_agrees_with_the_reference`）
    // ⇒ 这里剩下的仍是当场报错的那几档 ✓。
    let cases: [(&str, &str); 3] = [
        ("class C[*Ts]:\n    pass\n", "*Ts"),
        ("class C[**P]:\n    pass\n", "**P"),
        ("def f[T](x: T = 1):\n    return x\n", "默认值"),
    ];
    for (source, label) in cases {
        match compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0) {
            Err(CompileError::Unsupported(message)) => assert!(
                !message.is_empty(),
                "{label}：未接线的报错必须写明理由：{source:?}"
            ),
            Err(other) => panic!("{label}：应当是 Unsupported，实际 {other:?}"),
            Ok(_) => panic!("{label}：未接线的形态不许静默编过：{source:?}"),
        }
    }
}

/// **`type X = …`（PEP 695 别名语句）** ✓（task-24 收口 ③）：编译产物逐字节对参照（`--test compile`
/// 的 `type X = int`／`type X = list[int]` 两条语料 ✓）；这里钉运行期 —— `__name__`／`__value__`
/// （**惰性** ＋ **缓存**：`X.__value__ is X.__value__` ✓）／`__type_params__`（空 `()` ✓）／
/// 函数里定义（`STORE_FAST` ✓）。
///
/// **如实范围** ✗：`type Y[T] = …`（泛型别名）当场报未接线 ✓；`__value__` 是 `list[int]` 时本层
/// 的 `GenericAlias.__args__` 单参存的是**参数本身**（参照恒为元组 ✓，已登记 ✓）⇒ 抽例只钉 `__origin__` ✓。
const TYPE_ALIAS_SCRIPT: &str = r#"import _typing

type X = int
print("X", X.__name__, X.__value__ is int, X.__value__ is X.__value__, X.__type_params__, type(X).__name__)


def outer():
    type Y = list[int]
    return Y


Y = outer()
print("Y", Y.__name__, Y.__type_params__)
print("Y_origin", Y.__value__.__origin__ is list)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TYPE_ALIAS_EXPECTED: &str = "X X True True () TypeAliasType\nY Y ()\nY_origin True\n";

#[test]
fn type_alias_statement_agrees_with_the_reference() {
    let stdout = run("alias", TYPE_ALIAS_SCRIPT);
    assert_eq!(stdout, TYPE_ALIAS_EXPECTED, "{stdout}");
}


/// **`[T: bound]`／`[T = default]`** ✓（task-24 收口 ①）：惰性 thunk 求值 ＋ 缓存 ✓
/// （`__bound__`／`__default__` 都是首次读才算 ✓）；编译产物由 `--test compile` 的三条新语料
/// （`class C[T: int]`／`def f[T = str]`／`class C[T: int = str]`）**逐字节**钉住 ✓。
const TYPE_PARAM_BOUND_SCRIPT: &str = r#"import _typing

class C[T: int]:
    pass

print("bound", C.__type_params__[0].__bound__ is int, C.__type_params__[0].__bound__ is C.__type_params__[0].__bound__)


def f[T = str]():
    pass


print("default", f.__type_params__[0].__default__ is str)


class D[T: int = str]:
    pass


print("both", D.__type_params__[0].__bound__ is int, D.__type_params__[0].__default__ is str)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TYPE_PARAM_BOUND_EXPECTED: &str = "bound True True\ndefault True\nboth True True\n";

#[test]
fn type_param_bound_and_default_agree_with_the_reference() {
    let stdout = run("bound", TYPE_PARAM_BOUND_SCRIPT);
    assert_eq!(stdout, TYPE_PARAM_BOUND_EXPECTED, "{stdout}");
}

/// **方法注解引用类型参数** ✓（task-24 收口 ②，两层闭包）：编译产物由 `--test compile` 的
/// `class C[T]: def m(self) -> T: return self` 一条语料**逐字节**钉住 ✓（类体闭包元组
/// `T; __classdict__` ✓、注解单元 `LOAD_DEREF __classdict__; LOAD_FROM_DICT_OR_DEREF T` ✓、
/// 方法单元 `CO_NESTED` ✓）；这里钉运行期 —— 注解对象就是类型参数本身 ✓、`__parameters__` ✓、
/// 多参数按声明序 ✓、注解里的 `list[T]` ✓、**方法体**引用 `T` ✓。
const METHOD_ANNOTATION_SCRIPT: &str = r#"import _typing

class C[T]:
    def m(self) -> T:
        return self

print("m_qualname", C.m.__qualname__)
print("annot_keys", sorted(C.m.__annotations__.keys()))
print("annot_is_param", C.m.__annotations__["return"] is C.__type_params__[0])
print("type_params", [p.__name__ for p in C.__type_params__])
print("parameters", [p.__name__ for p in C.__parameters__])
print("call_self", C().m() is not None)

class D[T, U]:
    def pair(self, x: T) -> U:
        return x

print("d_params", [p.__name__ for p in D.__type_params__])
print("d_annot", D.pair.__annotations__["x"] is D.__type_params__[0], D.pair.__annotations__["return"] is D.__type_params__[1])

class E[T]:
    def wrap(self) -> list[T]:
        return []

print("e_annot", E.wrap.__annotations__["return"].__origin__ is list)

class F[T]:
    def ident(self):
        return T

print("f_body", F().ident() is F.__type_params__[0])
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const METHOD_ANNOTATION_EXPECTED: &str = "m_qualname C.m\n\
annot_keys ['return']\n\
annot_is_param True\n\
type_params ['T']\n\
parameters ['T']\n\
call_self True\n\
d_params ['T', 'U']\n\
d_annot True True\n\
e_annot True\n\
f_body True\n";

#[test]
fn method_annotation_referencing_type_params_agrees_with_the_reference() {
    let stdout = run("method-annotation", METHOD_ANNOTATION_SCRIPT);
    assert_eq!(stdout, METHOD_ANNOTATION_EXPECTED, "{stdout}");
}

/// **泛型别名 `type Y[T] = …`** ✓（task-24 收口 ②）：编译产物由 `--test compile` 的
/// `type Y[T] = list[T]` 一条语料**逐字节**钉住 ✓（壳的 `MAKE_CELL`／`TYPEVAR`／默认值元组／
/// thunk 闭包／`TYPEALIAS` ✓）；这里钉运行期 —— `__type_params__` ✓、`__value__` 的惰性 ＋ 缓存 ✓、
/// **未被引用的类型参数**（`type Z[T] = int` ⇒ 壳里走 `STORE_FAST` ✓，不建 cell ✓）。
const GENERIC_TYPE_ALIAS_SCRIPT: &str = r#"import _typing

type Y[T] = list[T]
print("Y_name", Y.__name__)
print("Y_params", [p.__name__ for p in Y.__type_params__])
print("Y_origin", Y.__value__.__origin__ is list)
print("Y_cache", Y.__value__ is Y.__value__)

type Z[T] = int
print("Z_params", [p.__name__ for p in Z.__type_params__], Z.__value__ is int)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const GENERIC_TYPE_ALIAS_EXPECTED: &str = "Y_name Y\n\
Y_params ['T']\n\
Y_origin True\n\
Y_cache True\n\
Z_params ['T'] True\n";

#[test]
fn generic_type_alias_agrees_with_the_reference() {
    let stdout = run("generic-alias", GENERIC_TYPE_ALIAS_SCRIPT);
    assert_eq!(stdout, GENERIC_TYPE_ALIAS_EXPECTED, "{stdout}");
}
