//! `binascii` 模块（`CM-4` 合约见 `docs/SPEC-c-modules.md`）。
//!
//! 要点（`DESIGN.md` :389「C 模块的 Python 层行为**必须做**」✓）：
//! - `Error` 是**真的** `ValueError` 子类 ✓（`new_type` ＋ `register_bases` ＋ `mark_has_instance_dict` ✓），
//!   非法输入**抛它** ✓（`call_value(class, [msg])` ＋ `ExecError::Raised` ✓）—— 不拿 `ValueError` 冒充 ✗；
//! - 本轮先落**实测用量最大的那几个** ✓：`hexlify`／`b2a_hex`／`unhexlify`／`a2b_hex`／
//!   `b2a_base64`／`a2b_base64`／`crc32` ✓；`b2a_uu`／`a2b_uu`（上游 4 处 ✓）**尚未接** ✗（如实 ✓）。
//!
//! 本模块**不碰**平台（`CX-4`）。

use core::cell::Cell;
use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`binascii`）。
pub const NAME: &str = "binascii";

/// 模块的 `__doc__`（照参照）。
pub const DOC: &str = "Conversion between binary data and ASCII";

fn native_fn(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    instance
        .alloc(pyawa_core::BuiltinFunctionObject::new(
            ty,
            Box::leak(name.to_string().into_boxed_str()),
            Cell::new(handler),
        ))
        .into_raw()
        .cast::<Header>()
}

/// 抛 `binascii.Error`（**用那个类** ✓，不是 `ValueError` ✗）。
/// **取模块里那个 `Error` 类**（第 314 轮 ✓）：`build` 可能被调用多次 ✗ ⇒ 每次都会新建一个 `Error` ✗
/// （实测 `type(e) is binascii.Error` ⇒ **False** ✗）⇒ 抛错时**从 `sys.modules` 里现取** ✓，
/// 保证与用户 `except binascii.Error` 看到的是**同一个类** ✓（一处真相 ✓）。
fn error_class(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    let namespace = pyawa_core::mounted_instance_dict(instance, module)?;
    instance.dict_get(namespace, "Error")
}

fn raise_error(instance: &Instance, class: NonNull<Header>, message: &str) -> ExecError {
    let text = instance.new_str(message);
    match pyawa_core::call_value(instance, class, &[text], &[]) {
        Ok(exception) => {
            // **消息要真的进实例** ✓：通用分配那条路不跑 `__init__` ✗（实测打印成
            // `<Error object at 0x…>` ✗）⇒ 直接写 `args` ✓（参照的 `str(exc)` 就是它 ✓）。
            // 属性面：`args`（参照口径 ✓）＋ `message`（core 的通用 `str` 读它 ✓）
            let text_value = instance.new_str(message);
            let _ = pyawa_core::executor::protocol::instance_attribute_set(
                instance,
                exception,
                "message",
                instance.retain(text_value),
                0,
            );
            let arguments = instance.new_tuple(vec![text_value]);
            let _ = pyawa_core::executor::protocol::instance_attribute_set(
                instance,
                exception,
                "args",
                arguments,
                0,
            );
            ExecError::Raised { exception }
        }
        Err(error) => error,
    }
}

/// 把实参读成字节：`bytes` 直接读 ✓，`str` 按 ASCII 读 ✓（参照两种都收 ✓）。
fn argument_bytes(instance: &Instance, value: NonNull<Header>) -> Option<Vec<u8>> {
    if let Some(bytes) = instance.bytes_value(value) {
        return Some(bytes.to_vec());
    }
    instance
        .text_of(value)
        .map(|text| text.as_bytes().to_vec())
}

fn hexlify_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let some = |message: &str| instance.raise_builtin_error("TypeError", message);
    let value = args.first().copied().ok_or_else(|| {
        some("hexlify() takes exactly one argument (0 given)")
    })?;
    let Some(bytes) = instance.bytes_value(value).map(<[u8]>::to_vec) else {
        return Err(some("argument should be a bytes-like object or ASCII string"));
    };
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(b"0123456789abcdef"[(byte >> 4) as usize]);
        out.push(b"0123456789abcdef"[(byte & 0x0f) as usize]);
    }
    Ok(instance.new_bytes(&out))
}

