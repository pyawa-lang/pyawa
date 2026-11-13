//! **每实例内存配额**（`OM-3` 扩；沙箱硬需求）的进程内验收。
//!
//! 口径：
//! * 配额按**活着的分配字节**（`Instance::bytes_allocated`）判；超限 ⇒ 执行器在指令边界抛
//!   **可捕获的** `MemoryError`（不是 abort／panic ✓）；
//! * 抛错后水位抬高一档 `slack`（`max(64 KiB, limit/16)`）⇒ Python 处理块里还能分配／清理 ✓；
//!   继续增长则每 `slack` 字节再抛一次 ⇒ 宿主占用上界 ≈ `limit + slack` 量级；
//! * 大块来源（字符串／字节／序列重复、`list(<大迭代器>)`）在**分配前**就问预算 ⇒ 不先吃满宿主。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
use core::ptr::NonNull;

use pyawa_core::{ExecError, Frame};

use common::Vm;

/// 跑一段模块脚本（`Vm` 脚手架没有 builtins ⇒ 本文件脚本只用**语法**与属性访问，
/// 捕获用裸 `except:` ✓）；返回 `Ok(命名空间)` 或携带待处理异常的 panic。
fn run_script(vm: &Vm, source: &str) -> Result<NonNull<pyawa_core::Header>, String> {
    let unit = compile(source, "quota", Mode::PurePython, CheckTier::Shallow, 0)
        .map_err(|error| format!("编译失败：{error:?}"))?;
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有；帧接手一份。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => Ok(namespace),
        Err(error) => {
            let pending = vm
                .pending_exception()
                .map(|(name, message)| format!("{name}: {message:?}"))
                .unwrap_or_else(|| "（没有异常状态）".to_owned());
            Err(format!("脚本失败：{error:?}；待处理异常 {pending}"))
        }
    }
}

fn int_global(vm: &Vm, namespace: NonNull<pyawa_core::Header>, name: &str) -> i64 {
    let value = vm
        .instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("脚本应写全局 {name}"));
    vm.instance
        .int_of(value)
        .and_then(|value| value.to_i64())
        .unwrap_or_else(|| panic!("{name} 应是整数"))
}

#[test]
fn allocation_over_the_quota_raises_a_catchable_memory_error() {
    let vm = Vm::new();
    // **相对基线**：脚手架本身的引导分配不参与本判据 ⇒ 用"当前已用 ＋ 256 KiB"当上限。
    let limit = vm.instance.bytes_allocated() + 256 * 1024;
    vm.instance.set_memory_limit(limit);
    let source = r#"
data = []
n = 0
try:
    while n < 200000:
        data.append("x" * 1000)
        n = n + 1
except:
    n = -1
"#;
    let namespace = run_script(&vm, source).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(int_global(&vm, namespace, "n"), -1, "超限应被裸 except 接住");
    assert!(
        vm.instance.bytes_allocated() <= limit + limit.div_ceil(16).max(64 * 1024) + 8 * 1024,
        "宿主占用应落在 limit+slack 量级（实际 {}，limit {limit}）",
        vm.instance.bytes_allocated()
    );
}

#[test]
fn uncaught_over_quota_allocation_is_reported_as_memory_error() {
    let vm = Vm::new();
    let limit = vm.instance.bytes_allocated() + 256 * 1024;
    vm.instance.set_memory_limit(limit);
    let source = r#"
data = []
n = 0
while n < 200000:
    data.append("x" * 1000)
    n = n + 1
"#;
    // 这里不能复用 `run_script`（它会 panic）⇒ 手工跑一次拿错误。
    let unit = compile(source, "quota", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    // SAFETY: namespace 由本测试持有；帧接手一份。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Err(ExecError::Raised { .. }) => {
            let (name, _message) = vm.pending_exception().expect("应带异常状态");
            assert_eq!(name, "MemoryError");
        }
        Ok(_) => panic!("超限必须抛 MemoryError"),
        Err(other) => panic!("应是可派发的异常，实际 {other:?}"),
    }
}

#[test]
fn big_single_allocations_are_refused_before_they_hit_the_host() {
    let vm = Vm::new();
    let limit = vm.instance.bytes_allocated() + 256 * 1024;
    vm.instance.set_memory_limit(limit);
    // 字符串重复：预算检查在**分配前** ⇒ 直接 MemoryError（不先吃满宿主）。
    let source = r#"
caught = 0
try:
    s = "y" * (1 << 40)
except:
    caught = 1
"#;
    let namespace = run_script(&vm, source).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(int_global(&vm, namespace, "caught"), 1);
    assert!(
        vm.instance.bytes_allocated() <= limit + 64 * 1024,
        "大块前置预算不应真的分配（已用 {}，limit {limit}）",
        vm.instance.bytes_allocated()
    );
}

#[test]
fn under_the_quota_still_runs_normally() {
    let vm = Vm::new();
    vm.instance.set_memory_limit(vm.instance.bytes_allocated() + 4 * 1024 * 1024);
    let source = r#"
data = []
i = 0
while i < 1000:
    data.append(i)
    i = i + 1
done = i
"#;
    let namespace = run_script(&vm, source).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(int_global(&vm, namespace, "done"), 1000);
    assert!(vm.instance.bytes_allocated() > 0);
}

#[test]
fn setting_tight_limit_below_current_usage_hits_immediately() {
    let vm = Vm::new();
    let used = vm.instance.bytes_allocated();
    assert!(used > 0);
    vm.instance.set_memory_limit(used / 2);
    // 下一次分配（造一个字符串）应置命中位；执行器边界抛 MemoryError。
    let _ = vm.instance.new_str("probe");
    assert!(vm.instance.take_memory_hit(), "已用量高于新上限 ⇒ 应立即命中");
    // 取消限制后不再命中。
    vm.instance.set_memory_limit(0);
    let _ = vm.instance.new_str("probe2");
    assert!(!vm.instance.take_memory_hit());
}
