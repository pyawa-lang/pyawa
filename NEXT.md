# NEXT（会话 13 · §12.17 A1–A3 已落地；ABI 冻结前收口）

> **本轮落地（2026-11-14，A1–A3）**：能力域 vtable 的 C 布局补齐（此前三份文档互相踢皮球、
> C 宿主无法实现能力域）：
> - **A1**：`crates/pyawa-abi/include/pa_capabilities.h`（`pa_cap_status` 0/1/2 ＋ 3 域
>   `fs`/`clock`/`net` vtable；**由 `tools/gen_pa_capabilities.py` 从 Rust `repr(C)` 生成**）；
>   `pa.h` include 它；C5 `PA_THANDLE` 注释修正。
> - **A2**：`examples/reference_host.c` 的最小 `clock` 域＋`pa_setcapability(CLOCK)`＋
>   `host_now_ns()`；`t_ref_host.py` 输出增 `clock=ok`（仍链最小 ABI 产物、不依赖 guest import）。
> - **A3**：`tests/ci/check_capability_layout.py`——生成物一致性＋C 探针 ↔ Rust 测试逐行
>   offset/size（56 行）＋ C/C++ 独立 include；`T-CX-19` 扫描两个头（剥 struct、只扫文件作用域
>   `pa_*` 原型、结构体不入 `AB-1`）；`selftest.py` 第二头注入用例；slowcheck/gate_chunk 均跑。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`c7f6e867`（本轮前；实现提交见 `git log`）· **工作区**：干净后提交
- **下一条命令**：`tools/slowcheck.sh --release`（十二项，含 A2/A3）
- **本轮验收**：`check.py` 20/20；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试；`slowcheck --release` 十二项；未 push、ABI 不发布。
- **已完成（可复核）**：`gen_pa_capabilities.py --check`；`check_capability_layout.py` 56 行一致；
  A3 漂移注入（改字段/加未在规格声明）两向红；`selftest.py` 25 项断言通过；`t_ref_host.py` 绿。
- **下一步（ABI 冻结前 B 组，需授权）**：B1 兼容性策略条款（minor 只追加/major 才破坏）；
  B2 `DESIGN §13-22` 与 `AB-44` 状态同步；B3 tag（如 `abi-1.14`）＋冻结生效日期。
  之后 C 组如实未提供项（`pa_exec_file`：`IM-15` 已落地、最小产物无能力提供者；`paL_dostring` 薄封装；`TS-45` 复核）。
- **如实边界**：其余 6 域（`proc`/`random`/`env`/`tty`/`locale`/`ipc`）**本 ABI 不定义布局**，
  宿主只能传 `NULL`（未提供）；能力域 vtable 字段编码钉死后随 ABI 冻结。
- **口径**：`PA_ABI_VERSION`＝**1.14**、指令集 **2**；两个公开头（`pa.h`＋`pa_capabilities.h`）；
  `pa_` 核心函数 **81**；编号最大 `TS-57`／`BC-67`／`AB-80`／`CP-39`／`CX-30`／`P3-42`／`T-AB-20`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 全绿才提交。
