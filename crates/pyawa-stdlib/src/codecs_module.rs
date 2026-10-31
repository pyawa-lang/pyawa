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

/// 取 **`str` 实参的 WTF-8 字节**（P3-38 S3a）：`text_argument` 用 `text_value`，对含孤立代理的
/// 表示取不到 ✗；`_codecs.*_encode` 的输入要走字节视图 ✓（消息与 `text_argument` 同形 ✓）。
fn wtf8_argument(
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
    instance.wtf8_of(value).map(<[u8]>::to_vec).ok_or_else(|| {
        instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{function}() argument must be str, not {}",
                instance.type_name(instance.type_of(value))
            ),
        )
    })
}

/// **WTF-8 字节 ⇒ `str` 对象**（S3a：解码结果可能含孤立代理 ✓）。
fn str_from_wtf8(instance: &Instance, bytes: &[u8]) -> NonNull<Header> {
    instance
        .new_str_wtf8(bytes)
        .expect("解码产物必须是合法 WTF-8")
}

/// **码点个数**（`_codecs.*_encode` 的"消耗码位数"口径 ✓：代理各算 1 ✓）。
fn code_point_count(bytes: &[u8]) -> usize {
    pyawa_core::wtf8::code_points(bytes).count()
}

/// 自定义错误处理器（`errors` 不是那三个内建的）—— 本层**如实报未实现** ✓。
fn custom_handler_error(instance: &Instance, errors: &str, encoding: &str) -> ExecError {
    // **已注册**的自定义处理器：契约要吃 `UnicodeEncodeError` 的结构化字段（object／start／end），
    // 本层异常对象还没接 ⇒ 如实 `NotImplementedError` ✓；**没注册的**照参照 `LookupError` ✓。
    let registered = module_state(instance, ERROR_HANDLERS)
        .is_some_and(|mapping| instance.dict_get(mapping, errors).is_some());
    if registered {
        instance.raise_builtin_error(
            "NotImplementedError",
            &format!(
                "`{encoding}` codec 的 `{errors}` 自定义错误处理器尚未接线（要 `UnicodeEncodeError` 的结构化字段）"
            ),
        )
    } else {
        instance.raise_builtin_error(
            "LookupError",
            &format!("unknown error handler name '{errors}'"),
        )
    }
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

/// **`unregister(search_function)`**：把**同一个对象**从注册表里删掉 ✓（照参照：按对象身份比；
/// 找不到就**静默**返回 `None`、不报错 ✓ —— 本机 3.14.4 实测）。
///
/// 动因（`P3-40` 顺带；分诊里的 `'module' object has no attribute 'unregister'` 实际是
/// **`codecs.unregister`**，不是 `os.unregister` —— CPython 的 `os` 没有这个名字）：
/// `test_charmapcodec`／`test_str` 都在 `addCleanup(codecs.unregister, …)` 上要它。
fn unregister_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("unregister() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let target = args[0];
    let mut kept: Vec<NonNull<Header>> = Vec::new();
    if let Some(list) = module_state(instance, SEARCH_FUNCTIONS) {
        if let Some(existing) = instance.list_items(list) {
            for entry in existing {
                if entry != target {
                    // 新列表是"接管"语义 ⇒ 留下的每一项自己 retain 一份 ✓（被删的那项由旧列表释放 ✓）。
                    kept.push(instance.retain(entry));
                }
            }
        }
    }
    set_module_state(instance, SEARCH_FUNCTIONS, instance.new_list(kept));
    Ok(instance.new_none())
}

/// **按注册顺序问一遍 search function**：都没有 ⇒ `None`（`codecs.register` 的注册表）。
///
/// 直接转发调用方那个**对象**（借用 ⇒ `call_value` 自己 retain ✓）；返回的是**新引用**。
fn ask_search_functions(
    instance: &Instance,
    query: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, ExecError> {
    let none = instance.singletons().none();
    let Some(list) = module_state(instance, SEARCH_FUNCTIONS) else {
        return Ok(None);
    };
    let Some(functions) = instance.list_items(list) else {
        return Ok(None);
    };
    for function in functions {
        let result = pyawa_core::executor::call::call_value(instance, function, &[query], &[])?;
        if result == none {
            // `call_value` 交的是**新引用** ⇒ 不要就当场还 ✓
            instance.release(result);
            continue;
        }
        return Ok(Some(result));
    }
    Ok(None)
}

/// **`lookup(encoding)`**：内建表 → 注册的 search function → **懒导入 `encodings`** 再试 → 都没有
/// ⇒ `LookupError: unknown encoding: X` ✓。
///
/// **懒导入**（task-9）：参照的 `_codecs.lookup` 在注册表未命中时会**自己**把 `encodings` 包导入
/// （`encodings/__init__.py:161` 在导入期 `codecs.register(search_function)` ✓）⇒
/// `import codecs; codecs.lookup('cp932')` **不需要**调用方先 `import encodings` ✓。先前本层要求
/// 先导入 ⇒ 与参照不符 ✗。导入失败（找不到包）就照旧报 `LookupError`，不把"没这个包"当异常抛 ✓。
fn lookup_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let name = text_argument(instance, args, 0, "lookup")?;
    // **内建编码表优先** ✓（第 726 轮）：`ascii`／`latin-1`／`utf-8`／`cp437` ＋ 别名 ✓ —— 由
    // **Rust 造的 `CodecInfo` 类**交付 ✓（与 `_sre` 的 Pattern／Match 同例 ✓）。
    if let Some(canonical) = canonical_codec(&name) {
        return codec_info_of(instance, canonical);
    }
    // search function 收**归一后的名字**（照参照 ✓）；报错文案用调用方原文 ✓。
    let normalized = normalize_encoding_name(&name);
    let query = instance.new_str(&normalized);
    let normalized_result = lookup_through_search_functions(instance, query);
    // `.0` 是归一后的名字对象（新引用）⇒ 查完就还 ✓。
    instance.release(query);
    if let Some(found) = normalized_result? {
        return Ok(found);
    }
    Err(instance.raise_builtin_error(
        "LookupError",
        &format!("unknown encoding: {name}"),
    ))
}

/// `lookup` 的后半：问注册的 search function（没有就**懒导入 `encodings`** 再问一遍）。
fn lookup_through_search_functions(
    instance: &Instance,
    query: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, ExecError> {
    if let Some(found) = ask_search_functions(instance, query)? {
        return Ok(Some(found));
    }
    // **懒导入 `encodings`**（照参照 ✓）：它一导入就 `codecs.register(search_function)`。
    match pyawa_core::executor::import::import_through_bridge(instance, "encodings") {
        Ok(Some(_module)) => {
            // `import_through_bridge` 交的是**借用**（模块表持着）⇒ 不释放 ✓
            if let Some(found) = ask_search_functions(instance, query)? {
                return Ok(Some(found));
            }
        }
        Ok(None) => {}
        Err(error) => return Err(error),
    }
    Ok(None)
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
    // **P3-38 S3a**：输入走 WTF-8 字节视图；四档内置编解码统一交给 `Instance` 的实现 ✓。
    let text = wtf8_argument(instance, args, 0, "ascii_encode")?;
    let errors = errors_argument(instance, args, 1);
    let out = instance.encode_with_builtin_codec(&text, "ascii", &errors)?;
    let bytes = instance.new_bytes(&out);
    let consumed = code_point_count(&text);
    Ok(pair(instance, bytes, consumed))
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
    let out = instance.decode_with_builtin_codec(&input, "ascii", &errors)?;
    let value = str_from_wtf8(instance, &out);
    Ok(pair(instance, value, input.len()))
}

/// `latin_1_encode(input, errors='strict')` ✓（**不可能失败** ✓）。
fn latin_1_encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let text = wtf8_argument(instance, args, 0, "latin_1_encode")?;
    let errors = errors_argument(instance, args, 1);
    let out = instance.encode_with_builtin_codec(&text, "latin-1", &errors)?;
    let bytes = instance.new_bytes(&out);
    let consumed = code_point_count(&text);
    Ok(pair(instance, bytes, consumed))
}

/// `latin_1_decode(input, errors='strict')` ✓（**不可能失败** ✓）。
fn latin_1_decode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let input = bytes_argument(instance, args, 0, "latin_1_decode")?;
    let out = instance.decode_with_builtin_codec(&input, "latin-1", "strict")?;
    let value = str_from_wtf8(instance, &out);
    Ok(pair(instance, value, input.len()))
}

