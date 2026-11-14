//! **类/类型对象面三件**（task-13 #b/#c/#d）的端到端对拍验收与回归。
//!
//! 探针与**本机 `python3` 3.14.4** 跑同一段脚本、逐行一致（期望串逐字节钉住）：
//!
//! - **#b `__slots__` 私有名改写**：`__slots__ = ("__x", "y")` ⇒ 类型字典里的槽名按 `BC-66`
//!   改写（`_A__x`）且是 `member_descriptor`（`__name__`／`__qualname__`／`__objclass__` 照参照）；
//!   槽读写/删除/未设置报错正常；`str` 单名、非标识符、非字符串元素、重复、冲突、不可迭代
//!   等声明错误文案与参照一致。
//! - **#c 类/方法的 `__qualname__`**：嵌套类 `N2.D`、函数内类 `outer.<locals>.K`、以及其中
//!   方法的完整限定名（本层编译器只发射 `<简单类名>.<源名>`，运行期按类的完整 qualname 补全）。
//! - **#d 类型属性 getset 面**：`type.__dict__['__name__'/'__qualname__'/'__module__'/'__bases__']`
//!   是 `getset_descriptor` 实例（`__name__`／`__objclass__` 齐全），`__get__` 读 TypeObject／
//!   命名空间；`inspect.getattr_static(类, "__name__")` 取到的就是它。
//!
//! **已知边界**（不在本探针里；工单报告里列了证据）：
//! - `type.__dict__['__name__']`／`__slots__` 的 `member_descriptor` 目前**没有 `__set__`**
//!   （`inspect.isdatadescriptor` 为 False，参照 True）——给共享的 `getset_descriptor` 类型加
//!   数据描述符会牵动 `function.__code__` 等持有位的**写**语义（`TypeObject`／`FunctionObject`
//!   存储字段在本任务写域外）；读面与写入口的**报错/值语义**已按参照实现（见探针里的
//!   `renamed name`／`renamed qualname` 行）；
//! - `__slots__` 里写 `__dict__`／`__weakref__` 时本层建的是 member_descriptor（参照是 getset）；
//! - 用户类的隐式 `__dict__`／`__weakref__` 描述符本层本来就不进类字典（既有偏差，与 #b 无关）；
//! - 类外直接写未改写的私有属性名（`c.__x`）仍放行（`instance/misc.rs` 的 round-150 兼容
//!   分支；该文件按 Lead 要求不碰）。
//!
//! ⚠ 只放一个 `#[test]`（探针 import 了 `inspect` ⇒ 依赖 `re`）：历史上 `_sre` 的进程级缓存
//! 会让同进程第二个 `Instance` 串到第一个实例的对象上（既有 bug，另有工单）。

use core::ffi::c_void;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_abi::status::*;
use pyawa_abi::*;
use pyawa_runtime::fs_posix::PosixFs;
use pyawa_runtime::PaState;

fn c_string(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).expect("测试用名字不含 NUL")
}

/// 装实例：stdlib ＋ 模块搜索路径（`Lib/`）＋ `fs` 域（`inspect` 要读 `Lib/inspect.py`）。
fn make_state(vtable: &pyawa_capabilities::fs::CpFsVtable) -> PaState {
    let state = PaState::new().expect("建实例");
    let raw = state.as_ptr();
    // SAFETY: `raw` 由 `PaState::new` 交回、活到本函数返回值被 drop。
    let instance = unsafe { &*raw }.instance();
    pyawa_stdlib::install(instance, "task13-class-surface", &[]);
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|parent| parent.parent())
        .expect("crates/<crate> 的上一级是仓库根")
        .join("Lib");
    pyawa_stdlib::set_module_search_path(instance, &[lib.to_string_lossy().into_owned()]);
    let implementation = (vtable as *const pyawa_capabilities::fs::CpFsVtable).cast::<c_void>();
    // SAFETY: `raw` 刚建成功；表由调用方持有（活到 state 之后）。
    unsafe {
        assert_eq!(
            pa_setcapability_async(raw, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK,
            "`fs` 域分类注册应当成功"
        );
        assert_eq!(
            pa_setcapability(raw, PA_DOMAIN_FS, implementation),
            PA_OK,
            "`fs` 域注册应当成功"
        );
    }
    state
}

fn error_message(state: *mut pa_state) -> String {
    // SAFETY: state 存活；借用只在本函数内用。
    let pointer = unsafe { pa_errmsg(state) };
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: `pa_errmsg` 返回 NUL 结尾借用（`AB-48`）。
    unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

/// 跑一段脚本；`python` 模式。
///
/// # Safety
///
/// `state` 存活。
unsafe fn exec(state: *mut pa_state, source: &str) {
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            b"python\0".as_ptr().cast(),
            core::ptr::null(),
        )
    };
    assert_eq!(status, PA_OK, "脚本执行失败：{}", error_message(state));
}

/// 取脚本写进全局的名字（字符串）。
///
/// # Safety
///
/// `state` 存活。
unsafe fn global_string(state: *mut pa_state, name: &str) -> String {
    let name = c_string(name);
    // SAFETY: state 存活。
    unsafe {
        assert_eq!(pa_getglobal(state, name.as_ptr()), PA_OK);
        let mut length = 0usize;
        let pointer = pa_tostring(state, -1, &mut length);
        assert!(!pointer.is_null(), "全局 {name:?} 不是字符串");
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), length);
        let text = String::from_utf8(bytes.to_vec()).expect("测试用文本是 UTF-8");
        assert_eq!(pa_settop(state, 0), PA_OK);
        text
    }
}

