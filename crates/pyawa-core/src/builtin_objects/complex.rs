//! `complex` 族：载荷、表示、属性与方法面（**`P3-34`**，task-30）。
//!
//! 参照 3.14.4 逐项实测（判据见 `tests/complex.rs` 的期望值；文案／形状都从实测来）：
//!
//! - `repr`／`str` **同一口径**：纯虚（`real` 是**正零**）⇒ `1j`／`-0j`；否则 `(1+2j)`；
//!   两个零的**符号位**都保留（`-1j` ⇒ `(-0-1j)`、`complex(-0.0,-0.0)` ⇒ `(-0-0j)`）✓
//! - `.real`／`.imag` 是 **float**；`conjugate()` 给新复数；`__complex__()` 返回自身 ✓
//! - 真值：`real != 0.0 or imag != 0.0`（`nan` 为真 ✓）；`abs()` 由 `executor::unary_public`
//!   给 `hypot(real, imag)`（**float**）✓
//! - 算术 `+ - * / **` 与 int／float／bool 混合在 `executor::arithmetic` 里（`TS-40` 的数值塔 ✓）；
//!   `//`／`%`／`divmod` 一律 `TypeError`（`unsupported_operand` 的既有文案 ✓）
//! - `hash()`：`hash(real) + 1000003 * hash(imag)`（CPython 的 `_PyHASH_IMAG` ✓；`imag == 0`
//!   ⇒ 与 `hash(real)` 相同）——算法在 [`crate::executor::hash_complex_parts`] ✓（stdlib 的
//!   `hash()` 内置入口调它 ✓）
//!
//! **如实说** ✗：`complex` 的**字符串解析**（`complex("1+2j")`）归 stdlib 的 `__new__`（task-31）；
//! 本文件的构造槽只收**数字面**（0／1／2 实参，含 `__complex__` 协议 ✓）。

use super::prelude::*;
use super::*;

py_object! {
    /// 复数实例：`real` ＋ `imag`（都是 `f64`；`inf`／`nan` 照旧）。
    ///
    /// **`TS-43`**：布局自选、不进 ABI（与 [`FloatObject`] 同一手法 ✓）。
    pub struct ComplexObject {
        real: f64,
        imag: f64,
    }
}

impl ComplexObject {
    /// 实部（**借用**）。
    pub fn real(&self) -> f64 {
        self.real
    }

    /// 虚部（**借用**）。
    pub fn imag(&self) -> f64 {
        self.imag
    }

    /// 类型槽表：构造／表示／属性面（`OM-11`）。
    pub fn slots() -> Slots {
        Slots::new(Self::dealloc)
            .with_new(complex_new)
            .with_repr(complex_repr)
            .with_str(complex_repr)
            .with_getattr(complex_getattr)
    }
}

/// **数字实参 → `f64`**（构造槽用）：`float` 原值；`int`／`bool` 折成 `f64`（**溢出**照参照报
/// `OverflowError: int too large to convert to float` ✓）；其余 `None`。
///
/// `complex` 实参**不**走这里（2 参形态另有"复数运算"那条历史口径 ✓，见 [`complex_new`]）。
fn number_to_f64(instance: &Instance, object: NonNull<Header>) -> Result<Option<f64>, ExecError> {
    if let Some(value) = instance.float_value(object) {
        return Ok(Some(value));
    }
    if let Some(value) = instance.int_of(object) {
        let number = value.to_bigint().to_f64();
        if number.is_infinite() {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "int too large to convert to float",
            ));
        }
        return Ok(Some(number));
    }
    Ok(None)
}

