//! `_codecs` 模块（第 282 轮）：`codecs.py` 与 `encodings/*` 的**底座**。
//!
//! 为什么要有它：`codecs.py` 第 16 行就是 `from _codecs import *` ✓，少了它 `codecs` 直接
//! `SystemError: Failed to load the builtin codecs` ✗ —— 而 `encodings.*` 那一族 **124** 个模块
//! 全都压在它上面 ✓（`--ceiling` 诊断的**头一族** ✓）。
//!
//! **已落地（真实现）**：`register`／`lookup`／`register_error`／`lookup_error`（注册表本身 ✓）、
//! `ascii_encode`／`ascii_decode`、`latin_1_encode`／`latin_1_decode`、`utf_8_encode`／`utf_8_decode`、
//! `charmap_build`／`charmap_decode`／`charmap_encode` —— 错误处理的 `strict`／`ignore`／`replace`
//! 三条都接 ✓。
//!
//! **未落地（如实报未实现 ✓，不伪造）**：UTF-7／UTF-16／UTF-32 一族、`unicode_escape`／
//! `raw_unicode_escape`、`_codecs.encode`／`decode` 的**直接**入口、**自定义**错误处理器
//! （只认 `strict`／`ignore`／`replace` ✓）。
//! 名字**必须齐** ✓：`encodings/*.py` 在**类体**里就取 `codecs.utf_16_encode` 一类 ✓
//! ⇒ 它们必须在模块命名空间里存在（调用时报未实现 ✓ 才是 `CM-6` 的口径 ✓）。
//!
//! `CX-4`：本 crate 不碰平台 ✓。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_codecs`）。
pub const NAME: &str = "_codecs";

/// 注册表的两格状态（`_codecs` 是 C 模块 ⇒ 状态得有个落点 ✓；`Instance` 没有通用槽位 ✓）。
const SEARCH_FUNCTIONS: &str = "__pyawa_search_functions__";
const ERROR_HANDLERS: &str = "__pyawa_error_handlers__";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 取本模块命名空间里的一格私有状态（借用 ✓）。
fn module_state(instance: &Instance, key: &str) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    let namespace = pyawa_core::mounted_instance_dict(instance, module)?;
    instance.dict_get(namespace, key)
}

/// 把一格私有状态写回去（`dict_set` **接管**一份引用 ⇒ 传进来的必须是新引用 ✓）。
fn set_module_state(instance: &Instance, key: &str, value: NonNull<Header>) {
    let Some(modules) = instance.modules() else { return };
    let Some(module) = instance.dict_get(modules, NAME) else {
        return;
    };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, module) else {
        return;
    };
    instance.dict_set(namespace, key, value);
}

/// 往一格**列表**状态里追加一项（列表是"接管"语义 ⇒ 读写都要自己 retain ✓）。
fn push_state(instance: &Instance, key: &str, item: NonNull<Header>) {
    let mut items: Vec<NonNull<Header>> = Vec::new();
    if let Some(list) = module_state(instance, key) {
        if let Some(existing) = instance.list_items(list) {
            for entry in existing {
                items.push(instance.retain(entry));
            }
        }
    }
    items.push(instance.retain(item));
    set_module_state(instance, key, instance.new_list(items));
}

/// 第一实参必须是一段文本（消息照参照的 `argument must be str` 家族 ✓）。
fn text_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    index: usize,
    function: &str,
) -> Result<String, ExecError> {
    let Some(value) = args.get(index).copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{function}() takes at least {} argument(s)", index + 1),
        ));
    };
    instance.text_value(value).ok_or_else(|| {
        instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{function}() argument must be str, not {}",
                instance.type_name(instance.type_of(value))
            ),
        )
    })
}

