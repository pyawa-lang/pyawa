//! **挂账 bug 批A（一）**：`global X` ＋ 函数内的**绑定语句**（`import X`／`for X`／`with … as X`）
//! ⇒ 读按局部、写按全局的**两张表不一致**。
//!
//! 根因（`compile/symbols.rs` 的 `collect_locals`）：`import`／`for` 目标（以及 `match` 捕获）
//! 的局部登记直接调 `slot_of`，**绕过了 `declare_local` 的 `global` 检查** ⇒ 名字被塞进
//! `co_varnames`；而发射期的 `store_target`／`emit_store_name` **认全局**（`STORE_GLOBAL`）⇒
//! 绑定写全局、读取却按 `LOAD_FAST` 找一个永远没人写的局部槽 ⇒
//! `UnboundLocalError: cannot access local variable …`。受害者：
//! `collections.Counter.most_common`（`global heapq` ＋ 惰性 `import heapq`）。
//!
//! **落点说明（为什么放 core tests）**：`import` 的执行链（`sys.path`／`sys.modules`）由
//! `pyawa-runtime` 的组合根装配，core 测试里没有；这里用**两层**回归把它钉住：
//! ① 编译面——精确的 `global heapq; import heapq` 形状，断言 `heapq` **不进 `varnames`**、
//!    绑定是 `STORE_GLOBAL`、读是 `LOAD_GLOBAL`（这正是与参照 `co_varnames`／`co_names` 同口径）；
//! ② 行为面——同一条 `collect_locals` 路径的 `for`／`with … as` 目标（不需要 import 机制）
//!    端到端执行，断言结果与参照 `python3` 一致（修复前必抛 `UnboundLocalError`）。
//! b1 探针与 `Counter.most_common` 的真实端到端对拍在验收里用 CLI 跑（见汇报）。

mod common;

use core::ptr::NonNull;

use pyawa_core::compile::{compile, instantiate, CheckTier, CompiledUnit, Constant, Mode};
use pyawa_core::{Frame, Header};

use common::Vm;

/// 编译一个模块（纯 Python 模式：`global`／`import` 的发射与模式无关）。
fn compile_module(source: &str) -> CompiledUnit {
    compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("应当编得过 {source:?}，却报了 {error:?}"))
}

/// 按 `co_name` 找嵌套 code 对象（函数体／类体都在常量表的 `Constant::Code` 里）。
fn nested<'a>(unit: &'a CompiledUnit, name: &str) -> Option<&'a CompiledUnit> {
    for constant in &unit.constants {
        if let Constant::Code(inner) = constant {
            if inner.name == name {
                return Some(inner);
            }
            if let Some(found) = nested(inner, name) {
                return Some(found);
            }
        }
    }
    None
}

/// 把指令流解出来（按**码元**走路：带 cache 的指令要跳过等宽零填充槽 ⇒ 与 `BC-35` 同口径）。
fn instructions(unit: &CompiledUnit) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor + 1 < unit.code.len() {
        let opcode = u16::from(unit.code[cursor]);
        let arg = u32::from(unit.code[cursor + 1]);
        let name = pyawa_core::opcode::opname(opcode)
            .expect("编号应当在表里")
            .to_owned();
        out.push((name, arg));
        cursor += 2 + 2 * pyawa_core::opcode::inline_cache_entries(opcode) as usize;
    }
    out
}

/// `STORE_GLOBAL <name>` 出现过吗。
fn stores_globally(unit: &CompiledUnit, name: &str) -> bool {
    instructions(unit).iter().any(|(opname, arg)| {
        opname == "STORE_GLOBAL" && unit.names.get(*arg as usize).is_some_and(|n| n == name)
    })
}

/// `LOAD_GLOBAL <name>` 出现过吗（oparg 低位是"压 NULL"标志 ⇒ 下标是 `arg >> 1`）。
fn loads_globally(unit: &CompiledUnit, name: &str) -> bool {
    instructions(unit).iter().any(|(opname, arg)| {
        opname == "LOAD_GLOBAL"
            && unit
                .names
                .get((arg >> 1) as usize)
                .is_some_and(|n| n == name)
    })
}