/// `utf_8_encode(input, errors='strict')` ✓（Rust 的 `String` 本来就是 UTF-8 ✓）。
fn utf_8_encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let text = wtf8_argument(instance, args, 0, "utf_8_encode")?;
    let errors = errors_argument(instance, args, 1);
    let out = instance.encode_with_builtin_codec(&text, "utf-8", &errors)?;
    let bytes = instance.new_bytes(&out);
    let consumed = code_point_count(&text);
    Ok(pair(instance, bytes, consumed))
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
    // 第三参 `final`（`encodings/utf_8.py` 传 True；缺省 False ✓）：只影响**末尾截断**序列。
    let final_round = match args.get(2) {
        Some(value) => {
            *value != instance.singletons().none()
                && *value != instance.singletons().boolean(false)
        }
        None => false,
    };
    let (out, consumed) = instance.decode_utf8_with_errors(&input, &errors, final_round)?;
    let value = str_from_wtf8(instance, &out);
    Ok(pair(instance, value, consumed))
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
    let mut out: Vec<u8> = Vec::with_capacity(input.len());
    for (position, byte) in input.iter().enumerate() {
        let character = match &table {
            Some(table) => table[*byte as usize],
            // 没有映射 ⇒ 恒等（`latin-1` 那一档 ✓）
            None => Some(*byte as char),
        };
        let Some(character) = character else {
            match errors.as_str() {
                "strict" | "surrogatepass" => {
                    return Err(instance.raise_builtin_error(
                        "UnicodeDecodeError",
                        &format!(
                            "'charmap' codec can't decode byte 0x{byte:02x} in position {position}: character maps to <undefined>"
                        ),
                    ))
                }
                "ignore" => continue,
                "replace" => {
                    pyawa_core::wtf8::encode_code_point(0xFFFD, &mut out)
                        .expect("U+FFFD 可编码");
                    continue;
                }
                // **P3-38 S3a**：`surrogateescape` 把该字节映射成 U+DC00+byte ✓。
                "surrogateescape" => {
                    pyawa_core::wtf8::encode_code_point(0xDC00 + u32::from(*byte), &mut out)
                        .expect("U+DC80..U+DCFF 在 WTF-8 范围内");
                    continue;
                }
                other => return Err(custom_handler_error(instance, other, "charmap")),
            }
        };
        pyawa_core::wtf8::encode_code_point(character as u32, &mut out)
            .expect("charmap 表的码点可编码");
    }
    let value = str_from_wtf8(instance, &out);
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
    let text = wtf8_argument(instance, args, 0, "charmap_encode")?;
    let errors = errors_argument(instance, args, 1);
    let mapping = args.get(2).copied().filter(|value| *value != instance.singletons().none());
    let table = encode_table(instance, mapping);
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    let mut position = 0usize;
    for point in pyawa_core::wtf8::code_points(&text) {
        let code = point.expect("str 载荷必须是合法 WTF-8");
        let byte = match &table {
            Some(table) if (code as usize) < table.len() => table[code as usize],
            // 没有映射 ⇒ 恒等（码点 ≤ 0xFF 才成立 ✓）
            _ => u8::try_from(code).ok(),
        };
        let Some(byte) = byte else {
            if errors == "surrogateescape" && (0xDC80..=0xDCFF).contains(&code) {
                out.push((code - 0xDC00) as u8);
                position += 1;
                continue;
            }
            match errors.as_str() {
                "strict" | "surrogatepass" => {
                    return Err(instance.raise_builtin_error(
                        "UnicodeEncodeError",
                        &format!(
                            "'charmap' codec can't encode character '\\u{:04x}' in position {position}: character maps to <undefined>",
                            code
                        ),
                    ))
                }
                "ignore" => {}
                "replace" => out.push(b'?'),
                other => return Err(custom_handler_error(instance, other, "charmap")),
            }
            position += 1;
            continue;
        };
        out.push(byte);
        position += 1;
    }
    let bytes = instance.new_bytes(&out);
    let consumed = code_point_count(&text);
    Ok(pair(instance, bytes, consumed))
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

