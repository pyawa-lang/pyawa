# pyawa-abi

Pyawa 的**稳定 C ABI**：只做嵌入，不做扩展模块。

## 归属规格

- `docs/SPEC-c-abi.md`（`AB-`，v0）——函数清单与预算、栈规则、错误码、
  宿主函数／类型注册、**签名元数据的格式与存放**、版本策略
- `docs/SPEC-capabilities.md`（`CP-`，v0）——能力接口 vtable 的**形状**由它定义，
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

**部分落地**：`docs/SPEC-c-abi.md` **§15** 的函数清单已写出（72 个），本 crate 已实现其中
**不依赖任何待裁口径**的那几件——

| 已落地 | 依据 |
|---|---|
| 状态码（`PA_OK`…`PA_ERR_ABI` ＋ 预留区） | `AB-19`／`AB-20` |
| 版本策略：`PA_ABI_VERSION`（主版本在高 16 位）、`version_compatible`（只比主版本）、`VersionMismatch`（可诊断信息） | `AB-39`…`AB-45`、`T-AB-4` |
| 宿主结构 `pa_host`（`abi_size` 在偏移 0）＋ **有界读取** `view_host`（`min(宿主 size, 自身 size)`，读不到就是 `None`，不落默认值） | `AB-8`／`AB-43`／`AB-40` |
| panic 边界 `boundary`（被捕获 ⇒ `PA_ERR_RUNTIME`） | `AB-3`／`CX-11`、`T-AB-2` |
| `pa_version`／`pa_abi_version`／`pa_abi_size` | `§15.3`、`AB-45` |
| 单一头文件 `include/pa.h`（含 `PA_ABI_VERSION`／`PA_ABI_SIZE` 宏） | `AB-45` |

**尚未落地**：`pa_create` 的签名待裁——`§15` 只写 `pa_create(const pa_host *)`、栈契约 `—`，
而 `AB-49` 要求返回值一律走状态码、`AB-13` 又把栈绑在实例上 ⇒ **实例经哪条路交回宿主**
这一处口径未定（连带 `T-AB-4` 的诊断信息落到哪、`pa_destroy` 之后 state 指针本身是释放
还是仅失效）。`pa_state`／`pa_destroy`／`pa_interrupt` 与其余函数都排在这条口径之后。

`unsafe` 的预期分布是**两处**：本 crate（**FFI 边界**）与 `pyawa-core`（**对象模型的内部表示**）；
其余 crate 维持 `forbid(unsafe_code)`。`OM-17`／`OM-18` 的 RAII 守卫约定在本 crate 的
宿主交接面上尤其关键。