/// 对拍用探针脚本（与 `python3` 3.14.4 跑的是同一段；把 `print` 重定向到一个列表里，
/// 末行拼成 `results` 供宿主取回 ⇒ 不依赖 stdout 能力域）。
const SCRIPT: &str = r#"import inspect

out = []
def print(*args):
    out.append(" ".join(str(arg) for arg in args))

# ---- #b __slots__ ----
class A:
    __slots__ = ("__x", "y")
    def set_x(self, v):
        self.__x = v
    def get_x(self):
        return self.__x

a = A()
a.set_x(5)
print("slots keys", sorted(k for k in A.__dict__ if k in ("_A__x", "y")))
d = A.__dict__["_A__x"]
print("slot desc", type(d).__name__, d.__name__, d.__qualname__, d.__objclass__ is A)
print("slot read", a.get_x())
print("slot static", inspect.getattr_static(A, "_A__x") is d)
try:
    A.__dict__["y"].__get__(a, A)
except AttributeError as e:
    print("slot unset", type(e).__name__, str(e))
a2 = A(); a2.set_x(9); del a2._A__x
try:
    a2._A__x
except AttributeError as e:
    print("slot del read", type(e).__name__, str(e))
for label, raw in [("str-single", "ab"), ("str-bad", "a b"), ("nonstr", (1,)), ("dup", ("q", "q"))]:
    try:
        C = type("Dyn", (), {"__slots__": raw})
        print(label, "ok", sorted(k for k in C.__dict__ if k in ("ab", "q")))
    except Exception as e:
        print(label, type(e).__name__, str(e))
try:
    class Conf:
        __slots__ = ("z",)
        z = 1
    print("conflict ok")
except Exception as e:
    print("conflict", type(e).__name__, str(e))
try:
    class NonIter:
        __slots__ = 5
    print("noniter ok")
except Exception as e:
    print("noniter", type(e).__name__, str(e))

# ---- #c __qualname__ ----
class N2:
    class D:
        def m(self):
            pass
def outer():
    class K:
        def km(self):
            pass
    return K
N2D = N2.D
K = outer()
print("qualifier", N2.D.__qualname__, N2.D.m.__qualname__, K.__qualname__, K.km.__qualname__)

# ---- #d type.__dict__ 类型属性面 ----
class C:
    pass
class CB:
    __name__ = "shadow"
print("class dict qualname", "__qualname__" in N2.D.__dict__, "__qualname__" in K.__dict__)
print("static qualname", type(inspect.getattr_static(N2.D, "__qualname__")).__name__)
print("class-body name", CB.__name__, CB.__dict__.get("__name__"))
C.__name__ = "Renamed"
print("renamed name", C.__name__, C.__dict__.get("__name__", "ABSENT"))
C.__qualname__ = "Renamed.Q"
print("renamed qualname", C.__qualname__, "__qualname__" in C.__dict__)
print("type dict", sorted(k for k in type.__dict__ if k in ("__name__", "__qualname__", "__module__", "__bases__")))
name_desc = type.__dict__["__name__"]
print("name desc", type(name_desc).__name__, name_desc.__name__, name_desc.__objclass__ is type)
print("name get", name_desc.__get__(C, type), name_desc.__get__(N2.D, type), name_desc.__get__(int, type))
print("name repr", repr(name_desc))
print("static C name", type(inspect.getattr_static(C, "__name__")).__name__, inspect.getattr_static(C, "__name__").__objclass__ is type)
print("module desc", type.__dict__["__module__"].__get__(C, type), type.__dict__["__module__"].__get__(int, type))
print("bases desc", type.__dict__["__bases__"].__get__(C, type)[0].__name__)
print("C surface", C.__name__, C.__qualname__, C.__module__)
print("signature", str(inspect.signature(N2.D)), str(inspect.signature(C)))

results = "\n".join(out) + "\n"
"#;

/// 参照值（本机 `python3` 3.14.4 跑同一段脚本的真实输出）。
const EXPECTED: &str = r#"slots keys ['_A__x', 'y']
slot desc member_descriptor _A__x A._A__x True
slot read 5
slot static True
slot unset AttributeError 'A' object has no attribute 'y'
slot del read AttributeError 'A' object has no attribute '_A__x'
str-single ok ['ab']
str-bad TypeError __slots__ must be identifiers
nonstr TypeError __slots__ items must be strings, not 'int'
dup ok ['q']
conflict ValueError 'z' in __slots__ conflicts with class variable
noniter TypeError 'int' object is not iterable
qualifier N2.D N2.D.m outer.<locals>.K outer.<locals>.K.km
class dict qualname False False
static qualname getset_descriptor
class-body name CB shadow
renamed name Renamed ABSENT
renamed qualname Renamed.Q False
type dict ['__bases__', '__module__', '__name__', '__qualname__']
name desc getset_descriptor __name__ True
name get Renamed D int
name repr <attribute '__name__' of 'type' objects>
static C name getset_descriptor True
module desc __main__ builtins
bases desc object
C surface Renamed Renamed.Q __main__
signature () ()
"#;

/// **task-13 #b/#c/#d 的对拍验收**：探针输出与参照逐行一致。
#[test]
fn class_surface_matches_reference() {
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    let state = make_state(&vtable);
    let raw = state.as_ptr();
    // SAFETY: state 存活。
    unsafe {
        exec(raw, SCRIPT);
        assert_eq!(
            global_string(raw, "results"),
            EXPECTED,
            "task-13 #b/#c/#d：类/类型对象面必须与本机 python3 3.14.4 逐行一致"
        );
    }
    // `PaState` 在作用域结束处销毁实例（不要在这里再 `pa_destroy`：那会双释放）。
}
