//! **`builtins.compile()`**（task-43）：把源码编成本层的 **code 对象**（可交给 `exec()`／`eval()`）。
//!
//! 参照签名（3.14.4）：`compile(source, filename, mode, flags=0, dont_inherit=False, optimize=-1)`。
//!
//! **最小诚实面**（如实登记的偏差见下）：
//! - `source`：`str` ✓；`bytes` 按 UTF-8 解码 ✓（参照按编码声明；本层无编码探测 ⇒ 只认 UTF-8）；
//!   AST 对象**未实现** ⇒ 诚实 `NotImplementedError`；
//! - `mode`：`"exec"`／`"eval"`／`"single"` ✓（`eval` 用"赋值到一个内部名字"包一层，与
//!   [`crate::Instance::eval_source`] 同一手法；`single` 按 `exec` 处理 ✗ 如实偏差：不打印交互式 repr）；
//! - `flags`：只接 `0` ✓（其余位如实 `NotImplementedError`）；
//! - `dont_inherit`／`optimize`：接受并**暂不参与发射**（本层没有优化器／特性继承）✓；
//! - 返回的 code 对象带**给定 filename** ✓（`co_filename`）、`co_name` 由编译单元给 ✓；
//! - 语法错 ⇒ `SyntaxError`（消息是本层编译器的文本；**未**对齐参照的 msg/offset 元组 ✗）。
//!
//! 与 `exec()`／`eval()` 的接口：`exec_code_public`／`eval_code_public`（stdlib 的 `exec`／`eval`
//! 内置在第一个实参是 code 对象时调它们 ✓）。

use core::ptr::NonNull;

use crate::CodeObject;
use crate::executor::ExecError;
use crate::header::Header;
use crate::instance::Instance;
use crate::refcount::Owned;

/// `eval` 模式的 code 对象把结果放在这个内部名字上（与 [`Instance::eval_source`] 同一手法 ✓；
/// 名字带前缀，避免和用户命名空间撞车 ✓）。
const EVAL_RESULT_NAME: &str = "__pyawa_compile_eval_result__";

/// 这个值是不是本层的 **code 对象**（stdlib 的 `exec`／`eval` 用它分流 ✓；不碰载荷布局 ✓）。
pub fn is_code_object(instance: &Instance, value: NonNull<Header>) -> bool {
    instance
        .type_named("CodeObject")
        .is_some_and(|code_type| instance.type_of(value) == code_type)
}

/// 把编译器的 [`crate::compile::CompileError`] 变成可读消息（`SyntaxError` 的文本）。
fn compile_error_text(error: &crate::compile::CompileError) -> String {
    match error {
        crate::compile::CompileError::Syntax(message) => message.clone(),
        crate::compile::CompileError::Unsupported(message) => message.clone(),
        crate::compile::CompileError::TypeAssignment(message) => message.clone(),
    }
}

