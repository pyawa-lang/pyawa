//! `_csv`（task-3，`PLAN-milestones.md` §12.3）：`Lib/csv.py` 的 C 底座。
//!
//! **语义出处**：逐条照参照本机 CPython 3.14.4 的 `Modules/_csv.c`（`parse_process_char`
//! 状态机／`join_append_data` 写侧规则／`dialect_new` 校验顺序与文案），并用
//! `python3 - <<'EOF' …` 实测钉住文案（不编造）。
//!
//! **导出面**（照参照 `dir(_csv)`）：`Error`／`Dialect`／`Reader`／`Writer`／`reader`／
//! `writer`／`register_dialect`／`unregister_dialect`／`get_dialect`／`list_dialects`／
//! `field_size_limit`／`QUOTE_MINIMAL`／`QUOTE_ALL`／`QUOTE_NONNUMERIC`／`QUOTE_NONE`／
//! `QUOTE_STRINGS`／`QUOTE_NOTNULL`／`_dialects`。
//!
//! **实现形态**（如实）：
//! - 文件里用到的**自定义类型**（`_csv.Dialect`／`_csv.reader`／`_csv.writer`）都是
//!   **每实例的原生类型**（`new_attribute_type` ＋ 类型字典挂 native，与
//!   `os.DirEntry` 同一手法）——实例状态放**实例字典**的私有键（`__pyawa_csv_*`）；
//!   参照的 C 载荷字段在本层不存在 ✓（`CX-22`：stdlib 不碰异常／对象私有布局）。
//! - `Dialect` 的 8 个数据属性用 **`property`（只读）** 实现 ⇒ `setattr`／`delattr` 报
//!   `AttributeError`（参照是只读 member；文案不同，属实现差异 ✓）。
//! - 读侧字段缓冲是 `Vec<Header>` ＋ `String`；写侧记录缓冲是 `String`。
//! - **未接面**：原生函数的 `__module__` 走 core 的统一口径 `"builtins"`（参照给 `"_csv"`）
//!   ⇒ `test.support.check__all__` 认不出这 7 个函数名（上游 `test_csv.test__all__` 的唯一
//!   失败点；修它要动 `pyawa-core`，见 `docs/SPEC-c-modules.md` §5.2.33）；模块级
//!   `__text_signature__`（原生函数没有 clinic 签名串）；`Reader`／
//!   `Writer`／`Dialect` 的 `repr` 用本层通用 repr；`Dialect` 上的 `_name`／`_valid`
//!   照参照**不提供**（那是 `Lib/csv.py` 的 Python 层类属性）。
//!
//! 本模块**不碰平台**（`CX-4`）。

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{AttributeObject, ExecError, Header, Instance, NativeFn};

/// 模块名（`_csv`）。
pub const NAME: &str = "_csv";

/// 模块的 `__doc__`（照参照）。
const DOC: &str = "CSV parsing and writing.\n";

/// `quoting` 常量（照参照 `quote_styles`）。
const QUOTE_MINIMAL: i64 = 0;
const QUOTE_ALL: i64 = 1;
const QUOTE_NONNUMERIC: i64 = 2;
const QUOTE_NONE: i64 = 3;
const QUOTE_STRINGS: i64 = 4;
const QUOTE_NOTNULL: i64 = 5;

/// `field_size_limit` 的默认值（`_csv.c` 的 `128 * 1024`）。
const DEFAULT_FIELD_LIMIT: i64 = 128 * 1024;

/// `Dialect(...)` 的形参名（照参照 `dialect_kws` 的次序 —— 校验／报错次序依赖它）。
const DIALECT_KWS: [&str; 9] = [
    "dialect",
    "delimiter",
    "doublequote",
    "escapechar",
    "lineterminator",
    "quotechar",
    "quoting",
    "skipinitialspace",
    "strict",
];

// ---- 模块命名空间里的状态键 ---------------------------------------------------------

/// `_dialects` 注册表（**照参照暴露**：`_csv._dialects` 是 dict）。
const KEY_DIALECTS: &str = "_dialects";
/// 字段大小上限（模块级状态；参照放在 C 的模块 state 里 ⇒ 本层放命名空间私有键）。
const KEY_FIELD_LIMIT: &str = "__pyawa_csv_field_limit";

// ---- 实例字典里的私有状态键 ---------------------------------------------------------

const R_INPUT: &str = "__pyawa_csv_input";
const R_DIALECT: &str = "__pyawa_csv_dialect";
const R_LINE_NUM: &str = "__pyawa_csv_line_num";
const W_WRITE: &str = "__pyawa_csv_write";

const D_DELIMITER: &str = "__pyawa_csv_delimiter";
const D_DOUBLEQUOTE: &str = "__pyawa_csv_doublequote";
const D_ESCAPECHAR: &str = "__pyawa_csv_escapechar";
const D_LINETERMINATOR: &str = "__pyawa_csv_lineterminator";
const D_QUOTECHAR: &str = "__pyawa_csv_quotechar";
const D_QUOTING: &str = "__pyawa_csv_quoting";
const D_SKIPINITIALSPACE: &str = "__pyawa_csv_skipinitialspace";
const D_STRICT: &str = "__pyawa_csv_strict";

// ---- 小工具 -------------------------------------------------------------------------

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

/// 取 `self`：绑定调用给 `bound`；非绑定调用（`__init__` 的通用分配路／属性 getter）拿第一个位置实参。
fn this_of(bound: Option<NonNull<Header>>, args: &[NonNull<Header>]) -> Option<NonNull<Header>> {
    bound.or_else(|| args.first().copied())
}

/// 取 `(self, 其余位置实参)`。
fn this_with_args<'a>(
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> Option<(NonNull<Header>, &'a [NonNull<Header>])> {
    match bound {
        Some(this) => Some((this, args)),
        None => args.split_first().map(|(first, rest)| (*first, rest)),
    }
}

/// 写实例属性（`value` 是**新引用**，本函数消费它）。
fn store(instance: &Instance, object: NonNull<Header>, name: &str, value: NonNull<Header>) {
    let _ = instance.set_attribute_value(object, name, value);
    instance.release(value);
}

/// 读实例属性（**新引用**；没有给 `None`）。
fn load(instance: &Instance, object: NonNull<Header>, name: &str) -> Option<NonNull<Header>> {
    instance.attribute_optional_discarding(object, name)
}

/// 读实例属性；没有 ⇒ 内部错误（`AttributeError`）。
fn load_required(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
) -> Result<NonNull<Header>, ExecError> {
    load(instance, object, name)
        .ok_or_else(|| instance.raise_builtin_error("AttributeError", name))
}

/// 类型名（`%T` 口径：不带模块前缀）。
fn type_display_name(instance: &Instance, value: NonNull<Header>) -> String {
    instance.type_name(instance.type_of(value))
}

fn type_error(instance: &Instance, message: String) -> ExecError {
    instance.raise_builtin_error("TypeError", &message)
}

/// `value` 是不是 `name` 异常（含子类）。
fn exception_is(instance: &Instance, exception: NonNull<Header>, name: &str) -> bool {
    instance
        .type_named(name)
        .is_some_and(|base| instance.is_subtype(instance.type_of(exception), base))
}

/// 吞掉一个「预期内」的 `ExecError::Raised`（两份引用都还：`pending_exception` 与 `Err` 那份）。
///
/// 照 `crates/pyawa-stdlib/src/builtins_module.rs` 的 `swallow_attribute_error` 口径（`T-CX-1`）。
fn swallow_raised(instance: &Instance, error: &ExecError) -> bool {
    let ExecError::Raised { exception } = error else {
        return false;
    };
    if let Some(previous) = instance.set_pending_exception(None) {
        instance.release(previous);
    }
    instance.release(*exception);
    true
}