/// **`complex(...)` 的构造槽**（`OM-11` 的 `new`）：**数字面最小档** ✓。
///
/// 实测口径（3.14.4）：
/// - 0 参 ⇒ `0j`；1 参数字 ⇒ `(x+0j)`；1 参 `complex` ⇒ 原值；1 参带 `__complex__` 的对象
///   ⇒ 调它（结果必须是 `complex`，否则 `TypeError: __complex__ returned non-complex (type X)` ✓）
/// - 2 参：两边都必须是**实数**，但**任一为 `complex`** 时照参照的**历史口径**按复数运算收：
///   `real + imag*1j`（实测 `complex(1+2j, 3)` ⇒ `(1+5j)`、`complex(1, 1+2j)` ⇒ `(-1+1j)` ✓）
/// - ≥3 参 ⇒ `TypeError: complex() takes at most 2 arguments (N given)` ✓
/// - 非数字（含**字符串**）：字符串解析归 stdlib 的 `__new__`（task-31）⇒ 这里如实报
///   `TypeError: complex() argument must be a string or a number, not 'X'` ✓
pub unsafe fn complex_new(
    class: NonNull<crate::TypeObject>,
    args: &[NonNull<Header>],
    instance: &Instance,
) -> Result<NonNull<Header>, ExecError> {
    let allocate = |real: f64, imag: f64| -> NonNull<Header> {
        instance
            .alloc(ComplexObject::new(class, real, imag))
            .into_raw()
            .cast::<Header>()
    };
    match args {
        [] => Ok(allocate(0.0, 0.0)),
        [only] => {
            // 参照：1 参且**恰是** `complex` ⇒ 原对象（`complex(z) is z` 为真 ✓）。
            if instance.type_of(*only) == instance.type_named("complex").expect("complex 已登记") {
                return Ok(instance.retain(*only));
            }
            if let Some(number) = number_to_f64(instance, *only)? {
                return Ok(allocate(number, 0.0));
            }
            // **`__complex__` 协议** ✓（参照先查协议；字符串在 stdlib 的 `__new__` 里解析 ✓）。
            let protocol = crate::executor::attribute_optional(instance, *only, "__complex__")?;
            if let Some(method) = protocol {
                let outcome = crate::executor::call_value(instance, method, &[], &[]);
                instance.release(method);
                let result = outcome?;
                if let Some((real, imag)) = instance.complex_value(result) {
                    instance.release(result);
                    return Ok(allocate(real, imag));
                }
                let name = instance.type_name(instance.type_of(result));
                instance.release(result);
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("__complex__ returned non-complex (type {name})"),
                ));
            }
            Err(instance.raise_builtin_error(
                "TypeError",
                &format!(
                    "complex() argument must be a string or a number, not {}",
                    instance.type_name(instance.type_of(*only))
                ),
            ))
        }
        [real_arg, imag_arg] => {
            let real_complex = instance.complex_value(*real_arg);
            let imag_complex = instance.complex_value(*imag_arg);
            // **复数实参的历史口径** ✓：`a + b*1j`（两边都按复数收 ✓；DeprecationWarning 面
            // 本层没有 warning 机制 ⇒ 如实不报 ✓）。
            if real_complex.is_some() || imag_complex.is_some() {
                let a = match real_complex {
                    Some(parts) => parts,
                    None => (
                        number_to_f64(instance, *real_arg)?.ok_or_else(|| {
                            instance.raise_builtin_error(
                                "TypeError",
                                &format!(
                                    "complex() argument 'real' must be a real number, not {}",
                                    instance.type_name(instance.type_of(*real_arg))
                                ),
                            )
                        })?,
                        0.0,
                    ),
                };
                let b = match imag_complex {
                    Some(parts) => parts,
                    None => (
                        number_to_f64(instance, *imag_arg)?.ok_or_else(|| {
                            instance.raise_builtin_error(
                                "TypeError",
                                &format!(
                                    "complex() argument 'imag' must be a real number, not {}",
                                    instance.type_name(instance.type_of(*imag_arg))
                                ),
                            )
                        })?,
                        0.0,
                    ),
                };
                // (a.re + a.im i) + (b.re + b.im i) * i
                return Ok(allocate(a.0 - b.1, a.1 + b.0));
            }
            let real = number_to_f64(instance, *real_arg)?.ok_or_else(|| {
                instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "complex() argument 'real' must be a real number, not {}",
                        instance.type_name(instance.type_of(*real_arg))
                    ),
                )
            })?;
            let imag = number_to_f64(instance, *imag_arg)?.ok_or_else(|| {
                instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "complex() argument 'imag' must be a real number, not {}",
                        instance.type_name(instance.type_of(*imag_arg))
                    ),
                )
            })?;
            Ok(allocate(real, imag))
        }
        _ => Err(instance.raise_builtin_error(
            "TypeError",
            &format!("complex() takes at most 2 arguments ({} given)", args.len()),
        )),
    }
}

/// **一个分量的文本**（参照 `PyOS_double_to_string(…, 'r', 0, 0, NULL)` 的等价物 ✓）：
/// 与浮点 repr 同口径，但**不带** `.0`（`complex(1.0)` ⇒ `(1+0j)` 里的 `1` ✓）。
pub(crate) fn complex_part_text(value: f64) -> String {
    let text = crate::builtin::float::float_repr_text(value);
    match text.strip_suffix(".0") {
        Some(trimmed) => trimmed.to_owned(),
        None => text,
    }
}

/// **复数的 `repr`／`str` 文本**（实测形状见模块文档 ✓）。
pub(crate) fn complex_repr_text(real: f64, imag: f64) -> String {
    let imag_negative = imag.is_sign_negative();
    let imag_text = complex_part_text(imag.abs());
    // 实部是**正零** ⇒ 纯虚形态（`1j`／`-0j` ✓）；负零要保留 ⇒ 落进一般形态 ✓。
    if real == 0.0 && !real.is_sign_negative() {
        let sign = if imag_negative { "-" } else { "" };
        return format!("{sign}{imag_text}j");
    }
    let real_text = complex_part_text(real);
    let sign = if imag_negative { "-" } else { "+" };
    format!("({real_text}{sign}{imag_text}j)")
}

/// `complex` 的 `repr`（`str` 同槽 ✓）。
pub unsafe fn complex_repr(ptr: *mut Header, _instance: &Instance) -> Result<String, ExecError> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ComplexObject>() };
    Ok(complex_repr_text(object.real, object.imag))
}

