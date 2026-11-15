# alpha 发行说明（v0.1.0-alpha.1）

**本版是 alpha**：功能可供自用宿主接入，但 **ABI 未冻结** —— 后续 alpha 之间仍可能做破坏性变更
（每次都会在 `CHANGELOG.md` 列出）。ABI 的冻结点随**首个稳定发行版**（`AB-81`）。

**范围**：宿主是自己写的宿主（C/C++ 或 CPython 经 ctypes），依赖 ABI 与 CLI。
**不承诺**第三方跨语言生态、第二平台，以及本文件"已知限制"里列出的项。

## 版本

- 发行版：**v0.1.0-alpha.1**（SemVer pre-release；打 tag 前可改）
- ABI：**1.14**（`SPEC-c-abi`）；指令集：**2**
- 冻结：**未冻结**；冻结随首个稳定发行版（`AB-81`）

## 构建

```bash
tools/cargo.sh build --release                    # CLI：target/release/pyawa
tools/cargo.sh build --release -p pyawa-abi       # 最小 ABI 库（libpyawa_abi.a/.so）
tools/cargo.sh build --release -p pyawa-runtime   # 电池版组合根（含 stdlib/能力域/paL_newstate）
```

## 集成（宿主三步）

1. `#include "pa.h"` —— **唯一入口头**（它 include 了 `pa_capabilities.h`）。
2. 建实例：电池版 `paL_newstate()`；或最小 ABI `pa_create(&host, &state)`（`PA_ABI_SIZE`／`PA_ABI_VERSION` 协商）。
3. 注册宿主函数／类型（`pa_register`；`pa_newtype` ＋ `pa_set_object_ops`），再 `pa_exec_string` 执行。

完整示例见 `docs/REFERENCE-HOST.md`（七项演示），由 `tests/ci/t_ref_host.py` **真编译真运行**验收。

## 支持矩阵（如实）

| 项 | 状态 |
|---|---|
| 平台 | **Linux x86_64（唯一已验证）** |
| 宿主语言 | C／C++（`pa.h`）；CPython 经 ctypes（已验证） |
| ABI | 1.14，**alpha 期未冻结** |
| 指令集 | 2 |

## 已知限制（本版不承诺）

- `pa_exec_file`／`paL_dofile` **需要 `fs` provider**：电池版 `paL_newstate` 自带；最小 ABI 产物需宿主注册 `fs`，否则如实 `PA_ERR_NOTIMPLEMENTED`。
- `paL_where` 是**尽力口径**（当前帧/`f_back` 的 `code:line`），**没有完整 traceback**（`OM-28`）；`paL_errorf` 仅为 C 头内联格式化（不新增导出符号）；`paL_requiref` 依赖模块系统（无模块表的裸实例如实未提供）。
- 能力域：仅 `fs`／`clock`／`net` 三域有 C 布局；其余 6 域调用时"未实现"（`CP-2`／`CP-3`）。
- buffer 面只到"一次性写回"（`pa_writetobytes`）；无 `memoryview`、无长期可写视图、无多维 buffer。
- 第二平台未验证；`Lib/test` 通过率未作为本版承诺。

## 验证证据

- 闸门 `tests/ci/check.py` **20/20**；`tools/slowcheck.sh --release` 十二项。
- 新 ABI 路径（`AB-73`…`AB-80`）valgrind **0 错误**；参考宿主七项演示绿。
- 布局一致性机检 `tests/ci/check_capability_layout.py` **PASS**（C↔Rust 56 行）。