/// 拿**模块命名空间**（`sys.modules['_csv']` 的 dict）。
fn module_namespace(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    pyawa_core::mounted_instance_dict(instance, module)
}

/// 取模块里的 `Error` 类（**一处真相**：`build` 里挂的那个；按 `sys.modules` 现取 ⇒ 同一性 ✓）。
fn error_class(instance: &Instance) -> Option<NonNull<Header>> {
    let namespace = module_namespace(instance)?;
    instance.dict_get(namespace, "Error")
}

/// 抛 `_csv.Error`（**用那个类** ✓，不是 `ValueError` 冒充 ✗）。
fn raise_csv_error(instance: &Instance, message: &str) -> ExecError {
    let Some(class) = error_class(instance) else {
        return instance.raise_builtin_error("RuntimeError", message);
    };
    let text = instance.new_str(message);
    let outcome = pyawa_core::call_value(instance, class, &[text], &[]);
    instance.release(text);
    match outcome {
        Ok(exception) => {
            // 通用分配那条路不跑 `__init__` ⇒ `args`／`message` 自己写（同 `binascii` 口径）。
            let message_value = instance.new_str(message);
            let _ = pyawa_core::executor::protocol::instance_attribute_set(
                instance,
                exception,
                "message",
                message_value,
                0,
            );
            let arguments = instance.new_tuple(vec![instance.new_str(message)]);
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

/// 注册表 dict（**借用** —— `dict_get` 的借用口径，调用方不得 release）。
fn dialects_dict(instance: &Instance) -> Result<NonNull<Header>, ExecError> {
    let namespace = module_namespace(instance)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "_csv 模块命名空间缺失"))?;
    instance
        .dict_get(namespace, KEY_DIALECTS)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "_csv._dialects 缺失"))
}

fn field_limit(instance: &Instance) -> i64 {
    module_namespace(instance)
        .and_then(|namespace| instance.dict_get(namespace, KEY_FIELD_LIMIT))
        .and_then(|value| instance.int_value(value))
        .unwrap_or(DEFAULT_FIELD_LIMIT)
}

fn set_field_limit(instance: &Instance, limit: i64) {
    if let Some(namespace) = module_namespace(instance) {
        let value = instance.new_int(limit);
        instance.dict_set(namespace, KEY_FIELD_LIMIT, value);
        instance.release(value);
    }
}

/// 注册表里按**任意键对象**查（`subscript_read` 走完整的 dict 协议：`KeyError`／`TypeError` 照参照）。
fn registry_lookup(
    instance: &Instance,
    name: NonNull<Header>,
) -> Result<Option<NonNull<Header>>, ExecError> {
    let dialects = dialects_dict(instance)?;
    let outcome = pyawa_core::executor::subscript::subscript_read(instance, dialects, name);
    match outcome {
        Ok(value) => Ok(Some(value)),
        Err(ExecError::Raised { exception }) => {
            if !exception_is(instance, exception, "KeyError") {
                return Err(ExecError::Raised { exception });
            }
            let error = ExecError::Raised { exception };
            swallow_raised(instance, &error);
            Ok(None)
        }
        Err(other) => Err(other),
    }
}

/// 注册表里删一个键（返回是否删掉；`TypeError` 原样上抛）。
fn registry_delete(instance: &Instance, name: NonNull<Header>) -> Result<bool, ExecError> {
    let dialects = dialects_dict(instance)?;
    let outcome = pyawa_core::executor::subscript::subscript_del(instance, dialects, name, 0);
    match outcome {
        Ok(()) => Ok(true),
        Err(ExecError::Raised { exception }) => {
            if !exception_is(instance, exception, "KeyError") {
                return Err(ExecError::Raised { exception });
            }
            let error = ExecError::Raised { exception };
            swallow_raised(instance, &error);
            Ok(false)
        }
        Err(other) => Err(other),
    }
}

// ---- 引用守卫（错误路径不泄漏） -----------------------------------------------------

/// 一批**本函数持有**的新引用，出错时统一归还。
struct RefGuard<'a> {
    instance: &'a Instance,
    refs: Vec<NonNull<Header>>,
}

impl<'a> RefGuard<'a> {
    fn new(instance: &'a Instance) -> Self {
        Self { instance, refs: Vec::new() }
    }

    fn own(&mut self, value: NonNull<Header>) -> NonNull<Header> {
        self.refs.push(value);
        value
    }
}

impl Drop for RefGuard<'_> {
    fn drop(&mut self) {
        for value in self.refs.drain(..) {
            self.instance.release(value);
        }
    }
}

// ---- 方言（校验后的规格） -----------------------------------------------------------

/// 校验后的方言值（与参照 `DialectObj` 的 C 字段同构）。
#[derive(Clone)]
struct DialectSpec {
    delimiter: char,
    quotechar: Option<char>,
    escapechar: Option<char>,
    lineterminator: String,
    quoting: i64,
    doublequote: bool,
    skipinitialspace: bool,
    strict: bool,
}

impl Default for DialectSpec {
    fn default() -> Self {
        Self {
            delimiter: ',',
            quotechar: Some('"'),
            escapechar: None,
            lineterminator: "\r\n".to_owned(),
            quoting: QUOTE_MINIMAL,
            doublequote: true,
            skipinitialspace: false,
            strict: false,
        }
    }
}

// 校验辅助：逐条照 `_csv.c` 的 `_set_char`／`_set_char_or_none`／`_set_str`／`_set_int`／`_set_bool`。

fn set_char(
    instance: &Instance,
    name: &str,
    source: Option<NonNull<Header>>,
    default: char,
) -> Result<char, ExecError> {
    let Some(value) = source else {
        return Ok(default);
    };
    let Some(text) = instance.text_of(value) else {
        return Err(type_error(
            instance,
            format!(
                "\"{name}\" must be a unicode character, not {}",
                type_display_name(instance, value)
            ),
        ));
    };
    let length = text.chars().count();
    if length != 1 {
        return Err(type_error(
            instance,
            format!("\"{name}\" must be a unicode character, not a string of length {length}"),
        ));
    }
    Ok(text.chars().next().expect("长度刚检查过"))
}

fn set_char_or_none(
    instance: &Instance,
    name: &str,
    source: Option<NonNull<Header>>,
    default: Option<char>,
) -> Result<Option<char>, ExecError> {
    let Some(value) = source else {
        return Ok(default);
    };
    if value == instance.singletons().none() {
        return Ok(None);
    }
    let Some(text) = instance.text_of(value) else {
        return Err(type_error(
            instance,
            format!(
                "\"{name}\" must be a unicode character or None, not {}",
                type_display_name(instance, value)
            ),
        ));
    };
    let length = text.chars().count();
    if length != 1 {
        return Err(type_error(
            instance,
            format!(
                "\"{name}\" must be a unicode character or None, not a string of length {length}"
            ),
        ));
    }
    Ok(Some(text.chars().next().expect("长度刚检查过")))
}

fn set_str(
    instance: &Instance,
    name: &str,
    source: Option<NonNull<Header>>,
    default: &str,
) -> Result<String, ExecError> {
    let Some(value) = source else {
        return Ok(default.to_owned());
    };
    match instance.text_of(value) {
        Some(text) => Ok(text.to_owned()),
        None => Err(type_error(
            instance,
            format!(
                "\"{name}\" must be a string, not {}",
                type_display_name(instance, value)
            ),
        )),
    }
}