/// **`_codecs.encode(obj, encoding='utf-8', errors='strict')`**（P3-38 S3a）：`str` 输入走
/// 核心编码器（含 `surrogateescape`／`surrogatepass`；非内置编码经 `_codecs.lookup` 委派 ✓）。
/// **如实范围**：任意对象编解码器（`rot13`／`base64` 等）与 bytes 输入未接 ⇒ `NotImplementedError`。
fn encode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(object) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "encode() takes at least 1 argument (0 given)",
        ));
    };
    let encoding = args
        .get(1)
        .and_then(|value| instance.text_value(*value))
        .unwrap_or_else(|| "utf-8".to_owned());
    let errors = args
        .get(2)
        .and_then(|value| instance.text_value(*value))
        .unwrap_or_else(|| "strict".to_owned());
    if let Some(text) = instance.wtf8_of(object).map(<[u8]>::to_vec) {
        return instance.encode_wtf8_with_encoding_and_errors(&text, &encoding, &errors);
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_codecs.encode`：本层只接了 str 输入的文本编码（任意对象编解码器随后补）",
    ))
}

/// **`_codecs.decode(obj, encoding='utf-8', errors='strict')`**（P3-38 S3a）：bytes 输入走
/// 核心解码器（含 `surrogateescape`／`surrogatepass` ✓）；其余对象形态如实未接。
fn decode_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(object) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "decode() takes at least 1 argument (0 given)",
        ));
    };
    let encoding = args
        .get(1)
        .and_then(|value| instance.text_value(*value))
        .unwrap_or_else(|| "utf-8".to_owned());
    let errors = args
        .get(2)
        .and_then(|value| instance.text_value(*value))
        .unwrap_or_else(|| "strict".to_owned());
    if let Some(bytes) = instance.bytes_value(object).map(<[u8]>::to_vec) {
        return instance.decode_with_encoding_and_errors(&bytes, &encoding, &errors);
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_codecs.decode`：本层只接了 bytes 输入的文本解码（任意对象编解码器随后补）",
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
    // **P3-38 S3a**：`surrogateescape`／`surrogatepass` 是**内建处理器** —— 命名照参照
    // （`codecs.lookup_error` 取到的函数 `__name__` 就是这两个短名 ✓）；本体**如实未接线**
    // （handler 要吃 `UnicodeEncodeError` 的结构化字段，本层异常对象还没接 ⇒ 调用时诚实
    // `NotImplementedError`）。编码/解码的**语义**在 `Instance` 的编解码器里真实现 ✓。
    for short in ["surrogateescape", "surrogatepass"] {
        let native = make_native(instance, short, not_implemented_handler_native);
        instance.retain(native);
        instance.dict_set(handlers, short, native);
    }
    // 注册表
    for (name, handler) in [
        ("register", register_native as NativeFn),
        ("unregister", unregister_native as NativeFn),
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
        // **P3-38 S3a**：`_codecs.encode`／`decode` 的文本编码/解码真实现（`codecs.encode/decode`
        // 直接转给它们 ✓）。
        ("encode", encode_native as NativeFn),
        ("decode", decode_native as NativeFn),
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

/// **`_codecs.lookup` 的名字归一**（照参照 C 侧 `codecs.c: normalize_encoding` ✓）：**转小写**、
/// 非字母数字（`.` 除外）折叠成单个 `_`、去首尾 `_`。
///
/// 为什么必须在这里做 ✓：`search_function` 收的是**归一后的名字**（参照的 C 实现先归一 ✓）——
/// 先前本层把调用方**原文**直接转给 search function ✗ ⇒ `codecs.lookup('ROT13')`／`lookup('Hex')`
/// 在 `encodings` 里按大小写找模块 ⇒ 找不到 ✗（参照都能查 ✓）。**报错文案仍用调用方原文** ✓
/// （`LookupError: unknown encoding: ROT13`）。
fn normalize_encoding_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut punct = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '.' {
            if punct && !out.is_empty() {
                out.push('_');
            }
            out.push(character.to_ascii_lowercase());
            punct = false;
        } else {
            punct = true;
        }
    }
    out
}

