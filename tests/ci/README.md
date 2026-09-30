# tests/ci — 不变量静态检查

`REQUIREMENTS.md` 后果 10：`Lib/` 逐字同步 ⇒ 三条不变量都可 **CI 强制**。
它们都是"会红会绿"的性质，**不靠纪律维持**。本目录负责实现这些检查。

## 检查清单

| # | 检查 | 来源 |
|---|---|---|
| 1 | `Lib/` 与上游 CPython 3.14.x **文件哈希零差异**；任何差异必须出现在显式例外清单里 | `DESIGN.md` §9、`REQUIREMENTS.md` 后果 10 |
| 2 | **禁止全局可变状态**：`static mut`、进程级对象堆／类型注册表／单例、`thread_local` 极少化 | `DESIGN.md` §3 不变量 2、`OM-1`／`OM-4`／`OM-15`／`OM-23` |
| 3 | **VM 核心 crate 无平台依赖**：禁 `std::fs`／`std::net`／libc、禁 `#[cfg(target_os)]` | `DESIGN.md` §7 原则 5 |

## 相关验收编号（可直接被测试引用）

- `T-OM-7` — CI 静态检查：禁止 `static mut`／`thread_local`（`OM-1`、`OM-4`）
- `T-OM-8` — CI 静态检查：VM 核心 crate 禁用 `std::fs`／`std::net`／libc 与 `#[cfg(target_os)]`

例外清单（`DESIGN.md` §9 称"显式例外清单"）当前**必须为空**：
`REQUIREMENTS.md` 的"Lib 例外清单 = 空"是决策，且 `DESIGN.md` §9 明确
**禁止本地给 `Lib/` 打补丁**——发现的 bug 只能通过 C 层修复或推给上游，
否则例外清单会长成第二个分叉。

检查清单的规范归属待 `docs/CONSTRAINTS.md`（`CX-`）写出后固化。

## 状态

**本目录目前只有这份说明，没有任何可执行的检查。** 也就是说"靠 CI 强制"
（`DESIGN.md` §3 不变量 2、§7 原则 5，`REQUIREMENTS.md` 后果 10）**当前尚未成立**，
不要当成已就位。

| # | 检查 | 现在能否落地 |
|---|---|---|
| 1 | `Lib/` 哈希零差异 | **不能**：`Lib/` 在 M3 引入（见根 `README.md`） |
| 2 | 禁全局可变状态 | **能**：`pyawa-core` 已有足够代码让它会红会绿 |
| 3 | 禁平台依赖 | **能**：同上 |

第 2／3 项不应跟着第 1 项一起拖到 M3。落地之后，`T-OM-7`／`T-OM-8` 才从纸面变成会红会绿。
