//! `_opcode`／`_opcode_metadata` 的 **Python 层包装**（`BC-38`、`SPEC-c-modules.md` §5.2.5）。
//!
//! 指令表的数据与纯函数**归属 `pyawa-core`**——指令集是 VM 的一部分；
//! 依赖方向 `pyawa-stdlib → pyawa-core`（`REQUIREMENTS.md` 的 crate 依赖边行）。
//!
//! 本模块是这两个 Python 模块的挂载点：**只转发，不复制任何数值**——复制会造出第二个真相源
//! （`BC-38`）。导出的符号清单照 `SPEC-bytecode.md` §2.1／§2.2 的"必须导出"表；用法错误的
//! 消息**逐条实测**（见各 native 的注释），未实测的错误路径在契约里如实标注。
//!
//! `_specializations` 与 `_specialized_opmap` **必须为空 dict**（`BC-32`：Pyawa 不做 CPython 式
//! 特化）——**别把参照实现的值当期望**（它非空，实测 17／84 项）。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance};

pub use pyawa_core::opcode::*;
pub use pyawa_core::opcode_metadata::*;

/// `_opcode` 的 `__doc__`（与参照同源）。
pub const OPCODE_DOC: &str = "Opcode support module.";

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
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

/// 取出**唯一**一个位置实参并当整数用；错误消息照参照实测的三种形态。
///
/// 实测（`_opcode.has_arg` 一族）：缺参 ⇒ `has_arg() missing required argument 'opcode' (pos 1)`；
/// 多参 ⇒ `has_const() takes at most 1 argument (2 given)`；
/// 非整数 ⇒ `'str' object cannot be interpreted as an integer`。
fn single_int_argument(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<i64, ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() missing required argument 'opcode' (pos 1)"),
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{name}() takes at most 1 argument ({} given)",
                args.len()
            ),
        ));
    }
    instance.int_value(args[0]).ok_or_else(|| {
        let type_name = instance.type_name(instance.type_of(args[0]));
        instance.raise_builtin_error(
            "TypeError",
            &format!("'{type_name}' object cannot be interpreted as an integer"),
        )
    })
}

/// 一个 `has_*(op)` 谓词。
fn predicate_native(
    instance: &Instance,
    name: &'static str,
    args: &[NonNull<Header>],
    predicate: fn(u16) -> bool,
) -> Result<NonNull<Header>, ExecError> {
    let value = single_int_argument(instance, name, args)?;
    let opcode = u16::try_from(value).map_err(|_| {
        instance.raise_builtin_error("ValueError", "invalid opcode")
    })?;
    Ok(instance.new_bool(predicate(opcode)))
}

macro_rules! predicate {
    ($rust_name:ident, $py_name:literal, $function:path) => {
        fn $rust_name(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            predicate_native(instance, $py_name, args, $function)
        }
    };
}

predicate!(has_arg_native, "has_arg", pyawa_core::opcode::has_arg);
predicate!(has_const_native, "has_const", pyawa_core::opcode::has_const);
predicate!(has_name_native, "has_name", pyawa_core::opcode::has_name);
predicate!(has_jump_native, "has_jump", pyawa_core::opcode::has_jump);
predicate!(has_free_native, "has_free", pyawa_core::opcode::has_free);
predicate!(has_local_native, "has_local", pyawa_core::opcode::has_local);
predicate!(has_exc_native, "has_exc", pyawa_core::opcode::has_exc);

/// `_opcode.is_valid(op)`（参照有这一项，`SPEC-bytecode.md` §2.1 没列 ⇒ 本层也导出，
/// 语义＝"在 `opmap` 的取值范围内"）。
fn is_valid_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let value = single_int_argument(instance, "is_valid", args)?;
    let valid = u16::try_from(value)
        .map(|opcode| pyawa_core::opcode::opname(opcode).is_some())
        .unwrap_or(false);
    Ok(instance.new_bool(valid))
}

/// 一个返回**列表**的 getter（元素由 `build` 给定）。
fn list_native(
    instance: &Instance,
    name: &'static str,
    args: &[NonNull<Header>],
    build: fn(&Instance) -> Vec<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes no arguments ({} given)", args.len()),
        ));
    }
    Ok(instance.new_list(build(instance)))
}

/// `get_intrinsic1_descs()` 的每一项（实测：返回 `list`，元素是 `str`）。
fn get_intrinsic1_descs_fn(instance: &Instance) -> Vec<NonNull<Header>> {
    pyawa_core::opcode::get_intrinsic1_descs()
        .iter()
        .map(|desc| instance.new_str(desc))
        .collect()
}

/// `get_intrinsic2_descs()` 的每一项。
fn get_intrinsic2_descs_fn(instance: &Instance) -> Vec<NonNull<Header>> {
    pyawa_core::opcode::get_intrinsic2_descs()
        .iter()
        .map(|desc| instance.new_str(desc))
        .collect()
}

/// `get_special_method_names()` 的每一项。
fn get_special_method_names_fn(instance: &Instance) -> Vec<NonNull<Header>> {
    pyawa_core::opcode::get_special_method_names()
        .iter()
        .map(|name| instance.new_str(name))
        .collect()
}