/// `_set_int`：参照要求**恰好** `int`（`PyLong_CheckExact` ⇒ `bool` 不算）。
fn set_int(
    instance: &Instance,
    name: &str,
    source: Option<NonNull<Header>>,
    default: i64,
) -> Result<i64, ExecError> {
    let Some(value) = source else {
        return Ok(default);
    };
    let exact_int = instance.type_of(value) == instance.singletons().int_type();
    if !exact_int {
        return Err(type_error(
            instance,
            format!(
                "\"{name}\" must be an integer, not {}",
                type_display_name(instance, value)
            ),
        ));
    }
    instance.int_value(value).ok_or_else(|| {
        instance.raise_builtin_error(
            "OverflowError",
            "Python int too large to convert to C int",
        )
    })
}

fn set_bool(
    instance: &Instance,
    source: Option<NonNull<Header>>,
    default: bool,
) -> Result<bool, ExecError> {
    match source {
        None => Ok(default),
        Some(value) => instance.truthiness_of(value),
    }
}

/// `dialect_check_char`（`allowspace` 与 `lineterminator` 的冲突表照参照）。
fn check_char(
    instance: &Instance,
    name: &str,
    value: Option<char>,
    spec: &DialectSpec,
    allow_space: bool,
) -> Result<(), ExecError> {
    let Some(character) = value else {
        return Ok(());
    };
    if character == '\r' || character == '\n' || (character == ' ' && !allow_space) {
        return Err(instance.raise_builtin_error("ValueError", &format!("bad {name} value")));
    }
    if spec.lineterminator.contains(character) {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("bad {name} or lineterminator value"),
        ));
    }
    Ok(())
}

fn check_chars(
    instance: &Instance,
    name1: &str,
    name2: &str,
    value1: Option<char>,
    value2: Option<char>,
) -> Result<(), ExecError> {
    if value1.is_some() && value1 == value2 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("bad {name1} or {name2} value"),
        ));
    }
    Ok(())
}

/// 校验后的完整检查（照 `dialect_new` 里那段 `dialect_check_*` 的**次序**）。
fn validate_spec(instance: &Instance, spec: &DialectSpec) -> Result<(), ExecError> {
    check_char(instance, "delimiter", Some(spec.delimiter), spec, true)?;
    check_char(
        instance,
        "escapechar",
        spec.escapechar,
        spec,
        !spec.skipinitialspace,
    )?;
    check_char(
        instance,
        "quotechar",
        spec.quotechar,
        spec,
        !spec.skipinitialspace,
    )?;
    check_chars(
        instance,
        "delimiter",
        "escapechar",
        Some(spec.delimiter),
        spec.escapechar,
    )?;
    check_chars(
        instance,
        "delimiter",
        "quotechar",
        Some(spec.delimiter),
        spec.quotechar,
    )?;
    check_chars(
        instance,
        "escapechar",
        "quotechar",
        spec.escapechar,
        spec.quotechar,
    )?;
    Ok(())
}

// ---- `_csv.Dialect` 类型 ------------------------------------------------------------

/// 把校验后的规格写进 `Dialect` 实例的私有键。
fn write_spec(instance: &Instance, object: NonNull<Header>, spec: &DialectSpec) -> Result<(), ExecError> {
    store(
        instance,
        object,
        D_DELIMITER,
        instance.new_str(&spec.delimiter.to_string()),
    );
    store(
        instance,
        object,
        D_DOUBLEQUOTE,
        instance.new_bool(spec.doublequote),
    );
    store(
        instance,
        object,
        D_ESCAPECHAR,
        match spec.escapechar {
            Some(character) => instance.new_str(&character.to_string()),
            None => instance.new_none(),
        },
    );
    store(
        instance,
        object,
        D_LINETERMINATOR,
        instance.new_str(&spec.lineterminator),
    );
    store(
        instance,
        object,
        D_QUOTECHAR,
        match spec.quotechar {
            Some(character) => instance.new_str(&character.to_string()),
            None => instance.new_none(),
        },
    );
    store(instance, object, D_QUOTING, instance.new_int(spec.quoting));
    store(
        instance,
        object,
        D_SKIPINITIALSPACE,
        instance.new_bool(spec.skipinitialspace),
    );
    store(instance, object, D_STRICT, instance.new_bool(spec.strict));
    Ok(())
}

/// 从 `Dialect` 实例（或任何带 8 个属性的对象）读回规格。
///
/// **读不到就是内部错误**（`AttributeError`）：`Dialect` 实例的属性由 `write_spec` 保证齐全。
fn spec_from_object(instance: &Instance, object: NonNull<Header>) -> Result<DialectSpec, ExecError> {
    let mut guard = RefGuard::new(instance);
    let read = |name: &str, guard: &mut RefGuard| -> Result<NonNull<Header>, ExecError> {
        let value = instance
            .attribute_optional_discarding(object, name)
            .ok_or_else(|| instance.raise_builtin_error("AttributeError", name))?;
        Ok(guard.own(value))
    };
    let delimiter_value = read("delimiter", &mut guard)?;
    let delimiter = instance
        .text_of(delimiter_value)
        .and_then(|text| text.chars().next())
        .ok_or_else(|| instance.raise_builtin_error("AttributeError", "delimiter"))?;
    let doublequote_value = read("doublequote", &mut guard)?;
    let escapechar_value = read("escapechar", &mut guard)?;
    let lineterminator_value = read("lineterminator", &mut guard)?;
    let quotechar_value = read("quotechar", &mut guard)?;
    let quoting_value = read("quoting", &mut guard)?;
    let skipinitialspace_value = read("skipinitialspace", &mut guard)?;
    let strict_value = read("strict", &mut guard)?;

    let none = instance.singletons().none();
    let to_char_or_none = |value: NonNull<Header>| -> Option<char> {
        if value == none {
            None
        } else {
            instance.text_of(value).and_then(|text| text.chars().next())
        }
    };
    let spec = DialectSpec {
        delimiter,
        quotechar: to_char_or_none(quotechar_value),
        escapechar: to_char_or_none(escapechar_value),
        lineterminator: instance
            .text_of(lineterminator_value)
            .unwrap_or("\r\n")
            .to_owned(),
        quoting: instance.int_value(quoting_value).unwrap_or(QUOTE_MINIMAL),
        doublequote: instance.truth_of(doublequote_value),
        skipinitialspace: instance.truth_of(skipinitialspace_value),
        strict: instance.truth_of(strict_value),
    };
    Ok(spec)
}

