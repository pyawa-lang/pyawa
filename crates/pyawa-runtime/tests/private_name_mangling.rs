//! **类体私有名改写（private name mangling）** 的运行期行为矩阵（`task-5`／缺口 1）。
//!
//! 参照值由本机 `python3` 3.14.4 跑**同一段探针**逐行取得（`EXPECTED` 逐字节比对）。
//! 覆盖（CPython 实测口径，见 `compile::mangle` 模块头注释）：
//! - 类体变量 `__x = 1`／方法内 `self.__x`（读／写）／类外显式 `obj._C__x`；
//! - 尾随下划线 `__x_` 改写、`__dunder__`／`__f___`（≥2 尾随）不改写、单下划线／3 前导；
//! - 方法绑定名 `M._M__m` 与 code 名 `__m`／`__qualname__ == M.__m`（CPython 同）；
//! - 嵌套 class 用**自己的**类名（`_D__v`）、嵌套 def／lambda 继承外层类名；
//! - 形参／仅关键字形参名改写 ⇒ 关键字实参名不改写（从外面只能用 `_P__b=` 传）；
//! - `global`／`nonlocal` 列表改写；`import … as __os`／`from … import … as __sqrt`；
//! - 关键字实参名、match 的捕获名、注解键、`__static_attributes__`（记**源码原名**）；
//! - 类名前导下划线全剥（`_Foo.__x` ⇒ `_Foo__x`、`__A_.__w` ⇒ `_A___w`）；类外不改写。
//!
//! 为什么在 runtime 侧跑：改写发生在**编译期**（`crates/pyawa-core/src/compile/mangle.rs`），
//! 这里用子进程跑真实 CLI 验证"编译 + 执行"整链与参照逐行一致。
//!
//! **`__qualname__` 的既有偏差已修**（task-13 #c）：嵌套 class／函数内 class 的限定名由运行期
//! 类构造按**类命名空间里的完整 qualname** 补全（`classes.rs` 的 `requalified_method`）⇒
//! `class N2: class D` 现与参照一致（`N2.D`／`outer.<locals>.K`）。本矩阵仍只钉私有名改写本身；
//! qualname 面由 `task13_class_surface_shapes.rs` 与 `crates/pyawa-core/tests/classes.rs` 钉住。

/// 探针脚本（与 `python3` 3.14.4 跑的是同一段）。
const SCRIPT: &str = r#"print('== class body variable ==')
class C:
    __x = 1
    def get(self):
        return self.__x
    def set(self, v):
        self.__x = v
c = C()
print(C.__dict__.get('_C__x'), '__x' in C.__dict__)
print(c.get())
c.set(9)
print(c.get(), c.__dict__.get('_C__x'), '__x' in c.__dict__)
print('== explicit _C__x access ==')
print(c._C__x, getattr(c, '_C__x'))
print('== trailing / dunder / single underscore ==')
class T:
    __x_ = 1
    __y__ = 2
    _z = 3
    ___w = 4
    __f___ = 5
print(T.__dict__.get('_T__x_'), T.__dict__.get('__y__'), T.__dict__.get('_z'), T.__dict__.get('_T___w'), T.__dict__.get('__f___'))
print('== method name binding (code name stays source) ==')
class M:
    def __m(self):
        return 7
    def call(self):
        return self.__m()
    def plain(self):
        return 1
print('_M__m' in M.__dict__, '__m' in M.__dict__, M().call(), M._M__m.__name__, M._M__m.__qualname__)
print(M.plain.__name__, M.plain.__qualname__)
print('== nested class (own class name) ==')
class N:
    class __D:
        __v = 5
        def get(self):
            return self.__v
    def __f(self):
        return 1
print('_N__D' in N.__dict__, N._N__D.__name__)
print(N._N__D().get(), N._N__D.__dict__.get('_D__v'), N._N__f.__name__)
print('== nested def / lambda inherit outer class name ==')
class F:
    __w = 3
    def m(self):
        __loc = 4
        def inner():
            return __loc + self.__w
        lam = lambda: self.__w
        return inner() + lam()
print(F().m())
print('== parameters / keyword-only ==')
class P:
    def m(self, __a, *, __b=2):
        return __a + __b
p = P()
print(p.m(1, _P__b=3), P.m.__code__.co_varnames)
for wrong in ('__b', 'b'):
    try:
        p.m(1, **{wrong: 3})
    except TypeError:
        print('TypeError', wrong)
print('== global / nonlocal ==')
_G__g = 10
class G:
    def m(self):
        global __g
        __g = __g + 1
        return __g
print(G().m(), _G__g, '_G__g' in globals(), '__g' in globals())
def outer():
    # 方法里 `nonlocal __n` 会被改写成 `_H__n`（H 是最近的外层类）⇒ 外层要真的绑定 `_H__n`
    #（参照实测：外层只绑 `__n` 时报 SyntaxError: no binding for nonlocal '_H__n'）。
    _H__n = 0
    class H:
        def bump(self):
            nonlocal __n
            __n += 1
            return __n
    return H
H = outer()
print(H().bump(), H().bump())
print('== import alias / module member ==')
class I:
    import os as __os
    from math import sqrt as __sqrt
    def m(self):
        return self.__os.sep, self.__sqrt(9)
