//! 辅助层 `paL_`（`§15.4`，18 个）：**不引入核心层没有的语义**（`AB-4`／`AB-6`）。
//!
//! §15.4 只给名字与语义、**没有给签名**；本层按 `AB-19`（跨边界函数必须返回状态码）统一取
//! "**状态码 ＋ 出参**"形态，签名写在 `include/pa.h` 里（那处就是唯一定义处）。
//!
//! 三处**结构性限制**（不是偷懒，写在 `README.md` 与 `pa.h`）：
//! - `paL_error` 的 C 变参格式化**做不到**：稳定版 Rust 不能定义 C 变参函数（要 `vsnprintf`）。
//!   本层取"调用方先拼好消息"的单参数形态；`format` 那一路等 `AB-44` 的版本策略裁定后再定。
//! - `paL_openlibs`／`paL_dostring`／`paL_dofile` 分别要标准库（`P3-14`）与编译器（`P3-12`）
//! - `paL_where` 要 traceback（`OM-28` 尚未接线）、`paL_requiref` 要模块系统（`IM-`）

use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

use pyawa_core::{DictObject, Header, Instance, ListObject, StrObject, TupleObject};

use crate::status::*;
use crate::{pa_state, stack};

/// 批量注册表项（`paL_setfuncs` 用；`AB-25` 要求每个函数都带签名，故这里也带）。
#[repr(C)]
pub struct pa_reg {
    /// 函数名（NUL 结尾的 UTF-8）。
    pub name: *const c_char,
    /// 宿主函数。
    pub function: crate::host::PaHostFn,
    /// 签名（`AB-25`：**必须**给）。
    pub sig: *const crate::host::pa_sig,
}

/// 把一个已存在的值放进状态（给辅助层内部用）。
pub(crate) fn state_of<'a>(state: *mut pa_state) -> Option<&'a mut pa_state> {
    // SAFETY: 调用方保证 state 是 `pa_create` 交回且尚未销毁的指针。
    unsafe { state.as_mut() }
}

/// 给状态写一条错误信息（`pa_errmsg` 会取回它）。
pub(crate) fn set_message(state: &mut pa_state, message: &str) {
    state.set_message(message);
}

/// 长度（`paL_len`）：字符串按**字节数**（`TS-22`：长度按码点，等的实现落地后再对齐），
/// 表按条目数。
fn length_of(instance: &Instance, object: NonNull<Header>) -> Option<usize> {
    // SAFETY: 调用方保证 object 存活。
    let ty = unsafe { object.as_ref() }.ty();
    if ty == instance.singletons().str_type() {
        // SAFETY: 类型身份已确认。
        return Some(unsafe { &*object.as_ptr().cast::<StrObject>() }.value().len());
    }
    if Some(ty) == instance.type_named("dict") {
        // SAFETY: 同上。
        return Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries().len());
    }
    if Some(ty) == instance.type_named("list") {
        // SAFETY: 同上。
        return Some(unsafe { &*object.as_ptr().cast::<ListObject>() }.len());
    }
    if Some(ty) == instance.type_named("tuple") {
        // SAFETY: 同上。
        return Some(unsafe { &*object.as_ptr().cast::<TupleObject>() }.len());
    }
    if Some(ty) == instance.type_named("set") {
        // SAFETY: 同上。
        return Some(unsafe { &*object.as_ptr().cast::<DictObject>() }.entries().len());
    }
    None
}

/// `paL_checkinteger(st, idx, out)`：必须是整数，否则**抛错**（`TypeError` 语义）。
///
/// # Safety
///
/// 同 `pa_gettop`；`out` 必须可写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_checkinteger(
    state: *mut pa_state,
    index: i32,
    out: *mut i64,
) -> i32 {
    let status = unsafe { crate::pa_tointeger(state, index, out) };
    if status == PA_OK {
        return PA_OK;
    }
    if let Some(state) = state_of(state) {
        set_message(state, "整数参数检查失败（类型不符）");
    }
    PA_ERR_RUNTIME
}