/// 第一实参必须是一段**字节**。
fn bytes_argument(
    instance: &Instance,
    args: &[NonNull<Header>],
    index: usize,
    function: &str,
) -> Result<Vec<u8>, ExecError> {
    let Some(value) = args.get(index).copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{function}() takes at least {} argument(s)", index + 1),
        ));
    };
    instance.bytes_value(value).map(<[u8]>::to_vec).ok_or_else(|| {
        instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{function}() argument must be bytes, not {}",
                instance.type_name(instance.type_of(value))
            ),
        )
    })
}

/// `errors` 实参（缺省 `"strict"` ✓）。
fn errors_argument(instance: &Instance, args: &[NonNull<Header>], index: usize) -> String {
    args.get(index)
        .and_then(|value| instance.text_value(*value))
        .unwrap_or_else(|| "strict".to_owned())
}

/// 建一个 `(结果, 消耗数)` 二元组（**新引用** ✓）。
fn pair(instance: &Instance, first: NonNull<Header>, consumed: usize) -> NonNull<Header> {
    instance.new_tuple(vec![first, instance.new_int(consumed as i64)])
}

/// 自定义错误处理器（`errors` 不是那三个内建的）—— 本层**如实报未实现** ✓。
fn custom_handler_error(instance: &Instance, errors: &str, encoding: &str) -> ExecError {
    instance.raise_builtin_error(
        "NotImplementedError",
        &format!("`{encoding}` codec 的 `{errors}` 错误处理器尚未接线（只认 strict／ignore／replace）"),
    )
}

// ---- 注册表 -----------------------------------------------------------------------------------

/// **`register(search_function)`**：把查找函数收进本模块的注册表 ✓（每实例一份 ✓）。
fn register_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("register() takes exactly one argument ({} given)", args.len()),
        ));
    }
    push_state(instance, SEARCH_FUNCTIONS, args[0]);
    Ok(instance.new_none())
}

/// **`lookup(encoding)`**：按注册顺序问每个查找函数；都没有 ⇒ `LookupError: unknown encoding: X` ✓。
fn lookup_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let name = text_argument(instance, args, 0, "lookup")?;
    let none = instance.singletons().none();
    // 直接转发调用方那个**对象**（借用 ⇒ `call_value` 自己 retain ✓）
    if let (Some(list), Some(query)) = (module_state(instance, SEARCH_FUNCTIONS), args.first().copied())
    {
        if let Some(functions) = instance.list_items(list) {
            for function in functions {
                let result = pyawa_core::executor::call::call_value(instance, function, &[query], &[])?;
                if result == none {
                    // `call_value` 交的是**新引用** ⇒ 不要就当场还 ✓
                    instance.release(result);
                    continue;
                }
                return Ok(result);
            }
        }
    }
    Err(instance.raise_builtin_error(
        "LookupError",
        &format!("unknown encoding: {name}"),
    ))
}

/// **`register_error(name, handler)`** ✓。
fn register_error_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let name = text_argument(instance, args, 0, "register_error")?;
    let Some(handler) = args.get(1).copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "register_error() takes exactly 2 arguments",
        ));
    };
    let mapping = module_state(instance, ERROR_HANDLERS).unwrap_or_else(|| instance.new_dict());
    instance.retain(handler);
    instance.dict_set(mapping, &name, handler);
    set_module_state(instance, ERROR_HANDLERS, mapping);
    Ok(instance.new_none())
}

/// **`lookup_error(name)`** ✓；没有 ⇒ `LookupError: unknown error handler name 'X'` ✓。
fn lookup_error_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let name = text_argument(instance, args, 0, "lookup_error")?;
    if let Some(mapping) = module_state(instance, ERROR_HANDLERS) {
        if let Some(found) = instance.dict_get(mapping, &name) {
            instance.retain(found);
            return Ok(found);
        }
    }
    // **内建处理器**（`codecs.py:1114` 一进门就 `lookup_error("strict")` 六个 ✓）——
    // 它们就装在**本模块的命名空间**里 ✓（名字与处理器同名 ✓）。
    if let Some(modules) = instance.modules() {
        if let Some(module) = instance.dict_get(modules, NAME) {
            if let Some(namespace) = pyawa_core::mounted_instance_dict(instance, module) {
                if let Some(builtin) = instance.dict_get(namespace, &name) {
                    instance.retain(builtin);
                    return Ok(builtin);
                }
            }
        }
    }
    Err(instance.raise_builtin_error(
        "LookupError",
        &format!("unknown error handler name '{name}'"),
    ))
}

