//! **enum B5＋B6 的回归抽例**（第 411 轮，task-22 ✓）——两侧逐行一致。
//!
//! 覆盖 B5 四格＋B6 换真本：
//! * **G6 元类 `property`**：类属性访问要走元类 MRO 的**数据描述符**（`C.size_of_name` ✓；
//!   `Meta.size_of_name` 仍给 property 对象 ✓）；
//! * **`int` 子类算术 dunder 优先**：`IntLike(1) | IntLike(2)` 与反射 `1 | IntLike(2)` ✓
//!   （真 `enum.py` 的 `IntFlag` 靠它合成伪成员 ✓）；
//! * **`__new__` 自动包成 `staticmethod`**（`type.__new__` 语义 ✓）＋ `staticmethod.__func__`／
//!   `classmethod.__func__` ✓（真 `_simple_enum` 的 `__new__.__func__` ✓）；
//! * **真 `enum.py`**（`Lib/enum.py` 已换成上游逐字节真本）：`Enum`／`IntFlag` 组合／`StrEnum`／
//!   `auto()`／`__members__` ✓；
//! * **`http` 一族**：`HTTPStatus.OK.phrase`／`__members__`／`http.client.responses`／`HTTPMethod` ✓。
//!
//! **如实登记的偏差** ✗（不进两侧对拍）：本层 `MappingProxyType = type(type.__dict__)` 仍是 `dict`
//! ⇒ `Color.__members__` 的类型是 `dict`（参照 `mappingproxy`），抽例只钉它的**内容/身份** ✓。
//!
//! 期望输出由本机 `python3`（3.14.4）跑**同一段脚本**取得（`-S`＋`PYTHONPATH=本仓 Lib` 保证
//! `enum`／`http` 两侧同源 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-enum-b56-{}-{name}", std::process::id()));
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

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const EXPECTED: &str = "meta_property 8 property\nint_dunder ('or', 1, 2) ('ror', 2, 1) ('ror', 4, 3)\nstatic_wrap staticmethod __new__\nstatic_func True False\nenum_basic 1 RED 2 ['RED', 'GREEN']\nenum_members ['GREEN', 'RED'] True\nintflag 3 A|B True True\nstrenum ex True ex\nauto 1 2\nhttp_status OK Not Found Not Found GET GET\n";

const SCRIPT: &str = "import enum\nimport http\nimport http.client\n\n\nclass Meta(type):\n    @property\n    def size_of_name(cls):\n        return len(cls.__name__)\n\n\nclass WithMeta(metaclass=Meta):\n    pass\n\n\nprint(\"meta_property\", WithMeta.size_of_name, type(Meta.size_of_name).__name__)\n\n\nclass IntLike(int):\n    def __or__(self, other):\n        return (\"or\", int(self), int(other))\n\n    def __ror__(self, other):\n        return (\"ror\", int(self), int(other))\n\n\nprint(\"int_dunder\", IntLike(1) | IntLike(2), 1 | IntLike(2), 3 | IntLike(4))\n\n\nclass Wrapped:\n    def __new__(cls, value):\n        return super().__new__(cls)\n\n    @staticmethod\n    def static_one():\n        return 1\n\n    @classmethod\n    def class_name(cls):\n        return cls.__name__\n\n\nprint(\"static_wrap\", type(Wrapped.__dict__[\"__new__\"]).__name__,\n      Wrapped.__dict__[\"__new__\"].__func__.__name__)\nprint(\"static_func\", Wrapped.__dict__[\"static_one\"].__func__ is Wrapped.static_one,\n      Wrapped.__dict__[\"class_name\"].__func__ is Wrapped.class_name)\n\n\nclass Color(enum.Enum):\n    RED = 1\n    GREEN = 2\n\n\nprint(\"enum_basic\", Color.RED.value, Color.RED.name, len(Color), [m.name for m in Color])\nprint(\"enum_members\", sorted(Color.__members__), Color.__members__[\"RED\"] is Color.RED)\n\n\nclass Flags(enum.IntFlag):\n    A = 1\n    B = 2\n\n\ncombined = Flags.A | Flags.B\nprint(\"intflag\", combined.value, combined.name, combined is (Flags.B | Flags.A), Flags.A in combined)\n\n\nclass Str(enum.StrEnum):\n    EX = \"ex\"\n\n\nprint(\"strenum\", str(Str.EX), Str.EX == \"ex\", Str.EX.value)\n\n\nclass Auto(enum.Enum):\n    ONE = enum.auto()\n    TWO = enum.auto()\n\n\nprint(\"auto\", Auto.ONE.value, Auto.TWO.value)\nprint(\"http_status\", http.HTTPStatus.OK.phrase, http.HTTPStatus.__members__[\"NOT_FOUND\"].phrase,\n      http.client.responses[404], http.HTTPMethod.GET.name, http.HTTPMethod.GET.value)\n";

#[test]
fn enum_b5_and_real_enum_swap_agree_with_the_reference() {
    let stdout = run("b56", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
