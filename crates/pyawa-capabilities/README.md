# pyawa-capabilities

能力域接口的**形状**——有哪些函数、可否取消、如何表达"未实现"。
宿主**直接实现**这些接口；本 crate 只定形状，**不提供任何实现**（真实机器实现在 `pyawa-runtime`）。

## 归属规格

`docs/SPEC-capabilities.md`（`CP-`，待写）。

`docs/SPEC-INDEX.md` §4 把**能力接口 vtable 的形状**唯一划归该文件，因此本 crate 的接口定义
在该规格写出之前**不得**自行定型；`pyawa-abi`、`pyawa-stdlib` 对形状一律引用 `CP-`，不重述。

## 已确定的设计前提（来自 `REQUIREMENTS.md`／`DESIGN.md`）

| 前提 | 来源 |
|---|---|
| 接口按**能力域**切分；宿主实现哪些域 ＝ 提供哪些能力 | `DESIGN.md` §7 |
| **未实现的槽位在调用时返回"未实现"错误**——不是创建期拒绝，不是权限语义 | `DESIGN.md` §7 原则 4 |
| Pyawa **没有"权限"概念**；宿主若要拒绝，是它自己在实现里返回真实机器错误 | `REQUIREMENTS.md` 权限概念行 |
| 能力分两类：**可异步化**（只涉及裸数据／OS 句柄）与**不可异步化**（需触碰 VM 对象） | `DESIGN.md` §7.2 |
| 权威在 provider，判定在 VM——路径规范化、句柄校验、预算核算在 VM 侧 | `DESIGN.md` §7 原则 1 |
| 跨线程的能力调用**禁止**传递或触碰任何 VM 对象 | `DESIGN.md` §3 不变量 4 |
| 能力调用是**同步接口** | `REQUIREMENTS.md` 能力调用模型行 |

## 状态

**占位 crate**：接口形状待 `docs/SPEC-capabilities.md` 写出后落地。
