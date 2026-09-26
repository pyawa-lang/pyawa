# pyawa-stdlib

CPython **C 实现层**的 Rust 重写：`_io`、`posix`、`_sre`、`_socket`、`_json`、`_struct`…
保的是**从 Python 看到的 API 与语义**（`DESIGN.md` §9 的第二类兼容），
**不是** CPython 的 C-API／ABI，也不要求数据格式跨实现兼容。

## 归属规格

- `docs/SPEC-c-modules.md`（`CM-`，待写）——113 个模块的实现顺序与逐模块合约
- `docs/SPEC-capabilities.md`（`CP-`，待写）——需要外部世界权威的模块（`posix`／`_io`／`_socket`…）
  在这里调用能力接口，接口形状引 `CP-`，本 crate 不自行定义

## 已知分界

本 crate 里有两类模块，界线由能力接口划定：

| 类别 | 例子 | 特性 |
|---|---|---|
| **需能力接口** | `posix`、`_io`、`_socket`、`select`、`time` | 走能力层，是**唯一的 I/O 出口**；宿主未实现的域在调用时报"未实现" |
| **纯计算** | `_sre`、`_struct`、`_json`、`_csv`、`math`、`binascii` | 不碰外部世界，无不变量风险 |

`DESIGN.md` §9 的解锁曲线（前 5 个 `sys`／`itertools`／`time`／`errno`／`builtins`
解锁 67% 的 `Lib/` 可 import）是**实现顺序**依据，不是估工依据。

## 硬约束

| 约束 | 来源 |
|---|---|
| **禁止**把 OS 与 libc 直接拖进模块实现——外部世界一律经能力层 | `DESIGN.md` §3 不变量 1 |
| 编译产物是自有的 `.pyac`，与 CPython `.pyc` 无关且不兼容 | `DESIGN.md` §2.1 |
| `marshal` 只需**存在**并保 Python 层行为（`_bootstrap_external.py` 会 import 它） | `DESIGN.md` §9 |
| `_opcode`／`_opcode_metadata` 暴露 **Pyawa 自己的**指令表与元数据，`opcode.py`／`dis.py` 一字不动 | `DESIGN.md` §9、`BC-1`／`BC-4` |
| `ctypes`／`_ctypes`／`_testcapi`／`_testbuffer`／`xxlimited*` 走"未实现"路径，**不是**兼容性破坏 | `REQUIREMENTS.md` C-API 概念模块行 |
| `sys.implementation.name` **必须**报 `pyawa` | `DESIGN.md` §9 |

## 状态

**占位 crate**：只有骨架。实现顺序与逐模块合约待 `docs/SPEC-c-modules.md`。
