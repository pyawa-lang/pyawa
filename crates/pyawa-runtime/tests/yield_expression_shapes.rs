//! **`x = yield`（`SEND` 取值形态）**——`PLAN-milestones.md` §12.1 的运行期验收。
//!
//! 修因：`statements_have_yield` 原先只下探 `return` 与裸表达式两种语句里的表达式 ⇒
//! `v = yield`（赋值右值）没被算成让出 ⇒ 所在函数**没被标成生成器**（`CO_GENERATOR` 没置）
//! ⇒ 运行期报"非生成器函数不该让出（码元被改坏了？）"✗。修法在编译器符号分析（按码元路径），
//! 不在解释器里特判。
//!
//! 期望值由**本机 `python3`（3.14.4）跑同一段脚本**取得，逐字节一致（`MS-17`）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-yield-expr-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn send_value_becomes_the_yield_expression_value() {
    let script = r#"def take():
    v = yield
    print("v", v)
    return "done"

g = take()
print("start", g.send(None))
try:
    print("resumed", g.send(7))
except StopIteration as stop:
    print("stop", stop.value)
"#;
    assert_eq!(
        run("send-value", script),
        "start None\nv 7\nstop done\n"
    );
}

#[test]
fn close_runs_the_finally_path_without_crashing() {
    let script = r#"closed = []

def closing():
    try:
        v = yield 1
        print("never", v)
    except GeneratorExit:
        closed.append("exit")
        raise
    finally:
        closed.append("finally")

g = closing()
print("suspended", g.send(None))
g.close()
print("closed", len(closed))
"#;
    assert_eq!(run("close", script), "suspended 1\nclosed 2\n");
}

#[test]
fn same_scope_statement_expressions_mark_generators() {
    // 三元分支 / f-string 插值 / 比较操作数 / 推导式最外层可迭代 —— 都在同一作用域求值。
    let script = r#"def branches():
    t = (yield) if True else 0
    print("ternary", t)
    s = f"<{(yield)}>"
    print("fstring", s)
    u = (yield) < 3
    print("compare", u)

def doubles():
    print("listcomp", [x * 2 for x in (yield)])

d = branches()
d.send(None)
d.send("branch")
d.send("value")
try:
    d.send(2)
except StopIteration:
    print("branches stop")

e = doubles()
e.send(None)
try:
    e.send([1, 2, 3])
except StopIteration:
    print("doubles stop")
"#;
    assert_eq!(
        run("same-scope", script),
        "ternary branch\nfstring <value>\ncompare True\nbranches stop\nlistcomp [2, 4, 6]\ndoubles stop\n"
    );
}

#[test]
fn chained_and_delegated_forms_match_the_reference() {
    let script = r#"def chained():
    a = b = yield
    print("chained", a, b)

c = chained()
c.send(None)
try:
    c.send(11)
except StopIteration:
    print("chained stop")

def delegated():
    v = yield from [1, 2]
    print("delegated", v)

h = delegated()
print("one", h.send(None))
print("two", h.send(None))
try:
    print("three", h.send(None))
except StopIteration:
    print("delegated stop")
"#;
    assert_eq!(
        run("chained-delegated", script),
        "chained 11 11\nchained stop\none 1\ntwo 2\ndelegated None\ndelegated stop\n"
    );
}

/// 子进程跑脚本并返回 `(退出码, stdout, stderr)`（失败形态用；成功形态仍走 [`run`]）。
fn run_raw(name: &str, script: &str) -> (Option<i32>, String, String) {
    let root = std::env::temp_dir().join(format!("pyawa-yield-fail-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn except_handler_type_expression_is_evaluated_in_the_enclosing_scope() {
    // 独立审查发现：`except (yield):` 的**类型表达式**在外层作用域求值（参照把 `f` 判成生成器）。
    // 先前 `statements_have_yield` 只扫 handler 的**体** ⇒ 漏标 ⇒ 运行期"非生成器函数不该让出"。
    let script = r#"import inspect

def f():
    try:
        raise ValueError("boom")
    except (yield):
        pass
    return "done"

print("isgen", inspect.isgeneratorfunction(f))
g = f()
print("first", g.send(None))
try:
    g.send(ValueError)
except StopIteration as stop:
    print("stop", stop.value)
"#;
    assert_eq!(
        run("except-handler-type", script),
        "isgen True\nfirst None\nstop done\n"
    );
}

#[test]
fn yield_from_operand_of_a_yield_is_still_rejected_in_async_functions() {
    // `yield (yield from ())`：操作数里的 `yield from` 仍属本作用域 ⇒ 参照在 async def 里报
    // `SyntaxError: 'yield from' inside async function`；查 `yield from` 的口径必须下潜 `yield` 的操作数。
    let (code, stdout, stderr) = run_raw(
        "async-yield-from",
        "async def f():\n    yield (yield from ())\n",
    );
    assert_eq!(code, Some(1), "应以语法错误退出：{stdout}{stderr}");
    assert!(
        stderr.contains("'yield from' inside async function"),
        "缺少参照的语法错误文案：{stderr}"
    );
}
