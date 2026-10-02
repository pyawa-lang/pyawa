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
| **虚拟栈与值转换**：`pa_gettop`／`pa_settop`／`pa_pushvalue`／`pa_pop`／`pa_type`／`pa_is*`／`pa_push*`／`pa_to*`／`pa_newtable`／`pa_newlist`／`pa_retain`／`pa_release`（类型标签取值由实现定，写进 `pa.h` 与 `src/stack.rs`） | `AB-9`…`AB-15` |
| `pa_pushbytes`／`pa_tobytes`／`pa_newhandle`：如实 `PA_ERR_NOTIMPLEMENTED`（字节串类型在 M3+；宿主对象 `OM-34` 未接线） | `AB-22` |
| 单一头文件 `include/pa.h`（含 `PA_ABI_VERSION`／`PA_ABI_SIZE` 宏） | `AB-45` |

**实例生命周期**（`AB-55`／`AB-56`／`AB-57`）：`pa_create` 经**出参**交回实例；
ABI 不匹配时返回 `PA_ERR_ABI` 并交出一个**诊断实例**（只有 `pa_errmsg`／`pa_destroy` 可用）；
`pa_destroy` **释放实例本身**；`pa_interrupt` 请求中断（按实例存，落到 `Instance::request_interrupt`）。

| **宿主类型**：`pa_newtype(st, name, payload_size, dealloc, traverse, sig)`（注册为**真实类型**；`AB-36` 要求 `dealloc` ＋ `traverse`，`traverse` 是"上下文 ＋ 回调"形态；`AB-37` 默认**可被继承**、`PA_TYPE_FINAL` 反向选择不可继承；宿主对象布局固定 ⇒ 实例字典**另行挂载**）。**`AB-58`：载荷由 VM 分配、归 VM 所有**——注册时声明尺寸，`pa_newhandle` 交回指针，宿主只填、**禁止** `free`／`realloc` | `AB-35`…`AB-38`、`AB-58`、`OM-34`…`OM-36` |
| **宿主对象创建**：`pa_newhandle(st, type_index, payload_out)`（`+1`）——`type_index` 按 `AB-9` 的**栈索引**给（宿主函数收到的类就在栈上；或先 `pa_getglobal` 把注册过的类型压栈）；`payload_size == 0` ⇒ 出参为 `NULL` | `AB-58` |
| **宿主函数**：`pa_register`（要求签名 `pa_sig`／`pa_param`，**自带尺寸**、按 `min` 有界读）／`pa_getglobal`／`pa_setglobal`／`pa_call`／`pa_pcall`／`pa_error`。宿主函数经虚拟栈收发参数、结果留栈顶（规格未钉的那条约定写在 `pa.h` 里）；`AB-26` 的 panic 捕获靠 `extern "C-unwind"` ＋ 边界 `catch_unwind` | `AB-24`…`AB-26`、`AB-51`／`AB-52` |

| **能力注册**：`pa_setcapability`／`pa_setcapability_async`（九域照 `CP-` 的表；`CP-25`：注册前必须显式声明异步分类，**缺失即失败**、不落默认值 ⇒ `T-AB-6`；`CP-2`：`NULL` vtable ＝ 整域未实现，调用时才报"未实现"） | `AB-32`…`AB-34`、`CP-25`／`CP-37` |
| **属性与下标**：`pa_getfield`／`pa_setfield`（走 `OM-11` 的 `getattr`／`setattr`）、`pa_gettable`／`pa_settable`（走 `BC-39` 的 `NB_SUBSCR`／`STORE_SUBSCR`）、`pa_rawget`／`pa_rawset`（不触发协议；本层只认 `dict` 的内部表） | `OM-11`、`BC-39`、§15.3 |

| **辅助层 `paL_`**：`checkinteger`／`optinteger`／`checkstring`／`optstring`／`len`／`getsubtable`／`ref`／`unref`／`traceback`／`error`／`execresult`／`setfuncs` 已落地；`openlibs`／`dostring`／`dofile`／`where`／`requiref` 各缺前置（标准库／编译器／traceback／模块系统）⇒ 如实 `PA_ERR_NOTIMPLEMENTED`；`paL_newstate` 在 `pyawa-runtime`（`§15.4` 指定） | `§15.4`、`AB-4`／`AB-6`、`AB-19` |

| **`.pyi` 导出**：`export::pyi(state)`（**Rust 侧** API，不是 C 导出）——遍历注册账本渲染 `.pyi`。`AB-53`：导出**只**靠注册信息（运行期不读 fs）；`AB-54`：导出与运行期读的是**同一份数据**（`pa_state.registrations`），禁止不一致 | `AB-53`／`AB-54`、`AB-31` |

| **头文件的 C／C++ 双包含**：`pa.h` 的声明外套 `extern "C"` 守卫（系统头留在守卫外），
  并有**真编译**检查（`tests/header.rs`：`cc -c` ＋ `c++ -c`，再用 `nm` 确认 `pa_create` 一类的
  导出符号**没有被 mangle**——守卫漏了这条会红） | `AB-46`、`AB-45` |

**尚未落地**：`AB-52` 的 `type_expr` **求值**——注解表达式要交给"Pyawa 自己的注解解析器"，
而那个解析器（`TS` 的注解面）还没落地；本层现在只**原样保留**表达式字符串（不发明 IDL，`AB-28`），
导出时也原样写进 `.pyi`。

**辅助层的签名**：`§15.4` **没有给签名**（只给名字与语义）⇒ 本层按 `AB-19` 统一取
"状态码 ＋ 出参"形态，**`include/pa.h` 是唯一处定义**。这是一处待补的规格缺口。

**栈契约的一处差异（已知）**：规格 §15.3 把 `getfield`／`setfield`／`gettable`／`settable`
记为 `±1`（就地替换 1 项）；本实现按自然语义取值——`setfield` **−1**（值被消耗，失败也弹）、
`settable`／`rawset` **−2**（键与值都被消耗，与 `STORE_SUBSCR` 的三元形状一致）。
`pa.h` 里逐条写明。**这一处待裁**（要么改规格记法，要么改实现）。

**尚未落地**：执行（`pa_exec_*`——字符串／文件要编译器，字节码要 `.pyac` 格式，二者分别是
`P3-12` 与编译器的事）、`pa_call` 的 `nresults != 1`（多返回值未定，如实 `PA_ERR_NOTIMPLEMENTED`）。

**已落地**（前面几轮陆续接上，别再当缺口）：宿主函数与类型注册（`pa_register`／`pa_newtype`
＋ `AB-51`…`AB-54` 的签名元数据；`AB-59` 起 `pa_newtype` 的栈契约是 `+1`，把类型对象压栈）、
属性与下标（`pa_getfield`／`pa_setfield`／`pa_gettable`／`pa_settable`／`pa_rawget`／`pa_rawset`）、
`paL_*` 辅助层。

`unsafe` 的预期分布是**两处**：本 crate（**FFI 边界**）与 `pyawa-core`（**对象模型的内部表示**）；
其余 crate 维持 `forbid(unsafe_code)`。`OM-17`／`OM-18` 的 RAII 守卫约定在本 crate 的
宿主交接面上尤其关键。
