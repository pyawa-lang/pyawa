//! 宿主函数与宿主类型注册（`AB-24`…`AB-26`、`AB-35`…`AB-38`）。
//!
//! - **`AB-24`**：宿主函数**经虚拟栈**收发参数（`DESIGN.md` §8.1）
//! - **`AB-25`**：注册**必须**同时提供签名（§9 的 `pa_sig`）；**禁止**无名签名的宿主函数
//! - **`AB-26`**：宿主函数内部的 panic **必须**被捕获并转成状态码（`pa.h` 的 `boundary` 同款）
//! - **`AB-51`／`AB-52`**：`pa_sig`／`pa_param` 都**自带尺寸**（宿主与自己编译时的形状不必等宽，
//!   运行时按 `min(宿主 size, 自身 size)` 读）
//!
//! **本实现补的一条约定**（规格未钉，写进 `pa.h`）：宿主函数返回**状态码**；它把结果留在
//! **栈顶**，调用方取走栈顶那一项当返回值（`PA_OK` 且栈空 ⇒ 返回 `None`）。

use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

use pyawa_core::{Header, Instance, StrObject};


/// `AB-52`：一个参数的签名元数据（**自带尺寸**）。
///
/// 字段顺序与 `pa.h` 一致；`size` 让运行时按宿主编译时的布局读。
#[repr(C)]
pub struct pa_param {
    /// 本结构体的字节数（宿主编译时的值）。
    pub size: usize,
    /// 参数名（NUL 结尾的 UTF-8；可为 `NULL`）。
    pub name: *const c_char,
    /// 注解表达式（如 `"list[int]"`；可为 `NULL` 表示未标注）。
    pub type_expr: *const c_char,
    /// 位置／仅关键字／可变位置／可变关键字／有无默认值（`AB-27` 的七项由这些位表达）。
    pub flags: u32,
    /// 默认值（以不透明句柄给出，`AB-14`）；没有就是 `NULL`。
    pub default_handle: *mut c_void,
}

/// 参数标志位（`AB-27`／`AB-52` 的取值，写进 `pa.h`）。
pub mod param_flags {
    /// 位置或关键字。
    pub const PA_PARAM_POSITIONAL: u32 = 1 << 0;
    /// 仅关键字。
    pub const PA_PARAM_KEYWORD_ONLY: u32 = 1 << 1;
    /// 可变位置（`*args`）。
    pub const PA_PARAM_VARARGS: u32 = 1 << 2;
    /// 可变关键字（`**kwargs`）。
    pub const PA_PARAM_VARKW: u32 = 1 << 3;
    /// 有默认值。
    pub const PA_PARAM_HAS_DEFAULT: u32 = 1 << 4;
}

/// `AB-51`：`pa_sig` **自带尺寸**：`{ size, flags, ret_expr, nparams, params }`。
#[repr(C)]
pub struct pa_sig {
    /// 本结构体的字节数（宿主编译时的值）。
    pub size: usize,
    /// 签名级标志（`AB-37` 的 `PA_TYPE_FINAL` 等）。
    pub flags: u32,
    /// 返回类型的注解表达式（可为 `NULL`）。
    pub ret_expr: *const c_char,
    /// 参数个数。
    pub nparams: usize,
    /// 参数数组（`nparams` 项）。
    pub params: *const pa_param,
}

/// `pa.h` 里 `pa_sig.flags` 的保留位（`AB-37`）：宿主用它**反向**选择"本类型不可继承"。
pub const PA_TYPE_FINAL: u32 = 1 << 0;