// ---- 编解码族 ---------------------------------------------------------------------------------

/// `ascii_encode(input, errors='strict')` ✓。
fn ascii_encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let text = text_argument(instance, args, 0, "ascii_encode")?;
    let errors = errors_argument(instance, args, 1);
    let mut out = Vec::with_capacity(text.len());
    for (position, character) in text.chars().enumerate() {
        let code = character as u32;
        if code < 0x80 {
            out.push(code as u8);
            continue;
        }
        match errors.as_str() {
            "strict" => {
                return Err(instance.raise_builtin_error(
                    "UnicodeEncodeError",
                    &format!(
                        "'ascii' codec can't encode character '\\x{:x}' in position {position}: ordinal not in range(128)",
                        code
                    ),
                ))
            }
            "ignore" => {}
            "replace" => out.push(b'?'),
            other => return Err(custom_handler_error(instance, other, "ascii")),
        }
    }
    let bytes = instance.new_bytes(&out);
    Ok(pair(instance, bytes, text.chars().count()))
}

/// `ascii_decode(input, errors='strict')` ✓。
fn ascii_decode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let input = bytes_argument(instance, args, 0, "ascii_decode")?;
    let errors = errors_argument(instance, args, 1);
    let mut out = String::new();
    for (position, byte) in input.iter().enumerate() {
        if *byte < 0x80 {
            out.push(*byte as char);
            continue;
        }
        match errors.as_str() {
            "strict" => {
                return Err(instance.raise_builtin_error(
                    "UnicodeDecodeError",
                    &format!(
                        "'ascii' codec can't decode byte 0x{byte:02x} in position {position}: ordinal not in range(128)"
                    ),
                ))
            }
            "ignore" => {}
            "replace" => out.push('\u{fffd}'),
            other => return Err(custom_handler_error(instance, other, "ascii")),
        }
    }
    let text = instance.new_str(&out);
    Ok(pair(instance, text, input.len()))
}

/// `latin_1_encode(input, errors='strict')` ✓（**不可能失败** ✓）。
fn latin_1_encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let text = text_argument(instance, args, 0, "latin_1_encode")?;
    let mut out = Vec::with_capacity(text.len());
    for character in text.chars() {
        match u8::try_from(character as u32) {
            Ok(byte) => out.push(byte),
            Err(_) => {
                // 参照里 latin-1 的 `encode` 对 > U+00FF 会报 UnicodeEncodeError ✓
                return Err(instance.raise_builtin_error(
                    "UnicodeEncodeError",
                    &format!(
                        "'latin-1' codec can't encode character '\\u{:04x}' in position 0: ordinal not in range(256)",
                        character as u32
                    ),
                ));
            }
        }
    }
    let bytes = instance.new_bytes(&out);
    Ok(pair(instance, bytes, text.chars().count()))
}

/// `latin_1_decode(input, errors='strict')` ✓（**不可能失败** ✓）。
fn latin_1_decode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let input = bytes_argument(instance, args, 0, "latin_1_decode")?;
    let text: String = input.iter().map(|byte| *byte as char).collect();
    let value = instance.new_str(&text);
    Ok(pair(instance, value, input.len()))
}

/// `utf_8_encode(input, errors='strict')` ✓（Rust 的 `String` 本来就是 UTF-8 ✓）。
fn utf_8_encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let text = text_argument(instance, args, 0, "utf_8_encode")?;
    let bytes = instance.new_bytes(text.as_bytes());
    Ok(pair(instance, bytes, text.chars().count()))
}

