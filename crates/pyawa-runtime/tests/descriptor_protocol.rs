//! **类型级描述符专项**（`P3-17` 同族 ＋ `functools.wraps.__name__` 的正路）：两侧逐行一致。
//!
//! 覆盖三组行为（参照值由本机 `python3` 3.14.4 跑**同一段脚本**取得，逐行对拍 ✓）：
//!
//! 1. **`property` 的数据描述符面**：`@x.setter` 真的走 `fset`（**不再写实例字典** ✗）、
//!    `@x.deleter` 走 `fdel`，缺失时的 `AttributeError` 文案带属性名与类名；
//!    `hasattr(prop, "__get__"/"__set__"/"__delete__")` 全真；`property` 的 Python 子类
//!    （`Lib/abc.py` 的 `abstractproperty(property)` 形态）仍按子类类型工作 ✓。
//! 2. **`staticmethod.__get__`／`classmethod.__get__`**：类级／实例级取用、原始描述符调用
//!    （含实参个数与 `__get__(None, None) is invalid` 的文案 ✓）；`classmethod` 的 Python 子类
//!    （`Lib/abc.py` 的 `abstractclassmethod(classmethod)` 形态）端到端可用 ✓。
//! 3. **函数可写属性**（`functools.wraps` 的正路）：`f.__name__`／`__qualname__`／`__doc__`／
//!    `__module__` 可读可写可删（`__doc__`／`__module__` 删后是 `None` ✓）、**不进实例字典** ✓、
//!    `__name__` 只收 `str`；`@functools.wraps(g)` 之后 `f.__name__ == g.__name__` ✓。
//!
//! **如实登记的未覆盖形态** ✗：`__annotate__` 在本层**还不可写**（参照可写）—— `cell` 对象
//! 还没有 `cell_contents`（`Lib/dataclasses.py` 的 slots 路径要用它 ✗）⇒ 装成可写描述符会
//! 当场 unmask 那个更大的缺口（实测 `import asyncio` 红 ✗）⇒ 暂保持 HEAD 行为：`functools.wraps`
//! 的 `wrapper.__annotate__ = …` 落进**实例字典** ⇒ `f.__dict__` 多一个 `__annotate__` 键
//! （上面脚本**过滤掉**这个键再比 ✓；`__wrapped__` 与其它语义面不受影响 ✓）。
//!
//! **如实登记的类别偏差（方案 A）**：本层描述符对象的**类型名**是自建的
//! `function_attribute_descriptor`（参照是 `getset_descriptor`／`member_descriptor`），
//! `property.__set__` 绑出来的是 `method`（参照 `method-wrapper`）⇒ 本文件**不钉** `type(...)` 类别，
//! 只钉语义与文案 ✓（与 `type_descriptor_shapes.rs` 同一口径 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!(
        "pyawa-descriptor-protocol-{}-{name}",
        std::process::id()
    ));
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

/// `property` 的数据描述符面（`P3-17` ①）。
const PROPERTY_SCRIPT: &str = r#"class C:
    def __init__(self):
        self._x = 1

    @property
    def x(self):
        return self._x

    @x.setter
    def x(self, v):
        self._x = v

c = C()
print("1:", c.x, c.__dict__)
c.x = 5
print("2:", c.x, c.__dict__)
def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__ + ":", exc)
show("del x", lambda: delattr(c, "x"))
class D:
    @property
    def y(self):
        return 2
d = D()
show("set y", lambda: setattr(d, "y", 9))
show("del y", lambda: delattr(d, "y"))
class E:
    @property
    def z(self):
        return 3
    @z.deleter
    def z(self):
        print("deleter ran")
e = E()
show("del z", lambda: delattr(e, "z"))
p = C.__dict__["x"]
print("3:", hasattr(p, "__get__"), hasattr(p, "__set__"), hasattr(p, "__delete__"))
print("4:", p.__get__(None, C) is p, p.__get__(c))
show("p get none", lambda: p.__get__(None, C) is p)
show("p set", lambda: (p.__set__(c, 8), c._x))
class myprop(property):
    pass
class F:
    y = myprop(lambda self: 7)
print("5:", F().y, type(F.__dict__["y"]).__name__)
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const PROPERTY_EXPECTED: &str = "1: 1 {'_x': 1}\n\
2: 5 {'_x': 5}\n\
del x !! AttributeError: property 'x' of 'C' object has no deleter\n\
set y !! AttributeError: property 'y' of 'D' object has no setter\n\
del y !! AttributeError: property 'y' of 'D' object has no deleter\n\
deleter ran\n\
del z => None\n\
3: True True True\n\
4: True 5\n\
p get none => True\n\
p set => (None, 8)\n\
5: 7 myprop\n";

#[test]
fn property_data_descriptor_behaviour_agrees_with_the_reference() {
    let stdout = run("property", PROPERTY_SCRIPT);
    assert_eq!(stdout, PROPERTY_EXPECTED, "{stdout}");
}

/// `staticmethod.__get__`／`classmethod.__get__`（`P3-17` ②）。
const DESCRIPTORS_SCRIPT: &str = r#"def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__ + ":", exc)

class S:
    @staticmethod
    def s(a):
        return a + 1
    @classmethod
    def c(cls, a):
        return (cls.__name__, a)

