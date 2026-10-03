//! Pyawa 的 C 层模块实现：从 Python 看到的 API 与语义。
//!
//! 逐模块合约归属 `docs/SPEC-c-modules.md`（`CM-`），见本 crate 的 `README.md`。
//!
//! **指令表的数据与纯函数归属 `pyawa-core`**（`BC-38` 的依赖边裁决：指令集是 VM 的一部分）。
//! 本 crate 只做 Python 层包装，见 [`opcode`]。

#![forbid(unsafe_code)]

use pyawa_core::AttributeObject;

pub mod _io_module;
pub mod builtins_module;
pub mod errno_map;
pub mod errno_module;
/// `operator`（契约 `docs/SPEC-c-modules.md` §5.2.7；本层第一刀：`eq`／`ne`／`is_`／`is_not`／`truth`／`not_`）
pub mod operator_module;
pub mod opcode;
/// `sys`（不依赖能力域的部分；契约 `docs/SPEC-c-modules.md` §5.2.3）
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
pub mod posix_module;
pub mod weakref_module;
pub mod unicode_tables;

/// 按**真实入口**之外的场合改写 `sys.path`（语料 harness 用 ✓：把语料目录放进去 ✓）。
///
/// 从模块表里找 `sys` **模块对象**，再取它的名字空间改 `path` ✓（与 `install` 同一条口径 ✓）。
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
    }
    let rust_modules: &[(&str, fn(&pyawa_core::Instance) -> core::ptr::NonNull<pyawa_core::Header>)] = &[
        (imp_module::NAME, imp_module::build),
        (posix_module::NAME, posix_module::build),
        (weakref_module::NAME, weakref_module::build),
        (itertools_module::NAME, itertools_module::build),
        (marshal_module::NAME, marshal_module::build),
        (operator_module::NAME, operator_module::build),
    ];
    for (name, build) in rust_modules {
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
}