/// 编译并执行一段模块源码，返回模块命名空间（帧跑完不清理）。
fn run_module<'a>(vm: &'a Vm, source: &str) -> NonNull<Header> {
    let unit = compile_module(source);
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本测试持有，存活到执行结束。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    if let Err(error) = pyawa_core::execute(&vm.instance, &frame) {
        panic!("跑挂了：{error:?}（{source:?}）");
    }
    namespace
}

/// 取命名空间里的一个整数结果。
fn int_global(vm: &Vm, namespace: NonNull<Header>, name: &str) -> i64 {
    let value = vm
        .instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("命名空间里应当有 {name}"));
    vm.instance
        .int_of(value)
        .unwrap_or_else(|| panic!("{name} 应当是一个 int"))
        .to_i64()
        .unwrap_or_else(|| panic!("{name} 应当装得进 i64"))
}

/// ① 编译面：**精确的 b1 形状**——`global heapq; import heapq` 之后
/// `heapq` 不是局部（不进 `varnames`），绑定走 `STORE_GLOBAL`、读走 `LOAD_GLOBAL`。
///
/// 参照（`python3` 3.14.4）同口径：`most_common.__code__.co_varnames == ()`、
/// `"heapq" in co_names`，`dis` 里是 `STORE_GLOBAL heapq`／`LOAD_GLOBAL heapq`。
#[test]
fn global_declared_import_is_bound_and_read_as_a_global() {
    let unit = compile_module(
        "def most_common(counter):\n    global heapq\n    import heapq\n    heapq.heapify([1, 2])\n    return \"ok\"\n",
    );
    let function = nested(&unit, "most_common").expect("应当有 most_common 的 code");
    assert!(
        !function.varnames.iter().any(|name| name == "heapq"),
        "`global heapq` ＋ `import heapq`：heapq 必须是全局，不许进 varnames（否则读 LAOD_FAST／写 STORE_GLOBAL）\n实际 varnames={:?}",
        function.varnames
    );
    assert!(
        stores_globally(function, "heapq"),
        "`import heapq` 的绑定必须是 STORE_GLOBAL：{:?}",
        instructions(function)
    );
    assert!(
        loads_globally(function, "heapq"),
        "读 `heapq` 必须是 LOAD_GLOBAL：{:?}",
        instructions(function)
    );
}

/// ① 编译面（`from … import` 那一支，同一个 `collect_locals` 缺陷）：`global heapify; from heapq import heapify`。
#[test]
fn global_declared_from_import_is_bound_as_a_global() {
    let unit = compile_module(
        "def f():\n    global heapify\n    from heapq import heapify\n    heapify([3, 1])\n    return \"ok\"\n",
    );
    let function = nested(&unit, "f").expect("应当有 f 的 code");
    assert!(
        !function.varnames.iter().any(|name| name == "heapify"),
        "`global heapify` ＋ `from heapq import heapify`：不许进 varnames，实际 {:?}",
        function.varnames
    );
    assert!(stores_globally(function, "heapify"), "绑定要 STORE_GLOBAL");
    assert!(loads_globally(function, "heapify"), "读要 LOAD_GLOBAL");
}

/// ② 行为面（同一缺陷的 `for` 目标那一支，不需要 import 机制）：`global x` ＋ `for x in …`
/// 之后读 `x` 必须是全局绑定到的值，不许抛 `UnboundLocalError`。
///
/// 参照（`python3` 3.14.4）：`print(f())` ⇒ `2`。
#[test]
fn global_declared_for_target_binds_the_global() {
    let vm = Vm::new();
    let namespace = run_module(
        &vm,
        "def f():\n    global x\n    for x in [1, 2]:\n        pass\n    return x\n\nresult = f()\n",
    );
    assert_eq!(int_global(&vm, namespace, "result"), 2);
}

/// ② 行为面（`with … as` 目标那一支）：`global y` ＋ `with C() as y` 之后读 `y`。
///
/// 参照（`python3` 3.14.4）：`print(f())` ⇒ `7`。
#[test]
fn global_declared_with_target_binds_the_global() {
    let vm = Vm::new();
    let namespace = run_module(
        &vm,
        "class C:\n    def __enter__(self):\n        return 7\n    def __exit__(self, *args):\n        return False\n\ndef f():\n    global y\n    with C() as y:\n        pass\n    return y\n\nresult = f()\n",
    );
    assert_eq!(int_global(&vm, namespace, "result"), 7);
}
