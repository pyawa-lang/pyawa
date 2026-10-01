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

**harness 定义已就位**（`docs/PLAN-milestones.md` §5，`MS-6`…`MS-14`）；
**实现尚未开始**——本目录暂无代码。基线语料与差异清单同样待在 `MS-13`／`MS-19` 的约束下建立。