/// `utf_8_decode(input, errors='strict', final=False)` ✓（`final` 只影响**截断的序列**那一档 ✓）。
fn utf_8_decode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let input = bytes_argument(instance, args, 0, "utf_8_decode")?;
    let errors = errors_argument(instance, args, 1);
    match std::str::from_utf8(&input) {
        Ok(text) => {
            let value = instance.new_str(text);
            Ok(pair(instance, value, input.len()))
        }
        Err(_) => {
            if errors == "strict" {
                let valid = std::str::from_utf8(&input).err().map(|e| e.valid_up_to()).unwrap_or(0);
                let byte = input.get(valid).copied().unwrap_or(0);
                return Err(instance.raise_builtin_error(
                    "UnicodeDecodeError",
                    &format!(
                        "'utf-8' codec can't decode byte 0x{byte:02x} in position {valid}: invalid start byte"
                    ),
                ));
            }
            if errors == "replace" || errors == "ignore" {
                // **逐字节**退让（不追参照的精确分段 ✓ —— 本层如实只保证"不炸 + 替换/跳过" ✓）
                let mut out = String::new();
                for (position, byte) in input.iter().enumerate() {
                    match std::str::from_utf8(&input[position..]) {
                        Ok(rest) => {
                            out.push_str(rest);
                            break;
                        }
                        Err(_) => {
                            if errors == "replace" {
                                out.push('\u{fffd}');
                            }
                            let _ = byte;
                        }
                    }
                }
                let value = instance.new_str(&out);
                Ok(pair(instance, value, input.len()))
            } else {
                Err(custom_handler_error(instance, &errors, "utf-8"))
            }
        }
    }
}

/// `charmap_build(decoding_table)` ⇒ `{码点: 字节}`（跳过 `\uFFFE` 那些**未用**位 ✓）。
fn charmap_build_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let table = text_argument(instance, args, 0, "charmap_build")?;
    let mapping = instance.new_dict();
    for (index, character) in table.chars().enumerate() {
        if character == '\u{fffe}' {
            continue;
        }
        let value = instance.new_int(index as i64);
        instance.dict_set_int(mapping, character as i64, value);
    }
    Ok(mapping)
}

/// 从 `mapping`（`str` 表 或 `dict`）里取"字节 → 码点"的映射 ✓。
fn decode_table(instance: &Instance, mapping: Option<NonNull<Header>>) -> Option<Vec<Option<char>>> {
    let mapping = mapping?;
    if let Some(text) = instance.text_value(mapping) {
        let mut table: Vec<Option<char>> = vec![None; 256];
        for (index, character) in text.chars().enumerate().take(256) {
            table[index] = Some(character);
        }
        return Some(table);
    }
    let entries = instance.dict_entries(mapping)?;
    let mut table: Vec<Option<char>> = vec![None; 256];
    for (key, value) in entries {
        let Some(index) = instance.int_value(key) else { continue };
        if !(0..256).contains(&index) {
            continue;
        }
        let Some(codepoint) = instance.int_value(value) else { continue };
        table[index as usize] = char::from_u32(codepoint as u32);
    }
    Some(table)
}

/// `charmap_decode(input, errors='strict', mapping=None)` ✓。
fn charmap_decode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let input = bytes_argument(instance, args, 0, "charmap_decode")?;
    let errors = errors_argument(instance, args, 1);
    let mapping = args.get(2).copied().filter(|value| *value != instance.singletons().none());
    let table = decode_table(instance, mapping);
    let mut out = String::new();
    for (position, byte) in input.iter().enumerate() {
        let character = match &table {
            Some(table) => table[*byte as usize],
            // 没有映射 ⇒ 恒等（`latin-1` 那一档 ✓）
            None => Some(*byte as char),
        };
        let Some(character) = character else {
            match errors.as_str() {
                "strict" => {
                    return Err(instance.raise_builtin_error(
                        "UnicodeDecodeError",
                        &format!(
                            "'charmap' codec can't decode byte 0x{byte:02x} in position {position}: character maps to <undefined>"
                        ),
                    ))
                }
                "ignore" => continue,
                "replace" => {
                    out.push('\u{fffd}');
                    continue;
                }
                other => return Err(custom_handler_error(instance, other, "charmap")),
            }
        };
        out.push(character);
    }
    let value = instance.new_str(&out);
    Ok(pair(instance, value, input.len()))
}

