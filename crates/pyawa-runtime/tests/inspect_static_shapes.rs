//! **`inspect.getattr_static`／`getset_descriptor` 面**（task-8／缺口4）的端到端验收。
//!
//! 缺口链（修前实测）：`Lib/types.py` 的 `GetSetDescriptorType = type(FunctionType.__code__)`
//! 原先取到的是**原生函数**（`builtin_function_or_method`）✗ ⇒ `inspect._shadowed_dict` 的
//! `type(class_dict) is types.GetSetDescriptorType` 永假 ⇒ `_check_class` 恒返回 sentinel
//! ⇒ `inspect.getattr_static(C, "m", "MISSING") == "MISSING"` ✗。
//!
//! 修法（本测试钉住的结果）：
//! - `function.__code__`／`__globals__` 的**类属性**是真正的 `getset_descriptor`／
//!   `member_descriptor` 实例（与 `type.__dict__['__dict__']` 同类型）——`type(FunctionType.__code__)`
//!   与 `type(FunctionType.__globals__)` 分别落到两个类型上（与参照 3.14.4 一致）；
//! - 描述符实例带 `__name__`／`__objclass__`／`_kind` 内联属性（`_shadowed_dict` 逐字检查前两项）；
//! - `type.__dict__['__dict__']` 的 `__name__ == "__dict__"`、`__objclass__ is type`；
//! - **实例属性语义不变**：`f.__code__`／`f.__globals__` 仍走 `function` 的 getattr 槽
//!   （本层 code 类型的 `__name__` 是 `CodeObject`，参照叫 `code` —— 既有登记偏差，与本工单无关）。
//!
//! ⚠ **本测试文件只放一个 `#[test]`**：`pyawa-stdlib` 的 `_sre_module` 里有**进程级**缓存
//! （`CLASSES`／`PATTERNS`／`MATCHES` 是 `static Mutex`，见该文件）⇒ 同进程内**第二个**
//! `Instance` 再 `import re`／`json` 时会把第一个实例的 `re.Pattern` 类型当成自己的
//! ⇒ 已登记的跨实例串状态 bug（**先于本工单存在**，与本修复无关；已报 Lead）。
//! 单实例测试可避开它，同时仍然完整覆盖验收面。

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
    pyawa_stdlib::install(instance, "inspect-static", &[]);
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

