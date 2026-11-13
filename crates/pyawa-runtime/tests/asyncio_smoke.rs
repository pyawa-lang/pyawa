//! **§12.2 验收闸：`asyncio` 冒烟**（子进程跑 pyawa；期望值由本机 `python3` 3.14.4 跑同一段脚本取得 ✓）。
//!
//! 覆盖 `PLAN-milestones.md` §12.2 验收：
//!   ① `asyncio.run(coro())` 返回正确结果；
//!   ② `await asyncio.sleep(0)`／`async for`／异步推导式可用；
//! 并钉住 `Runner.run` 的 SIGINT **记账**路径：`Lib/asyncio/runners.py:112` 读
//! `signal.getsignal(signal.SIGINT) is signal.default_int_handler`、`:116` 记账注册、`:138` 收尾恢复
//! ⇒ 脚本前后两次都必须是 `True`（若 `_signal` 仍在 `getsignal` 上抛 `NotImplementedError`，
//! `asyncio.run` 在事件循环初始化后、跑协程前即失败）。
//!
//! 为什么用子进程：`asyncio.run` 会 `set_event_loop`／建 self-pipe／`os.register_at_fork`，
//! 与测试进程共享解释器会互相污染；子进程也保证每个用例从**新实例**开始（`_signal` 记账表初值干净）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-asyncio-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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
    assert!(
        output.stderr.is_empty(),
        "stderr 必须为空：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 冒烟脚本（`python3` 3.14.4 与 pyawa 逐行一致 ✓）。
const SCRIPT: &str = r##"import asyncio
import signal

async def coro():
    return 42

async def sleeper():
    await asyncio.sleep(0)
    return "slept"

async def agen(n):
    for i in range(n):
        yield i

async def consume():
    out = []
    async for x in agen(3):
        out.append(x)
    return out

async def comp():
    return [x * 2 async for x in agen(4)]

async def nested():
    await asyncio.sleep(0)
    values = [x async for x in agen(3)]
    async for v in agen(2):
        values.append(v)
    return await sleeper(), values

print("sigint-before", signal.getsignal(signal.SIGINT) is signal.default_int_handler)
print("run", asyncio.run(coro()))
print("sigint-after", signal.getsignal(signal.SIGINT) is signal.default_int_handler)
print("async-for", asyncio.run(consume()))
print("async-comp", asyncio.run(comp()))
print("nested", asyncio.run(nested()))
"##;

/// 期望（本机 `python3` 3.14.4 跑 `SCRIPT` 实测 ✓，逐行）。
const EXPECTED: &str = r##"sigint-before True
run 42
sigint-after True
async-for [0, 1, 2]
async-comp [0, 2, 4, 6]
nested ('slept', [0, 1, 2, 0, 1])
"##;

#[test]
fn asyncio_run_and_async_syntax_work() {
    assert_eq!(run("smoke", SCRIPT), EXPECTED);
}