/// **把编译器的错误变成参照形状的 `SyntaxError`**（task-43）✓。
///
/// 参照 3.14.4 实测：`compile("x =", "<f>", "exec")` 的异常是
/// `SyntaxError('invalid syntax', ('<f>', 1, 4, 'x =\n', 1, 5))`，并有
/// `.msg`／`.filename`／`.lineno`／`.offset`／`.text`／`.end_lineno`／`.end_offset` 属性 ✓
/// —— `traceback` 的 SyntaxError 格式化与 `test_decorators.test_errors` 都要它们 ✓
///（先前只抛裸消息 ⇒ 运行器格式化时 `'SyntaxError' object has no attribute 'filename'` ✗）。
///
/// **如实偏差**：行/列是从本层编译器的消息里**尽力**解析的（消息文本尚未与参照逐字对齐）✗；
/// `text` 取源码对应那一行 ✓。
fn syntax_error(
    instance: &Instance,
    filename: &str,
    source: &str,
    message: &str,
) -> ExecError {
    // 尽力从消息里取行号／列号（本层消息形如「…（第 1 行，列 2-2）」）。
    let mut lineno = 1i64;
    let mut offset = 0i64;
    let mut end_offset = 0i64;
    if let Some(position) = message.find("第 ") {
        let rest = &message[position + "第 ".len()..];
        let digits: String = rest.chars().take_while(|character| character.is_ascii_digit()).collect();
        if let Ok(value) = digits.parse::<i64>() {
            lineno = value;
        }
    }
    if let Some(position) = message.find("列 ") {
        let rest = &message[position + "列 ".len()..];
        let digits: String = rest.chars().take_while(|character| character.is_ascii_digit()).collect();
        if let Ok(value) = digits.parse::<i64>() {
            offset = value + 1;
        }
        if let Some(dash) = rest.find('-') {
            let trailing: String = rest[dash + 1..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect();
            if let Ok(value) = trailing.parse::<i64>() {
                end_offset = value + 1;
            }
        }
    }
    let text = source
        .lines()
        .nth((lineno.max(1) - 1) as usize)
        .unwrap_or("");
    let message_value = instance.new_str(message);
    let filename_value = instance.new_str(filename);
    let text_value = instance.new_str(text);
    let lineno_value = instance.new_int(lineno);
    let offset_value = instance.new_int(offset);
    let end_lineno_value = instance.new_int(lineno);
    let end_offset_value = instance.new_int(end_offset);
    let location = instance.new_tuple(vec![
        instance.retain(filename_value),
        instance.retain(lineno_value),
        instance.retain(offset_value),
        instance.retain(text_value),
        instance.retain(end_lineno_value),
        instance.retain(end_offset_value),
    ]);
    let Some(syntax_type) = instance.type_named("SyntaxError") else {
        return instance.raise_builtin_error("SyntaxError", message);
    };
    let exception = crate::executor::format::new_exception_with_args(
        instance,
        syntax_type,
        vec![instance.retain(message_value), location],
    );
    // 参照的 SyntaxError 属性面（`traceback`／`test_decorators` 读它们 ✓）。
    for (name, value) in [
        ("msg", message_value),
        ("filename", filename_value),
        ("lineno", lineno_value),
        ("offset", offset_value),
        ("text", text_value),
        ("end_lineno", end_lineno_value),
        ("end_offset", end_offset_value),
    ] {
        let _ = instance.set_attribute_value(exception, name, value);
        instance.release(value);
    }
    ExecError::Raised { exception }
}

/// 从"测试路径"的 code 模板复制出一个**带给定 filename** 的 code 对象 ✓。
///
/// 为什么要复制：`compile::instantiate` 的 `co_filename` 固定成 `<pyawa-test>`（测试路径的口径）；
/// `compile()` 要给**调用者给的 filename** ✓。常量／positions／异常表逐项搬（常量各 retain 一份 ✓）。
fn code_with_filename(
    instance: &Instance,
    template: &Owned<'_, CodeObject>,
    filename: &str,
) -> NonNull<Header> {
    let source = template.get();
    let code_type = instance
        .type_named("CodeObject")
        .expect("CodeObject 在引导期已登记");
    let mut names: Vec<String> = Vec::new();
    let mut index = 0usize;
    while let Some(name) = source.name_at(index) {
        names.push(name.to_owned());
        index += 1;
    }
    let mut varnames: Vec<String> = Vec::new();
    index = 0;
    while let Some(name) = source.varname(index) {
        varnames.push(name.to_owned());
        index += 1;
    }
    let mut constants: Vec<Option<NonNull<Header>>> = Vec::with_capacity(source.const_count());
    for slot in 0..source.const_count() {
        match source.constant(slot) {
            Some(value) => {
                // SAFETY: 常量由模板持有，存活；新对象要自己那份。
                unsafe { instance.incref_object(value.as_ptr()) };
                constants.push(Some(value));
            }
            None => constants.push(None),
        }
    }
    let code = instance.alloc(CodeObject::new(
        code_type,
        source.name(),
        source.qualname().to_owned(),
        filename.to_owned(),
        source.firstlineno(),
        source.stacksize(),
        source.nlocals(),
        source.argcount(),
        source.posonlyargcount(),
        source.kwonlyargcount(),
        source.flags(),
        varnames,
        names,
        source.cellvars().to_vec(),
        source.freevars().to_vec(),
        source.code().to_vec(),
        source.exceptiontable().to_vec(),
        constants,
        source.positions().to_vec(),
    ));
    code.into_raw().cast::<Header>()
}

/// 造一个执行 `code` 的帧并跑起来（`globals` 与 `locals` 不同时分别装 ✓）。
fn execute_code(
    instance: &Instance,
    code: NonNull<Header>,
    globals: Option<NonNull<Header>>,
    locals: NonNull<Header>,
) -> Result<(), ExecError> {
    if !is_code_object(instance, code) {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "exec／eval：第一个实参不是本层的 code 对象",
        });
    }
    let frame_type = instance.type_named("frame").ok_or(ExecError::Unsupported {
        opcode: 0,
        what: "exec／eval：`frame` 类型未登记",
    })?;
    // 帧接手一份新引用 ⇒ 这里给 `Owned` 一份（其 Drop 归还 ✓）。
    // SAFETY: code 由调用方保证存活；guard 持有这份新引用。
    unsafe { instance.incref_object(code.as_ptr()) };
    let code_owned: Owned<'_, CodeObject> = Owned::new(code.cast::<CodeObject>(), instance);
    // SAFETY: locals 由调用方保证存活；帧接手一份新引用。
    unsafe { instance.incref_object(locals.as_ptr()) };
    let frame = crate::frame::Frame::for_code_with_namespace(frame_type, &code_owned, locals);
    if let Some(globals) = globals {
        if globals != locals {
            // SAFETY: globals 由调用方保证存活；`set_globals` 接手新引用。
            unsafe { instance.incref_object(globals.as_ptr()) };
            frame.set_globals(globals);
        }
    }
    let frame = instance.alloc(frame);
    crate::execute(instance, &frame).map(|_| ())
}