/// `_csv.Dialect.__init__`：照 `dialect_new`：取源对象属性 → 逐项转换 → 校验 → 写私有键。
fn dialect_init_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, positional)) = this_with_args(bound, args) else {
        return Err(type_error(
            instance,
            "descriptor '__init__' of '_csv.Dialect' object needs an argument".to_owned(),
        ));
    };
    if positional.len() > DIALECT_KWS.len() {
        return Err(type_error(
            instance,
            format!(
                "function takes at most {} arguments ({} given)",
                DIALECT_KWS.len(),
                positional.len()
            ),
        ));
    }
    let mut slots: [Option<NonNull<Header>>; 9] = [None; 9];
    for (index, value) in positional.iter().enumerate() {
        slots[index] = Some(*value);
    }
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            return Err(type_error(instance, "keywords must be strings".to_owned()));
        };
        let Some(index) = DIALECT_KWS.iter().position(|candidate| *candidate == name) else {
            return Err(type_error(
                instance,
                format!("this function got an unexpected keyword argument '{name}'"),
            ));
        };
        if slots[index].is_some() {
            return Err(type_error(
                instance,
                format!(
                    "argument for dialect_new() given by name ('{name}') and position ({})",
                    index + 1
                ),
            ));
        }
        slots[index] = Some(*value);
    }

    let mut guard = RefGuard::new(instance);
    // `dialect` 实参：str ⇒ 查注册表（查不到 ⇒ `Error: unknown dialect`）；否则就是源对象本身。
    let source: Option<NonNull<Header>> = match slots[0] {
        Some(value) => match instance.text_of(value) {
            Some(_) => match registry_lookup(instance, value)? {
                Some(found) => Some(guard.own(found)),
                None => return Err(raise_csv_error(instance, "unknown dialect")),
            },
            None => Some(value),
        },
        None => None,
    };
    // 缺的属性从源对象取（**取不到就静默用默认值** —— 照参照 `DIALECT_GETATTR` 的 `PyErr_Clear`）。
    let mut raw = slots;
    if let Some(origin) = source {
        for (index, name) in DIALECT_KWS.iter().enumerate().skip(1) {
            if raw[index].is_none() {
                if let Some(value) = instance.attribute_optional_discarding(origin, name) {
                    raw[index] = Some(guard.own(value));
                }
            }
        }
    }

    let mut spec = DialectSpec::default();
    spec.delimiter = set_char(instance, "delimiter", raw[1], ',')?;
    spec.doublequote = set_bool(instance, raw[2], true)?;
    spec.escapechar = set_char_or_none(instance, "escapechar", raw[3], None)?;
    spec.lineterminator = set_str(instance, "lineterminator", raw[4], "\r\n")?;
    spec.quotechar = set_char_or_none(instance, "quotechar", raw[5], Some('"'))?;
    spec.quoting = set_int(instance, "quoting", raw[6], QUOTE_MINIMAL)?;
    spec.skipinitialspace = set_bool(instance, raw[7], false)?;
    spec.strict = set_bool(instance, raw[8], false)?;

    if !matches!(spec.quoting, QUOTE_MINIMAL | QUOTE_ALL | QUOTE_NONNUMERIC | QUOTE_NONE | QUOTE_STRINGS | QUOTE_NOTNULL)
    {
        return Err(type_error(instance, "bad \"quoting\" value".to_owned()));
    }
    let quotechar_is_none = raw[5].is_some_and(|value| value == instance.singletons().none());
    let quoting_given = raw[6].is_some();
    if quotechar_is_none && !quoting_given {
        spec.quoting = QUOTE_NONE;
    }
    if spec.quoting != QUOTE_NONE && spec.quotechar.is_none() {
        return Err(type_error(
            instance,
            "quotechar must be set if quoting enabled".to_owned(),
        ));
    }
    validate_spec(instance, &spec)?;
    write_spec(instance, this, &spec)?;
    Ok(instance.new_none())
}

/// `Dialect.__reduce__`／`__reduce_ex__`：照参照报 `TypeError`（`copy.copy`／`pickle` 的口径）。
fn dialect_reduce_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(type_error(
        instance,
        "cannot pickle 'Dialect' instances".to_owned(),
    ))
}

fn dialect_getter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    key: &str,
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("AttributeError", key));
    };
    load_required(instance, this, key)
}

macro_rules! dialect_getter_native {
    ($name:ident, $key:expr) => {
        fn $name(
            instance: &Instance,
            bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            dialect_getter(instance, bound, args, $key)
        }
    };
}

dialect_getter_native!(dialect_delimiter_getter, D_DELIMITER);
dialect_getter_native!(dialect_doublequote_getter, D_DOUBLEQUOTE);
dialect_getter_native!(dialect_escapechar_getter, D_ESCAPECHAR);
dialect_getter_native!(dialect_lineterminator_getter, D_LINETERMINATOR);
dialect_getter_native!(dialect_quotechar_getter, D_QUOTECHAR);
dialect_getter_native!(dialect_quoting_getter, D_QUOTING);
dialect_getter_native!(dialect_skipinitialspace_getter, D_SKIPINITIALSPACE);
dialect_getter_native!(dialect_strict_getter, D_STRICT);

/// 造一个只读 `property`（`fget` 用 native；照 `_socket.socket_readonly_property` 手法）。
fn readonly_property(instance: &Instance, getter: NativeFn) -> Option<NonNull<Header>> {
    let property_type = instance.type_named("property")?;
    let callable = make_native(instance, "fget", getter);
    let type_value = instance.type_value(property_type).cast::<Header>();
    let outcome = pyawa_core::call_value(instance, type_value, &[callable], &[]);
    instance.release(type_value);
    instance.release(callable);
    outcome.ok()
}

/// 取/建 `_csv.Dialect` 类型（标量属性只读，`__init__` 做校验，`__reduce__` 拒绝序列化）。
fn dialect_type(instance: &Instance) -> NonNull<pyawa_core::TypeObject> {
    if let Some(existing) = instance.type_named("_csv.Dialect") {
        return existing;
    }
    let ty = instance.new_attribute_type("_csv.Dialect");
    for (name, handler) in [
        ("__init__", dialect_init_native as NativeFn),
        ("__reduce__", dialect_reduce_native as NativeFn),
        ("__reduce_ex__", dialect_reduce_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.set_type_attribute(ty, name, function);
    }
    for (name, getter) in [
        ("delimiter", dialect_delimiter_getter as NativeFn),
        ("doublequote", dialect_doublequote_getter as NativeFn),
        ("escapechar", dialect_escapechar_getter as NativeFn),
        ("lineterminator", dialect_lineterminator_getter as NativeFn),
        ("quotechar", dialect_quotechar_getter as NativeFn),
        ("quoting", dialect_quoting_getter as NativeFn),
        ("skipinitialspace", dialect_skipinitialspace_getter as NativeFn),
        ("strict", dialect_strict_getter as NativeFn),
    ] {
        if let Some(property) = readonly_property(instance, getter) {
            instance.set_type_attribute(ty, name, property);
        }
    }
    ty
}

/// `Dialect(源对象?, **fmtparams)`（等价参照 `_call_dialect`）：**新引用**的 `Dialect` 实例。
fn build_dialect(
    instance: &Instance,
    source: Option<NonNull<Header>>,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let ty = dialect_type(instance);
    let type_value = instance.type_value(ty);
    let positional: Vec<NonNull<Header>> = source.into_iter().collect();
    let outcome = pyawa_core::call_value(instance, type_value, &positional, kwargs);
    instance.release(type_value);
    outcome
}

// ---- `_csv.reader` ------------------------------------------------------------------

fn reader_type(instance: &Instance) -> NonNull<pyawa_core::TypeObject> {
    if let Some(existing) = instance.type_named("_csv.reader") {
        return existing;
    }
    let ty = instance.new_attribute_type("_csv.reader");
    for (name, handler) in [
        ("__init__", disallow_instantiation_native as NativeFn),
        ("__iter__", reader_iter_native as NativeFn),
        ("__next__", reader_next_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.set_type_attribute(ty, name, function);
    }
    for (name, getter) in [
        ("dialect", reader_dialect_getter as NativeFn),
        ("line_num", reader_line_num_getter as NativeFn),
    ] {
        if let Some(property) = readonly_property(instance, getter) {
            instance.set_type_attribute(ty, name, property);
        }
    }
    ty
}

/// `_csv.Reader()`／`_csv.Writer()`：照参照 `DISALLOW_INSTANTIATION` 报 `TypeError`。
fn disallow_instantiation_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(type_error(
        instance,
        "cannot create '_csv.reader' instances".to_owned(),
    ))
}

fn reader_dialect_getter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("AttributeError", "dialect"));
    };
    load_required(instance, this, R_DIALECT)
}

fn reader_line_num_getter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("AttributeError", "line_num"));
    };
    load_required(instance, this, R_LINE_NUM)
}

/// 分配一个挂着私有状态的实例。
fn make_instance(
    instance: &Instance,
    ty: NonNull<pyawa_core::TypeObject>,
) -> NonNull<Header> {
    instance
        .alloc(AttributeObject::new(ty, RefCell::new(None)))
        .into_raw()
        .cast::<Header>()
}