/// 宿主函数的 C 形态（`AB-24`）：参数从虚拟栈取、结果留在栈顶、返回状态码。
///
/// 返回值约定见本模块文档（规格未钉的那一条，`pa.h` 里写明）。
///
/// **为什么是 `extern "C-unwind"`**：`AB-26` 要求宿主函数内部的 panic 被捕获并转成状态码
/// （`T-AB-2`）。Rust 里 `extern "C"` 的函数**不允许展开**，panic 会直接 abort，边界上的
/// `catch_unwind` 就没机会接手；`extern "C-unwind"` 与 `"C"` **同一套调用约定**，只是允许
/// 展开到边界被捕获。C 宿主本来不 panic，改这个对其无影响。
pub type PaHostFn = unsafe extern "C-unwind" fn(*mut crate::pa_state) -> i32;

pyawa_core::py_object! {
/// 注册进实例的宿主函数对象（`OM-14`：宿主类型注册为**真实类型**）。
///
/// 它由核心的 `new_type` ＋ `Slots::with_call` 注册（载荷就是本结构体），
/// 于是脚本侧看到的是一个**真实类型**的实例（`AB-35`）。
pub struct HostFunction {
    /// 宿主的 C 函数。
    function: PaHostFn,
    /// 它所属的实例句柄（回调时要交回宿主）。
    state: *mut crate::pa_state,
    /// 签名（**注册时拷贝**的一份，宿主不必让它长期存活；`AB-31`：权威在运行时注册）。
    name: String,
    /// 签名里的返回注解。
    ret_expr: Option<String>,
    /// 签名里的参数。
    params: Vec<HostParam>,
    /// 签名级标志（含 [`PA_TYPE_FINAL`]）。
    flags: u32,
}
}

/// 注册时拷贝下来的一个参数。
#[derive(Clone, Debug)]
pub struct HostParam {
    /// 参数名。
    pub name: Option<String>,
    /// 注解表达式。
    pub type_expr: Option<String>,
    /// 参数标志。
    pub flags: u32,
    /// 默认值（**本对象持有**一份引用；`None` 表示没有）。
    pub default: Option<NonNull<Header>>,
}

impl HostFunction {
    /// 参数个数。
    pub fn arity(&self) -> usize {
        self.params.len()
    }
}

/// 从一个 C 字符串读一份 owned 文本（`NULL` ⇒ `None`）。**有界**：最多读 `limit` 字节。
///
/// # Safety
///
/// `text` 要么是 `NULL`，要么指向 NUL 结尾的可读字符串。
pub unsafe fn read_c_string(text: *const c_char, limit: usize) -> Option<String> {
    if text.is_null() {
        return None;
    }
    let mut length = 0usize;
    // SAFETY: 调用方保证 NUL 结尾；这里限制最长扫描长度，避免指针坏掉时无限跑。
    while length < limit && unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: 上面确认了 length 字节可读。
    let bytes = unsafe { core::slice::from_raw_parts(text.cast::<u8>(), length) };
    String::from_utf8(bytes.to_vec()).ok()
}

/// 按 `AB-51` 的**自带尺寸**读取宿主传来的签名（`min(宿主 size, 自身 size)`，禁止越界读）。
///
/// # Safety
///
/// `sig` 要么是 `NULL`，要么指向一块至少 `size_of::<usize>()` 字节可读的内存
/// （宿主编译时的 `pa_sig`）。
pub unsafe fn read_signature(sig: *const pa_sig) -> Option<Signature> {
    if sig.is_null() {
        return None;
    }
    // SAFETY: 调用方保证至少能读 `size` 字段。
    let declared = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*sig).size)) };
    let limit = declared.min(core::mem::size_of::<pa_sig>());
    let mut signature = Signature {
        flags: 0,
        ret_expr: None,
        params: Vec::new(),
    };
    let flags_offset = core::mem::offset_of!(pa_sig, flags);
    if limit >= flags_offset + core::mem::size_of::<u32>() {
        // SAFETY: 偏移落在宿主声明的尺寸内。
        signature.flags = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*sig).flags)) };
    }
    let ret_offset = core::mem::offset_of!(pa_sig, ret_expr);
    if limit >= ret_offset + core::mem::size_of::<*const c_char>() {
        // SAFETY: 同上。
        let ret = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*sig).ret_expr)) };
        // SAFETY: 同上。
        signature.ret_expr = unsafe { read_c_string(ret, 4096) };
    }
    let nparams_offset = core::mem::offset_of!(pa_sig, nparams);
    let params_offset = core::mem::offset_of!(pa_sig, params);
    if limit >= params_offset + core::mem::size_of::<*const pa_param>() {
        // SAFETY: 同上。
        let count = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*sig).nparams)) };
        // SAFETY: 同上。
        let list = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*sig).params)) };
        if !list.is_null() && count <= 256 {
            for index in 0..count {
                // SAFETY: 宿主保证 params 有 count 项。
                let entry = unsafe { list.add(index) };
                signature.params.push(unsafe { read_param(entry) });
            }
        }
    }
    let _ = nparams_offset;
    Some(signature)
}

