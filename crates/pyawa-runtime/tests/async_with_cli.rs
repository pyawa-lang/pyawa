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

/// **Windows Proactor subprocess**：`create_subprocess_exec` + `communicate()`。
/// 需要 CLI 的 Windows 兼容层（`os.close` 合成 fd、`_ProactorBasePipeTransport._empty_waiter`、
/// `Overlapped.address`/`BrokenPipeError`）。**不显式设策略**：走 CLI 默认（Proactor，`P4`）。
#[cfg(windows)]
#[test]
fn proactor_subprocess_communicate_returns_stdout() {
    let script = r#"
import asyncio
async def main():
    proc = await asyncio.create_subprocess_exec(
        "cmd.exe", "/d", "/c", "echo", "hi",
        stdin=None, stdout=asyncio.subprocess.PIPE, stderr=None)
    out, err = await asyncio.wait_for(proc.communicate(), 10)
    return out.strip(), err, proc.returncode
print(asyncio.run(main()))
"#;
    assert_eq!(run("proactor-subprocess", script), "(b'hi', None, 0)\n");
}

/// **Windows Proactor TCP echo**：`start_server`（AcceptEx）＋ `open_connection`（ConnectEx）。
/// `P4` 起 CLI 默认策略就是 Proactor，这里同样**不显式切换**。
#[cfg(windows)]
#[test]
fn proactor_tcp_echo_accept_connect() {
    let script = r#"
import asyncio
async def handle(reader, writer):
    data = await reader.readline()
    writer.write(data)
    await writer.drain()
    writer.close()
async def main():
    server = await asyncio.start_server(handle, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    reader, writer = await asyncio.open_connection("127.0.0.1", port)
    writer.write(b"hello\n")
    await writer.drain()
    line = await asyncio.wait_for(reader.readline(), 10)
    writer.close()
    server.close()
    await server.wait_closed()
    return line
print(asyncio.run(main()))
"#;
    assert_eq!(run("proactor-tcp-echo", script), "b'hello\\n'\n");
}

/// **Windows 默认事件循环 = Proactor**（`P4`，round 282）：CLI 不再用 selector shim。
///
/// 只断言 loop 类型（`new_event_loop` 走默认策略）；不调 `get_event_loop_policy`——
/// 它在本层（CPython 3.14 口径）已弃用，会往 stderr 打 `DeprecationWarning`，而 `run` 要求
/// stderr 为空。
#[cfg(windows)]
#[test]
fn windows_default_loop_policy_is_proactor() {
    let script = r#"
import asyncio
loop = asyncio.new_event_loop()
print(type(loop).__name__)
loop.close()
"#;
    assert_eq!(run("default-policy", script), "ProactorEventLoop\n");
}

/// **Proactor 默认策略下真实定时器可用**（selector 时代 `sleep(>0)` 曾有 `InvalidStateError`）。
#[cfg(windows)]
#[test]
fn windows_default_proactor_timer_runs() {
    let script = r#"
import asyncio
async def main():
    await asyncio.sleep(0.01)
    return "timer-ok"
print(asyncio.run(main()))
"#;
    assert_eq!(run("proactor-timer", script), "timer-ok\n");
}

/// **selector 逃生口仍可用**：脚本显式切策略仍能跑（文档里承诺的回退方式）。
#[cfg(windows)]
#[test]
fn windows_selector_opt_in_still_works() {
    let script = r#"
import asyncio
import asyncio.events
import asyncio.windows_events
asyncio.events._set_event_loop_policy(
    asyncio.windows_events._WindowsSelectorEventLoopPolicy())
async def main():
    await asyncio.sleep(0)
    return "selector-ok"
print(asyncio.run(main()))
"#;
    assert_eq!(run("selector-opt-in", script), "selector-ok\n");
}

/// **Windows 口径的 `OSError.winerror`**：属性恒存在（非 Win32 错误为 `None`），子类继承。
/// asyncio 的 `exc.winerror` 分支（`windows_events.py:591`／`:650`）因此不会 AttributeError。
#[cfg(windows)]
#[test]
fn windows_oserror_has_winerror_attribute() {
    let script = r#"
print(getattr(OSError("x"), "winerror", "missing"))
print(getattr(FileNotFoundError(2, "nope"), "winerror", "missing"))
"#;
    assert_eq!(run("oserror-winerror", script), "None\nNone\n");
}

/// **Windows Proactor 命名管道**（`_overlapped.ConnectPipe` ＋ `PipeHandle`）：
/// `start_serving_pipe`（服务端 ConnectNamedPipe）＋ `create_pipe_connection`（客户端 ConnectPipe）
/// 走重叠 ReadFile／WriteFile 完成一次双向收发。
#[cfg(windows)]
#[test]
fn proactor_named_pipe_connect_roundtrip() {
    let script = r#"
import asyncio

ADDRESS = r'\\.\pipe\pyawa-proactor-pipe-test'

class ServerProtocol(asyncio.Protocol):
    def connection_made(self, transport):
        transport.write(b"hello-from-server\n")

class ClientProtocol(asyncio.Protocol):
    def __init__(self, done):
        self.done = done
        self.data = b""

    def connection_made(self, transport):
        transport.write(b"hello-from-client\n")

    def data_received(self, data):
        self.data += data
        if b"hello-from-server" in self.data:
            self.done.set_result(self.data)

async def main():
    loop = asyncio.get_running_loop()
    servers = await loop.start_serving_pipe(ServerProtocol, ADDRESS)
    done = loop.create_future()
    transport, _ = await loop.create_pipe_connection(
        lambda: ClientProtocol(done), ADDRESS)
    data = await asyncio.wait_for(done, 10)
    transport.close()
    for server in servers:
        server.close()
    return data

print(asyncio.run(main()))
"#;
    assert_eq!(run("proactor-named-pipe", script), "b'hello-from-server\\n'\n");
}