/// 把 `handler` 绑到 `self` 上交出（与 `dict_getattr`／`float_getattr` 同一手法 ✓）。
fn bind_native(
    instance: &Instance,
    ptr: *mut Header,
    handler: NativeFn,
) -> NonNull<Header> {
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("引导期已登记");
    let native = instance.alloc(BuiltinFunctionObject::new(method_type, "complex", Cell::new(handler)));
    let native_raw = native.into_raw().cast::<Header>();
    // SAFETY: 方法对象要自己那份 self（`OM-16`）。
    unsafe { instance.incref_object(ptr) };
    // SAFETY: ptr 由槽位契约保证是本类型的存活对象。
    let owner = unsafe { NonNull::new_unchecked(ptr) };
    let bound = instance.alloc(MethodObject::new(
        instance.type_named("method").expect("method 已登记"),
        native_raw,
        owner,
    ));
    bound.into_raw().cast::<Header>()
}

/// **`complex` 的属性面**（`OM-11` 的 `getattr` 槽）：`.real`／`.imag`（`float` ✓）、
/// `conjugate()`、`__complex__()`、`__hash__()`。
///
/// **未接线**：`complex.real`（在**类型对象**上读描述符）——本层没有 getset 描述符面；
/// 实例面与 `attribute_optional_of` 都经这里 ✓。
pub unsafe fn complex_getattr(
    ptr: *mut Header,
    name: &str,
    instance: &Instance,
) -> Option<NonNull<Header>> {
    // SAFETY: 调用方保证 ptr 指向本类型的存活对象。
    let object = unsafe { &*ptr.cast::<ComplexObject>() };
    let handler: NativeFn = match name {
        "real" => return Some(instance.new_float(object.real)),
        "imag" => return Some(instance.new_float(object.imag)),
        "conjugate" => complex_conjugate_native,
        "__complex__" => complex_dunder_native,
        "__hash__" => complex_hash_native,
        "__abs__" => complex_abs_native,
        "__bool__" => complex_bool_native,
        "__neg__" => complex_neg_native,
        "__pos__" => complex_pos_native,
        _ => return None,
    };
    Some(bind_native(instance, ptr, handler))
}

/// 这个 native 的 `self`（绑定形态取 `bound`，**类型级**调用取第一个位置实参 ✓）。
fn native_self(
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
) -> Result<(NonNull<Header>, bool), ExecError> {
    if let Some(this) = bound {
        return Ok((this, true));
    }
    match args.first() {
        Some(this) => Ok((*this, false)),
        None => Err(ExecError::Unsupported {
            opcode: 0,
            what: "complex 方法缺少 self",
        }),
    }
}

/// `complex.conjugate()` ✓：`(a-bi)`（新对象 ✓）。
pub unsafe fn complex_conjugate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    let Some((real, imag)) = instance.complex_value(this) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "'conjugate' 只接 complex",
        ));
    };
    Ok(instance.new_complex(real, -imag))
}

/// `complex.__complex__()` ✓：返回**自身**（同一对象 ✓；参照实测 `(1+2j).__complex__() is`
/// 那个对象 —— 值语义返回自身即可，参照对精确 `complex` 直接给 `self` ✓）。
pub unsafe fn complex_dunder_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    if instance.complex_value(this).is_none() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "descriptor '__complex__' for 'complex' objects doesn't apply to a '{}' object",
                instance.type_name(instance.type_of(this))
            ),
        ));
    }
    Ok(instance.retain(this))
}

/// `complex.__hash__()` ✓：算法在 [`crate::executor::hash_complex_parts`]（与 `hash()` 内置面
/// 同一处真相 ✓）。
pub unsafe fn complex_hash_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    let Some((real, imag)) = instance.complex_value(this) else {
        return Err(instance.raise_builtin_error("TypeError", "'__hash__' 只接 complex"));
    };
    Ok(instance.new_int(crate::executor::hash_complex_parts(real, imag)))
}

/// `complex.__abs__()` ✓：模（**float** ✓，`hypot(real, imag)` ✓）。
pub unsafe fn complex_abs_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    let Some((real, imag)) = instance.complex_value(this) else {
        return Err(instance.raise_builtin_error("TypeError", "'__abs__' 只接 complex"));
    };
    Ok(instance.new_float(real.hypot(imag)))
}

/// `complex.__bool__()` ✓：`real != 0.0 || imag != 0.0` ✓。
pub unsafe fn complex_bool_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    let Some((real, imag)) = instance.complex_value(this) else {
        return Err(instance.raise_builtin_error("TypeError", "'__bool__' 只接 complex"));
    };
    Ok(instance.new_bool(real != 0.0 || imag != 0.0))
}

/// `complex.__neg__()` ✓：`-z` ✓。
pub unsafe fn complex_neg_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    let Some((real, imag)) = instance.complex_value(this) else {
        return Err(instance.raise_builtin_error("TypeError", "'__neg__' 只接 complex"));
    };
    Ok(instance.new_complex(-real, -imag))
}

/// `complex.__pos__()` ✓：`+z` ✓。
pub unsafe fn complex_pos_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, _) = native_self(bound, args)?;
    let Some((real, imag)) = instance.complex_value(this) else {
        return Err(instance.raise_builtin_error("TypeError", "'__pos__' 只接 complex"));
    };
    Ok(instance.new_complex(real, imag))
}
