//! **`import typing` 收链**（task-24 尾件 ✓）：把打通 `Lib/typing.py` 的几处真 bug 钉在运行期。
//!
//! 每条的期望值都由本机 `python3` 3.14 **跑同一段探针**取得 ✓（与 `pep695_shell_shapes.rs` 同口径 ✓）：
//! - **`import typing` 本体** ✓（PEP 695 全套 ＋ `_collections.defaultdict` ＋ `_typing` 面）；
//! - **`from typing import Match`** ✓ ⇒ **PEP 562** 的模块级 `__getattr__`（`typing.py:3827` 懒建 ✓）；
//! - **`class SupportsAbs[T](Protocol)`** ✓ ⇒ `_ProtocolMeta.__new__` 读别名基类的 `__name__`
//!   （`GenericAlias.__getattr__` 转发 ✓）＋ `__module__`（`_typing` 类型注册在 `typing` 名下 ✓）＋
//!   自定义元类拿到的是**解析后**的基类表 ✓；
//! - **`__init_subclass__` 的类关键字与绑定** ✓（`class X(_Final, _root=True)` ⇒ `**kwds` 里是 `_root` ✓；
//!   `classmethod(...)` 形态绑到**新类** ✓；同一关键字名用两次不串 ✓ —— 那条双重释放真 bug ✓）；
//! - **`super()` 沿 MRO 续查** ✓（`_SpecialGenericAlias(_NotIterable, _BaseGenericAlias)` 的
//!   `super().__init__(…)` 必须落到 `_BaseGenericAlias`，不能提前命中 `object` 的默认 ✓）；
//! - **大模块的类名常量** ✓（300 个模块级常量之后、同类关键字的第二个类的名字仍对 ✓ ——
//!   类名常量延迟回填 `>255` 截断的真 bug ✓）。


fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-typing-import-{}-{name}", std::process::id()));
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

/// **`import typing` 收链**：`typing.Protocol` 的 PEP 695 泛型子类、方法注解引用类型参数、
/// 延迟属性（`Match`）、`DefaultDict` 的 origin、协议的 `__protocol_attrs__`。
const TYPING_IMPORT_SCRIPT: &str = r#"import typing
print("typing", typing.__name__)
print("dd_origin", typing.DefaultDict.__origin__.__name__)

class SupportsAbs[T](typing.Protocol):
    def __abs__(self) -> T: ...

print("proto", SupportsAbs.__name__, [p.__name__ for p in SupportsAbs.__type_params__])
print("annot", SupportsAbs.__abs__.__annotations__["return"] is SupportsAbs.__type_params__[0])
from typing import Match
print("match_from", type(Match).__name__)
print("proto_interface", len(SupportsAbs.__protocol_attrs__) > 0)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const TYPING_IMPORT_EXPECTED: &str = "typing typing\n\
dd_origin defaultdict\n\
proto SupportsAbs ['T']\n\
annot True\n\
match_from _SpecialGenericAlias\n\
proto_interface True\n";

#[test]
fn import_typing_unlocks_the_chain() {
    let stdout = run("import", TYPING_IMPORT_SCRIPT);
    assert_eq!(stdout, TYPING_IMPORT_EXPECTED, "{stdout}");
}

/// **`__init_subclass__`**（类关键字 ＋ 绑定）＋ **`super()` 沿 MRO 续查** ＋ **classmethod 形态**。
const INIT_SUBCLASS_SCRIPT: &str = r#"class _Final:
    def __init_subclass__(cls, /, *args, **kwds):
        print("hook", cls.__name__, sorted(kwds))
        if "_root" not in kwds:
            raise TypeError("Cannot subclass special typing classes")

class _NotIterable:
    pass

class _SpecialForm(_Final, _NotIterable, _root=True):
    pass

class _TypedCacheSpecialForm(_SpecialForm, _root=True):
    pass

class _BaseGenericAlias(_Final, _root=True):
    def __init__(self, origin=None, *, inst=True, name=None):
        self._name = name
        self._inst = inst

class _SpecialGenericAlias(_NotIterable, _BaseGenericAlias, _root=True):
    def __init__(self, origin, nparams, *, inst=True, name=None):
        super().__init__(origin, inst=inst, name=name)
        self._nparams = nparams

sg = _SpecialGenericAlias(frozenset, 0, name="ByteString")
print("sg", sg._name, sg._nparams, sg._inst)

class Generic:
    def __init_subclass__(cls, /, **kw):
        print("generic_hook", cls.__name__)

class M(type):
    pass

class Protocol(Generic, metaclass=M):
    pass

print("protocol", Protocol.__name__)

def hook(cls, /, **kw):
    print("cm_hook", cls.__name__, sorted(kw))

class _Final2:
    pass

_Final2.__init_subclass__ = classmethod(hook)

class Child(_Final2, _root=True):
    pass
print("child", Child.__name__)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const INIT_SUBCLASS_EXPECTED: &str = "hook _SpecialForm ['_root']\n\
hook _TypedCacheSpecialForm ['_root']\n\
hook _BaseGenericAlias ['_root']\n\
hook _SpecialGenericAlias ['_root']\n\
sg ByteString 0 True\n\
generic_hook Protocol\n\
protocol Protocol\n\
cm_hook Child ['_root']\n\
child Child\n";

#[test]
fn init_subclass_keywords_and_super_agree_with_the_reference() {
    let stdout = run("init-subclass", INIT_SUBCLASS_SCRIPT);
    assert_eq!(stdout, INIT_SUBCLASS_EXPECTED, "{stdout}");
}

/// **大模块的类名常量**（task-24 尾件真 bug 回归 ✓）：300 个模块级常量之后，两个用**同一个**
/// 类关键字的类的名字不能串（`flush_pending` 的 `index as u8` 截断 ⇒ 第二个类张冠李戴 ✓）。
#[test]
fn large_module_class_keywords_agree_with_the_reference() {
    let mut script = String::new();
    for index in 0..300 {
        script.push_str(&format!("g{index} = {index}\n"));
    }
    script.push_str(
        "class Base:\n    def __init_subclass__(cls, /, **kw):\n        print('kw', cls.__name__, sorted(kw))\n\nclass C1(Base, _root=True):\n    pass\n\nclass C2(Base, _root=True):\n    pass\n",
    );
    let expected = "kw C1 ['_root']\nkw C2 ['_root']\n";
    let stdout = run("large-module", &script);
    assert_eq!(stdout, expected, "{stdout}");
}

/// **`_collections.defaultdict` 最小面** ✓（`typing.py` 的 `from collections import defaultdict` ✓）。
const DEFAULTDICT_SCRIPT: &str = r#"from collections import defaultdict
import functools
d = defaultdict(dict)
d["a"]["b"] = 1
print("nested", d["a"]["b"])
print("contains", "a" in d, "z" in d)
e = defaultdict(functools.partial(defaultdict, dict))
e["m"]["q"]["line"] = 7
print("deep", e["m"]["q"]["line"])
print("factory", e.default_factory is not None)
e.clear()
print("cleared", "m" in e)
print("module", defaultdict.__module__, defaultdict.__name__)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const DEFAULTDICT_EXPECTED: &str = "nested 1\n\
contains True False\n\
deep 7\n\
factory True\n\
cleared False\n\
module collections defaultdict\n";

#[test]
fn collections_defaultdict_agrees_with_the_reference() {
    let stdout = run("defaultdict", DEFAULTDICT_SCRIPT);
    assert_eq!(stdout, DEFAULTDICT_EXPECTED, "{stdout}");
}
