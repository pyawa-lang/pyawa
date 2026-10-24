//! `gc` 模块（`P3-37` 第一片）：**最小诚实面**。
//!
//! 本层的 GC ＝ **引用计数 ＋ 循环回收**（core 的 `Instance::collect`：单代标记-清除，释放不可达的
//! 环）。参照的 `gc` 是**分代**收集器，二者能力面不同 ⇒ 这里只做**能真做的**，做不了的**如实
//! `NotImplementedError`**（`P3-37` 的挂账），**不编假数据**：
//!
//! ## 真实实现（有真语义，不假）
//! - `collect([generation])`：真跑一次 core 的标记-清除，返回**本次释放的不可达对象数**。
//!   参照是分代收集（只回收该代）；本层**单代** ⇒ `0/1/2` 三个取值都触发同一趟全量回收
//!   （**如实偏差**：`collect(0)` 也会回收整环，不只是第 0 代）。
//! - `isenabled`／`enable`／`disable`：**真开关**。core 没有"GC 开关"字段，但自动回收由
//!   **分配阈值**触发（`Instance::alloc` 的 `gc_threshold.0`）——`disable()` 把内部阈值顶到
//!   `usize::MAX`（自动回收实际停止，**显式** `collect()` 仍可跑），`enable()`／`set_threshold`
//!   再按**用户可见阈值**恢复。用户可见的阈值另存于本模块的 `_state`（`get_threshold` 读它）。
//! - `get_threshold`／`set_threshold`：真映射到 core 阈值；`t0 <= 0` 照参照保存（等于"不自动回收"，
//!   与 `disable()` 同一内部落法）。
//! - `garbage`：空表（本层不产生 uncollectable 对象）；`callbacks`：空表（本层没有回调机制，
//!   往里 append 不会触发 —— **如实偏差**，见字段注释）。
//! - `get_freeze_count()`：恒 `0`（没有 `freeze` 机制 ⇒ 冻结数为 0 是真话）。
//! - `DEBUG_*` 常量：数值照参照（`DEBUG_LEAK` 随参照由三项按位或出）。
//!
//! ## 如实未实现（`NotImplementedError`，`P3-37` 挂账）
//! `get_objects`（无活对象全量表）／`get_referrers`／`get_referents`（无引用图反查）／`is_tracked`／
//! `is_finalized`（无逐对象追踪位访问器）／`freeze`／`unfreeze`（无冻结区）／`get_count`／`get_stats`
//! （无分代计数器）／`get_debug`／`set_debug`（无调试通道）。
//!
//! 动因：官方套件里 `ModuleNotFoundError: No module named 'gc'` ×10（`test.support` 的
//! `gc_collect()`／`disable_gc()`／`gc_threshold()` 也直接用它）。

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`gc`）。
pub const NAME: &str = "gc";

/// 模块 `__doc__`（如实：最小诚实面）。
pub const DOC: &str = "This module provides a minimal, honest garbage-collector interface: reference \
counting plus cycle collection. collect/enable/disable/thresholds are real; \
object enumeration and referrer queries are not implemented.";

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

// --------------------------------------------------------------------------- //
// 模块私有状态（放在 gc 模块命名空间的 `_state` 里；参照里这些是解释器状态）
// --------------------------------------------------------------------------- //

/// `gc` 模块的**命名空间**（经 `sys.modules['gc']` 反查；组合根 `install` 装上模块对象之后可用）。
fn gc_namespace(instance: &Instance) -> Option<NonNull<Header>> {
    let modules = instance.modules()?;
    let module = instance.dict_get(modules, NAME)?;
    pyawa_core::mounted_instance_dict(instance, module)
}

/// 模块状态字典（`{"enabled": bool, "threshold": (t0, t1, t2)}`）。
fn state(instance: &Instance) -> Option<NonNull<Header>> {
    instance.dict_get(gc_namespace(instance)?, "_state")
}

/// 用户可见的"GC 是否启用"（取不到状态时按 core 的实际情形给 `true`）。
fn read_enabled(instance: &Instance) -> bool {
    state(instance)
        .and_then(|state| instance.dict_get(state, "enabled"))
        .and_then(|value| instance.bool_value(value))
        .unwrap_or(true)
}