/// **`exec(code_obj, globals=None, locals=None)` 的核心面**（stdlib 的 `exec` 内置调它 ✓）。
pub fn exec_code_public(
    instance: &Instance,
    code: NonNull<Header>,
    globals: Option<NonNull<Header>>,
    locals: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    execute_code(instance, code, globals, locals)?;
    Ok(instance.retain(instance.singletons().none()))
}

/// **`eval(code_obj, globals=None, locals=None)` 的核心面**：执行 `compile(…, "eval")` 产出的
/// code 对象并取回结果 ✓（内部名字契约见 [`EVAL_RESULT_NAME`] ✓）。
pub fn eval_code_public(
    instance: &Instance,
    code: NonNull<Header>,
    globals: Option<NonNull<Header>>,
    locals: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    execute_code(instance, code, globals, locals)?;
    // 结果落在**帧的命名空间**里（模块帧 ＝ `locals` ✓）——先查 `locals`，再兜底 `globals` ✓
    //（`eval(expr)` 的隐式命名空间：globals 是函数的 `__globals__`、locals 是 `f_locals` 快照 ✗
    // ⇒ 先查 globals 会拿不到结果 ✗，`test_decorators.test_dbcheck` 实测踩到 ✓）。
    let found = instance
        .dict_get(locals, EVAL_RESULT_NAME)
        .or_else(|| globals.and_then(|globals| instance.dict_get(globals, EVAL_RESULT_NAME)));
    match found {
        Some(value) => Ok(instance.retain(value)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

/// **调用者帧的隐式命名空间**（`eval(src)`／`exec(src)` 无命名空间时用 ✓；task-43 顺带补齐）：
/// `globals` ＝ 当前帧的有效全局映射（**新引用**，可能 `None` ✓）；`locals` ＝ 当前帧的
/// `f_locals` 快照（**新引用**，一定是 `dict` ✓）。没有当前帧 ⇒ 只给一个空 `dict` ✓。
pub fn implicit_namespaces(instance: &Instance) -> (Option<NonNull<Header>>, NonNull<Header>) {
    match instance.current_frame() {
        Some(frame) => {
            // SAFETY: 当前帧由执行器守卫持有，存活。
            let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() };
            let globals = frame_ref.effective_globals().map(|value| instance.retain(value));
            (globals, frame_ref.locals_snapshot(instance))
        }
        None => (None, instance.new_dict()),
    }
}

/// **`eval(源码, …)` 的核心面**（stdlib 的 `eval` 内置在 sources 是 str 时用 ✓）：
/// 与 [`crate::Instance::eval_source`] 同一手法（赋值到内部名字再取回 ✓），但允许
/// `globals`／`locals` 分开给 ✓。
pub fn eval_source_public(
    instance: &Instance,
    source: &str,
    globals: Option<NonNull<Header>>,
    locals: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    let wrapped = format!("{EVAL_RESULT_NAME} = (\n{source}\n)\n");
    let unit = crate::compile::compile(
        &wrapped,
        "<eval>",
        crate::compile::Mode::PurePython,
        crate::compile::CheckTier::Shallow,
        0,
    )
    .map_err(|error| {
        instance.raise_builtin_error("SyntaxError", &compile_error_text(&error))
    })?;
    let template = crate::compile::instantiate(instance, &unit);
    let code = code_with_filename(instance, &template, "<eval>");
    execute_code(instance, code, globals, locals)?;
    // SAFETY: code 是 code_with_filename 给的新引用；用完归还。
    unsafe { instance.release_object(code.as_ptr()) };
    let namespace = globals.or(Some(locals)).expect("locals 一定在");
    match instance.dict_get(namespace, EVAL_RESULT_NAME) {
        Some(value) => Ok(instance.retain(value)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

/// **`builtins.compile()`** ✓（task-43）。参数装配：前 6 个名字位置／关键字都认 ✓。
pub fn compile_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    const NAMES: [&str; 6] = [
        "source",
        "filename",
        "mode",
        "flags",
        "dont_inherit",
        "optimize",
    ];
    if args.len() > NAMES.len() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "compile() takes at most {} arguments ({} given)",
                NAMES.len(),
                args.len()
            ),
        ));
    }
    let mut values: [Option<NonNull<Header>>; NAMES.len()] = [None; NAMES.len()];
    for (index, value) in args.iter().enumerate() {
        values[index] = Some(*value);
    }
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else {
            continue;
        };
        let Some(position) = NAMES.iter().position(|candidate| *candidate == name) else {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("compile() got an unexpected keyword argument '{name}'"),
            ));
        };
        if values[position].is_some() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("compile() got multiple values for argument '{name}'"),
            ));
        }
        values[position] = Some(*value);
    }
    let Some(source_value) = values[0] else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compile() missing required argument 'source' (pos 1)",
        ));
    };
    let Some(filename_value) = values[1] else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compile() missing required argument 'filename' (pos 2)",
        ));
    };
    let Some(mode_value) = values[2] else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compile() missing required argument 'mode' (pos 3)",
        ));
    };
    // source：str ✓／bytes 按 UTF-8 ✓／AST 对象如实未实现。
    let source = if let Some(text) = instance.text_of(source_value) {
        text.to_owned()
    } else if let Some(bytes) = instance.bytes_value(source_value) {
        match core::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(_) => {
                return Err(instance.raise_builtin_error(
                    "SyntaxError",
                    "compile(): bytes 源码不是合法 UTF-8（本层只认 UTF-8，无编码探测）",
                ))
            }
        }
    } else {
        let type_name = instance.type_name(instance.type_of(source_value));
        if type_name.contains("AST") || instance.type_name(instance.type_of(source_value)).is_empty() {
            return Err(ExecError::Unsupported {
                opcode: 0,
                what: "compile()：AST 对象作为 source 尚未接线",
            });
        }
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compile() arg 1 must be a string, bytes or AST object",
        ));
    };
    let Some(filename) = instance.text_of(filename_value).map(str::to_owned) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compile() arg 2 must be a string",
        ));
    };
    let Some(mode) = instance.text_of(mode_value).map(str::to_owned) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "compile() arg 3 must be a string",
        ));
    };
    if !matches!(mode.as_str(), "exec" | "eval" | "single") {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "compile() mode must be 'exec', 'eval' or 'single'",
        ));
    }
    // flags：只接 0（`PyCF_ONLY_AST` 等如实未实现）。
    let flags = match values[3] {
        None => 0i64,
        Some(value) => instance.int_of(value).and_then(|integer| integer.to_i64()).ok_or_else(|| {
            instance.raise_builtin_error("TypeError", "compile() arg 4（flags）要整数")
        })?,
    };
    // `flags`：只认 `PyCF_ONLY_AST`（0x400）／`PyCF_ALLOW_TOP_LEVEL_AWAIT`（0x2000）这两个**已知**位；
    // 其它位 ⇒ 参照的 `ValueError: compile(): unrecognised flags` ✓（实测）；已知位本身尚未接线 ⇒
    // 诚实 `NotImplementedError`（AST 面 / 顶层 await）。
    const PYCF_ONLY_AST: i64 = 0x400;
    const PYCF_ALLOW_TOP_LEVEL_AWAIT: i64 = 0x2000;
    let unknown = flags & !(PYCF_ONLY_AST | PYCF_ALLOW_TOP_LEVEL_AWAIT);
    if unknown != 0 {
        return Err(instance.raise_builtin_error("ValueError", "compile(): unrecognised flags"));
    }
    if flags & PYCF_ONLY_AST != 0 {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "compile()：`PyCF_ONLY_AST`（返回 AST 对象）尚未接线",
        });
    }
    if flags & PYCF_ALLOW_TOP_LEVEL_AWAIT != 0 {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "compile()：`PyCF_ALLOW_TOP_LEVEL_AWAIT` 尚未接线",
        });
    }
    // optimize：接受 `-1`（默认）／0..2；本层**没有优化器** ⇒ 暂不参与发射 ✓（不影响产物语义）。
    let optimize = match values[5] {
        None => -1i64,
        Some(value) => instance.int_of(value).and_then(|integer| integer.to_i64()).ok_or_else(|| {
            instance.raise_builtin_error("TypeError", "compile() arg 6（optimize）要整数")
        })?,
    };
    let optimization = u8::try_from(optimize.clamp(0, 2)).unwrap_or(0);
    // `eval` 模式：赋值到内部名字（与 `Instance::eval_source` 同一手法 ✓）。
    let wrapped = if mode == "eval" {
        format!("{EVAL_RESULT_NAME} = (\n{source}\n)\n")
    } else {
        source
    };
    let unit = crate::compile::compile(
        &wrapped,
        &filename,
        crate::compile::Mode::PurePython,
        crate::compile::CheckTier::Shallow,
        optimization,
    )
    .map_err(|error| syntax_error(instance, &filename, &wrapped, &compile_error_text(&error)))?;
    let template = crate::compile::instantiate(instance, &unit);
    Ok(code_with_filename(instance, &template, &filename))
}
