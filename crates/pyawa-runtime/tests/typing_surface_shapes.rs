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
//! **如实范围** ✗：`TypeVar` 的关键字形态（`bound=`／`covariant=`／`default=`）这一轮还收不到
//! （`new` 槽没有 kwargs 通道）—— 轮 3 与 `Lib/typing.py` 一起收口。

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
