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
pub mod unicode_tables;

/// **组合根装配**（`CM-14`）：把**内建名字空间**与 `sys`（含 `stdout`／`stderr`）装进实例 ✓。
///
/// `print` 的目的地就是这里的那个 `sys.stdout` 对象 ✓ —— `CM-26` 的链路
/// `print ⇒ sys.stdout ⇒ _io ⇒ fs` ✓。**调用方是组合根**（CLI／语料 harness ✓）；
/// 本函数只做装配，不碰平台 ✓（`CX-4`）。
pub fn install(instance: &pyawa_core::Instance) {
    let builtins = builtins_module::build(instance);
    let sys = sys_module::build(instance);
    let stdout = instance
        .dict_get(sys, "stdout")
        .expect("`sys.stdout` 由 `sys_module::build` 装好");
    let handle = instance.new_int(_io_module::STDOUT_HANDLE as i64);
    instance.dict_set(builtins, "__stdout__", stdout);
    instance.dict_set(builtins, "__stdout_handle__", handle);
    instance.set_builtins(Some(builtins));
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
    instance.set_modules(Some(modules));
}