/// `paL_optinteger(st, idx, def, out)`：缺省（`nil`）时用 `def`。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_optinteger(
    state: *mut pa_state,
    index: i32,
    default: i64,
    out: *mut i64,
) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    let absent = matches!(state_ref.stack.get(index), Some(slot) if stack::tag_of(&state_ref.instance, slot.object) == stack::tag::PA_TNIL);
    if absent {
        if out.is_null() {
            return PA_ERR_INVALID;
        }
        // SAFETY: 调用方保证 out 可写。
        unsafe { *out = default };
        return PA_OK;
    }
    unsafe { paL_checkinteger(state, index, out) }
}

/// `paL_checkstring(st, idx, out, len)`：必须是字符串，否则抛错。返回**只读视图**（借用）。
///
/// # Safety
///
/// 同 `pa_tostring`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_checkstring(
    state: *mut pa_state,
    index: i32,
    out: *mut *const c_char,
    len: *mut usize,
) -> i32 {
    if out.is_null() {
        return PA_ERR_INVALID;
    }
    let view = unsafe { crate::pa_tostring(state, index, len) };
    if view.is_null() {
        if let Some(state) = state_of(state) {
            set_message(state, "字符串参数检查失败（类型不符）");
        }
        return PA_ERR_RUNTIME;
    }
    // SAFETY: 调用方保证 out 可写。
    unsafe { *out = view };
    PA_OK
}

/// `paL_optstring(st, idx, def, out, len)`：缺省时用 `def`（NUL 结尾；`NULL` 表示空串）。
///
/// # Safety
///
/// 同 [`paL_checkstring`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_optstring(
    state: *mut pa_state,
    index: i32,
    default: *const c_char,
    out: *mut *const c_char,
    len: *mut usize,
) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    let absent = matches!(state_ref.stack.get(index), Some(slot) if stack::tag_of(&state_ref.instance, slot.object) == stack::tag::PA_TNIL);
    if absent {
        if out.is_null() {
            return PA_ERR_INVALID;
        }
        // SAFETY: 调用方保证 out／len 可写。
        unsafe {
            *out = default;
            if !len.is_null() {
                *len = if default.is_null() {
                    0
                } else {
                    core::ffi::CStr::from_ptr(default).to_bytes().len()
                };
            }
        }
        return PA_OK;
    }
    unsafe { paL_checkstring(state, index, out, len) }
}

/// `paL_len(st, idx, out)`：长度（字符串按字节、表/列表/元组/集合按项数）。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_len(state: *mut pa_state, index: i32, out: *mut usize) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    if out.is_null() {
        return PA_ERR_INVALID;
    }
    let Some(slot) = state_ref.stack.get(index) else {
        return PA_ERR_INVALID;
    };
    match length_of(&state_ref.instance, slot.object) {
        Some(length) => {
            // SAFETY: 调用方保证 out 可写。
            unsafe { *out = length };
            PA_OK
        }
        None => {
            set_message(state_ref, "取长度的对象既不是字符串也不是容器");
            PA_ERR_RUNTIME
        }
    }
}

/// `paL_getsubtable(st, idx, name)`：取或建一个子表（+1）。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_getsubtable(
    state: *mut pa_state,
    index: i32,
    name: *const c_char,
) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    // SAFETY: 调用方保证 name 是 NUL 结尾。
    let Some(text) = (unsafe { crate::host::read_c_string(name, 4096) }) else {
        return PA_ERR_INVALID;
    };
    let Some(container) = state_ref.stack.get(index).map(|slot| slot.object) else {
        return PA_ERR_INVALID;
    };
    // 自己造一份键并**持有到本次调用结束**（`subscript_read`／`subscript_write` 都只借用它）
    let key = state_ref.instance.new_str(&text);
    let outcome = match pyawa_core::subscript_read(&state_ref.instance, container, key) {
        Ok(value) => {
            // 已经有子表：直接用；不是表就覆盖掉
            // SAFETY: value 是新引用，存活。
            let is_table = Some(unsafe { value.as_ref() }.ty()) == state_ref.instance.type_named("dict");
            if is_table {
                // SAFETY: 键那份引用由本函数持有，归还。
                unsafe { state_ref.instance.release_object(key.as_ptr()) };
                return state_ref.stack.push_owned(value);
            }
            // SAFETY: 不是表 ⇒ 归还这个值，改建一张表。
            unsafe { state_ref.instance.release_object(value.as_ptr()) };
            build_subtable(state_ref, container, key)
        }
        Err(_) => build_subtable(state_ref, container, key),
    };
    // SAFETY: 键那份引用由本函数持有，归还。
    unsafe { state_ref.instance.release_object(key.as_ptr()) };
    outcome
}