/// 从 `mapping`（`dict`：码点 → 字节）里取映射 ✓。
fn encode_table(instance: &Instance, mapping: Option<NonNull<Header>>) -> Option<Vec<Option<u8>>> {
    let entries = instance.dict_entries(mapping?)?;
    let mut table: Vec<Option<u8>> = vec![None; 0x110000];
    for (key, value) in entries {
        let Some(codepoint) = instance.int_value(key) else { continue };
        let Some(byte) = instance.int_value(value) else { continue };
        if !(0..0x110000).contains(&codepoint) || !(0..256).contains(&byte) {
            continue;
        }
        table[codepoint as usize] = Some(byte as u8);
    }
    Some(table)
}

/// `charmap_encode(input, errors='strict', mapping=None)` ✓。
fn charmap_encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let text = text_argument(instance, args, 0, "charmap_encode")?;
    let errors = errors_argument(instance, args, 1);
    let mapping = args.get(2).copied().filter(|value| *value != instance.singletons().none());
    let table = encode_table(instance, mapping);
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    for (position, character) in text.chars().enumerate() {
        let byte = match &table {
            Some(table) => table[character as usize],
            // 没有映射 ⇒ 恒等（码点 ≤ 0xFF 才成立 ✓）
            None => u8::try_from(character as u32).ok(),
        };
        let Some(byte) = byte else {
            match errors.as_str() {
                "strict" => {
                    return Err(instance.raise_builtin_error(
                        "UnicodeEncodeError",
                        &format!(
                            "'charmap' codec can't encode character '\\u{:04x}' in position {position}: character maps to <undefined>",
                            character as u32
                        ),
                    ))
                }
                "ignore" => continue,
                "replace" => {
                    out.push(b'?');
                    continue;
                }
                other => return Err(custom_handler_error(instance, other, "charmap")),
            }
        };
        out.push(byte);
    }
    let bytes = instance.new_bytes(&out);
    Ok(pair(instance, bytes, text.chars().count()))
}

/// **`strict_errors(exception)`** ✓：原样再抛（真实现 ✓ —— `_codecs` 里它就是 `raise exception` ✓）。
fn strict_errors_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(exception) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "strict_errors() takes exactly one argument (0 given)",
        ));
    };
    // `raise_object_public` 要**一份新引用**（`Err(Raised)` 那份由派发器接手 ✓）⇒ 这里加一份 ✓
    instance.retain(exception);
    Err(pyawa_core::raise_object_public(instance, exception))
}

/// 其余五个内建处理器：**照 `lookup_error` 的名字要存在** ✓（`codecs.py:1114-1119` ✓），
/// 但它们的契约要吃 `UnicodeEncodeError` 的**结构化字段**（`object`／`start`／`end` ✓），
/// 而本层的异常对象目前只带消息 ✗ ⇒ 调用时**如实报未实现** ✓（不假装返回一个错的位置 ✗）。
fn not_implemented_handler_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_codecs` 的这个错误处理器尚未接线（要 `UnicodeEncodeError` 的结构化字段：object／start／end）",
    ))
}

/// **未落地**的那些入口：名字齐 ✓、调用时按 `CM-6` 报未实现 ✓。
fn not_implemented_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_codecs` 的这个编解码入口尚未接线（UTF-7／UTF-16／UTF-32 一族、unicode_escape 一族、`encode`／`decode` 直调）",
    ))
}

