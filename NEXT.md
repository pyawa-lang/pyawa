# NEXT（会话 13 · ABI 冻结前收口：§12.17 能力域 vtable 的 C 布局）

> **本轮状态（2026-11-14）**：`AB-80` 已落地并推送（1.14；`write_tobytes.rs` 4 passed，valgrind 0 errors）。
> **新任务（`PLAN §12.17`）**：能力域 vtable 的 C 布局此前无人定义（`pa.h`／`CP-30`／`SPEC-c-abi` 三处互相踢皮球），
> C 宿主今天无法实现能力域。A1 只覆盖**已有 Rust `repr(C)` 布局的 3 域**（`fs`／`clock`／`net`）；其余 6 域
> 保持"未提供"（`CP-2`／`CP-3`），布局后续单独领号。
> **A2 取舍（已定）**：`examples/reference_host.c` **不换产物**（保持最小 ABI 定位），验收改为
> "`pa_setcapability(PA_DOMAIN_CLOCK, …)` 注册成功 ＋ 宿主回调真被调用"，不依赖 guest `import time`。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`3787beb1` · **工作区**：干净
- **下一条命令**：A1（新增 `pa_capabilities.h`：3 域结构体 ＋ `pa_cap_status`；`pa.h` include）→
  A2（clock 域示例 ＋ `t_ref_host` 同步）→ A3（布局一致性机检 ＋ `T-CX-19` 扫两个头）
- **A2 验收细化**：`reference_host.c` 新增宿主函数 `host_now_ns()`（内部调 clock vtable 的
  `now_ns`）→ guest 调它并断言拿到非负时间；`t_ref_host.py` 的 `EXPECTED` 增加对应行。
- **A1 头文件细节**：`pa_capabilities.h` 自带 include guard／`extern "C"`／`<stdint.h>`／`<stddef.h>`；
  可**独立 include**（不依赖先 include `pa.h`）；`pa_cap_status` 用 `typedef enum`（0 Ok／1 Machine／2 Unimplemented）。
- **A3 口径**：只扫**文件作用域** `pa_*` 函数原型（先剥 struct 体，结构体/函数指针字段不参与）；
  结构体不计入 `AB-1` 预算；加"声明了但规格未提及 ⇒ 红"的注入自检；`pa_cap_status` 值 `0/1/2` ≡ Rust `CapStatus`。
- **6 域边界**：`proc`／`random`／`env`／`tty`／`locale`／`ipc` 的 vtable **本 ABI 不定义布局**；
  宿主对它们只能传 `NULL`（整域未实现，`CP-2`／`CP-3`），非 `NULL` 注册不在冻结面。
- **§16 口径**：`AB-45` 的伴随头条款**自 A1 落地起生效**（此前只允许单一 `pa.h`）
- **其他待办（承接）**：ABI 冻结（等 A1–A3 ＋ 兼容性策略）· `pa_exec_file`（等 `IM-15`）· `TS-45` 复核 ·
  `PA_THANDLE` 头注释过时（随 A1 改）· 官方测试面/性能主线 · push/合并需授权
- **口径**：`PA_ABI_VERSION`＝**1.14**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-80`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-20`；`pa_` 核心函数 **81**
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 全绿才提交
