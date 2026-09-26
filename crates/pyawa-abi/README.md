# pyawa-abi

Pyawa 的**稳定 C ABI**：只做嵌入，不做扩展模块。

## 归属规格

- `docs/SPEC-c-abi.md`（`AB-`，待写）——函数清单与预算、栈规则、错误码、
  宿主函数／类型注册、**签名元数据的格式与存放**、版本策略
- `docs/SPEC-capabilities.md`（`CP-`，待写）——能力接口 vtable 的**形状**由它定义，
  本 crate 只写"如何注册它"，形状一律引 `CP-`（`docs/SPEC-INDEX.md` §4）

## 本 crate 的硬约束

| 约束 | 来源 |
|---|---|
| **panic 绝不允许跨 FFI 边界**——每个入口 `catch_unwind` | `DESIGN.md` §3 不变量 3 |
| 公开接口**不得出现泛型／单态化** | `REQUIREMENTS.md` 后果 4 |
| 对象头**禁止**出现在任何 ABI 签名里；宿主只见**不透明句柄** | `OM-6` |
| 线格式为**虚拟栈**（Lua 风格）；栈是稳定的线格式，不随语言演进变化 | `DESIGN.md` §8.1 |
| 异常绝不跨边界逃逸；签名是类型元数据，并可生成 `.pyi` 存根 | `DESIGN.md` §8.4／§8.2 |
| **ABI 版本策略**必须在第一个公开版本定死（嵌入 API ＋ provider 契约 ＋ 宿主对象契约三者同时冻结） | `DESIGN.md` §8.4、§13-2 |

## 状态

**占位 crate**：函数清单与栈规则待 `docs/SPEC-c-abi.md` 写出后落地。
本 crate 是工作区内**唯一**预期需要 `unsafe` 的地方（FFI 边界）；`OM-17`／`OM-18` 的
RAII 守卫约定在此处与宿主交接面上尤其关键。
