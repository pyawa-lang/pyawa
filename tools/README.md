# tools — 工具链

`DESIGN.md` §10：REPL、调试器、profiler、LSP／类型检查器。
四个工具**共享同一套核心**，模式只是参数——不存在两套工具链（`DESIGN.md` §2.1）。

| 工具 | 依赖 | 备注 |
|---|---|---|
| REPL | `code`／`_pyrepl` | `_pyrepl` 是 6,946 行纯 Python，可直接移植 |
| 调试器 | `sys.monitoring` | 3.12+ 事件模型；**VM 架构约束**（`BC-19`…`BC-21`） |
| profiler | `sys.setprofile`／`sys.monitoring` | 同上；`sys.settrace`／`setprofile` 允许排在 `sys.monitoring` 之后（`BC-22`） |
| LSP／类型检查器 | 类型系统 | 在"完全不推断"前提下**只覆盖标注代码**（`DESIGN.md` §6 结构性局限） |

⚠ 事件点**不是后加功能**：`BC-19` 要求主循环里预留，`BC-20` 要求可整体关闭且关闭时不改变语义。

## 数据生成器（不是上表那四个工具）

| 脚本 | 作用 |
|---|---|
| `gen_opcode_tables.py` | 向本机 CPython 运行时探测指令表，拟合并校验后生成 **`crates/pyawa-core/src/opcode_metadata.rs`**（归属 `pyawa-core`，见 `BC-38`）；**数值不落进脚本** |
| `gen_opcode_fixture.py` | 导出 **`crates/pyawa-core/tests/fixture-opcode-3.14.json`**——对拍用的**期望值**，不是实现 |
| `gen_argval_fixture.py` | 编译 §10 语料，导出 **`crates/pyawa-core/tests/fixture-argval-3.14.json`**（每条指令的 `opname`／`oparg`／`argval`／`argrepr`）——供 `BC-59`／`T-BC-19`…`T-BC-21` 用 `dis` 作完备 oracle 对拍 |
| `gen_builtin_types.py` | 从参照实现**探测**内建类型的集合与基类关系，生成 **`crates/pyawa-core/src/builtin_types.rs`**（`TS-41`：**禁止手写枚举**）与对拍夹具 **`crates/pyawa-core/tests/fixture-builtin-types.json`** |
| `gen_code_fixture.py` | 编译若干小片段，导出 **`crates/pyawa-core/tests/fixture-code-3.14.json`**（`co_code`／`co_exceptiontable` 的 hex ＋ 每条跳转的 `dis` 偏移／`argval` ＋ `dis._parse_exception_table` 的记录）——供 `T-BC-17` 验证 `BC-55` 的跳转算术、供 `BC-54` 验证异常表解析 |
| `gen_int_fixture.py` | 任意精度整数（`TS-45`／`P1-11`）的参照夹具 → **`crates/pyawa-core/tests/fixture-int-3.14.json`**：四则／floor 整除取模／幂／比较／`hash`／十进制文本／`float` 互转／**4300 位上限的两条消息**。整数一律编码成**十进制字符串**（`i64` 装不下） |

两者都要求本机能 `import _opcode`／`_opcode_metadata`（基线 CPython 3.14）；参照实现升补丁版本时重生成。

## 测量（不是生成器）

| 文件 | 作用 |
|---|---|
| `footprint_host.c` | 足迹测量宿主，三种模式：**裸档**（`footprint_host [iterations]`：`pa_create`／`pa_exec_string` 的单调时钟时长 ＋ 本进程 RSS／`VmHWM`）· **单进程多实例档**（`--multi <instances> <iterations>`：N 个 `pa_state` 同时存活，逐实例 RSS 曲线＋总常驻＋建／干活墙钟）· **测量脚手架**（`--spawn <program> [参数…]`：从小 C 进程 fork/exec 目标，报目标的墙钟与 `ru_maxrss`——直接由 Python 父进程 fork 会污染 `ru_maxrss`，见文件头） |
| `measure_footprint.py` | 驱动（一条命令跑完三档）：**① 裸实例**（C 宿主／ABI 级，debug＋release）· **② 全栈单实例**（`pyawa` CLI 进程级，`+site`＋常用模块，同轮现测 CPython 对照）· **③ 多实例**（③a 单进程多实例＝裸 ABI ＋ ③b 多进程各一实例＝全栈 CLI，N=1/2/4/8，墙钟／并行度／峰值 RSS／总常驻）。`cargo build` → `cc` 链静态库 → 采数。**缺 `cc`／`python3`／`cargo` 即红**，不跳过 |

跑法（复跑就是重测；数字随工作树漂移）：

```
python3 tools/measure_footprint.py --profile both      # ① 两档 profile；②③ 只取 release
```

数值的**唯一出处**是 `DESIGN.md` §9 的两节实测基线：「Pyawa（M1 最小内核）实测基线」（M1 裸档的
历史快照，原文不动）与「Pyawa（M5 实测基线：三档实例体量）」（`P3-28` 的重量，含 ②③ 的口径）。
一处真相：这里只记怎么跑与脚本边界。

## 状态

**部分就位**：数据生成器已入库，**产物路径已随依赖边裁决落在 `pyawa-core`**（`BC-38`）；
`pyawa-stdlib` 只做转发。
`DESIGN.md` §10 的四个工具（REPL／调试器／profiler／LSP）仍是占位，M6 之后才有内容。
