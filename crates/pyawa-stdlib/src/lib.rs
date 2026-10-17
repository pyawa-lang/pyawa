//! Pyawa 的 C 层模块实现：从 Python 看到的 API 与语义。
//!
//! 逐模块合约归属 `docs/SPEC-c-modules.md`（`CM-`），见本 crate 的 `README.md`。
//!
//! **指令表的数据与纯函数归属 `pyawa-core`**（`BC-38` 的依赖边裁决：指令集是 VM 的一部分）。
//! 本 crate 只做 Python 层包装，见 [`opcode`]。

#![forbid(unsafe_code)]

use pyawa_core::AttributeObject;

pub mod _io_module;
pub mod binascii_module;
pub mod builtins_module;
pub mod errno_map;
pub mod errno_module;
/// `operator`（契约 `docs/SPEC-c-modules.md` §5.2.7；本层第一刀：`eq`／`ne`／`is_`／`is_not`／`truth`／`not_`）
pub mod operator_module;
pub mod opcode;
/// `_random`（第 722 轮；`Lib/random.py` 的原生基类 —— xoshiro256\*\*，差异见模块头）
pub mod random_module;
/// `sys`（不依赖能力域的部分；契约 `docs/SPEC-c-modules.md` §5.2.3）
pub mod _sre_module;
pub mod _typing_module;
pub mod atexit_module;
pub mod sys_module;

/// `itertools`（契约 `docs/SPEC-c-modules.md` §5.2.6；本层已落地 **18** 个：`count`／`repeat`／
/// `islice`／`chain`／`takewhile`／`dropwhile`／`filterfalse`／`accumulate`／`starmap`／`cycle`／
/// `pairwise`／`batched`／`zip_longest`／`compress`／`combinations`／`permutations`／
/// `combinations_with_replacement`／`product`；参照 20 个公开名，剩 `groupby`／`tee`／
/// `chain.from_iterable`）
pub mod itertools_module;
pub mod marshal_module;

/// `_imp`（契约 `docs/SPEC-c-modules.md` §5.2.4；本层落地 `pyc_magic_number_token` 与 `is_builtin`，
/// 其余逐条记在 §5.2.4 的"未落地"）
pub mod imp_module;
/// `_ast`（契约归属见 `docs/SPEC-c-modules.md`；本层=**名字齐、调用报未接线**）
pub mod ast_module;
/// **`_socket`**（`P3-29` socket 战役第一轮）：常量/`has_ipv6`/类与异常面为真；
/// 一切 I/O 函数面如实报 `NotImplementedError`（能力域 `CM-8` 未接）——见模块头。
pub mod _socket_module;
pub mod io_module;
pub mod warnings_module;

/// **`sys.platform` 的值** ✓（第 194 轮）：本 crate 不碰平台（`CX-4` ✓）⇒ 先给常量 ✓，
/// 真值应由平台集中点（`pyawa-runtime` ✓）注入 ✓ —— 已登记 ✓。
pub const PLATFORM: &str = "linux";