/// 读取一个参数的签名（同样按自带尺寸有界读）。
///
/// # Safety
///
/// `param` 指向宿主编译时的 `pa_param`。
pub unsafe fn read_param(param: *const pa_param) -> HostParam {
    // SAFETY: 调用方保证至少能读 `size`。
    let declared = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*param).size)) };
    let limit = declared.min(core::mem::size_of::<pa_param>());
    let mut out = HostParam {
        name: None,
        type_expr: None,
        flags: 0,
        default: None,
    };
    let name_offset = core::mem::offset_of!(pa_param, name);
    if limit >= name_offset + core::mem::size_of::<*const c_char>() {
        // SAFETY: 偏移在宿主声明的尺寸内。
        let name = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*param).name)) };
        // SAFETY: 同上。
        out.name = unsafe { read_c_string(name, 4096) };
    }
    let type_offset = core::mem::offset_of!(pa_param, type_expr);
    if limit >= type_offset + core::mem::size_of::<*const c_char>() {
        // SAFETY: 同上。
        let expr = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*param).type_expr)) };
        // SAFETY: 同上。
        out.type_expr = unsafe { read_c_string(expr, 4096) };
    }
    let flags_offset = core::mem::offset_of!(pa_param, flags);
    if limit >= flags_offset + core::mem::size_of::<u32>() {
        // SAFETY: 同上。
        out.flags = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*param).flags)) };
    }
    out
}

/// 拷贝下来的签名。
#[derive(Clone, Debug, Default)]
pub struct Signature {
    /// 签名级标志。
    pub flags: u32,
    /// 返回注解。
    pub ret_expr: Option<String>,
    /// 参数。
    pub params: Vec<HostParam>,
}

/// 把字符串变成实例里的 `str` 对象（**新引用**）。
pub fn new_text(instance: &Instance, text: &str) -> NonNull<Header> {
    instance.new_str(text)
}

/// 读一个 `str` 对象的文本（**借用**）。
///
/// # Safety
///
/// `object` 必须是本实例里存活的 `str`。
pub unsafe fn text_of(instance: &Instance, object: NonNull<Header>) -> Option<String> {
    // SAFETY: 调用方保证 object 存活。
    if unsafe { object.as_ref() }.ty() != instance.singletons().str_type() {
        return None;
    }
    // SAFETY: 类型身份已确认。
    Some(unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned())
}

// ---- 宿主类型注册（`AB-35`…`AB-38`）----

/// 宿主类型的 `dealloc`（`AB-36`／`OM-34`）：宿主自己释放它的不透明载荷。
pub type PaHostDealloc = unsafe extern "C" fn(*mut c_void);

/// 宿主类型的 `traverse`（`AB-36`／`OM-36`）：宿主把自己持有的**脚本对象引用**报给 VM。
///
/// C 侧没法传闭包，所以形状是"**上下文 ＋ 回调**"：宿主对每个直接引用调用
/// `visit(句柄, context)`。**漏报会永久泄漏、虚报会误回收**（`OM-36`：两者都是缺陷）。
///
/// 宿主**禁止**把 `context`／`visit` 存起来在调用返回之后再用。
pub type PaHostTraverse = unsafe extern "C" fn(
    payload: *mut c_void,
    context: *mut c_void,
    visit: unsafe extern "C" fn(*mut c_void, *mut c_void),
);