/// 用户可见的阈值三元组（缺状态时直接用 core 的值）。
fn read_threshold(instance: &Instance) -> [i64; 3] {
    if let Some(threshold) = state(instance).and_then(|state| instance.dict_get(state, "threshold")) {
        if let Some(items) = instance.tuple_items(threshold) {
            if items.len() == 3 {
                let mut values = [0i64; 3];
                for (slot, item) in values.iter_mut().zip(items.iter()) {
                    if let Some(value) = instance.int_value(*item) {
                        *slot = value;
                    }
                }
                return values;
            }
        }
    }
    let (t0, t1, t2) = instance.gc_threshold();
    [t0 as i64, t1 as i64, t2 as i64]
}

fn write_enabled(instance: &Instance, enabled: bool) {
    if let Some(state) = state(instance) {
        let value = instance.new_bool(enabled);
        instance.dict_set(state, "enabled", value);
    }
}

fn write_threshold(instance: &Instance, threshold: [i64; 3]) {
    if let Some(state) = state(instance) {
        let values: Vec<NonNull<Header>> = threshold.iter().map(|value| instance.new_int(*value)).collect();
        let tuple = instance.new_tuple(values);
        instance.dict_set(state, "threshold", tuple);
    }
}

/// 把**用户可见**的"启用＋阈值"落到 core 的真实现上：
/// - 启用且 `t0 > 0` ⇒ core 阈值就是用户阈值（自动回收真的按它触发）；
/// - 停用（或 `t0 <= 0`，参照语义＝不做自动回收）⇒ core 的 `t0` 顶到 `usize::MAX`（实际停用），
///   `t1`／`t2` 仍是用户值（core 目前只存不用，如实）。
fn apply_threshold(instance: &Instance) {
    let [t0, t1, t2] = read_threshold(instance);
    let enabled = read_enabled(instance);
    let core_first = if enabled && t0 > 0 {
        usize::try_from(t0).unwrap_or(usize::MAX)
    } else {
        usize::MAX
    };
    let non_negative = |value: i64| usize::try_from(value.max(0)).unwrap_or(usize::MAX);
    instance.set_gc_threshold((core_first, non_negative(t1), non_negative(t2)));
}

/// 无参函数的参数校验（照参照实测的 `xxx() takes no arguments (N given)`）。
fn require_no_args(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    name: &str,
) -> Result<(), ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes no keyword arguments"),
        ));
    }
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{name}() takes no arguments ({} given)", args.len()),
        ));
    }
    Ok(())
}

/// 一个整数实参（`i` 口径；超出 `i64` ⇒ 参照的 `C int`／`C long` `OverflowError`）。
fn integer_argument(instance: &Instance, value: NonNull<Header>, c_type: &str) -> Result<i64, ExecError> {
    if let Some(number) = instance.int_of(value) {
        return number.to_i64().ok_or_else(|| {
            instance.raise_builtin_error(
                "OverflowError",
                &format!("Python int too large to convert to C {c_type}"),
            )
        });
    }
    let name = instance.type_name(instance.type_of(value));
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("'{name}' object cannot be interpreted as an integer"),
    ))
}

// --------------------------------------------------------------------------- //
// 真实实现的入口
// --------------------------------------------------------------------------- //