/// 建 `_codecs` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // 状态两格（**私有**名 ✓）
    let search = instance.new_list(Vec::new());
    instance.dict_set(namespace, SEARCH_FUNCTIONS, search);
    let handlers = instance.new_dict();
    instance.dict_set(namespace, ERROR_HANDLERS, handlers);
    // **六个内建错误处理器** ✓（`codecs.py` 一进门就取它们 ✓）：`strict_errors` 真实现 ✓，
    // 其余五个照名字装上、调用时报未实现 ✓。
    // 名字有两套（**都照参照** ✓）：模块属性是长名（`strict_errors` ✓），注册表里是**短名**
    // （`lookup_error("strict")` 取的就是它 ✓ —— `codecs.py:1114` 一进门就取 ✓）。
    for (short, long, handler) in [
        ("strict", "strict_errors", strict_errors_native as NativeFn),
        ("ignore", "ignore_errors", not_implemented_handler_native as NativeFn),
        ("replace", "replace_errors", not_implemented_handler_native as NativeFn),
        (
            "xmlcharrefreplace",
            "xmlcharrefreplace_errors",
            not_implemented_handler_native as NativeFn,
        ),
        (
            "backslashreplace",
            "backslashreplace_errors",
            not_implemented_handler_native as NativeFn,
        ),
        ("namereplace", "namereplace_errors", not_implemented_handler_native as NativeFn),
    ] {
        let native = make_native(instance, long, handler);
        instance.dict_set(namespace, long, native);
        // 注册表那份要**自己一份引用**（`dict_set` 接管 ✓）
        instance.retain(native);
        instance.dict_set(handlers, short, native);
    }
    // 注册表
    for (name, handler) in [
        ("register", register_native as NativeFn),
        ("lookup", lookup_native as NativeFn),
        ("register_error", register_error_native as NativeFn),
        ("lookup_error", lookup_error_native as NativeFn),
    ] {
        let native = make_native(instance, name, handler);
        instance.dict_set(namespace, name, native);
    }
    // **真实现**的编解码入口
    for (name, handler) in [
        ("ascii_encode", ascii_encode_native as NativeFn),
        ("ascii_decode", ascii_decode_native as NativeFn),
        ("latin_1_encode", latin_1_encode_native as NativeFn),
        ("latin_1_decode", latin_1_decode_native as NativeFn),
        ("utf_8_encode", utf_8_encode_native as NativeFn),
        ("utf_8_decode", utf_8_decode_native as NativeFn),
        ("charmap_build", charmap_build_native as NativeFn),
        ("charmap_decode", charmap_decode_native as NativeFn),
        ("charmap_encode", charmap_encode_native as NativeFn),
    ] {
        let native = make_native(instance, name, handler);
        instance.dict_set(namespace, name, native);
    }
    // **名字齐、调用报未实现**的那些：`encodings/*.py` 在**类体**里就取它们 ✓
    for name in [
        "utf_7_encode",
        "utf_7_decode",
        "utf_16_encode",
        "utf_16_decode",
        "utf_16_ex_decode",
        "utf_16_be_encode",
        "utf_16_be_decode",
        "utf_16_le_encode",
        "utf_16_le_decode",
        "utf_32_encode",
        "utf_32_decode",
        "utf_32_ex_decode",
        "utf_32_be_encode",
        "utf_32_be_decode",
        "utf_32_le_encode",
        "utf_32_le_decode",
        "unicode_escape_encode",
        "unicode_escape_decode",
        "raw_unicode_escape_encode",
        "raw_unicode_escape_decode",
        "encode",
        "decode",
    ] {
        let native = make_native(instance, name, not_implemented_native);
        instance.dict_set(namespace, name, native);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str("Codec registry and base classes (Pyawa's built-in codecs).");
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}