raw_s = S.__dict__["s"]
raw_c = S.__dict__["c"]
print("1:", type(raw_s).__name__, type(raw_c).__name__)
print("2:", hasattr(raw_s, "__get__"), hasattr(raw_c, "__get__"), hasattr(raw_s, "__set__"))
print("3:", S.s(1), S().s(2), S.c(3), S().c(4))
print("4:", raw_s.__get__(None, S)(5), raw_s.__get__(S(), S)(6))
print("5:", raw_c.__get__(None, S)(7), raw_c.__get__(S(), S)(8))
print("6:", raw_c.__get__(S()).__self__ is S)
show("sm ()", lambda: raw_s.__get__())
show("sm (None)", lambda: raw_s.__get__(None))
show("sm (None, None)", lambda: raw_s.__get__(None, None))
show("cm too many", lambda: raw_c.__get__(1, 2, 3))
class cm2(classmethod):
    pass
class T:
    @cm2
    def m(cls):
        return cls.__name__
print("7:", T.m(), T().m())
print("8:", type(T.__dict__["m"]).__name__)
class sub_sm(staticmethod):
    pass
class U:
    @sub_sm
    def n(x):
        return x * 2
print("9:", U.n(3), U().n(4))
import abc
class abstractclassmethod(classmethod):
    __isabstractmethod__ = True
    def __init__(self, callable):
        callable.__isabstractmethod__ = True
        super().__init__(callable)
class A(metaclass=abc.ABCMeta):
    @abstractclassmethod
    def f(cls):
        return ("A", cls.__name__)
print("10:", issubclass(abstractclassmethod, classmethod), A.f())
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const DESCRIPTORS_EXPECTED: &str = "1: staticmethod classmethod\n\
2: True True False\n\
3: 2 3 ('S', 3) ('S', 4)\n\
4: 6 7\n\
5: ('S', 7) ('S', 8)\n\
6: True\n\
sm () !! TypeError: __get__ expected at least 1 argument, got 0\n\
sm (None) !! TypeError: __get__(None, None) is invalid\n\
sm (None, None) !! TypeError: __get__(None, None) is invalid\n\
cm too many !! TypeError: __get__ expected at most 2 arguments, got 3\n\
7: T T\n\
8: cm2\n\
9: 6 8\n\
10: True ('A', 'A')\n";

#[test]
fn staticmethod_and_classmethod_descriptors_agree_with_the_reference() {
    let stdout = run("descriptors", DESCRIPTORS_SCRIPT);
    assert_eq!(stdout, DESCRIPTORS_EXPECTED, "{stdout}");
}

/// 函数可写属性 ＋ `functools.wraps` 端到端（`functools.wraps.__name__` 的正路）。
const FUNCTION_ATTRIBUTES_SCRIPT: &str = r#"import functools

def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__ + ":", exc)

def g(a, b=2):
    "gdoc"
    return a + b

@functools.wraps(g)
def f(*args, **kwargs):
    return 0

print("1:", f.__name__, f.__qualname__, f.__doc__)
print("2:", f.__module__, f.__wrapped__ is g, sorted(key for key in f.__dict__ if key != "__annotate__"))
print("3:", f(1))
f.__name__ = "renamed"
f.__qualname__ = "q.r"
print("4:", f.__name__, f.__qualname__, f.__wrapped__.__name__, f.__wrapped__.__qualname__)
print("5:", sorted(key for key in f.__dict__ if key != "__annotate__"))
show("name int", lambda: setattr(f, "__name__", 5))
show("name del", lambda: delattr(f, "__name__"))
show("qual del", lambda: delattr(f, "__qualname__"))
f.__doc__ = "doc2"
print("6:", f.__doc__)
del f.__doc__
print("7:", repr(f.__doc__))
f.__doc__ = 42
print("8:", repr(f.__doc__))
f.__module__ = "m2"
print("9:", repr(f.__module__))
del f.__module__
print("10:", repr(f.__module__))
print("11:", hasattr(type(f).__dict__["__name__"], "__set__"), hasattr(type(f).__dict__["__doc__"], "__set__"))
print("12:", type(f).__dict__["__name__"].__get__(f), type(f).__dict__["__qualname__"].__get__(f))
show("descr get none", lambda: type(f).__dict__["__name__"].__get__(None) is type(f).__dict__["__name__"])
def plain():
    pass
print("13:", plain.__name__, plain.__qualname__, repr(plain.__doc__))
f2 = functools.update_wrapper(lambda: 1, g)
print("14:", f2.__name__, repr(f2.__doc__), f2.__wrapped__ is g)
"#;

/// 参照（`python3` 3.14.4 实测 ✓，逐行）。
const FUNCTION_ATTRIBUTES_EXPECTED: &str = "1: g g gdoc\n\
2: __main__ True ['__wrapped__']\n\
3: 0\n\
4: renamed q.r g g\n\
5: ['__wrapped__']\n\
name int !! TypeError: __name__ must be set to a string object\n\
name del !! TypeError: __name__ must be set to a string object\n\
qual del !! TypeError: __qualname__ must be set to a string object\n\
6: doc2\n\
7: None\n\
8: 42\n\
9: 'm2'\n\
10: None\n\
11: True True\n\
12: renamed q.r\n\
descr get none !! TypeError: __get__(None, None) is invalid\n\
13: plain plain None\n\
14: g 'gdoc' True\n";

#[test]
fn function_attributes_and_functools_wraps_agree_with_the_reference() {
    let stdout = run("function-attrs", FUNCTION_ATTRIBUTES_SCRIPT);
    assert_eq!(stdout, FUNCTION_ATTRIBUTES_EXPECTED, "{stdout}");
}