/// **内建模块表**（`CM-14` 的组合根 ✓、**一处真相** ✓）。
///
/// `sys.builtin_module_names` **由它派生** ✓（第 403 轮）：先前两张表各写一份 ✗ ⇒ 新加 `_ast` 时
/// 只改了这里、没同步 `sys` 那张 ✗ ⇒ `_bootstrap._setup` 按旧表给 `_ast` 建了**非内建**的 spec ✓
/// ⇒ 上限诊断反而 **-1** ✗（这正是"表错一行，import 链就断"的又一例 ✓）。
pub(crate) const RUST_MODULES: &[(&str, ModuleBuilder)] = &[
        (imp_module::NAME, imp_module::build),
        (atexit_module::NAME, atexit_module::build),
        (_typing_module::NAME, _typing_module::build),
        (ast_module::NAME, ast_module::build),
        (io_module::NAME, io_module::build),
        (warnings_module::NAME, warnings_module::build),
        (posix_module::NAME, posix_module::build),
        (weakref_module::NAME, weakref_module::build),
        (thread_module::NAME, thread_module::build),
        (codecs_module::NAME, codecs_module::build),
        (collections_module::NAME, collections_module::build),
        (tokenize_module::NAME, tokenize_module::build),
        (math_module::NAME, math_module::build),
        (contextvars_module::NAME, contextvars_module::build),
        (itertools_module::NAME, itertools_module::build),
        (marshal_module::NAME, marshal_module::build),
        // **`_socket`**（`P3-29` 第一轮 ✓）：`Lib/socket.py` 的 `import _socket` ＋
        // `from _socket import *` ＋ `class socket(_socket.socket)` 的底座 ✓。
        // 常量/`has_ipv6`/类与异常面为真；I/O 函数面如实报 `NotImplementedError`（能力域未接 ✓）。
        (_socket_module::NAME, _socket_module::build),
        (operator_module::NAME, operator_module::build),
        // **`_operator`**（第 406 轮 ✓）：`hmac.py` 的 `from _operator import _compare_digest` ✓
        // （参照里是 C 模块；按既有"再挂一个名字"的形态 ✓）。
        ("_operator", operator_module::build_underscore),
        (time_module::NAME, time_module::build),
        (binascii_module::NAME, binascii_module::build),
        // **`_random`**（第 722 轮 ✓）：`Lib/random.py` 直接 `class Random(_random.Random)` ✓
        // ⇒ 真类 ＋ `seed`／`random`／`getrandbits`／`getstate`／`setstate` ✓。
        (random_module::NAME, random_module::build),
        // **`_string`／`_struct` 占位**（第 722 轮 ✓）：`Lib/string/` 与 `Lib/struct.py` 是
        // **导入期只读名字**的模块 ✓（`string.Formatter` 与 `struct.pack` 都在**调用期**才用）
        // ⇒ 先按"名字齐、调用如实报未接线"落地 ✓，真实现随后按 `CM-8` 走能力域 ✓。
        ("_string", build_string_stub),
        (struct_module::NAME, struct_module::build),
        ("_opcode", build_opcode_module),
];

/// 建模块命名空间的函数类型（模块表用 ✓）。
pub(crate) type ModuleBuilder = fn(&pyawa_core::Instance) -> core::ptr::NonNull<pyawa_core::Header>;

/// **`sys.builtin_module_names` 的唯一出处** ✓：模块表里的名字 ＋ 两个特殊项 ✓。
pub(crate) fn builtin_module_names() -> Vec<&'static str> {
    RUST_MODULES
        .iter()
        .map(|(name, _)| *name)
        .chain(["builtins", "sys"])
        .collect()
}

/// **"名字齐、调用报未接线"的占位模块** ✓（第 194 轮，**一处真相** ✓）。
///
/// 给 `_io`／`_warnings` 这类**要先导入成功**、但**真实实现要按 `CM-8` 走能力域**的内建模块用 ✓：
/// 每个名字都注册成 native ✓，调用时**如实报未接线** ✗（不静默给假值 ✗）。
pub fn build_stub_module(
    instance: &pyawa_core::Instance,
    module_name: &str,
    names: &[&str],
    _note: &str,
) -> core::ptr::NonNull<pyawa_core::Header> {
    let namespace = instance.new_dict();
    for name in names {
        let ty = instance
            .type_named("builtin_function_or_method")
            .expect("builtin_function_or_method 在引导期已登记");
        let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
            ty,
            Box::leak(name.to_string().into_boxed_str()),
            core::cell::Cell::new(stub_native),
        ));
        let native = object.into_raw().cast::<pyawa_core::Header>();
        instance.dict_set(namespace, name, native);
    }
    let exports = instance.new_list(names.iter().map(|name| instance.new_str(name)).collect());
    instance.dict_set(namespace, "__all__", exports);
    let name_object = instance.new_str(module_name);
    instance.dict_set(namespace, "__name__", name_object);
    namespace
}

/// 占位 native：调用即**如实报未接线** ✗。
fn stub_native(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        &String::from("该内建的真实现要按 CM-8 走能力域，随后接"),
    ))
}

