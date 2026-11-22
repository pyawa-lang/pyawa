//! **P0 修复验收（CLI 子进程）**：`async with` 的 `__aexit__` 必须被 await。
//!
//! 走真实 CLI（fs/sys/stdout 全链路），因此不依赖 ABI 测试里的裸实例能力。

use std::process::Command;

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-async-with-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = Command::new(env!("CARGO_BIN_EXE_pyawa"))
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

#[test]
fn async_with_awaits_aexit() {
    let script = r#"
import asyncio
class CM:
    exited = False
    async def __aenter__(self):
        return 7
    async def __aexit__(self, et, exc, tb):
        await asyncio.sleep(0)
        CM.exited = True
        return False
async def main():
    cm = CM()
    async with cm as v:
        pass
    return (v, CM.exited)
print(asyncio.run(main()))
"#;
    assert_eq!(run("aexit", script), "(7, True)\n");
}

#[test]
fn task_group_waits_for_children() {
    let script = r#"
import asyncio
async def coro(n):
    await asyncio.sleep(0)
    return n * 2
async def main():
    async with asyncio.TaskGroup() as tg:
        a = tg.create_task(coro(4))
        b = tg.create_task(coro(5))
    return a.result() + b.result()
print(asyncio.run(main()))
"#;
    assert_eq!(run("taskgroup", script), "18\n");
}

#[test]
fn async_with_aexit_can_suppress_exception() {
    let script = r#"
import asyncio
class CM:
    exited = False
    async def __aenter__(self):
        return self
    async def __aexit__(self, et, exc, tb):
        await asyncio.sleep(0)
        CM.exited = True
        return True
async def main():
    try:
        async with CM():
            raise ValueError("boom")
    except ValueError:
        return (CM.exited, False)
    return (CM.exited, True)
print(asyncio.run(main()))
"#;
    assert_eq!(run("aexit-suppress", script), "(True, True)\n");
}
