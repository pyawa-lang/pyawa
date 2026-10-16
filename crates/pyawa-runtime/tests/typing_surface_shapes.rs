//! **`_typing` 最小可用面**（task-24 轮 1 ✓）。
//!
//! 参照里 `_typing` 是 **C 模块**（`Lib/_typing.py` 不存在）⇒ 本层落成"core 造类型 ＋ stdlib 装配模块"
//! （与 `memoryview` 同款：`alloc_type_raw` ＋ `new` 槽 ＋ `getattr` 槽）。口径**照参照实测**：
//! `TypeVar('T')` ⇒ `__name__='T'`／`__constraints__=()`／`__bound__=None`／`__covariant__=False`／
//! `__contravariant__=False`／`__infer_variance__=False`／`__default__ is NoDefault`；
//! `TypeVar('C', int, str)` ⇒ `__constraints__ == (int, str)`；`type(NoDefault)() is NoDefault`；
//! `_idfunc(5)==5`；`dir(_typing)` 的十个名字（`NoDefaultType` **不**导出，照参照）。
//!
//! 同轮还落了 **PEP 695 的四件 intrinsic** 编号与前三件语义（`INTRINSIC_TYPEVAR`／
//! `INTRINSIC_SUBSCRIPT_GENERIC`／`INTRINSIC_TYPEALIAS`（只注册编号）／
//! `INTRINSIC_SET_FUNCTION_TYPE_PARAMS`）—— 它们在**轮 2** 的编译器外壳里才会被发出来，故本抽例
//! 只验 `_typing` 表面（intrinsic 由轮 2 的逐字节夹具与运行时抽例覆盖）。
//!
//! **如实范围**（轮 1 的口径，收口 ① 已改）✗：`TypeVar` 的**关键字通道**已接线 ✓
//! （见下方第二个用例 ✓）；`__repr__`／`__or__` 等方法面仍缺 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-typing-{}-{name}", std::process::id()));
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

/// `_typing` 基础面（`TypeVar` 属性 ✓／`NoDefault` 单例 ✓／`_idfunc` ✓／导出名表 ✓）。
const TYPING_SURFACE_SCRIPT: &str = r#"import _typing

T = _typing.TypeVar("T")
print(T.__name__, T.__constraints__, T.__bound__, T.__covariant__, T.__contravariant__, T.__infer_variance__)
print(T.__default__ is _typing.NoDefault)
print(type(T).__name__)
C = _typing.TypeVar("C", int, str)
print(C.__constraints__)
print(type(_typing.NoDefault)() is _typing.NoDefault)
print(_typing._idfunc(5), _typing._idfunc("x"))
print(type(_typing.NoDefault).__name__)
print(_typing.Generic.__name__)
print([n for n in ("TypeVar", "TypeVarTuple", "ParamSpec", "ParamSpecArgs", "ParamSpecKwargs", "TypeAliasType", "Union", "NoDefault", "_idfunc", "Generic") if hasattr(_typing, n)])
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TYPING_SURFACE_EXPECTED: &str = "T () None False False False\nTrue\nTypeVar\n(<class 'int'>, <class 'str'>)\nTrue\n5 x\nNoDefaultType\nGeneric\n['TypeVar', 'TypeVarTuple', 'ParamSpec', 'ParamSpecArgs', 'ParamSpecKwargs', 'TypeAliasType', 'Union', 'NoDefault', '_idfunc', 'Generic']\n";

#[test]
fn typing_minimal_surface_agrees_with_the_reference() {
    let stdout = run("surface", TYPING_SURFACE_SCRIPT);
    assert_eq!(stdout, TYPING_SURFACE_EXPECTED, "{stdout}");
}

/// **`TypeVar` 的关键字通道** ✓（task-24 收口 ① ✓）：`bound=`／`covariant=`／`default=` 与
/// 约束／报错文案（缺名／名字类型／单个约束／未知关键字／`infer_variance` 与变型并用 ✓）。
const TYPEVAR_KWARGS_SCRIPT: &str = r#"import _typing

B = _typing.TypeVar("B", bound=int)
print("bound", B.__bound__ is int, B.__default__ is _typing.NoDefault)
V = _typing.TypeVar("V", covariant=True)
print("variance", V.__covariant__, V.__contravariant__)
D = _typing.TypeVar("D", default=5)
print("default", D.__default__)
I = _typing.TypeVar("I", infer_variance=True)
print("infer", I.__infer_variance__)
for label, call in (
    ("none", lambda: _typing.TypeVar()),
    ("badname", lambda: _typing.TypeVar(3)),
    ("bogus", lambda: _typing.TypeVar("X", bogus=1)),
    ("single", lambda: _typing.TypeVar("X", int)),
    ("variance", lambda: _typing.TypeVar("V", covariant=True, infer_variance=True)),
):
    try:
        call()
        print(label, "ok")
    except Exception as exc:
        print(label, type(exc).__name__ + ":", exc)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TYPEVAR_KWARGS_EXPECTED: &str = "bound True True\n\
variance True False\n\
default 5\n\
infer True\n\
none TypeError: typevar() missing required argument 'name' (pos 1)\n\
badname TypeError: typevar() argument 'name' must be str, not int\n\
bogus TypeError: typevar() got an unexpected keyword argument 'bogus'\n\
single TypeError: A single constraint is not allowed\n\
variance ValueError: Variance cannot be specified with infer_variance.\n";

#[test]
fn typevar_keyword_channel_agrees_with_the_reference() {
    let stdout = run("kwargs", TYPEVAR_KWARGS_SCRIPT);
    assert_eq!(stdout, TYPEVAR_KWARGS_EXPECTED, "{stdout}");
}