/// `gc.collect([generation])`：真跑一次标记-清除，返回本次释放的对象数。
///
/// 参照是**分代**收集；本层**单代** ⇒ `0/1/2` 都触发同一趟全量回收（如实偏差，见模块头）。
/// **数量口径**：返回的是**本次新释放**的不可达环对象数 —— 引用计数本来就会立刻释放无环垃圾，
/// 所以空跑的 `collect()` 经常是 `0`（参照在同一进程里是 21，因为 CPython 启动期留了环；
/// 两边"这一次回收放掉多少个环"的语义相同，具体数字不可比、**不假造**）。
/// 参数/错误文案照参照实测：`generation=` 关键字收；3／-1 ⇒ `ValueError: invalid generation`；
/// 非整数 ⇒ `TypeError: 'X' object cannot be interpreted as an integer`；2 个实参 ⇒
/// `collect() takes at most 1 argument (2 given)`；未知关键字 ⇒ `... unexpected keyword argument`。
fn collect_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let total = args.len() + kwargs.len();
    if total > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("collect() takes at most 1 argument ({total} given)"),
        ));
    }
    let mut generation = 2i64;
    if let Some(value) = args.first() {
        generation = integer_argument(instance, *value, "int")?;
    } else if let Some((key, value)) = kwargs.first() {
        let name = instance.text_of(*key).unwrap_or_default().to_owned();
        if name != "generation" {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("collect() got an unexpected keyword argument '{name}'"),
            ));
        }
        generation = integer_argument(instance, *value, "int")?;
    }
    if !(0..=2).contains(&generation) {
        return Err(instance.raise_builtin_error("ValueError", "invalid generation"));
    }
    // **单代**：任何合法代都跑同一趟全量循环回收（如实偏差见模块头）。
    Ok(instance.new_int(instance.collect() as i64))
}

/// `gc.isenabled()`：用户可见的启用状态（默认启用；`disable()` 之后为假）。
fn isenabled_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    require_no_args(instance, args, kwargs, "gc.isenabled")?;
    Ok(instance.new_bool(read_enabled(instance)))
}

/// `gc.enable()`：恢复自动回收（按用户可见阈值；core 没有开关字段，用阈值实现，见模块头）。
fn enable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    require_no_args(instance, args, kwargs, "gc.enable")?;
    write_enabled(instance, true);
    apply_threshold(instance);
    Ok(instance.new_none())
}

/// `gc.disable()`：停用**自动**回收（显式 `collect()` 仍可跑）。
fn disable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    require_no_args(instance, args, kwargs, "gc.disable")?;
    write_enabled(instance, false);
    apply_threshold(instance);
    Ok(instance.new_none())
}

/// `gc.get_threshold()`：用户可见阈值三元组（真实映射到 core 阈值，见模块头）。
fn get_threshold_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    require_no_args(instance, args, kwargs, "gc.get_threshold")?;
    let threshold = read_threshold(instance);
    let values: Vec<NonNull<Header>> = threshold.iter().map(|value| instance.new_int(*value)).collect();
    Ok(instance.new_tuple(values))
}

/// `gc.set_threshold(t0[, t1[, t2]])`：真设 core 阈值（`t0 <= 0` ⇒ 不做自动回收，照参照保存）。
fn set_threshold_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // 参照实测：这条**不收关键字**，且文案不带 `gc.` 前缀。
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "set_threshold() takes no keyword arguments",
        ));
    }
    if args.is_empty() || args.len() > 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "gc.set_threshold requires 1 to 3 arguments",
        ));
    }
    let mut parsed = [0i64; 3];
    for (index, value) in args.iter().enumerate() {
        parsed[index] = integer_argument(instance, *value, "long")?;
    }
    // 参照 3.14.4 实测：**第三参数收但被忽略**（`set_threshold(100, 5, 3)` 之后
    // `get_threshold()` 仍是 `(100, 5, 0)`）⇒ 用户可见阈值只保留前两位、第三位恒 0。
    let mut threshold = read_threshold(instance);
    threshold[0] = parsed[0];
    if args.len() >= 2 {
        threshold[1] = parsed[1];
    }
    threshold[2] = 0;
    write_threshold(instance, threshold);
    apply_threshold(instance);
    Ok(instance.new_none())
}

/// `gc.get_freeze_count()`：恒 `0`（没有 `freeze` 机制 ⇒ 冻结数为 0 是如实值）。
fn get_freeze_count_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    require_no_args(instance, args, kwargs, "gc.get_freeze_count")?;
    Ok(instance.new_int(0))
}

// --------------------------------------------------------------------------- //
// 如实未实现的入口（`NotImplementedError`：能力不存在，用明确消息挂账）
// --------------------------------------------------------------------------- //