/// 建一个空表并写进容器（`paL_getsubtable` 的后半段）。
///
/// **`key` 是借用**（`subscript_write` 的契约）——归还由调用方负责。
fn build_subtable(state: &mut pa_state, container: NonNull<Header>, key: NonNull<Header>) -> i32 {
    let dict_type = match state.instance.type_named("dict") {
        Some(ty) => ty,
        None => return PA_ERR_RUNTIME,
    };
    let table = state
        .instance
        .alloc(DictObject::new(dict_type, core::cell::RefCell::new(Vec::new())))
        .into_raw()
        .cast::<Header>();
    // 容器要一份（`subscript_write` 接管值），栈也要一份
    // SAFETY: table 存活。
    unsafe { state.instance.incref_object(table.as_ptr()) };
    if pyawa_core::subscript_write(&state.instance, container, key, table).is_err() {
        // SAFETY: 写失败：把容器那份归还。
        unsafe { state.instance.release_object(table.as_ptr()) };
        return PA_ERR_RUNTIME;
    }
    state.stack.push_owned(table)
}

/// `paL_ref(st, idx, out)`：把栈项存入**注册表**，返回整数键（`AB-15`：转为持有）。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_ref(state: *mut pa_state, index: i32, out: *mut i32) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    let Some(slot) = state_ref.stack.get(index) else {
        return PA_ERR_INVALID;
    };
    // 注册表持有一份（`AB-15` 的借用 → 持有）
    // SAFETY: 槽位借用着存活引用。
    unsafe { state_ref.instance.incref_object(slot.object.as_ptr()) };
    state_ref.registry.push(Some(slot.object));
    let reference = state_ref.registry.len() as i32;
    if !out.is_null() {
        // SAFETY: 调用方保证 out 可写。
        unsafe { *out = reference };
    }
    PA_OK
}

/// `paL_unref(st, ref)`：释放注册表里的键（**1 起**；与 `paL_ref` 的返回值对应）。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_unref(state: *mut pa_state, reference: i32) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    if reference < 1 || reference as usize > state_ref.registry.len() {
        return PA_ERR_INVALID;
    }
    let slot = &mut state_ref.registry[reference as usize - 1];
    match slot.take() {
        Some(object) => {
            // SAFETY: 注册表持有这份引用。
            unsafe { state_ref.instance.release_object(object.as_ptr()) };
            PA_OK
        }
        // 幂等：空槽再释放什么也不做（**不是**去 decref 单例）
        None => PA_OK,
    }
}

/// `paL_traceback(st, msg)`：附加上下文信息。
///
/// 本层**尚无 traceback 对象**（`OM-28` 未接线）⇒ 目前只把信息并入 `pa_errmsg`，不伪造结构。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_traceback(state: *mut pa_state, msg: *const c_char) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    // SAFETY: 调用方保证 msg 是 NUL 结尾或 NULL。
    let text = unsafe { crate::host::read_c_string(msg, 4096) }
        .unwrap_or_else(|| "（无附加信息）".to_owned());
    let merged = match state_ref.message_text() {
        Some(existing) => format!("{existing}\n{text}"),
        None => text,
    };
    set_message(state_ref, &merged);
    PA_OK
}