/// `_csv.reader(iterable, dialect='excel', **fmtparams)`。
fn reader_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(type_error(
            instance,
            "reader expected at least 1 argument, got 0".to_owned(),
        ));
    }
    if args.len() > 2 {
        return Err(type_error(
            instance,
            format!(
                "reader expected at most 2 arguments, got {}",
                args.len()
            ),
        ));
    }
    let iterator = instance.iter_object(args[0])?;
    let dialect = match build_dialect(instance, args.get(1).copied(), kwargs) {
        Ok(dialect) => dialect,
        Err(error) => {
            instance.release(iterator);
            return Err(error);
        }
    };
    let reader = make_instance(instance, reader_type(instance));
    store(instance, reader, R_INPUT, iterator);
    store(instance, reader, R_DIALECT, dialect);
    store(instance, reader, R_LINE_NUM, instance.new_int(0));
    Ok(reader)
}

/// `reader.__iter__()`：返回自身 ✓。
fn reader_iter_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = this_of(bound, args) else {
        return Err(type_error(instance, "__iter__() 缺少 self".to_owned()));
    };
    Ok(instance.retain(this))
}

// 解析状态（照 `_csv.c` 的 `ParserState`）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParserState {
    StartRecord,
    StartField,
    EscapedChar,
    InField,
    InQuotedField,
    EscapeInQuotedField,
    QuoteInQuotedField,
    EatCrnl,
    AfterEscapedCrnl,
}

/// 读侧的字段列表（**持有**一批新引用；出错时统一归还）。
struct FieldList<'a> {
    instance: &'a Instance,
    items: Vec<NonNull<Header>>,
}

impl<'a> FieldList<'a> {
    fn new(instance: &'a Instance) -> Self {
        Self { instance, items: Vec::new() }
    }

    fn push(&mut self, value: NonNull<Header>) {
        self.items.push(value);
    }

    fn into_list(mut self) -> NonNull<Header> {
        let items = core::mem::take(&mut self.items);
        self.instance.new_list(items)
    }

    fn take_from(&mut self) -> NonNull<Header> {
        let mut moved = Self { instance: self.instance, items: Vec::new() };
        core::mem::swap(&mut moved, self);
        moved.into_list()
    }
}

impl Drop for FieldList<'_> {
    fn drop(&mut self) {
        for value in self.items.drain(..) {
            self.instance.release(value);
        }
    }
}

/// 一次 `__next__` 的解析机器（状态在**本函数内**；参照也是 `parse_reset` 每次重来）。
struct Parser<'a, 'b> {
    instance: &'a Instance,
    spec: &'b DialectSpec,
    field_limit: i64,
    fields: FieldList<'a>,
    field: String,
    state: ParserState,
    unquoted: bool,
}

impl<'a, 'b> Parser<'a, 'b> {
    fn add_char(&mut self, character: char) -> Result<(), ExecError> {
        if self.field.chars().count() as i64 >= self.field_limit {
            return Err(raise_csv_error(
                self.instance,
                &format!("field larger than field limit ({})", self.field_limit),
            ));
        }
        self.field.push(character);
        Ok(())
    }

    /// 照 `parse_save_field`：`QUOTE_NOTNULL`／`QUOTE_STRINGS` 的空**未加引号**字段是 `None`；
    /// `QUOTE_NONNUMERIC`／`QUOTE_STRINGS` 的非空未加引号字段转 `float`。
    fn save_field(&mut self) -> Result<(), ExecError> {
        let quoting = self.spec.quoting;
        if self.unquoted
            && self.field.is_empty()
            && (quoting == QUOTE_NOTNULL || quoting == QUOTE_STRINGS)
        {
            self.fields.push(self.instance.new_none());
            return Ok(());
        }
        let text = core::mem::take(&mut self.field);
        if self.unquoted
            && !text.is_empty()
            && (quoting == QUOTE_NONNUMERIC || quoting == QUOTE_STRINGS)
        {
            let float_type = self
                .instance
                .type_named("float")
                .ok_or_else(|| self.instance.raise_builtin_error("RuntimeError", "float 未登记"))?;
            let text_value = self.instance.new_str(&text);
            let type_value = self.instance.type_value(float_type);
            let outcome =
                pyawa_core::call_value(self.instance, type_value, &[text_value], &[]);
            self.instance.release(type_value);
            self.instance.release(text_value);
            self.fields.push(outcome?);
        } else {
            self.fields.push(self.instance.new_str(&text));
        }
        Ok(())
    }

    /// 照 `parse_process_char`（逐分支转写；注释标出与 C 的分支对应）。
    fn process_char(&mut self, character: Option<char>) -> Result<(), ExecError> {
        if self.state == ParserState::StartRecord {
            let Some(character) = character else {
                // 空行 - 返回 []
                return Ok(());
            };
            if character == '\n' || character == '\r' {
                self.state = ParserState::EatCrnl;
                return Ok(());
            }
            self.state = ParserState::StartField;
        }
        if self.state == ParserState::StartField {
            self.unquoted = true;
            let character = match character {
                None => {
                    self.save_field()?;
                    self.state = ParserState::StartRecord;
                    return Ok(());
                }
                Some(character) if character == '\n' || character == '\r' => {
                    self.save_field()?;
                    self.state = ParserState::EatCrnl;
                    return Ok(());
                }
                Some(character) => character,
            };
            if Some(character) == self.spec.quotechar && self.spec.quoting != QUOTE_NONE {
                self.unquoted = false;
                self.state = ParserState::InQuotedField;
            } else if Some(character) == self.spec.escapechar {
                self.state = ParserState::EscapedChar;
            } else if character == ' ' && self.spec.skipinitialspace {
                // 跳过字段前导空格
            } else if character == self.spec.delimiter {
                self.save_field()?;
            } else {
                self.add_char(character)?;
                self.state = ParserState::InField;
            }
            return Ok(());
        }
        match self.state {
            ParserState::EscapedChar => {
                if character == Some('\n') || character == Some('\r') {
                    self.add_char(character.expect("刚判过是 Some"))?;
                    self.state = ParserState::AfterEscapedCrnl;
                } else {
                    let value = character.unwrap_or('\n');
                    self.add_char(value)?;
                    self.state = ParserState::InField;
                }
            }
            ParserState::AfterEscapedCrnl => {
                if character.is_some() {
                    self.in_unquoted_field(character)?;
                }
            }
            ParserState::InField => self.in_unquoted_field(character)?,
            ParserState::InQuotedField => {
                let Some(character) = character else {
                    // 引号内的行尾 ⇒ 留给下一行
                    return Ok(());
                };
                if Some(character) == self.spec.escapechar {
                    self.state = ParserState::EscapeInQuotedField;
                } else if Some(character) == self.spec.quotechar && self.spec.quoting != QUOTE_NONE {
                    if self.spec.doublequote {
                        self.state = ParserState::QuoteInQuotedField;
                    } else {
                        self.state = ParserState::InField;
                    }
                } else {
                    self.add_char(character)?;
                }
            }
            ParserState::EscapeInQuotedField => {
                let value = character.unwrap_or('\n');
                self.add_char(value)?;
                self.state = ParserState::InQuotedField;
            }
            ParserState::QuoteInQuotedField => {
                let Some(character) = character else {
                    self.save_field()?;
                    self.state = ParserState::StartRecord;
                    return Ok(());
                };
                if self.spec.quoting != QUOTE_NONE && Some(character) == self.spec.quotechar {
                    // `""` ⇒ `"`（doublequote）
                    self.add_char(character)?;
                    self.state = ParserState::InQuotedField;
                } else if character == self.spec.delimiter {
                    self.save_field()?;
                    self.state = ParserState::StartField;
                } else if character == '\n' || character == '\r' {
                    self.save_field()?;
                    self.state = ParserState::EatCrnl;
                } else if !self.spec.strict {
                    self.add_char(character)?;
                    self.state = ParserState::InField;
                } else {
                    let quotechar = self.spec.quotechar.unwrap_or('"');
                    return Err(raise_csv_error(
                        self.instance,
                        &format!("'{}' expected after '{}'", self.spec.delimiter, quotechar),
                    ));
                }
            }
            ParserState::EatCrnl => {
                match character {
                    Some('\n') | Some('\r') => {
                        // 吃掉行尾
                    }
                    None => self.state = ParserState::StartRecord,
                    Some(_) => {
                        return Err(raise_csv_error(
                            self.instance,
                            "new-line character seen in unquoted field - do you need to open the file with newline=''?",
                        ));
                    }
                }
            }
            ParserState::StartRecord | ParserState::StartField => {
                unreachable!("START_RECORD／START_FIELD 已在上面处理")
            }
        }
        Ok(())
    }