print(I().m(), '_I__os' in I.__dict__, '_I__sqrt' in I.__dict__)
print('== keyword argument names are NOT mangled ==')
def kw(**kw):
    return sorted(kw)
class K:
    def m(self):
        return kw(__x=1, __y__=2)
print(K().m())
print('== annotation keys ==')
class A:
    __x: int = 1
    def m(self, __p: str) -> bytes:
        return b''
print(sorted(A.__annotations__.keys()), sorted(A.m.__annotations__.keys()))
print('== __static_attributes__ keeps source names ==')
class S:
    def m(self):
        self.__p = 1
        self.__q_ = 2
print(S.__static_attributes__)
print('== class name leading underscores ==')
class _Foo:
    __x = 1
class __A_:
    __w = 2
print(_Foo.__dict__.get('_Foo__x'), __A_.__dict__.get('_A___w'))
print('== outside class: unchanged ==')
def free():
    __x = 1
    def inner():
        return __x
    return inner()
print(free(), '__x' in free.__code__.co_varnames)

print('== comprehension in class body ==')
class C1:
    y = [__c for __c in range(3)]
    z = (__d for __d in range(2))
    def m(self):
        return [__e for __e in range(2)]
print(C1.y, list(C1.z), C1().m())
print('== with / except / for targets ==')
class C2:
    def m(self):
        out = []
        for __i in range(2):
            out.append(__i)
        with open(__file__, 'r', encoding='utf-8') as __f:
            pass
        try:
            raise ValueError('x')
        except ValueError as __e:
            out.append(str(__e))
        return out
print(C2().m())
print('== match capture / as ==')
class C3:
    def m(self, v):
        match v:
            case __cap:
                return __cap
        return None
    def n(self, v):
        match v:
            case [__first, *__rest]:
                return __first, __rest
print(C3().m(5), C3().n([1, 2, 3]))
print('== walrus / del / augmented attr ==')
class C4:
    def __init__(self):
        self.__n = 1
    def m(self):
        (__w := 5)
        self.__n += __w
        del self.__n
        return __w, hasattr(self, '_C4__n')
print(C4().m())
print('== type alias in class body ==')
class C5:
    type __T = int
    def m(self):
        return C5._C5__T
print('typealias-key', '_C5__T' in C5.__dict__, C5._C5__T is C5().m())
print('== nested class in method ==')
class C6:
    def m(self):
        class __Inner:
            def get(self):
                return self.__v
        return __Inner
Inner = C6().m()
i = Inner()
i._Inner__v = 3
print(Inner.__name__, i.get())
print('== super() with private method ==')
class Base:
    def __m(self):
        return 'base'
class Derived(Base):
    def __m(self):
        return 'derived'
    def call(self):
        return self.__m(), Base._Base__m(self)
print(Derived().call())
print('== f-string / decorators / property ==')
class C7:
    __v = 7
    @property
    def __p(self):
        return self.__v
    def m(self):
        return f'{self.__p}-{self.__v}'
print(C7().m())
print('== all-underscore class name: no mangling ==')
class __:
    __x = 1
print(__.__dict__.get('__x'), '_' + '__x' in __.__dict__)
class ___:
    __y = 2
print(_.__dict__.get('__y') if False else ___.__dict__.get('__y'))
print('== explicit pre-mangled and double underscore in same class ==')
class C8:
    _C8__a = 1
    def m(self):
        self.__a = 2
c8 = C8()
c8.m()
print(C8._C8__a, c8._C8__a)
"#;

/// 参照（`python3` 3.14.4 实测，逐行）。
const EXPECTED: &str = r#"== class body variable ==
1 False
1
9 9 False
== explicit _C__x access ==
9 9
== trailing / dunder / single underscore ==
1 2 3 4 5
== method name binding (code name stays source) ==
True False 7 __m M.__m
plain M.plain
== nested class (own class name) ==
True __D
5 5 __f
== nested def / lambda inherit outer class name ==
10
== parameters / keyword-only ==
4 ('self', '_P__a', '_P__b')
TypeError __b
TypeError b
== global / nonlocal ==
11 11 True False
1 2
== import alias / module member ==
('/', 3.0) True True
== keyword argument names are NOT mangled ==
['__x', '__y__']
== annotation keys ==
['_A__x'] ['_A__p', 'return']
== __static_attributes__ keeps source names ==
('__p', '__q_')
== class name leading underscores ==
1 2
== outside class: unchanged ==
1 False
== comprehension in class body ==
[0, 1, 2] [0, 1] [0, 1]
== with / except / for targets ==
[0, 1, 'x']
== match capture / as ==
5 (1, [2, 3])
== walrus / del / augmented attr ==
(5, False)
== type alias in class body ==
typealias-key True True
== nested class in method ==
__Inner 3
== super() with private method ==
('derived', 'base')
== f-string / decorators / property ==
7-7
== all-underscore class name: no mangling ==
1 False
2
== explicit pre-mangled and double underscore in same class ==
1 2
"#;

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-mangle-{}-{name}", std::process::id()));
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
    assert!(
        output.stderr.is_empty(),
        "stderr 必须为空：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn private_name_mangling_matches_the_reference() {
    let stdout = run("matrix", SCRIPT);
    assert_eq!(stdout, EXPECTED, "改写矩阵与参照不一致\n--- got ---\n{stdout}");
}
