# NEXT（会话 13 · §12.12 协议派发唯一化与 ABI 收敛 · 收口）

> **项目落地（2026-11-14，§12.12 五步全部完成）**：按 `SPEC-c-abi` §17 的设计原则完成
> `AB-75`…`AB-78`——**步 1** `pa_typeid`（真实类型身份；`pa_type` 保留 `AB-63` 值类别）；
> **步 2** VM 协议 dunder 收敛（`resolve_dunder`＋`call_special_method` 唯一调用入口；
> `type_has_own_dunder`/`overrides_compare`/`has_custom_eq`/`call_compare_dunder` 薄委托；
> `protocol_unified.rs` 5 passed、core 601 passed 零回归）；**步 3** `pa_set_object_ops`
> （core 每对象 ops 表：类型/实例/模块同表、对象释放摘除；`pa_set_type_ops` 同路径投影；
> 模块注入四函数＝向模块对象写属性的投影）；**步 4** `pa_newtype` 的 `base_index` 真实 MRO
> （`ValueError` 的 `issubclass`/`isinstance`；布局边界如实）；**步 5** 投影等价差分
> （`object_ops.rs` 5 passed：三形态/NULL/隔离、type↔object 投影、模块注入↔属性写、
> `pa_tointeger`↔`pa_tointstring`、`pa_raise_host_exception`↔构造+记录）。
> `PA_ABI_VERSION` **1.12**；`SPEC-c-abi` §16 **无未实现条目**；`T-AB-11…14` 全部落地。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`12122915`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **§12.12 验收（已过）**：`type_identity.rs` 2 passed（T-AB-11/13）、`protocol_unified.rs` 5 passed、
  `object_ops.rs` 5 passed（T-AB-12/14）、`host_visibility.rs` 5 passed（§12.11 演练）；
  full workspace tests 0 failed。
- **如实边界**：`AB-77` 的基类只影响 MRO／身份（`isinstance`/`issubclass`），**不**自动适配
  宿主载荷方法；宿主异常类型的 `raise` 需异常实例布局（`AB-74` 当前走 guest 异常类，`AB-78`
  已注明依赖）；`pa_tointeger` 越 `i64` 如实报 `PA_ERR_NOTIMPLEMENTED`（大整数 ABI 通道待
  `TS-45`）；`pa_type_ops` 只覆盖属性读写/方法绑定/`__len__`/`__iter__`/`__add__` 协议
  （无 `__delattr__`、下标/比较钩子）；ABI **仍不发布**。
- **其他待办（承接）**：ABI `#2` `pa_exec_file`（等 `IM-15`）· ABI **发布**＝用户决定
  （`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）· 性能主线（dict 哈希化）·
  `CX-30` 白名单 C 类（低价值）· 本地提交 push/合并需授权。
- **口径**：`PA_ABI_VERSION`＝**1.12**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-78`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-14`；`pa_` 核心函数 **79**（§15.3 表已同步：`pa_typeid`／`pa_set_object_ops` 均在列）。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