/// **`_string` 占位**（第 722 轮 ✓）：`Lib/string/__init__.py` 导入期只 `import _string` ✓，
/// `string.Formatter` 的方法**调用期**才用它 ✓ ⇒ 名字齐、调用**如实报未接线** ✓（不静默给假值 ✓）。
/// 名字取自同版本参照 `dir(_string)` ✓（只有两个 ✓）。
fn build_string_stub(instance: &pyawa_core::Instance) -> core::ptr::NonNull<pyawa_core::Header> {
    build_stub_module(
        instance,
        "_string",
        &["formatter_field_name_split", "formatter_parser"],
        "string.Formatter 的真实现随后接",
    )
}

/// **`_opcode` 模块**（第 723 轮 ✓）：`Lib/opcode.py` 在**导入期**要用它一大片面 ✓
/// —— `has_arg`／`has_const`／`has_name`／`has_jump`／`has_free`／`has_local`／`has_exc`（收**编号** ✓）、
/// `get_intrinsic1_descs`／`get_intrinsic2_descs`／`get_special_method_names`／`get_nb_ops`
/// （`opcode.py:28-44` ✓），以及 `Lib/dis.py:22` 的 `get_executor` ✓。
///
/// **一处真相** ✓：全部转发到 `pyawa-core` 的 [`pyawa_core::opcode`]／`opcode_metadata`
/// 表（`BC-30`／`BC-37`／`BC-38` 的**数值唯一出处** ✓），本层不另造表 ✓。
/// **如实偏差** ✗：只有 `get_executor`（JIT 执行器）**调用即报未接线** ✓ —— 它在 `dis()` 一族里
/// 走**运行期**才用 ✓，故 `import opcode`／`import dis` 不受影响 ✓。
fn build_opcode_module(instance: &pyawa_core::Instance) -> core::ptr::NonNull<pyawa_core::Header> {
    let namespace = instance.new_dict();
    let method_type = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let handlers: [(&str, pyawa_core::NativeFn); 12] = [
        ("has_arg", has_arg_native as pyawa_core::NativeFn),
        ("has_const", has_const_native as pyawa_core::NativeFn),
        ("has_name", has_name_native as pyawa_core::NativeFn),
        ("has_jump", has_jump_native as pyawa_core::NativeFn),
        ("has_free", has_free_native as pyawa_core::NativeFn),
        ("has_local", has_local_native as pyawa_core::NativeFn),
        ("has_exc", has_exc_native as pyawa_core::NativeFn),
        ("get_intrinsic1_descs", get_intrinsic1_descs_native as pyawa_core::NativeFn),
        ("get_intrinsic2_descs", get_intrinsic2_descs_native as pyawa_core::NativeFn),
        ("get_special_method_names", get_special_method_names_native as pyawa_core::NativeFn),
        ("get_nb_ops", get_nb_ops_native as pyawa_core::NativeFn),
        ("stack_effect", stack_effect_native as pyawa_core::NativeFn),
    ];
    for (name, handler) in handlers {
        let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
            method_type,
            Box::leak(name.to_string().into_boxed_str()),
            core::cell::Cell::new(handler),
        ));
        let native = object.into_raw().cast::<pyawa_core::Header>();
        instance.dict_set(namespace, name, native);
    }
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        method_type,
        "get_executor",
        core::cell::Cell::new(get_executor_native),
    ));
    let native = object.into_raw().cast::<pyawa_core::Header>();
    instance.dict_set(namespace, "get_executor", native);
    let exports = instance.new_list(
        [
            "has_arg", "has_const", "has_name", "has_jump", "has_free", "has_local",
            "has_exc", "get_intrinsic1_descs", "get_intrinsic2_descs",
            "get_special_method_names", "get_nb_ops", "stack_effect", "get_executor",
        ]
        .iter()
        .map(|name| instance.new_str(name))
        .collect(),
    );
    instance.dict_set(namespace, "__all__", exports);
    // **`__name__` 必须有** ✗（第 723 轮实测）：`importlib._bootstrap._setup` 会遍历 `sys.modules`、
    // 逐个读内建模块的 `__name__` ✓ ⇒ 缺它当场 `AttributeError` ⇒ **`import importlib` 全挂** ✗
    //（实测：`_opcode` 进 `RUST_MODULES` 后 importlib 一族 7 个 ＋ `profile` 共 **8** 个模块转红 ✗）。
    instance.dict_set(namespace, "__name__", instance.new_str("_opcode"));
    instance.dict_set(
        namespace,
        "__doc__",
        instance.new_str("Pyawa 的 _opcode 面（BC-30／BC-37 的表转发）"),
    );
    namespace
}