fn unhexlify_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let class = error_class(instance);
    let missing = || instance.raise_builtin_error("TypeError", "unhexlify() takes exactly one argument (0 given)");
    let value = args.first().copied().ok_or_else(missing)?;
    let Some(input) = argument_bytes(instance, value) else {
        return Err(instance.raise_builtin_error("TypeError", "argument should be a bytes-like object or ASCII string"));
    };
    let digits: Vec<u8> = input
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let bad = |instance: &Instance| match class {
        Some(class) => raise_error(instance, class, "Non-hexadecimal digit found"),
        None => instance.raise_builtin_error("ValueError", "Non-hexadecimal digit found"),
    };
    let value_of = |byte: u8| -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    };
    if digits.len() % 2 != 0 {
        return Err(bad(instance));
    }
    let mut out = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks(2) {
        let (Some(high), Some(low)) = (value_of(pair[0]), value_of(pair[1])) else {
            return Err(bad(instance));
        };
        out.push((high << 4) | low);
    }
    Ok(instance.new_bytes(&out))
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b2a_base64_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = args.first().copied().ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "b2a_base64() takes exactly one argument (0 given)")
    })?;
    let Some(bytes) = instance.bytes_value(value).map(<[u8]>::to_vec) else {
        return Err(instance.raise_builtin_error("TypeError", "argument should be a bytes-like object or ASCII string"));
    };
    // `newline` 关键字（参照默认 `True` ⇒ 末尾补一个 `\n` ✓）。
    let mut newline = true;
    for (key, flag) in kwargs {
        if instance.text_of(*key) == Some("newline") {
            newline = Some(*flag) != Some(instance.singletons().boolean(false));
        }
    }
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len().div_ceil(3) * 4 + 1);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from);
        let b2 = chunk.get(2).copied().map(u32::from);
        out.push(BASE64_ALPHABET[(b0 >> 2) as usize]);
        out.push(BASE64_ALPHABET[(((b0 & 0x03) << 4) | (b1.unwrap_or(0) >> 4)) as usize]);
        out.push(match b1 {
            Some(b1) => BASE64_ALPHABET[(((b1 & 0x0f) << 2) | (b2.unwrap_or(0) >> 6)) as usize],
            None => b'=',
        });
        out.push(match b2 {
            Some(b2) => BASE64_ALPHABET[(b2 & 0x3f) as usize],
            None => b'=',
        });
    }
    if newline {
        out.push(b'\n');
    }
    Ok(instance.new_bytes(&out))
}

fn a2b_base64_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let class = error_class(instance);
    let value = args.first().copied().ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "a2b_base64() takes exactly one argument (0 given)")
    })?;
    let Some(input) = argument_bytes(instance, value) else {
        return Err(instance.raise_builtin_error("TypeError", "argument should be a bytes-like object or ASCII string"));
    };
    let bad = |instance: &Instance| match class {
        Some(class) => raise_error(instance, class, "Invalid base64-encoded string"),
        None => instance.raise_builtin_error("ValueError", "Invalid base64-encoded string"),
    };
    // 参照的 `a2b_base64` 是**宽松**的 ✓：跳过空白与非法字符（`=` 视为结束 ✓）。
    let mut sextets: Vec<u32> = Vec::with_capacity(input.len());
    for byte in input {
        match byte {
            b'A'..=b'Z' => sextets.push(u32::from(byte - b'A')),
            b'a'..=b'z' => sextets.push(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => sextets.push(u32::from(byte - b'0') + 52),
            b'+' => sextets.push(62),
            b'/' => sextets.push(63),
            b'=' => break,
            _ => {}
        }
    }
    let mut out: Vec<u8> = Vec::with_capacity(sextets.len() * 3 / 4);
    for chunk in sextets.chunks(4) {
        if chunk.len() == 1 {
            return Err(bad(instance));
        }
        let b0 = chunk[0];
        let b1 = chunk[1];
        out.push(((b0 << 2) | (b1 >> 4)) as u8);
        if let Some(b2) = chunk.get(2) {
            out.push((((b1 & 0x0f) << 4) | (b2 >> 2)) as u8);
            if let Some(b3) = chunk.get(3) {
                out.push((((b2 & 0x03) << 6) | b3) as u8);
            }
        }
    }
    Ok(instance.new_bytes(&out))
}

fn crc32_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 参照：`crc32(data[, value])` ⇒ 返回无符号 32 位 ✓。
    let value = args.first().copied().ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "crc32() takes at least one argument (0 given)")
    })?;
    let Some(bytes) = instance.bytes_value(value).map(<[u8]>::to_vec) else {
        return Err(instance.raise_builtin_error("TypeError", "argument should be a bytes-like object or ASCII string"));
    };
    let start = args
        .get(1)
        .and_then(|object| instance.int_value(*object))
        .unwrap_or(0);
    let mut crc = ((start as u64 & 0xffff_ffff) as u32) ^ 0xffff_ffff;
    for byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    Ok(instance.new_int(i64::from(crc ^ 0xffff_ffff)))
}

/// 建 `binascii` 的**命名空间 dict**（`lib.rs` 会包成模块 ✓，与 `errno_module` 同形 ✓）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    if let Some(class) = instance.new_exception_subclass("binascii.Error", "ValueError") {
        // **类的名字照参照** ✓：`<class 'binascii.Error'>`（不是 `<class 'Error'>` ✗）。
        let _ = pyawa_core::executor::protocol::instance_attribute_set(
            instance,
            class,
            "__module__",
            instance.new_str(NAME),
            0,
        );
        let _ = pyawa_core::executor::protocol::instance_attribute_set(
            instance,
            class,
            "__qualname__",
            instance.new_str("Error"),
            0,
        );
        instance.dict_set(namespace, "Error", class);
    }
    for (name, handler) in [
        ("hexlify", hexlify_native as NativeFn),
        ("b2a_hex", hexlify_native as NativeFn),
        ("unhexlify", unhexlify_native as NativeFn),
        ("a2b_hex", unhexlify_native as NativeFn),
        ("b2a_base64", b2a_base64_native as NativeFn),
        ("a2b_base64", a2b_base64_native as NativeFn),
        ("crc32", crc32_native as NativeFn),
    ] {
        instance.dict_set(namespace, name, native_fn(instance, name, handler));
    }
    namespace
}