    fn in_unquoted_field(&mut self, character: Option<char>) -> Result<(), ExecError> {
        let Some(character) = character else {
            self.save_field()?;
            self.state = ParserState::StartRecord;
            return Ok(());
        };
        if character == '\n' || character == '\r' {
            self.save_field()?;
            self.state = ParserState::EatCrnl;
        } else if Some(character) == self.spec.escapechar {
            self.state = ParserState::EscapedChar;
        } else if character == self.spec.delimiter {
            self.save_field()?;
            self.state = ParserState::StartField;
        } else {
            self.add_char(character)?;
        }
        Ok(())
    }
}

/// `reader.__next__()`：照 `Reader_iternext` 逐行取、逐字符喂状态机。
fn reader_next_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = this_of(bound, args) else {
        return Err(type_error(instance, "__next__() 缺少 self".to_owned()));
    };
    let input = load_required(instance, this, R_INPUT)?;
    let dialect_object = load_required(instance, this, R_DIALECT)?;
    let limits = field_limit(instance);
    let mut line_num = match load(instance, this, R_LINE_NUM) {
        Some(value) => {
            let number = instance.int_value(value).unwrap_or(0);
            instance.release(value);
            number
        }
        None => 0,
    };
    let spec = match spec_from_object(instance, dialect_object) {
        Ok(spec) => spec,
        Err(error) => {
            instance.release(input);
            instance.release(dialect_object);
            return Err(error);
        }
    };
    instance.release(dialect_object);
    let mut parser = Parser {
        instance,
        spec: &spec,
        field_limit: limits,
        fields: FieldList::new(instance),
        field: String::new(),
        state: ParserState::StartRecord,
        unquoted: false,
    };
    let outcome = reader_next_core(instance, input, this, &mut parser, &mut line_num);
    instance.release(input);
    store(instance, this, R_LINE_NUM, instance.new_int(line_num));
    match outcome {
        Ok(Some(fields)) => Ok(fields),
        Ok(None) => Err(instance.raise_builtin_error("StopIteration", "")),
        Err(error) => Err(error),
    }
}

/// `__next__` 的主体：返回 `Some(row)` ＝ 一行；`None` ＝ 输入耗尽。
fn reader_next_core(
    instance: &Instance,
    input: NonNull<Header>,
    this: NonNull<Header>,
    parser: &mut Parser<'_, '_>,
    line_num: &mut i64,
) -> Result<Option<NonNull<Header>>, ExecError> {
    loop {
        match instance.advance_iterator(input)? {
            None => {
                if !parser.field.is_empty() || parser.state == ParserState::InQuotedField {
                    if parser.spec.strict {
                        return Err(raise_csv_error(instance, "unexpected end of data"));
                    }
                    parser.save_field()?;
                } else {
                    return Ok(None);
                }
                break;
            }
            Some(line) => {
                let Some(text) = instance.text_of(line).map(<str>::to_owned) else {
                    if instance.is_str_object(line) {
                        instance.release(line);
                        return Err(ExecError::Unsupported {
                            opcode: 0,
                            what: "_csv.reader：含孤立代理的 str 行未接线（P3-38 S3b）",
                        });
                    }
                    let name = type_display_name(instance, line);
                    instance.release(line);
                    return Err(raise_csv_error(
                        instance,
                        &format!(
                            "iterator should return strings, not {name} (the file should be opened in text mode)"
                        ),
                    ));
                };
                instance.release(line);
                *line_num += 1;
                store(instance, this, R_LINE_NUM, instance.new_int(*line_num));
                for character in text.chars() {
                    parser.process_char(Some(character))?;
                }
                parser.process_char(None)?;
            }
        }
        if parser.state == ParserState::StartRecord {
            break;
        }
    }
    Ok(Some(parser.fields.take_from()))
}

// ---- `_csv.writer` ------------------------------------------------------------------

fn writer_type(instance: &Instance) -> NonNull<pyawa_core::TypeObject> {
    if let Some(existing) = instance.type_named("_csv.writer") {
        return existing;
    }
    let ty = instance.new_attribute_type("_csv.writer");
    for (name, handler) in [
        ("__init__", writer_disallow_instantiation_native as NativeFn),
        ("writerow", writer_writerow_native as NativeFn),
        ("writerows", writer_writerows_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.set_type_attribute(ty, name, function);
    }
    if let Some(property) = readonly_property(instance, writer_dialect_getter as NativeFn) {
        instance.set_type_attribute(ty, "dialect", property);
    }
    ty
}

fn writer_disallow_instantiation_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(type_error(
        instance,
        "cannot create '_csv.writer' instances".to_owned(),
    ))
}

fn writer_dialect_getter(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(this) = this_of(bound, args) else {
        return Err(instance.raise_builtin_error("AttributeError", "dialect"));
    };
    load_required(instance, this, R_DIALECT)
}

/// `_csv.writer(fileobj, dialect='excel', **fmtparams)`。
fn writer_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(type_error(
            instance,
            "writer expected at least 1 argument, got 0".to_owned(),
        ));
    }
    if args.len() > 2 {
        return Err(type_error(
            instance,
            format!(
                "writer expected at most 2 arguments, got {}",
                args.len()
            ),
        ));
    }
    // 参照顺序：先取 `write` 方法（取不到 ⇒ TypeError），再建方言。
    let write = match instance.attribute_optional_of(args[0], "write") {
        Ok(Some(write)) => write,
        Ok(None) => {
            return Err(type_error(
                instance,
                "argument 1 must have a \"write\" method".to_owned(),
            ))
        }
        Err(error) => return Err(error),
    };
    if !instance.is_callable(write) {
        instance.release(write);
        return Err(type_error(
            instance,
            "argument 1 must have a \"write\" method".to_owned(),
        ));
    }
    let dialect = match build_dialect(instance, args.get(1).copied(), kwargs) {
        Ok(dialect) => dialect,
        Err(error) => {
            instance.release(write);
            return Err(error);
        }
    };
    let writer = make_instance(instance, writer_type(instance));
    store(instance, writer, W_WRITE, write);
    store(instance, writer, R_DIALECT, dialect);
    Ok(writer)
}

/// `PyNumber_Check` 的本层近似：`int`／`bool`／大整数／`float`／`complex`（数值类型）。
///
/// **如实偏差**：参照按 `tp_as_number->nb_*` 槽判（`Decimal` 一类也算数值）；本层按
/// 内建数值类型判 ⇒ 自定义数值类的 `QUOTE_NONNUMERIC` 判定可能不同（记在 §5.2.33）。
fn is_number(instance: &Instance, value: NonNull<Header>) -> bool {
    let ty = instance.type_of(value);
    if instance.int_of(value).is_some() {
        return true;
    }
    if instance.float_value(value).is_some() {
        return true;
    }
    ["complex"].iter().any(|name| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(ty, base))
    })
}

