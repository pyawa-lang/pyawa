# alpha 后 dev 提交采用台账（选择性采用）

本台账是 `dev`（`fe23ed78`）相对 `main`（alpha）的 **88 个提交**的逐条裁决记录。**已全部裁决**：6 组已采用进 `main`；asyncio/JSPI/wasm-async 仅报告、不采用代码。`dev` 作为临时来源已在 2026-11-21 删除；**不建 `archive/alpha-dev`**。

- 前置：`docs/migrations/history-map.md`（pre-alpha dev 已在 `main` 历史中）。
- 来源：`git log --reverse main..dev`（88 条；`main` = `a907cf…` alpha）。
- 采收入口：一律从 `main` 切 `feat/*`；不采用 dev 作为分支基底。
- 可追溯：采用提交写 `Ported-from: <原 hash>`（或 `git cherry-pick -x`）；改造的写差异说明。
- 裁决口径：采用 / 改造采用 / 仅报告 / 丢弃；**仅报告与丢弃不进 `main`**。

## 分组汇总

| 组 | 提交数 | 裁决 | 目标 |
|---|---:|---|---|
| `docs-other` | 3 | 随迁移收口文档采用 |
| `portability` | 5 | 已采用 → main `7d1b0f0e` |
| `frame-stack` | 5 | 已采用 → main `09800d64` |
| `wasm-embedding` | 8 | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| `windows-platform` | 40 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| `win32-real-semantics` | 19 | 采用 | feat/win32-real-semantics |
| `asyncio-windows` | 3 | 不采用代码（仅报告；ADR-002） |
| `wasm-async-spike` | 5 | 仅报告（不采用；ADR-002） |

## 逐条清单

