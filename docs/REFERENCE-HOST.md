# 通用参考宿主（`examples/reference_host.c`）

> **定位**：对接方**第一个**看的东西（PLAN 收口后下一阶段 ④）。它是**规格级示例**——
> **与具体宿主身份无关**：没有网络、没有 UI、没有框架、没有"某个应用"的命名或假设；
> 下游**照抄**这串调用顺序与责任划分即可，业务逻辑由下游自己写。
>
> 相关判据的**一处真相**：`DESIGN.md` §7.5（威胁模型与宿主责任）、`SPEC-c-abi.md` 的
> `AB-5`／`AB-24`／`AB-51`／`AB-55`／`AB-57`／`AB-64`／`AB-65`／`AB-66`、`SPEC-object-model.md`
> 的 `OM-1`／`OM-41`。本文只讲"示例怎么用"，不重述判据。

## 1. 它演示什么

| # | 演示 | 对应责任／编号 |
|---|---|---|
| 1 | 版本兼容的 `pa_host` → `pa_create`；`pa_destroy` 收尾 | `AB-43`／`AB-55`／`AB-57` |
| 2 | **宿主定义脚本 API**：`host_sum`／`host_echo`／`host_read`／`host_tick` 经 `pa_register` 注入；脚本 → 宿主回调 → 回脚本 | `AB-24`／`AB-51` |
| 3 | **沙箱拒绝越界**：`host_read` 只接受宿主根下的**单段名字**（`..`／`/`／绝对路径 ⇒ `pa_error` ⇒ 脚本 `RuntimeError`） | `DESIGN` §7.4／§7.5 |
| 4 | **超时由宿主兜**：`host_tick` 检查单调时钟，超预算 `pa_interrupt` ⇒ `PA_ERR_INTERRUPT`（不是 abort） | `AB-5`①／`DESIGN` §7.5-1 |
| 5 | **每实例内存配额**：`pa_set_memory_limit`；超限在脚本里是**可捕获的** `MemoryError`；`pa_memory_used` 取指标 | `AB-65`／`AB-66`／`OM-41` |
| 6 | **实例隔离**：两个 `pa_state` 互不可见（B 读不到 A 的全局） | `OM-1`／`AB-64` |

## 2. 构建与运行（可复制粘贴）

```sh
cargo build -p pyawa-abi
cc examples/reference_host.c -I crates/pyawa-abi/include \
   target/debug/libpyawa_abi.a -lpthread -ldl -lm -o /tmp/reference_host
/tmp/reference_host
```

预期 stdout（**逐行**；CI 的 `tests/ci/t_ref_host.py` 就是钉这一串）：

```text
abi=ok
callback=42
echoed=hello
sandbox-escape=rejected
sandbox-inside=hello from host
quota=caught
memory-used=positive
instances=isolated
timeout=interrupted
done
```

缺 `cc` 或静态库 ⇒ **红**（不跳过、不弱化）：参考宿主的判据就是"真编译真运行"。

## 3. 宿主侧"照抄清单"

1. **创建**：填 `pa_host`（`abi_size`／`abi_version`／`capabilities`）→ `pa_create`；出参非空即
   必须 `pa_destroy`（诊断实例也一样，`AB-56`）。
2. **能力面**：要开哪个域就按 `SPEC-capabilities.md` 注册哪个 vtable；**一个都不注册＝默认封闭**
   （`CP-5`：`open()` 一类报 `OSError`，而不是悄悄直通宿主）。
3. **脚本 API**：`pa_register` 时**必须**带内联 `pa_sig`（`AB-25`／`AB-51`）；参数从虚拟栈取、
   返回值留在栈顶；出错用 `pa_error` 写消息（VM 转成脚本异常，**异常绝不跨 ABI 逃逸**，`AB-21`）。
4. **每实例配额**：创建后 `pa_set_memory_limit`；用 `pa_memory_used` 做看门狗。
5. **壁钟超时**：宿主自己设截止时间，到点 `pa_interrupt`；**超时后实例处于中断状态，宿主应重建
   或丢弃**（本示例把超时演示放在最后 ✓，正是这个原因）。
6. **并发**：同一实例同一时刻只允许一个宿主线程（`AB-64`）；实例之间可并行。
7. **销毁顺序**：先销毁其它实例、再销毁主实例；`pa_destroy` 之后指针不可再用（`AB-18`）。

## 4. 如实边界（✗，不掩盖）

- 本示例的 C 宿主**没有标准库**：`paL_openlibs`／`paL_dostring` 仍如实报"未提供"（`P3-14`），
  ⇒ 示例脚本只用裸语法与宿主 API，**不用** `open()`／`print()` 等 builtins；因此"C 宿主能开
  文件面"目前**不是**本示例演示的内容。
- **沙箱边界**在示例里用**宿主 API 白名单**演示（`host_read` 拒绝越界）；**能力域 vtable** 的完整
  样例是 CLI 的 Rust provider（`crates/pyawa-runtime` 的 fs／clock 实现）——形状见
  `SPEC-capabilities.md`，C 侧结构体布局与之逐字段一致。
- 示例用**静态宿主状态**（单宿主进程；多实例共享同一份沙箱配置）。要"每实例不同配置"，
  用宿主对象／句柄机制（`AB-35`）或宿主自建映射。
- ABI **尚未发布**（用户 2026-11-13 口径：暂不发布）；本示例随 `AB-65`／`AB-66` 等钩子族一起
  在冻结前可能调整——但它演示的**顺序与责任划分**是稳定的。