/// 验收①②：`getattr_static` 返回类体对象、描述符别名/名称/objclass 与参照一致、
/// `inspect.signature` 与函数 `__code__`／`__globals__` 的实例语义不回退。
#[test]
fn inspect_static_and_descriptor_surface() {
    let provider = PosixFs::new();
    let vtable = provider.vtable();
    let state = make_state(&vtable);
    let raw = state.as_ptr();
    let script = concat!(
        "import inspect, types\n",
        "class C:\n",
        "    class_attr = 42\n",
        "    def __init__(self, x=1):\n",
        "        self.x = x\n",
        "    def m(self):\n",
        "        return 1\n",
        "    @property\n",
        "    def p(self):\n",
        "        return 'p'\n",
        "class D(C):\n",
        "    pass\n",
        "class E:\n",
        "    def __getattr__(self, name):\n",
        "        raise RuntimeError('dynamic lookup called')\n",
        "    def real(self):\n",
        "        return 1\n",
        "def f(a, b=2, *args, c=3, **kw):\n",
        "    pass\n",
        "c = C()\n",
        "c.inst_attr = 7\n",
        // ① getattr_static：返回类体里的真对象 / 实例属性 / 继承 / 不触发 __getattr__ / 缺省值。
        "static = ' '.join([\n",
        "    str(inspect.getattr_static(C, 'm') is C.__dict__['m']),\n",
        "    str(inspect.getattr_static(C, 'p') is C.__dict__['p']),\n",
        "    str(inspect.getattr_static(C, 'class_attr')),\n",
        "    str(inspect.getattr_static(c, 'm') is C.__dict__['m']),\n",
        "    str(inspect.getattr_static(c, 'class_attr')),\n",
        "    str(inspect.getattr_static(c, 'inst_attr')),\n",
        "    str(inspect.getattr_static(C, 'nope', 'MISSING')),\n",
        "    str(inspect.getattr_static(D, 'class_attr')),\n",
        "    type(inspect.getattr_static(E(), 'real')).__name__,\n",
        "    str(inspect.getattr_static(E(), 'fake', 'DEFAULT')),\n",
        "])\n",
        // ② 描述符类型面：types 别名 / getset 名称与 objclass / _shadowed_dict 判定。
        "descriptors = ' '.join([\n",
        "    types.GetSetDescriptorType.__name__,\n",
        "    types.MemberDescriptorType.__name__,\n",
        "    str(types.GetSetDescriptorType is types.MemberDescriptorType),\n",
        "    type(types.FunctionType.__code__).__name__,\n",
        "    type(types.FunctionType.__globals__).__name__,\n",
        "    type.__dict__['__dict__'].__name__,\n",
        "    str(type.__dict__['__dict__'].__objclass__ is type),\n",
        "    str(type.__dict__['__mro__'].__objclass__ is type),\n",
        "    str(types.FunctionType.__code__.__objclass__ is types.FunctionType),\n",
        "    str(type.__dict__['__dict__'].__get__(None, type) is type.__dict__['__dict__']),\n",
        "    str(inspect._shadowed_dict(C) is inspect._sentinel),\n",
        "    str(inspect._shadowed_dict(type) is inspect._sentinel),\n",
        "    str(inspect.isgetsetdescriptor(type.__dict__['__dict__'])),\n",
        "    str(inspect.ismemberdescriptor(types.FunctionType.__globals__)),\n",
        "])\n",
        // ③ signature 与函数 __code__／__globals__ 的实例语义。
        "signature = ' '.join([\n",
        "    str(inspect.signature(f)),\n",
        "    str(inspect.signature(C)),\n",
        "    type(f.__code__).__name__,\n",
        "    type(f.__globals__).__name__,\n",
        "    str(types.FunctionType.__code__.__get__(f) is f.__code__),\n",
        "    str(types.FunctionType.__globals__.__get__(f) is f.__globals__),\n",
        "    str(types.FunctionType.__code__.__get__(None, types.FunctionType) is types.FunctionType.__code__),\n",
        "])\n",
    );
    // SAFETY: state 存活。
    unsafe {
        exec(raw, script);
        // ① 参照 3.14.4 实跑同一断言的输出（`getattr_static` 返回真对象 ✓）。
        assert_eq!(
            global_string(raw, "static"),
            "True True 42 True 42 7 MISSING 42 function DEFAULT",
            "验收①：inspect.getattr_static 必须返回类体里真正的对象"
        );
        // ② 参照 3.14.4：GetSet= getset_descriptor、Member= member_descriptor（不同类）。
        assert_eq!(
            global_string(raw, "descriptors"),
            "getset_descriptor member_descriptor False getset_descriptor member_descriptor \
             __dict__ True True True True True True True True",
            "描述符别名／__name__／__objclass__ 必须与参照一致，_shadowed_dict 才认得出未遮蔽"
        );
        // ③ 参照：`(a, b=2, *args, c=3, **kw)`／`(x=1)` 逐字一致；code 类型名 `CodeObject`
        //    是本层既有登记偏差（参照叫 `code`），与本工单无关。
        assert_eq!(
            global_string(raw, "signature"),
            "(a, b=2, *args, c=3, **kw) (x=1) CodeObject dict True True True",
            "验收②：inspect.signature 与函数 __code__／__globals__ 的实例语义不得回退"
        );
    }
    // `PaState` 在作用域结束处销毁实例（不要在这里再 `pa_destroy`：那会双释放）。
}