/// 把一段字段文本转成本层可写的 `String`（`str` 直接取；`None` 给 `None`；其余 `str()`）。
fn field_text(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Option<String>, ExecError> {
    if let Some(text) = instance.text_of(value) {
        return Ok(Some(text.to_owned()));
    }
    if value == instance.singletons().none() {
        return Ok(None);
    }
    let str_type = instance
        .type_named("str")
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "str 未登记"))?;
    let type_value = instance.type_value(str_type);
    let outcome = pyawa_core::call_value(instance, type_value, &[value], &[]);
    instance.release(type_value);
    let converted = outcome?;
    let text = instance
        .text_of(converted)
        .map(<str>::to_owned)
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "str() 没返回 str"))?;
    instance.release(converted);
    Ok(Some(text))
}

/// 写侧记录长度（只需知道“非空”与拼接，用 `String` 即可）。
fn join_append(
    instance: &Instance,
    spec: &DialectSpec,
    record: &mut String,
    num_fields: &mut usize,
    field: Option<&str>,
    mut quoted: bool,
) -> Result<(), ExecError> {
    let empty = field.map_or(true, <str>::is_empty);
    if empty && spec.delimiter == ' ' && spec.skipinitialspace {
        if spec.quoting == QUOTE_NONE
            || (field.is_none()
                && (spec.quoting == QUOTE_STRINGS || spec.quoting == QUOTE_NOTNULL))
        {
            return Err(raise_csv_error(
                instance,
                "empty field must be quoted if delimiter is a space and skipinitialspace is true",
            ));
        }
        quoted = true;
    }
    let mut data = String::new();
    if let Some(text) = field {
        for character in text.chars() {
            // 照 `join_append_data` 的**逐字符**判定（顺序与分支都一致 ✓）。
            let special = character == spec.delimiter
                || Some(character) == spec.escapechar
                || Some(character) == spec.quotechar
                || character == '\n'
                || character == '\r'
                || spec.lineterminator.contains(character);
            if special {
                let mut want_escape = false;
                if spec.quoting == QUOTE_NONE {
                    want_escape = true;
                } else {
                    if Some(character) == spec.quotechar {
                        if spec.doublequote {
                            // `""`（doublequote）：先加一个引号，再由公共追加写第二个。
                            data.push(character);
                        } else {
                            want_escape = true;
                        }
                    } else if Some(character) == spec.escapechar {
                        want_escape = true;
                    }
                    if !want_escape {
                        quoted = true;
                    }
                }
                if want_escape {
                    let Some(escapechar) = spec.escapechar else {
                        return Err(raise_csv_error(
                            instance,
                            "need to escape, but no escapechar set",
                        ));
                    };
                    data.push(escapechar);
                }
            }
            data.push(character);
        }
    }
    if *num_fields > 0 {
        record.push(spec.delimiter);
    }
    if quoted {
        if let Some(quotechar) = spec.quotechar {
            record.push(quotechar);
        }
    }
    record.push_str(&data);
    if quoted {
        if let Some(quotechar) = spec.quotechar {
            record.push(quotechar);
        }
    }
    *num_fields += 1;
    Ok(())
}

/// 写一行（核心）：`iter` 是**已取好的行迭代器**（由调用方持有并归还）。
fn write_row(
    instance: &Instance,
    writer: NonNull<Header>,
    iter: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    let dialect_object = load_required(instance, writer, R_DIALECT)?;
    let spec = match spec_from_object(instance, dialect_object) {
        Ok(spec) => spec,
        Err(error) => {
            instance.release(dialect_object);
            return Err(error);
        }
    };
    instance.release(dialect_object);
    let write = load_required(instance, writer, W_WRITE)?;
    let mut guard = RefGuard::new(instance);
    guard.own(write);
    let mut record = String::new();
    let mut num_fields = 0usize;
    let mut null_field = false;
    loop {
        let Some(field) = instance.advance_iterator(iter)? else {
            break;
        };
        guard.own(field);
        let quoted = match spec.quoting {
            QUOTE_NONNUMERIC => !is_number(instance, field),
            QUOTE_ALL => true,
            QUOTE_STRINGS => instance.is_str_object(field),
            QUOTE_NOTNULL => field != instance.singletons().none(),
            _ => false,
        };
        null_field = field == instance.singletons().none();
        let text = field_text(instance, field)?;
        join_append(
            instance,
            &spec,
            &mut record,
            &mut num_fields,
            text.as_deref(),
            quoted,
        )?;
    }
    if num_fields > 0 && record.is_empty() {
        if spec.quoting == QUOTE_NONE
            || (null_field
                && (spec.quoting == QUOTE_STRINGS || spec.quoting == QUOTE_NOTNULL))
        {
            return Err(raise_csv_error(
                instance,
                "single empty field record must be quoted",
            ));
        }
        num_fields -= 1;
        join_append(instance, &spec, &mut record, &mut num_fields, None, true)?;
    }
    record.push_str(&spec.lineterminator);
    let line = instance.new_str(&record);
    let outcome = pyawa_core::call_value(instance, write, &[line], &[]);
    instance.release(line);
    outcome
}

/// `writer.writerow(row, /)`：不可迭代（`TypeError`）⇒ `Error: iterable expected, not <类型>`。
fn writer_writerow_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, positional)) = this_with_args(bound, args) else {
        return Err(type_error(instance, "writerow() 缺少 self".to_owned()));
    };
    let Some(row) = positional.first().copied() else {
        return Err(type_error(
            instance,
            "writerow() takes exactly one argument (0 given)".to_owned(),
        ));
    };
    if positional.len() > 1 {
        return Err(type_error(
            instance,
            format!(
                "writerow() takes exactly one argument ({} given)",
                positional.len()
            ),
        ));
    }
    let iter = iter_for_row(instance, row)?;
    let result = write_row(instance, this, iter);
    instance.release(iter);
    result
}

/// 取行迭代器：`TypeError` ⇒ 参照口径的 `Error: iterable expected, not <类型>`；
/// 其余异常（`OSError` 一类）原样上抛（`test_write_arg_valid` 钉住）。
fn iter_for_row(
    instance: &Instance,
    row: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    match instance.iter_object(row) {
        Ok(iter) => Ok(iter),
        Err(ExecError::Raised { exception }) => {
            if !exception_is(instance, exception, "TypeError") {
                return Err(ExecError::Raised { exception });
            }
            let name = type_display_name(instance, row);
            let error = ExecError::Raised { exception };
            swallow_raised(instance, &error);
            Err(raise_csv_error(
                instance,
                &format!("iterable expected, not {name}"),
            ))
        }
        Err(other) => Err(other),
    }
}

/// `writer.writerows(rows, /)`：照参照逐个 `writerow`，返回 `None`。
fn writer_writerows_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some((this, positional)) = this_with_args(bound, args) else {
        return Err(type_error(instance, "writerows() 缺少 self".to_owned()));
    };
    let Some(rows) = positional.first().copied() else {
        return Err(type_error(
            instance,
            "writerows() takes exactly one argument (0 given)".to_owned(),
        ));
    };
    if positional.len() > 1 {
        return Err(type_error(
            instance,
            format!(
                "writerows() takes exactly one argument ({} given)",
                positional.len()
            ),
        ));
    }
    let iter = instance.iter_object(rows)?;
    let mut guard = RefGuard::new(instance);
    guard.own(iter);
    loop {
        let Some(row) = instance.advance_iterator(iter)? else {
            break;
        };
        guard.own(row);
        let row_iter = iter_for_row(instance, row)?;
        guard.own(row_iter);
        let result = write_row(instance, this, row_iter)?;
        instance.release(result);
    }
    Ok(instance.new_none())
}

