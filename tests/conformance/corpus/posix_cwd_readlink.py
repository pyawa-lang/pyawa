# task-10：`posix.getcwd`／`posix.getcwdb`／`posix.readlink`（`sysconfig` 一族的 import 闸）。
# 两侧 cwd 都是 `cargo test` 的工作目录 ⇒ `MS-9` 把工作区路径前缀规范化成 `<WS>` 后可直接对拍；
# 报错走 `fs` 能力域 ⇒ 文案按 errno 折成 `[Errno N] …` ✓。
# harness 的 `fs` 提供者本轮也补了 `readlink` 槽（否则被测侧恒「槽未实现」⇒ 假阳性 ✗）。
import os

print(os.getcwd())
print(os.getcwdb())
print(type(os.getcwd()).__name__)
print(type(os.getcwdb()).__name__)
print(os.path.isabs(os.getcwd()))

try:
    os.readlink("no-such-link-xyz")
except OSError as exc:
    print(type(exc).__name__, str(exc))

try:
    os.readlink(b"no-such-link-xyz")
except OSError as exc:
    print(type(exc).__name__, str(exc))

try:
    os.readlink(".")
except OSError as exc:
    print(type(exc).__name__, str(exc))