/// **编码名归一**（`_codecs.lookup` 的内建表口径 ✓；与 core 的 `normalized_encoding` 同一套别名 ✓）。
fn canonical_codec(name: &str) -> Option<&'static str> {
    let lowered = name.to_ascii_lowercase().replace(['-', ' '], "_");
    match lowered.as_str() {
        "utf_8" | "utf8" | "u8" | "cp65001" => Some("utf-8"),
        "ascii" | "us_ascii" | "646" => Some("ascii"),
        "latin_1" | "latin1" | "iso_8859_1" | "iso8859_1" | "8859" | "cp819" | "l1" => {
            Some("latin-1")
        }
        "cp437" | "437" | "ibm437" | "csibm437" => Some("cp437"),
        _ => None,
    }
}

/// **造一个 `CodecInfo`** ✓（Rust 造类 ✓；`name` ／ `encode` ／ `decode` 三个字段可读可调 ✓
/// —— **如实口径** ✗：参照的 `CodecInfo` 是 namedtuple（`incrementalencoder` 一族暂无 ✓，
/// 等内建容器子类布局落地后再对齐 ✓）。
fn codec_info_of(instance: &Instance, canonical: &str) -> Result<NonNull<Header>, ExecError> {
    let object_type = instance.type_named("object").expect("object 已登记");
    let class_namespace = instance.new_dict();
    for (name, handler) in [
        ("encode", codec_info_encode as NativeFn),
        ("decode", codec_info_decode as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(class_namespace, name, function);
    }
    let class = pyawa_core::build_class_from_parts(
        instance,
        "CodecInfo".to_owned(),
        vec![object_type.cast()],
        class_namespace,
        None,
    )?;
    let info = pyawa_core::executor::call::call_value(instance, class, &[], &[])?;
    let name_value = instance.new_str(canonical);
    let _ = instance.set_attribute_value(info, "name", name_value);
    Ok(info)
}

/// `CodecInfo.encode(文本, errors='strict')` ✓（按实例的 `name` 字段派发 ✓）。
fn codec_info_encode(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "CodecInfo.encode 缺少 self"));
    };
    let name = instance
        .attribute_optional_of(this, "name")?
        .and_then(|value| instance.text_of(value).map(|text| text.to_owned()))
        .unwrap_or_else(|| "utf-8".to_owned());
    let Some(text) = args.first().and_then(|value| instance.text_of(*value)) else {
        return Err(instance.raise_builtin_error("TypeError", "encode() 的实参要是 str"));
    };
    instance.encode_with_encoding(text, &name)
}

/// `CodecInfo.decode(字节, errors='strict')` ✓（同上 ✓）。
fn codec_info_decode(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = bound else {
        return Err(instance.raise_builtin_error("TypeError", "CodecInfo.decode 缺少 self"));
    };
    let name = instance
        .attribute_optional_of(this, "name")?
        .and_then(|value| instance.text_of(value).map(|text| text.to_owned()))
        .unwrap_or_else(|| "utf-8".to_owned());
    let Some(raw) = args.first().and_then(|value| instance.bytes_value(*value).map(<[u8]>::to_vec))
    else {
        return Err(instance.raise_builtin_error("TypeError", "decode() 的实参要是 bytes"));
    };
    instance.decode_with_encoding(&raw, &name)
}