pyawa_core::py_object! {
/// 宿主对象（`AB-35`：注册为**真实类型**的实例载荷）。
///
/// 本结构体是**脚本侧**的那一半（VM 的载荷）；宿主的不透明数据在 `payload` 里，
/// 由宿主的 `dealloc` 负责释放（`OM-35`：脚本侧计数、宿主交出所有权）。
pub struct HostObject {
    /// 宿主的不透明数据（可为 `NULL`）。
    payload: *mut c_void,
    /// 宿主的 `dealloc`（`AB-36` 要求必填）。
    dealloc: PaHostDealloc,
    /// 宿主的 `traverse`（`AB-36` 要求必填；`OM-36`：必须列出**全部**直接引用）。
    traverse: PaHostTraverse,
    /// 注册时给这个类型分配的 `kind`（`pa_newhandle` 用它选类型）。
    kind: i32,
}
}

/// 注册的宿主类型记录（每实例一份）。
#[derive(Clone, Copy)]
pub struct RegisteredType {
    /// 类型对象。
    pub ty: NonNull<pyawa_core::TypeObject>,
    /// `pa_newhandle` 用的编号。
    pub kind: i32,
    /// 宿主的 `dealloc`。
    pub dealloc: PaHostDealloc,
    /// 宿主的 `traverse`。
    pub traverse: PaHostTraverse,
}

/// 宿主对象类型的 `dealloc`：先请宿主放掉它的载荷，再放 VM 这一半。
///
/// # Safety
///
/// 由 `Instance` 在计数归零、`clear` 跑过之后调用（`OM-20` ③）。
pub unsafe fn host_object_dealloc(ptr: *mut Header) {
    // SAFETY: 调用方保证 ptr 是本类型的对象。
    let object = unsafe { &*ptr.cast::<HostObject>() };
    if !object.payload.is_null() {
        // SAFETY: 载荷由宿主交来、`dealloc` 由宿主提供（`OM-34`）。
        unsafe { (object.dealloc)(object.payload) };
    }
    // SAFETY: 同上（VM 这一半由 VM 自己放）。
    drop(unsafe { Box::from_raw(ptr.cast::<HostObject>()) });
}

/// 把 VM 的访客包成 C 回调时用的上下文。
struct VisitContext<'a> {
    visit: &'a mut dyn FnMut(*mut Header),
}

/// C 回调：宿主每报一个子引用，就往 VM 的访客里塞一次。
///
/// # Safety
///
/// `context` 必须是 [`VisitContext`] 的指针，且生命周期覆盖这次调用。
unsafe extern "C" fn visit_bridge(handle: *mut c_void, context: *mut c_void) {
    // SAFETY: 由 `host_object_traverse` 保证。
    let context = unsafe { &mut *context.cast::<VisitContext>() };
    (context.visit)(handle.cast::<Header>());
}

/// 宿主对象类型的 `traverse`：请宿主报出它的直接引用（`OM-36`）。
///
/// # Safety
///
/// 由 `Instance` 在标记阶段调用；`visit` 是 VM 的收集回调。
pub unsafe fn host_object_traverse(ptr: *mut Header, visit: &mut dyn FnMut(*mut Header)) {
    // SAFETY: 调用方保证 ptr 是本类型的存活对象。
    let object = unsafe { &*ptr.cast::<HostObject>() };
    if object.payload.is_null() {
        return;
    }
    let mut context = VisitContext { visit };
    // SAFETY: 载荷由宿主交来、`traverse` 由宿主提供；上下文只在这段时间内有效（契约里写明）。
    unsafe {
        (object.traverse)(
            object.payload,
            (&mut context as *mut VisitContext).cast::<c_void>(),
            visit_bridge,
        );
    }
}