| # | 原提交 | 日期 | 标题 | 组 | 裁决 | 状态 |
|---:|---|---|---|---|---|---|
| 1 | `0369d101` | 2026-11-15 | docs(plan): 稳定版平台矩阵工作线——Linux／Windows／Web（macOS 待定） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 2 | `ebc4ab9c` | 2026-11-15 | docs: 文档一致性收尾（NEXT/README/SPEC-INDEX/REQUIREMENTS/PLAN/ROUNDS） | `docs-other` | 随迁移收口文档采用 | 已采用 → main `2884288d`（迁移收口文档） |
| 3 | `e86f82b6` | 2026-11-15 | feat(runtime): Web（WASM）E1 平台门控——wasm32-unknown-unknown 构建通过 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 4 | `ac5c02d0` | 2026-11-15 | feat(runtime): Web（WASM）E2 trampoline 层——JS 注册宿主函数、guest 可调用 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 5 | `7f15c396` | 2026-11-15 | test(wasm): E3 ABI 面验收——JS 经 pa_create 建实例、执行脚本、取错误消息 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 6 | `2faee406` | 2026-11-15 | feat(runtime): Web（WASM）E4 JS clock 能力域——fs／net 如实未提供 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 7 | `2dc1857b` | 2026-11-15 | test(ci): Web（WASM）E5 测试与 CI——wasm job ＋ 纯计算／协议派发冒烟 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 8 | `ce547de4` | 2026-11-15 | docs(release): Web（WASM）E6——RELEASE 增 Web 行／限制／验证证据 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 9 | `bb981b87` | 2026-11-15 | feat(ci): Windows W1 最小「能编译」——CLI 收进 unix 门控 ＋ windows-latest job | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 10 | `09a255f5` | 2026-11-16 | fix(ci): check.py 在 Windows（cp1252 控制台）显式切 UTF-8 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 11 | `3ca13101` | 2026-11-16 | docs(plan): Windows W1 收口——记录 CI 两次实跑（首跑编码失败→修复后三 job 全绿） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 12 | `cc6c428d` | 2026-11-16 | docs(plan): 治理 PLAN 无界增长 —— 已收口正文归档、非规范内容移出、加 CX-31 | `docs-other` | 随迁移收口文档采用 | 已采用 → main `2884288d`（迁移收口文档） |
| 13 | `9b3d0ac5` | 2026-11-16 | docs(plan): 新增 §12.22 调用栈工单（脚本调用不吃原生栈）＋登记 DIV-11 | `frame-stack` | 采用 | 已采用 → main `09800d64` |
| 14 | `375550f6` | 2026-11-16 | feat(runtime): Windows W2 clock ＋ W3 fs provider（CLI 仍「未接」；端到端经 ABI） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 15 | `9d8c336a` | 2026-11-16 | docs(plan): W2/W3 正文按 CX-31 归档到 rounds，PLAN 只留状态＋归档指针 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 16 | `292baf2d` | 2026-11-16 | docs: 记录 Windows W2/W3 的 CI 实跑（run 43846653765，clock 4/4、fs 8/8） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 17 | `26bd12c3` | 2026-11-17 | feat(runtime+stdlib): Windows W4 net provider ＋ W5 nt／sys.platform／os 分支 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 18 | `ee2c809e` | 2026-11-17 | fix(compile): universal newlines（CRLF／lone CR）—— Windows W5 的 import os 阻塞修复 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 19 | `d5e3cbaa` | 2026-11-17 | docs: 记录 Windows W4/W5 的 CI 实跑（run 43911783517，19/19）与 CRLF 修复 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 20 | `65fe53e7` | 2026-11-17 | ci(windows)+docs: W6 首批——core/lsp 测试＋MSVC 参考宿主；内存口径；Windows CLI 评估 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 21 | `9a5a598b` | 2026-11-17 | fix(ci): 参考宿主 Windows 分支——MSVC link/cl 路径优先＋UTF-8 输出 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 22 | `147613b7` | 2026-11-17 | fix(ci): 参考宿主 Windows——glob 定位 MSVC link.exe ＋ CARGO_TARGET_..._LINKER | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 23 | `74c0fc7f` | 2026-11-17 | fix(ci): 参考宿主失败时打印 cl 的两端输出（MSVC 把错误写 stdout） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 24 | `87eb8993` | 2026-11-17 | feat(example): 参考宿主移植 Windows（`PLAN §12.20` W6）——Win32 clock／临时目录／清理 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 25 | `79db9196` | 2026-11-17 | fix(ci): 参考宿主 Windows 链接补 Rust staticlib 所需的系统库 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 26 | `1aebcc1f` | 2026-11-17 | docs: W6 首批收口记录——Windows CI 全绿（run 43919581141）＋参考宿主 MSVC 实跑 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 27 | `fcfc805c` | 2026-11-17 | feat(runtime): Windows 原生 CLI（PLAN §12.23 选项 A）＋ W6 的 abi 目标门禁 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 28 | `a7305a87` | 2026-11-17 | test(windows): runtime 测试面全 targets 可编译（Unix 专测按平台 gate）＋ conformance Windows 试跑 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 29 | `6756fb9a` | 2026-11-17 | fix(conformance): 模块搜索路径按 guest 名给（Windows 上 Lib/ 导入恢复） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 30 | `2f5ce2eb` | 2026-11-17 | fix(conformance): harness fs 槽把 guest 名映射回宿主路径（Windows 导入的第二层原因） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 31 | `71dd4d4a` | 2026-11-17 | docs: Windows 原生 CLI 落地 ＋ W6 收尾进展（含 conformance Windows 基线 198/408） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 32 | `0bf8989b` | 2026-11-17 | test(conformance): harness 补 stat/fstat/listdir/stat_times（guest→host）——Windows 导入探测所需槽 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 33 | `235b3d51` | 2026-11-17 | test(conformance): W6 诊断探针（env 开关）——打印 sys.path 与各条目 listdir 结果 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 34 | `4fe9d275` | 2026-11-17 | test(conformance): 路径 canonicalize ＋ 有效诊断（nt.listdir／builtin open） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 35 | `ce34622a` | 2026-11-17 | fix(conformance): canonicalize 后剥 Windows verbatim 前缀（`\\?\`） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 36 | `bed863eb` | 2026-11-17 | fix(conformance): harness_host_path 归一分隔符并剥 verbatim／前导 `\`（Errno 123 的根因） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 37 | `224b22a8` | 2026-11-17 | test(conformance): provider 侧诊断——记录 raw 名字与映射后的宿主路径 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 38 | `0b5d6bf1` | 2026-11-17 | fix(conformance): Windows readlink 探测的 errno 映射（reparse 一族 ⇒ EINVAL） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 39 | `fb58c39f` | 2026-11-17 | fix(conformance): 把 reparse errno 映射放到 harness_readlink（上一笔误插到 harness_write） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 40 | `65b482d2` | 2026-11-17 | test(conformance): 移除诊断探针＋参照侧强制 UTF-8（Windows 真实基线） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 41 | `de9cc3e6` | 2026-11-17 | docs: W6 conformance Windows 收敛 198→364（harness 修复链）＋ 新工作单 §12.24 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 42 | `87951fc2` | 2026-11-17 | feat(stdlib): Windows-only C 模块的最小导入面（PLAN §12.24） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 43 | `4fdd3c74` | 2026-11-18 | feat(stdlib): §12.24 补 _winapi／winreg 的 from-import 常量面 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 44 | `9d8f63a6` | 2026-11-18 | feat(stdlib): §12.24 桩模块按 Lib 的 AST 全量清单补名（含导入期默认参数） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 45 | `79600926` | 2026-11-18 | feat(stdlib)+ci: §12.24 补 _multiprocessing 桩；参考 CPython 钉 3.14.4（与 Lib 同步版本一致） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 46 | `099b5012` | 2026-11-18 | docs: §12.24 首批落地归档 ＋ W6 conformance Windows 404/408（余 4 已分类） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 47 | `8ccf1d5c` | 2026-11-18 | feat(w6): Windows 表面收口——隐藏 Unix-only socket/os 名；conformance 转必需；abi 测试真跑 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 48 | `7041dd96` | 2026-11-18 | fix(abi): `pa_setpath`（AB-68）在 Windows 上把宿主路径转成 guest 名 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 49 | `9a4ce24c` | 2026-11-18 | fix(abi-test): `host_path` 的测试 provider 在 Windows 上把 guest 名映射回宿主路径 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 50 | `b33276a6` | 2026-11-18 | docs: **W6 ✓ 收口**——Windows CI 矩阵全绿（run 44290668045，conformance 必需 0 新差异、abi 19 目标） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 51 | `577402e9` | 2026-11-18 | ci(windows): runtime 运行矩阵试跑（信息性；计划线① triage 用） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 52 | `ac20424f` | 2026-11-18 | ci(windows): runtime 试跑加 `--no-fail-fast`（一次收全失败面） | `docs-other` | 随迁移收口文档采用 | 已采用 → main `2884288d`（迁移收口文档） |
| 53 | `11b4d8e6` | 2026-11-18 | test(runtime)+ci: runtime 运行矩阵 triage 完成（Windows 必需步骤）；参考宿主补 Synchronization.lib | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 54 | `d21578a6` | 2026-11-18 | docs: 计划线① runtime 运行矩阵归档（Windows 必需且绿，290 通过） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 55 | `f65fa8a8` | 2026-11-18 | test(runtime)+docs: §12.22 **S1** 验收——回调深度守卫专项（跨平台）＋ PLAN 状态 | `frame-stack` | 采用 | 已采用 → main `09800d64` |
| 56 | `b1b65954` | 2026-11-19 | feat(runtime): `winreg` 真实 Win32 语义首批（PLAN §12.24①） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 57 | `366769a9` | 2026-11-19 | fix(stdlib+w6-test): winreg 桩补全值类型常量；winstubs 改钉"真实实现" | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 58 | `0e89b3bc` | 2026-11-19 | fix(runtime): winreg 两段式大小探测——`ERROR_MORE_DATA` 不算错误；`EnumKey` 扩容重试 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 59 | `a27dd89e` | 2026-11-19 | fix(runtime): `EnumValue` 第二次调用重置 name_length（两段式缓冲长度语义） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 60 | `63dc5cfe` | 2026-11-19 | docs: §12.24 真实语义首批归档（winreg 真实 Win32 语义 + Windows CI 绿） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 61 | `83ccd9a5` | 2026-11-19 | docs(release): Windows 行更新到 W6 完成后的实际状态 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 62 | `f1119c1e` | 2026-11-19 | refactor(core): §12.22 S2 第一步——`execute` 的单帧循环抽成 `FrameSlot`／`run_frame` | `frame-stack` | 采用 | 已采用 → main `09800d64` |
| 63 | `4acbd23b` | 2026-11-19 | feat(core): §12.22 **S2/S3/S4**——显式帧栈（脚本调用不吃原生栈）＋ 1000 深度语义 ＋ wasm 验证 | `frame-stack` | 采用 | 已采用 → main `09800d64` |
| 64 | `93a3c163` | 2026-11-19 | docs: §12.22 调用栈工单 **✓ 收口**（S1–S4）＋递归上限规格条款 | `frame-stack` | 采用 | 已采用 → main `09800d64` |
| 65 | `28f7cb6c` | 2026-11-19 | feat(runtime): §12.24 真实语义第二批——`_winapi`／`_overlapped` 最小真实语义 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 66 | `347fd55f` | 2026-11-19 | fix(runtime): `_overlapped.Overlapped.__init__` 兼容两种 self 形态 | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 67 | `1832a0fc` | 2026-11-19 | fix(runtime): `_overlapped.Close()` 后地址 0 视为已关闭（避免 null 解引用 abort） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 68 | `d8e212d4` | 2026-11-19 | docs: §12.24 真实语义首批 **✓ 收口归档**（winreg ＋ `_winapi`／`_overlapped` 最小真实语义） | `win32-real-semantics` | 采用 | 已采用 → main `0cbb1ed0` |
| 69 | `6b1aeba7` | 2026-11-19 | feat(runtime): Windows 平台面补齐第一批（clock 日历三槽／fs 管道／net socketpair） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 70 | `3bfb10b2` | 2026-11-19 | fix(runtime+stdlib): Windows 平台面补齐第一批的配套（struct_time 11 元面＋形状测试更新） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 71 | `344c4816` | 2026-11-19 | fix(test): Windows 平台面测试的 struct_time 口径 | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 72 | `cb1a7048` | 2026-11-19 | feat(runtime): Windows `fs` 的 `dev`／`ino` 真值（卷序列号／文件索引） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 73 | `a8001c4b` | 2026-11-19 | docs: Windows 平台面补齐第一批 ✓ 收口（§12.20 W2/W3/W4 缺口清零） | `windows-platform` | 采用 | 已采用 → main `4b034ffa`（W1–W6）＋ `09b5c10a`（平台面补齐） |
| 74 | `ea88bf2d` | 2026-11-20 | docs(portability): 脚本可移植性契约（生死线）＋ PLON §12.26 工单＋DIV-13 | `portability` | 采用 | 已采用 → main `7d1b0f0e` |
| 75 | `52c63ea4` | 2026-11-20 | feat(portability): P0-2 可移植门——同脚本跨平台同结果（归一化执行器＋CI 必需 job） | `portability` | 采用 | 已采用 → main `7d1b0f0e` |
| 76 | `dfcc65bb` | 2026-11-20 | fix(ci): portability job 用系统 Python 3（Ubuntu 26.04 无 setup-python 3.14.4 构建） | `portability` | 采用 | 已采用 → main `7d1b0f0e` |
| 77 | `a4b7de2c` | 2026-11-20 | fix(portability): runner 汇总输出固定 UTF-8（Windows 控制台 cp1252 实测红） | `portability` | 采用 | 已采用 → main `7d1b0f0e` |
| 78 | `1a5816c2` | 2026-11-20 | docs(portability): P0-2 可移植门 ✓ 归档（24/24 Linux×Windows 一致） | `portability` | 采用 | 已采用 → main `7d1b0f0e` |
| 79 | `1e3bd26b` | 2026-11-20 | feat(runtime): P0-3 asyncio Windows 默认后端＝selector（不改 Lib/；DIV-13） | `asyncio-windows` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 80 | `22d9ea73` | 2026-11-20 | fix(runtime): asyncio 钩子改用无警告的私有 setter（Windows stderr 实测） | `asyncio-windows` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 81 | `1410354a` | 2026-11-20 | docs: §12.26 P0-3 ✓ 归档——Windows asyncio 默认后端＝selector（DIV-13 生效） | `asyncio-windows` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 82 | `9b28c080` | 2026-11-20 | feat(wasm): P1 wasm 库嵌入档——宿主 stdout／E7 库嵌入／E8 可移植子集同结果 | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 83 | `c0efbd99` | 2026-11-20 | docs: §12.26 P1 ✓ 归档——wasm 库嵌入档（E7／E8；子集同结果） | `wasm-embedding` | 采用（ADR 确认定位后） | 已采用 → main `4f9490cc`（E1–E5）＋ `a8d6ad8a`（P1） |
| 84 | `8a8f989f` | 2026-11-20 | feat(wasm): wasm async 栈切换评估——JSPI 原型接入 wasm job | `wasm-async-spike` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 85 | `e108a602` | 2026-11-20 | docs: wasm async 栈切换评估 ✓ 归档——JSPI-first（asyncio-on-wasm 仍未承诺） | `wasm-async-spike` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 86 | `55803542` | 2026-11-20 | feat(wasm): asyncio-on-wasm 里程碑——fs 读取＋内存 socketpair＋JSPI，跑通 gather/sleep | `wasm-async-spike` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 87 | `66b93438` | 2026-11-20 | fix(wasm): E3／E5／E6／E8 补 fs 读取 import 桩（新 import 面） | `wasm-async-spike` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |
| 88 | `fe23ed78` | 2026-11-20 | docs: asyncio-on-wasm（无真实 I/O）✓ 归档——E9 与 native 同一份脚本同输出 | `wasm-async-spike` | 仅报告（不采用代码；ADR-002） | 仅报告（不采用代码；ADR-002） |

## 说明

- `asyncio-windows`（P0-3）与 `wasm-async-spike`（JSPI／asyncio-on-wasm）依赖架构 ADR；在 ADR accepted 前不产生 `main` 代码。
- `docs-other` 中的计划/发布/rounds 文档随对应功能 PR 选择性采用；探索期“✓ 收口”口径需按最终决策重写后再进 `main`。
- 每完成一组，把该组“状态”改为 `已采用 → main <hash>` 或 `已报告`。