/// `paL_where(st, level, out, len)`：当前位置串——**要 traceback**（`OM-28`）⇒ 未提供。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_where(
    _state: *mut pa_state,
    _level: i32,
    _out: *mut *const c_char,
    _len: *mut usize,
) -> i32 {
    PA_ERR_NOTIMPLEMENTED
}

/// `paL_error(st, msg)`：抛错（信息进 `pa_errmsg`）。
///
/// **形态说明**：C 变参格式化在稳定版 Rust 里做不到（要 `vsnprintf`），故这里只收**拼好的**
/// 消息；`format` 那一路等 `AB-44` 的版本策略裁定后再定（见 `README.md`）。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_error(state: *mut pa_state, msg: *const c_char) -> i32 {
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    // SAFETY: 调用方保证 msg 是 NUL 结尾或 NULL。
    let text = unsafe { crate::host::read_c_string(msg, 4096) }
        .unwrap_or_else(|| "宿主报告错误".to_owned());
    set_message(state_ref, &text);
    PA_ERR_RUNTIME
}

/// `paL_execresult(st, status)`：状态码 → 统一收尾。
///
/// 约定（本层定，写在 `pa.h`）：`PA_OK` 压入 `True`（+1）；其余把状态码原样返回并把
/// `pa_errmsg` 留给宿主读。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_execresult(state: *mut pa_state, status: i32) -> i32 {
    if status != PA_OK {
        return status;
    }
    let Some(state_ref) = state_of(state) else {
        return PA_ERR_INVALID;
    };
    let flag = state_ref.instance.singletons().boolean(true);
    // SAFETY: 单例由实例持有；栈要自己那份。
    unsafe { state_ref.instance.incref_object(flag.as_ptr()) };
    state_ref.stack.push_owned(flag)
}

/// `paL_requiref(st, name, openf, glb)`：取或加载模块——**要模块系统**（`IM-`）⇒ 未提供。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_requiref(
    _state: *mut pa_state,
    _name: *const c_char,
    _openf: *const c_void,
    _global: i32,
) -> i32 {
    PA_ERR_NOTIMPLEMENTED
}

/// `paL_setfuncs(st, regs, n)`：批量注册（`n < 0` 表示以 `name == NULL` 结尾）。
///
/// # Safety
///
/// `regs` 指向 `n`（或 NUL 结尾）个 [`pa_reg`]；每项的名字与签名有效。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_setfuncs(
    state: *mut pa_state,
    regs: *const pa_reg,
    count: i32,
) -> i32 {
    if regs.is_null() {
        return PA_ERR_INVALID;
    }
    let limit = if count < 0 { i32::MAX } else { count };
    let mut index = 0i32;
    while index < limit {
        // SAFETY: 调用方保证 regs 至少有 index+1 项（或以 NULL 结尾）。
        let entry = unsafe { &*regs.add(index as usize) };
        if entry.name.is_null() {
            break;
        }
        // SAFETY: 契约同 `pa_register`。
        let status = unsafe { crate::pa_register(state, entry.name, entry.function, entry.sig) };
        if status != PA_OK {
            return status;
        }
        index += 1;
    }
    PA_OK
}

/// `paL_openlibs(st)`：打开标准库——**要标准库**（`P3-14`）⇒ 未提供。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_openlibs(_state: *mut pa_state) -> i32 {
    PA_ERR_NOTIMPLEMENTED
}

/// `paL_dostring(st, s, mode)`：执行字符串——**要编译器**（`P3-12`）⇒ 未提供。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_dostring(
    _state: *mut pa_state,
    _source: *const c_char,
    _mode: i32,
) -> i32 {
    PA_ERR_NOTIMPLEMENTED
}

/// `paL_dofile(st, path, mode)`：执行文件——**要编译器**（`P3-12`）⇒ 未提供（且本层不碰文件系统）。
///
/// # Safety
///
/// 同 [`paL_checkinteger`]。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn paL_dofile(
    _state: *mut pa_state,
    _path: *const c_char,
    _mode: i32,
) -> i32 {
    PA_ERR_NOTIMPLEMENTED
}