// ---- 方言注册表 ---------------------------------------------------------------------

/// `register_dialect(name, /, dialect='excel', **fmtparams)`。
fn register_dialect_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(type_error(
            instance,
            "register_dialect expected at least 1 argument, got 0".to_owned(),
        ));
    }
    if args.len() > 2 {
        return Err(type_error(
            instance,
            format!(
                "register_dialect expected at most 2 arguments, got {}",
                args.len()
            ),
        ));
    }
    let Some(name) = instance.text_of(args[0]).map(<str>::to_owned) else {
        return Err(type_error(
            instance,
            "dialect name must be a string".to_owned(),
        ));
    };
    let dialect = build_dialect(instance, args.get(1).copied(), kwargs)?;
    let dialects = match dialects_dict(instance) {
        Ok(dialects) => dialects,
        Err(error) => {
            instance.release(dialect);
            return Err(error);
        }
    };
    instance.dict_set(dialects, &name, dialect);
    instance.release(dialect);
    Ok(instance.new_none())
}

/// 取唯一位置实参（`get_dialect`／`unregister_dialect` 的 clinic 形参 `name: object`）。
fn single_name_argument(
    instance: &Instance,
    function: &str,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        let name = kwargs
            .first()
            .and_then(|(key, _)| instance.text_of(*key))
            .unwrap_or("?");
        return Err(type_error(
            instance,
            format!("{function}() takes no keyword arguments (got '{name}')"),
        ));
    }
    match args {
        [name] => Ok(*name),
        [] => Err(type_error(
            instance,
            format!("{function}() missing required argument 'name' (pos 1)"),
        )),
        _ => Err(type_error(
            instance,
            format!(
                "{function}() takes exactly one argument ({} given)",
                args.len()
            ),
        )),
    }
}

/// `get_dialect(name)`：查注册表；没有 ⇒ `Error: unknown dialect`。
fn get_dialect_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let name = single_name_argument(instance, "get_dialect", args, kwargs)?;
    match registry_lookup(instance, name)? {
        Some(dialect) => Ok(dialect),
        None => Err(raise_csv_error(instance, "unknown dialect")),
    }
}

/// `unregister_dialect(name)`：删注册表项；没有 ⇒ `Error: unknown dialect`。
fn unregister_dialect_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let name = single_name_argument(instance, "unregister_dialect", args, kwargs)?;
    if registry_delete(instance, name)? {
        Ok(instance.new_none())
    } else {
        Err(raise_csv_error(instance, "unknown dialect"))
    }
}

/// `list_dialects()`：`PyDict_Keys` 口径 ⇒ 插入序的 `list[str]`。
fn list_dialects_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(type_error(
            instance,
            "list_dialects() takes no keyword arguments".to_owned(),
        ));
    }
    if !args.is_empty() {
        return Err(type_error(
            instance,
            format!(
                "_csv.list_dialects() takes no arguments ({} given)",
                args.len()
            ),
        ));
    }
    let dialects = dialects_dict(instance)?;
    let keys: Vec<NonNull<Header>> = instance
        .dict_entries(dialects)
        .unwrap_or_default()
        .into_iter()
        .map(|(key, _)| instance.retain(key))
        .collect();
    Ok(instance.new_list(keys))
}

/// `field_size_limit(new_limit=...)`：返回旧值；不给就只查。`new_limit` 必须**恰好** `int`。
fn field_size_limit_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        let name = kwargs
            .first()
            .and_then(|(key, _)| instance.text_of(*key))
            .unwrap_or("?");
        return Err(type_error(
            instance,
            format!("field_size_limit() got an unexpected keyword argument '{name}'"),
        ));
    }
    if args.len() > 1 {
        return Err(type_error(
            instance,
            format!(
                "field_size_limit() takes at most 1 argument ({} given)",
                args.len()
            ),
        ));
    }
    let old_limit = field_limit(instance);
    if let Some(value) = args.first().copied() {
        let exact_int = instance.type_of(value) == instance.singletons().int_type();
        if !exact_int {
            return Err(type_error(instance, "limit must be an integer".to_owned()));
        }
        let Some(new_limit) = instance.int_value(value) else {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "Python int too large to convert to C ssize_t",
            ));
        };
        set_field_limit(instance, new_limit);
    }
    Ok(instance.new_int(old_limit))
}

// ---- 组装 ---------------------------------------------------------------------------

/// 建 `_csv` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    instance.release(module_name);
    let module_doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", module_doc);
    instance.release(module_doc);
    // `Error`：参照是 `Exception` 的**真子类**（`_csv.Error`，可被 `except` 接住），
    // 且**从 Python 构造时 `args`／`str` 都要对** ⇒ 走 `build_class_from_parts`（异常载荷路径；
    // `_socket.gaierror` 同款 ✓），不用 `new_subclass_with_instance_dict`（那是通用属性载荷 ⇒
    // `E("x")` 的 `args` 读不到 ✓ 实测过 ✗）。
    if let Some(exception_base) = instance.type_named("Exception") {
        let class_namespace = instance.new_dict();
        let base = instance.type_value(exception_base);
        let outcome = pyawa_core::build_class_from_parts(
            instance,
            "Error".to_owned(),
            vec![base],
            class_namespace,
            None,
        );
        instance.release(base);
        if let Ok(class) = outcome {
            let _ = pyawa_core::executor::protocol::instance_attribute_set(
                instance,
                class,
                "__module__",
                instance.new_str(NAME),
                0,
            );
            instance.dict_set(namespace, "Error", class);
            instance.release(class);
        }
    }
    // 三个类型（`Reader`／`Writer`／`Dialect`）。
    for (name, ty) in [
        ("Dialect", dialect_type(instance)),
        ("Reader", reader_type(instance)),
        ("Writer", writer_type(instance)),
    ] {
        let value = instance.type_value(ty);
        instance.dict_set(namespace, name, value);
        instance.release(value);
    }
    // 常量。
    for (name, value) in [
        ("QUOTE_MINIMAL", QUOTE_MINIMAL),
        ("QUOTE_ALL", QUOTE_ALL),
        ("QUOTE_NONNUMERIC", QUOTE_NONNUMERIC),
        ("QUOTE_NONE", QUOTE_NONE),
        ("QUOTE_STRINGS", QUOTE_STRINGS),
        ("QUOTE_NOTNULL", QUOTE_NOTNULL),
    ] {
        let constant = instance.new_int(value);
        instance.dict_set(namespace, name, constant);
        instance.release(constant);
    }
    // 函数面。
    for (name, handler) in [
        ("reader", reader_native as NativeFn),
        ("writer", writer_native as NativeFn),
        ("register_dialect", register_dialect_native as NativeFn),
        ("unregister_dialect", unregister_dialect_native as NativeFn),
        ("get_dialect", get_dialect_native as NativeFn),
        ("list_dialects", list_dialects_native as NativeFn),
        ("field_size_limit", field_size_limit_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
        instance.release(function);
    }
    // 注册表（**照参照暴露** `_csv._dialects`）。
    let dialects = instance.new_dict();
    instance.dict_set(namespace, KEY_DIALECTS, dialects);
    instance.release(dialects);
    // 字段大小上限（模块级状态；初始 128 KiB）。
    let default_limit = instance.new_int(DEFAULT_FIELD_LIMIT);
    instance.dict_set(namespace, KEY_FIELD_LIMIT, default_limit);
    instance.release(default_limit);
    namespace
}