/// 生成一个"如实未实现"的原生入口（消息里点名 `gc.<名字>` 与原因）。
macro_rules! unimplemented_native {
    ($function:ident, $display:literal, $reason:literal) => {
        fn $function(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            _args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            Err(instance.raise_builtin_error(
                "NotImplementedError",
                concat!("`gc.", $display, "`：P3-37 第一片如实未实现（", $reason, "）"),
            ))
        }
    };
}

unimplemented_native!(get_objects_native, "get_objects", "本层没有活对象全量表可供枚举");
unimplemented_native!(get_referrers_native, "get_referrers", "本层没有引用图可反查");
unimplemented_native!(get_referents_native, "get_referents", "本层没有逐对象引用遍历口");
unimplemented_native!(is_tracked_native, "is_tracked", "本层没有逐对象追踪位访问器");
unimplemented_native!(is_finalized_native, "is_finalized", "本层没有逐对象终结状态访问器");
unimplemented_native!(freeze_native, "freeze", "本层没有冻结区");
unimplemented_native!(unfreeze_native, "unfreeze", "本层没有冻结区");
unimplemented_native!(get_count_native, "get_count", "本层是单代回收，没有分代计数器");
unimplemented_native!(get_stats_native, "get_stats", "本层是单代回收，没有分代统计");
unimplemented_native!(get_debug_native, "get_debug", "本层没有调试通道");
unimplemented_native!(set_debug_native, "set_debug", "本层没有调试通道");

/// 建 `gc` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();

    // 模块私有状态：`enabled` ＋ 用户可见阈值（初值取 core 的现值）。
    let state = instance.new_dict();
    instance.dict_set(state, "enabled", instance.new_bool(true));
    let (t0, t1, t2) = instance.gc_threshold();
    let threshold = instance.new_tuple(vec![
        instance.new_int(t0 as i64),
        instance.new_int(t1 as i64),
        instance.new_int(t2 as i64),
    ]);
    instance.dict_set(state, "threshold", threshold);
    instance.dict_set(namespace, "_state", state);

    for (name, handler) in [
        ("collect", collect_native as NativeFn),
        ("isenabled", isenabled_native as NativeFn),
        ("enable", enable_native as NativeFn),
        ("disable", disable_native as NativeFn),
        ("get_threshold", get_threshold_native as NativeFn),
        ("set_threshold", set_threshold_native as NativeFn),
        ("get_freeze_count", get_freeze_count_native as NativeFn),
        // 如实未实现（`NotImplementedError`，见各入口的注释与模块头）
        ("get_objects", get_objects_native as NativeFn),
        ("get_referrers", get_referrers_native as NativeFn),
        ("get_referents", get_referents_native as NativeFn),
        ("is_tracked", is_tracked_native as NativeFn),
        ("is_finalized", is_finalized_native as NativeFn),
        ("freeze", freeze_native as NativeFn),
        ("unfreeze", unfreeze_native as NativeFn),
        ("get_count", get_count_native as NativeFn),
        ("get_stats", get_stats_native as NativeFn),
        ("get_debug", get_debug_native as NativeFn),
        ("set_debug", set_debug_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }

    // `DEBUG_*`：数值照参照 3.14.4 实测（`DEBUG_LEAK` 是三项按位或 ⇒ 38）。
    for (name, value) in [
        ("DEBUG_STATS", 1i64),
        ("DEBUG_COLLECTABLE", 2),
        ("DEBUG_UNCOLLECTABLE", 4),
        ("DEBUG_SAVEALL", 32),
        ("DEBUG_LEAK", 38),
    ] {
        instance.dict_set(namespace, name, instance.new_int(value));
    }
    // `garbage`：本层不产生 uncollectable 对象 ⇒ 空表（真话）。
    instance.dict_set(namespace, "garbage", instance.new_list(Vec::new()));
    // `callbacks`：本层没有回调机制 ⇒ 空表。**如实偏差**：向它 append 不会触发任何回调。
    instance.dict_set(namespace, "callbacks", instance.new_list(Vec::new()));

    instance.dict_set(namespace, "__name__", instance.new_str(NAME));
    instance.dict_set(namespace, "__doc__", instance.new_str(DOC));
    namespace
}
