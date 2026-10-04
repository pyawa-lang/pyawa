# tests/conformance — 与 CPython 对拍

`DESIGN.md` §12 M6 的靶子。判据已定于 `docs/PLAN-milestones.md`
（`REQUIREMENTS.md` 张力 F 已决）；本目录负责把它**跑起来**——判据存在不等于能执行。

## 归属规格

`docs/PLAN-milestones.md`（`MS-`，v0）——里程碑、验收、**对拍 harness 定义**。

## 已确定的用途

| 用途 | 依据 |
|---|---|
| 同一段脚本在 Pyawa 与 CPython 3.14 上跑，比对语义级结果 | `DESIGN.md` §12 M6 |
| 把 `Lib/` 与纯 Python 语料**全部以 `.py` 模式编译**，断言行为与 CPython 一致 | `DESIGN.md` §2.1 |
| 异常组、`finally` 中 `return`、`with` 嵌套、推导式作用域对拍 | `T-BC-5` |
| 生成器／协程挂起恢复（递归、嵌套 `yield from`）对拍 | `T-BC-6` |
| 固定脚本集上 `sys.getrefcount` 的返回值比对 | `T-OM-1` |

⚠ 对拍**不可**覆盖"实现观测面"：`sys.implementation`、`platform.*`、`sysconfig`、
`dis`／`opcode`、`gc` 统计**必然不同**（`DESIGN.md` §9）——差异清单必须显式维护，
否则对拍会被假阳性淹没。

## 状态

**harness 的减配首版已就位**（`crates/pyawa-abi/tests/conformance.rs`；语料在 `corpus/`）。
跑法：

```sh
cargo test -p pyawa-abi --test conformance
```

两个用例：`the_corpus_has_no_new_divergences`（`MS-10` 的三分类，有新差异即红——**M2 的判据
就是这个形态**）与 `the_harness_self_check_is_green`（`MS-12`／`T-MS-3`：两侧都指向参照实现
时必须全绿；自检不过时**禁止**采信任何对拍结论）。报告落在 `target/conformance/report.md`
与 `self-check.md`（`MS-14`：参照版本、语料清单与模式、三分类计数、差异清单快照）。

### 减配在哪（**不是**差异登记，`MS-19`）

| `MS-8` 要求的比对项 | 首版 | 为什么 |
|---|---|---|
| 退出码 | ✅ | — |
| 未捕获异常的类型与消息 | ✅ | — |
| 指定的全局值（探针） | ✅（标量渲染） | 通用 `repr` 要类型面再厚一些；`str`／`int`／`bool`／`None` 够用 |
| stdout／stderr | ✅ **都比**（第 93–95 轮起） | stdout：由 harness 的 `fs` 提供者按句柄 `1` 记字节、在观测区块里 `stdout_line=` 回报；stderr 同法（句柄 `2`），但**仅在两侧都正常退出时**比（否则会把参照侧的 traceback 算成差异 ✗） |

其余已实现的口径：`MS-6`（模式显式，缺了就失败）、`MS-7`（两侧各跑一次）、`MS-9`
（规范化：行尾／末尾换行 ＋ `0x…` 地址，首版语料里没有路径／耗时）、`MS-10`（三分类）、
`MS-12`（自检）、`MS-14`（报告）、`MS-15`（两侧都有超时；超时计新差异、禁止重试）、
`MS-24`（只用程序级比语义，不拿逐指令 trace 当语义对拍）。

### 实测边界（**"尚未实现"**，不进语料，`MS-19` 的适用范围）

首版语料只放**已实现**的语义；下列三条当轮实测到、按规矩**不进语料**（进去了就该红）：

| 边界 | 实测 | 去处 |
|---|---|---|
| 编译器的**下标表达式** | `x = a[1]` ⇒ `语句结尾多出了 Some(LeftBracket)` | `PLAN-milestones.md` §9.2 的 `P1-10` 行 |
| 编译器的**括号表达式** | `x = (1)` ⇒ `表达式解析到尾出现了 Some(LeftParen)` | 同上（探针注入也因此不写括号） |
| **类对象上的属性读** | `class C: v = 5` 后 `x = C.v` ⇒ `'type' object has no attribute 'v'` | §9.2 的 `P1-6` 行 |
| ABI 实例**没有 `builtins` 映射** | `raise ValueError('x')` ⇒ `NameError: name 'ValueError' is not defined` | `builtins` 模块归 `P3-14`／`CM-14` |
| **ABI 没有大整数通道** | `pa_tointeger` 对超出 `i64` 的整数返 `PA_ERR_NOTIMPLEMENTED`（**不是 0**）；`pa_tostring` 只认 `str` | 探针暂时**放不了**大整数（放进去会红，但那是通道缺失而非语义差异）；通道待定 |

> 当轮 harness 还抓到一处**可观察语义缺口**并**已修**：`pa_exec_string` 跑模块时不补
> `__name__` ⇒ 任何 `class` 语句报 `NameError`。修法是"未绑定时补 `__main__`，宿主绑过就不动"
> （CPython 的 `-c`／脚本同款），验收在 `crates/pyawa-abi/tests/abi.rs`。

**差异清单的落点是 [`divergences.md`](divergences.md)**（`MS-19` 规定，五列格式）；
它已就位并**已按 `MS-19` 的适用范围分诊**（主表只收"参照未规定"与"实现观测面"两类，
"尚未实现"移入 `PLAN-milestones.md` §9.2）——已知差异**禁止**只留在代码注释或提交说明里。

## 语料下限与规范化容差（第 270 轮裁定 ✓）

- **语料下限**（`MS-13` ①）：总数 **≥ 112** ✓，且 M2 内容面各设下限 —— **类 ≥ 15**／**异常 ≥ 11**／
  **import ≥ 14**／**生成器 ≥ 4**／**描述符 ≥ 3**／**元类 ≥ 2** ✓（现测值即下限 ✓，只许涨不许落 ✓）；
  `MS-13` ② 的 `Lib/` 语料**仍暂空** ✓（依赖 M3 ✓），已登记 ✓。
- **规范化容差**（`MS-9`）：只归一 **路径前缀**／**内存地址**（`0x…`）／**行尾与末尾换行** ✓；
  其余**逐字比** ✓。这不是"加宽比对范围" ✗ —— 归的是**表示**、不是**语义** ✓。