/// `_opcode.has_*(编号)` ✓：收的是 **opcode 编号**（`opcode.py:28` 迭代的是 `opmap.values()` ✓）
/// —— 先前按**名字**查表 ⇒ 静默给 `False` ✗，这里按 `pyawa-core` 的标志表判 ✓。
fn opcode_flag(
    instance: &pyawa_core::Instance,
    args: &[core::ptr::NonNull<pyawa_core::Header>],
    predicate: fn(u16) -> bool,
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let flag = args
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .and_then(|value| u16::try_from(value).ok())
        .map(predicate)
        .unwrap_or(false);
    Ok(instance.new_bool(flag))
}

macro_rules! opcode_flag_native {
    ($name:ident, $predicate:path) => {
        fn $name(
            instance: &pyawa_core::Instance,
            _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
            args: &[core::ptr::NonNull<pyawa_core::Header>],
            _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
        ) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
            opcode_flag(instance, args, $predicate)
        }
    };
}

opcode_flag_native!(has_arg_native, pyawa_core::opcode::has_arg);
opcode_flag_native!(has_const_native, pyawa_core::opcode::has_const);
opcode_flag_native!(has_name_native, pyawa_core::opcode::has_name);
opcode_flag_native!(has_jump_native, pyawa_core::opcode::has_jump);
opcode_flag_native!(has_free_native, pyawa_core::opcode::has_free);
opcode_flag_native!(has_local_native, pyawa_core::opcode::has_local);
opcode_flag_native!(has_exc_native, pyawa_core::opcode::has_exc);

