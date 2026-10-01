# pyawa-core

Pyawa 的 VM 核心：实例（`State`）生命周期、对象模型、帧与字节码解释器、编译管线。

## 归属规格

- `docs/SPEC-object-model.md`（`OM-`）——对象头、类型对象、引用计数协议、循环回收、弱引用
- `docs/SPEC-bytecode.md`（`BC-`）——code object、指令集契约、编译管线、帧与执行、异常表、事件点
- `docs/SPEC-imports-and-modes.md`（`IM-`）——模式开关、import 钩子、`.pyac` 的判据、重名检查
- `docs/SPEC-type-system.md`（`TS-`）——渐进类型的框架、边界检查语义、槽位语义、覆盖率报告
- **指令表与元数据的归属 crate**（`BC-30`…`BC-42`、`BC-54`）；`pyawa-stdlib` 的
  `_opcode`／`_opcode_metadata` 是它的 Python 层包装（`BC-38`、根 `Cargo.toml`）

## 本 crate 的硬约束（CI 检查靶子）

前两条有对应约束（`CX-3`／`CX-4`），**检查尚未落地**；其余四条目前**只是纪律，尚无检查**。
约束的定义见 `docs/CONSTRAINTS.md`。

| 约束 | 来源 | 约束编号 |
|---|---|---|
| 禁止全局可变状态 | `OM-1`／`OM-4`／`OM-15`／`OM-23`、`DESIGN.md` §3 不变量 2 | `CX-3`（未落地） |
| 禁止平台依赖 | `DESIGN.md` §7 原则 5 | `CX-4`（未落地） |
| 禁止以 `Rc`／`Arc` 作对象引用；禁止在业务代码裸写 incref／decref | `OM-17`／`OM-18` | `CX-6`（未落地） |
| 借用引用不得跨"可能触发 decref 的调用"保存 | `OM-19` | —（无可机械检查） |
| 循环回收清空容器须用显式待处理栈，禁止朴素递归 | `OM-21` | —（同上） |
| 实例销毁必须释放全部内存，不依赖回收器先跑完 | `OM-2` | —（同上） |

## 状态

**已有实现，但远未完整**：对象模型骨架已落地。
**已落地与尚未接线的逐条清单唯一出处为 `src/lib.rs` 的 crate 文档**（只引编号），本文件不重述。
M1 的内容与验证方式见 `docs/DESIGN.md` §12。

依赖边（本 crate 是否依赖 `pyawa-capabilities`）**仍未定**——接口形状已由
`docs/SPEC-capabilities.md`（`CP-`）定下，但**依赖方向不属该规格的范围**。
当前为零依赖的有意空档（见根 `Cargo.toml` 注释）。