/// `get_nb_ops()` 的每一项（实测：返回 `list`，元素是 `(NB_名字, 符号)` **二元组**）。
fn get_nb_ops_fn(instance: &Instance) -> Vec<NonNull<Header>> {
    pyawa_core::opcode::get_nb_ops()
        .iter()
        .map(|(name, symbol)| {
            let left = instance.new_str(name);
            let right = instance.new_str(symbol);
            instance.new_tuple(vec![left, right])
        })
        .collect()
}

fn intrinsic1_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    list_native(instance, "get_intrinsic1_descs", args, get_intrinsic1_descs_fn)
}

fn intrinsic2_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    list_native(instance, "get_intrinsic2_descs", args, get_intrinsic2_descs_fn)
}

fn special_method_names_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    list_native(instance, "get_special_method_names", args, get_special_method_names_fn)
}

fn nb_ops_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    list_native(instance, "get_nb_ops", args, get_nb_ops_fn)
}

/// `get_executor(code, offset)`：Pyawa 无 JIT ⇒ 恒 `None`（`SPEC-bytecode.md` §2.1 明文允许）。
fn get_executor_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// `stack_effect(opcode, oparg=None, *, jump=None)`。
///
/// 实测：0 个位置实参 ⇒ `stack_effect() takes at least 1 positional argument (0 given)`；
/// 3 个 ⇒ `stack_effect() takes at most 2 positional arguments (3 given)`（`jump` 是**仅关键字**）；
/// 越界 ⇒ `ValueError: invalid opcode or oparg`。
fn stack_effect_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "stack_effect() takes at least 1 positional argument (0 given)",
        ));
    }
    if args.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "stack_effect() takes at most 2 positional arguments ({} given)",
                args.len()
            ),
        ));
    }
    let mut jump = None;
    for (key, value) in kwargs {
        let key = instance.text_value(*key).unwrap_or_default();
        if key != "jump" {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("stack_effect() got an unexpected keyword argument '{key}'"),
            ));
        }
        if jump.is_some() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "stack_effect() got multiple values for keyword argument 'jump'",
            ));
        }
        jump = instance.bool_value(*value);
    }
    let opcode = instance.int_value(args[0]).and_then(|value| u16::try_from(value).ok());
    let oparg = match args.get(1) {
        Some(value) => match instance.int_value(*value) {
            Some(number) => Some(number),
            None => None,
        },
        None => None,
    };
    let Some(opcode) = opcode else {
        return Err(instance.raise_builtin_error("ValueError", "invalid opcode or oparg"));
    };
    match pyawa_core::opcode::stack_effect(opcode, oparg, jump) {
        Ok(effect) => Ok(instance.new_int(i64::from(effect))),
        Err(_) => Err(instance.raise_builtin_error("ValueError", "invalid opcode or oparg")),
    }
}

/// 建 `_opcode` 的命名空间（**新引用** 的 `dict`）。
pub fn build_opcode(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("stack_effect", stack_effect_native as pyawa_core::NativeFn),
        ("has_arg", has_arg_native as pyawa_core::NativeFn),
        ("has_const", has_const_native as pyawa_core::NativeFn),
        ("has_name", has_name_native as pyawa_core::NativeFn),
        ("has_jump", has_jump_native as pyawa_core::NativeFn),
        ("has_free", has_free_native as pyawa_core::NativeFn),
        ("has_local", has_local_native as pyawa_core::NativeFn),
        ("has_exc", has_exc_native as pyawa_core::NativeFn),
        ("is_valid", is_valid_native as pyawa_core::NativeFn),
        (
            "get_intrinsic1_descs",
            intrinsic1_native as pyawa_core::NativeFn,
        ),
        (
            "get_intrinsic2_descs",
            intrinsic2_native as pyawa_core::NativeFn,
        ),
        (
            "get_special_method_names",
            special_method_names_native as pyawa_core::NativeFn,
        ),
        ("get_nb_ops", nb_ops_native as pyawa_core::NativeFn),
        ("get_executor", get_executor_native as pyawa_core::NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    let module_name = instance.new_str("_opcode");
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(OPCODE_DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

/// 建 `_opcode_metadata` 的命名空间（**新引用** 的 `dict`）。
pub fn build_opcode_metadata(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // `opmap`：名字 → 编号（转发 `pyawa-core` 的探测表，**不复制数值**）
    let opmap = instance.new_dict();
    for (name, number) in pyawa_core::opcode_metadata::OPMAP {
        let value = instance.new_int(i64::from(*number));
        // 键是 `&'static str`（探测表里的原样），`dict_set` 自己另存一份
        instance.dict_set(opmap, name, value);
    }
    instance.dict_set(namespace, "opmap", opmap);
    // `BC-32`：Pyawa **不做** CPython 式特化 ⇒ 这两个表必须为空
    let specializations = instance.new_dict();
    instance.dict_set(namespace, "_specializations", specializations);
    let specialized_opmap = instance.new_dict();
    instance.dict_set(namespace, "_specialized_opmap", specialized_opmap);
    let have_argument = instance.new_int(i64::from(
        pyawa_core::opcode_metadata::HAVE_ARGUMENT,
    ));
    instance.dict_set(namespace, "HAVE_ARGUMENT", have_argument);
    let min_instrumented = instance.new_int(i64::from(
        pyawa_core::opcode_metadata::MIN_INSTRUMENTED_OPCODE,
    ));
    instance.dict_set(namespace, "MIN_INSTRUMENTED_OPCODE", min_instrumented);
    let module_name = instance.new_str("_opcode_metadata");
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}