/// 把一串 `&'static str` 变成 `list[str]`（**新引用** ✓）。
fn new_str_list(
    instance: &pyawa_core::Instance,
    items: &[&'static str],
) -> core::ptr::NonNull<pyawa_core::Header> {
    instance.new_list(items.iter().map(|item| instance.new_str(item)).collect())
}

fn get_intrinsic1_descs_native(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    Ok(new_str_list(instance, pyawa_core::opcode::get_intrinsic1_descs()))
}

fn get_intrinsic2_descs_native(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    Ok(new_str_list(instance, pyawa_core::opcode::get_intrinsic2_descs()))
}

fn get_special_method_names_native(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    Ok(new_str_list(instance, pyawa_core::opcode::get_special_method_names()))
}

/// `_opcode.get_nb_ops()` ⇒ `[(名字, 符号), …]` ✓（与参照实测逐项一致 ✓：
/// `[('NB_ADD', '+'), ('NB_AND', '&')]` ✓）。
fn get_nb_ops_native(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let items: Vec<core::ptr::NonNull<pyawa_core::Header>> = pyawa_core::opcode::get_nb_ops()
        .iter()
        .map(|(name, symbol)| {
            let pair = vec![instance.new_str(name), instance.new_str(symbol)];
            instance.new_tuple(pair)
        })
        .collect();
    Ok(instance.new_list(items))
}

/// `_opcode.stack_effect(op, oparg=None, jump=None)` ✓：转发到 `pyawa-core` 的同一张栈效应表 ✓。
fn stack_effect_native(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let opcode = args
        .first()
        .and_then(|value| instance.int_of(*value))
        .and_then(|value| value.to_i64())
        .and_then(|value| u16::try_from(value).ok());
    let oparg = args.get(1).and_then(|value| instance.int_of(*value)).and_then(|value| value.to_i64());
    let jump = args.get(2).map(|value| instance.truth_of(*value));
    let Some(opcode) = opcode else {
        return Err(instance.raise_builtin_error("TypeError", "stack_effect: 第一个实参要是 opcode 编号"));
    };
    match pyawa_core::opcode::stack_effect(opcode, oparg, jump) {
        Ok(effect) => Ok(instance.new_int(effect as i64)),
        Err(_) => Err(instance.raise_builtin_error("ValueError", "stack_effect: 未知 opcode")),
    }
}

/// `_opcode.get_executor(code, offset=None)` ⇒ **如实报未接线** ✓（JIT 执行器；运行期才用 ✓）。
fn get_executor_native(
    _instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    Err(pyawa_core::ExecError::Unsupported {
        opcode: 0,
        what: "_opcode.get_executor：JIT 执行器未接线",
    })
}

pub mod struct_module;
pub mod posix_module;
pub mod weakref_module;
pub mod thread_module;
mod time_module;
pub mod codecs_module;
pub mod math_module;
pub mod contextvars_module;
pub mod collections_module;
/// **`_tokenize`** ✓（task-19 ✓）：`Lib/tokenize.py` 的词法底层 ✓。
pub mod tokenize_module;
pub mod unicode_tables;

/// 按**真实入口**之外的场合改写 `sys.path`（语料 harness 用 ✓：把语料目录放进去 ✓）。
///
/// 从模块表里找 `sys` **模块对象**，再取它的名字空间改 `path` ✓（与 `install` 同一条口径 ✓）。

fn native_fn(
    instance: &pyawa_core::Instance,
    name: &'static str,
    handler: pyawa_core::NativeFn,
) -> core::ptr::NonNull<pyawa_core::Header> {
    let ty = instance.type_named("builtin_function_or_method").expect("引导期已登记");
    instance
        .alloc(pyawa_core::BuiltinFunctionObject::new(
            ty,
            Box::leak(name.to_string().into_boxed_str()),
            core::cell::Cell::new(handler),
        ))
        .into_raw()
        .cast::<pyawa_core::Header>()
}

/// **按 Python 侧被证明干净的造法**造模块型对象 ✓（第 236 轮 ✓）：调 `module` 类型对象
/// （`types.ModuleType("x")` 同一条路 ✓），而不是手工挂名字空间 ✗。
fn module_like(
    instance: &pyawa_core::Instance,
    name: &str,
) -> Option<core::ptr::NonNull<pyawa_core::Header>> {
    let ty = instance.type_named("module")?.cast::<pyawa_core::Header>();
    pyawa_core::call_value(instance, ty, &[instance.new_str(name)], &[]).ok()
}

fn set_field(
    instance: &pyawa_core::Instance,
    object: core::ptr::NonNull<pyawa_core::Header>,
    name: &str,
    value: core::ptr::NonNull<pyawa_core::Header>,
) {
    let _ = pyawa_core::executor::protocol::instance_attribute_set(instance, object, name, value, 0);
}

/// loader 的 `create_module`：没有特别的模块类型 ⇒ 返回 `None` ✓（`_bootstrap` 自己会造 ✓）。
fn loader_create(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    _args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    Ok(instance.retain(instance.singletons().none()))
}

/// loader 的 `exec_module`：把源码装进 `_bootstrap` 给的那个模块的名字空间 ✓（`IM-31` 的 loader 半 ✓）。
fn loader_exec(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let Some(module) = args.first().copied() else {
        return Ok(instance.retain(instance.singletons().none()));
    };
    if let Some(namespace) = pyawa_core::mounted_instance_dict(instance, module) {
        if let Some(name) = instance
            .dict_get(namespace, "__name__")
            .and_then(|object| instance.text_of(object))
            .map(|text| text.to_owned())
        {
            pyawa_core::executor::import::exec_module_into_namespace(instance, &name, namespace)?;
        }
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// finder 的 `find_spec`：**只定位** ✓（`IM-31` 的 finder 半 ✓ —— 第 216／217 轮证明：把"装"放进来
/// 会让嵌套导入**重入**导入机制 ✗），命中就给 spec ✓，否则 `None` ✓。
fn finder_find_spec(
    instance: &pyawa_core::Instance,
    _bound: Option<core::ptr::NonNull<pyawa_core::Header>>,
    args: &[core::ptr::NonNull<pyawa_core::Header>],
    _kwargs: &[(core::ptr::NonNull<pyawa_core::Header>, core::ptr::NonNull<pyawa_core::Header>)],
) -> Result<core::ptr::NonNull<pyawa_core::Header>, pyawa_core::ExecError> {
    let none = instance.retain(instance.singletons().none());
    let Some(name) = args.first().and_then(|object| instance.text_of(*object)).map(|t| t.to_owned())
    else {
        return Ok(none);
    };
    if !pyawa_core::executor::import::can_locate_through_bridge(instance, &name) {
        return Ok(none);
    }
    let Some(spec) = module_like(instance, "spec") else {
        return Ok(none);
    };
    set_field(instance, spec, "name", instance.new_str(&name));
    set_field(instance, spec, "origin", instance.new_str(&name));
    set_field(instance, spec, "has_location", instance.retain(instance.singletons().boolean(false)));
    if let Some(loader) = module_like(instance, "loader") {
        set_field(instance, loader, "create_module", native_fn(instance, "create_module", loader_create));
        set_field(instance, loader, "exec_module", native_fn(instance, "exec_module", loader_exec));
        set_field(instance, spec, "loader", loader);
    }
    Ok(spec)
}

/// 把 finder 挂进 `sys.meta_path` ✓ —— **必须在 `sys` 模块建好之后**调用 ✓（第 230 轮实测的挂载点 ✓）。
pub fn install_meta_path(instance: &pyawa_core::Instance) {
    let Some(modules) = instance.modules() else { return };
    let Some(sys_object) = instance.dict_get(modules, "sys") else { return };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, sys_object) else { return };
    let Some(finder) = module_like(instance, "finder") else { return };
    set_field(instance, finder, "find_spec", native_fn(instance, "find_spec", finder_find_spec));
    let meta_path = instance.new_list(vec![finder]);
    instance.dict_set(namespace, "meta_path", meta_path);
}

pub fn set_module_search_path(instance: &pyawa_core::Instance, directories: &[String]) {
    let Some(modules) = instance.modules() else {
        return;
    };
    let Some(sys_object) = instance.dict_get(modules, "sys") else {
        return;
    };
    let Some(namespace) = pyawa_core::mounted_instance_dict(instance, sys_object) else {
        return;
    };
    sys_module::set_path(instance, namespace, directories);
}

/// **组合根装配**（`CM-14`）：把**内建名字空间**与 `sys`（含 `stdout`／`stderr`）装进实例 ✓。
///
/// `print` 的目的地就是这里的那个 `sys.stdout` 对象 ✓ —— `CM-26` 的链路
/// `print ⇒ sys.stdout ⇒ _io ⇒ fs` ✓。**调用方是组合根**（CLI／语料 harness ✓）；
/// 本函数只做装配，不碰平台 ✓（`CX-4`）。
pub fn install(instance: &pyawa_core::Instance, program: &str, arguments: &[String]) {
    let builtins = builtins_module::build(instance);
    let sys = sys_module::build(instance);
    let stdout = instance
        .dict_get(sys, "stdout")
        .expect("`sys.stdout` 由 `sys_module::build` 装好");
    let handle = instance.new_int(_io_module::STDOUT_HANDLE as i64);
    instance.dict_set(builtins, "__stdout__", stdout);
    instance.dict_set(builtins, "__stdout_handle__", handle);
    instance.set_builtins(Some(builtins));
    // `sys.path`：`site.py`（`IM-24`）还没接 ⇒ 组合根先把**脚本所在目录**放进去 ✓
    // （与参照实现的 `sys.path[0]` 同义 ✓；没有目录（如占位名）就留空表 ✓）
    let mut path_entries: Vec<String> = Vec::new();
    if let Some(directory) = std::path::Path::new(program).parent() {
        let text = directory.to_string_lossy().into_owned();
        if !text.is_empty() {
            path_entries.push(text);
        }
    }
    // **暂由组合根把 `Lib/` 放进 `sys.path`**（第 138 轮）——按 `IM-24`／`IM-26`（**A2**）这活
    // 最终归 `site.py` ✗，在它落地之前先由组合根代劳 ✓（与 `sys.path[0]` 同一条口径 ✓）。
    // 取可执行文件旁的 `Lib/`（仓库布局：`<root>/Lib` ✓）；不存在就**不放**（如实 ✓）。
    // **向上找**含 `Lib/os.py` 的目录（最多 4 级 ✓）——比硬编码层级稳 ✓；找不到就**不放**（如实 ✓）。
    let mut cursor = Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut lib_directory = None;
    for _ in 0..4 {
        let Some(directory) = cursor else { break };
        let candidate = directory.join("Lib");
        if candidate.join("os.py").is_file() {
            lib_directory = Some(candidate);
            break;
        }
        cursor = directory.parent();
    }
    if let Some(lib_directory) = lib_directory {
        path_entries.push(lib_directory.to_string_lossy().into_owned());
    }
    sys_module::set_path(instance, sys, &path_entries);
    // `sys.argv` 按**真实入口**改写 ✓（`["<程序名>", <参数>…]`）
    sys_module::set_argv(instance, sys, program, arguments);
    // **模块表**（`IM-`：`import` 查的就是它 ✓）——与 `sys.modules` 是**同一份 dict** ✓（一处真相）；
    // `sys` 先放进去（别的模块随各自落地再加 ✓）
    let modules = instance
        .dict_get(sys, "modules")
        .expect("`sys.modules` 由 `sys_module::build` 装好");
    // 模块表里要放**模块对象**（不是名字空间字典 ✗）：属性查找走 `mounted_instance_dict` ✓
    // `module` 在 `TS-41` 的探测表里 ✓，但引导期不一定要用到它（可能没登记）⇒ 缺就建 ✓
    let module_type = instance
        .type_named("module")
        .unwrap_or_else(|| instance.new_attribute_type("module"));
    let sys_object = instance
        .alloc(AttributeObject::new(
            module_type,
            core::cell::RefCell::new(Some(sys)),
        ))
        .into_raw()
        .cast::<pyawa_core::Header>();
    instance.dict_set(modules, "sys", sys_object);
    // **其余 Rust 侧模块**（第 134 轮）：它们都已实现 ✓，只是**没登记** ✗ ⇒ `import itertools`
    // 之类会掉进"按 sys.path 找不到" ✗。这里照 `sys` 那套包成模块对象放进模块表 ✓
    // （`sys.modules` 与模块表是**同一份** dict ✓ —— 一处真相 ✓）。
    // 注意：`errno` 的常量按 `CM-20` 由**宿主注入**（`install_errno` 另接 ✓），这里不碰 ✓。
    // **名字用各模块自己的 `NAME`** ✓（一处真相 ✓）——参照里 `_imp` 是这个名字 ✓
    //（`imp` 在 3.14 **已被移除** ✗，我先前硬编码 `imp` 当场被对拍抓住 ✓）。
    // **`errno`**（第 134 轮）：它的常量按 `CM-20` 由**宿主注入**到实例 ✓ ⇒ 从实例取整表 ✓
    //（`PLAN` §9.4 第 4 条点名它是"不依赖能力域的第一个 stdlib 模块" ✓，也是 M3 档位里的一个 ✓）。
    {
        let constants = instance.platform_constants();
        let namespace = errno_module::build(instance, &constants);
        let module = instance
            .alloc(AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<pyawa_core::Header>();
        instance.dict_set(modules, errno_module::NAME, module);
let namespace = _sre_module::build(instance);
        let module = instance
            .alloc(AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<pyawa_core::Header>();
        instance.dict_set(modules, _sre_module::NAME, module);
    }
    // **`builtins` 模块** ✓（第 195 轮）：参照里它就是**内建命名空间本身** ✓
    //（`import builtins` 之后 `builtins.len is len` ✓）⇒ 这里**不另建 dict** ✓，直接把那份命名空间
    // 登记成一个模块 ✓。为什么单独写 ✓：其余模块用 `build` **新建** dict ✓，它不建 ✓。
    if let Some(namespace) = instance.builtins() {
        let module = instance
            .alloc(AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<pyawa_core::Header>();
        instance.dict_set(modules, "builtins", module);
    }
    for (name, build) in RUST_MODULES {

        let namespace = build(instance);
        let module = instance
            .alloc(AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<pyawa_core::Header>();
        instance.dict_set(modules, name, module);
    }
    instance.set_modules(Some(modules));
    install_meta_path(instance);

}
