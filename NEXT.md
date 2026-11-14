# NEXT（会话 13 · ABI 边界不可感知 · 收口）

> **项目落地（2026-11-14，A→B2b 全部收口；`PLAN` §12.11）**：用户定向「ABI 边界不可感知」——
> **A 规格补丁**（`AB-73`/`AB-74` 可执行化、`PA_ERR_NOTFOUND=8`、`pa_set_type_ops`/`paL_getref`、
> `PA_ABI_VERSION` 1.10→1.11）→ **B1**（`pa_type_ops` 界读、core `host_getattr`、`pa_set_type_ops`、
> getattr 回退槽、三态错误）→ **B2a**（core `host_setattr`、ABI 写回退、`HostBoundMethod` 方法绑定、
> `len`/`for`/`+` 协议链路）→ **B2b**（`AB-74` `pa_raise_host_exception`：按普通构造实例化、
> 边界重抛、guest `except` 可捕获；`§12.11` 验收演练：三个宿主对象注入 guest、guest 只用对象
> 语法、源码无 `host`/`handle`/`pa_` 字样）。测试 `host_visibility` **5 passed**；`SPEC-c-abi`
> §16 **未实现条目清零**；`T-AB-8/9/10` 均有可执行验收。**本页规矩**：≤ 40 行。

- **HEAD**：`278e50f4`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项）
- **闸门**：`check.py` 20/20；`slowcheck --release` 十二项全绿；`cargo check --workspace
  --all-targets` 0 警告；`PYAWA_DANGLING=1` 全量测试绿；conformance 408×2；`bench_gate` 四档 ✓。
- **§12.11 三条判据**：①可感知痕迹只剩跨界性能与 `is`/`id`（宿主对象身份＝guest 看到的对象
  身份；绑定方法只包一层内部对象，不暴露边界）；②guest 源码只用对象语法（机检断言不含
  `host`/`handle`/`pa_`）；③透明不以"假成功"为代价（成功/未处理/出错三态可区分，`AB-22`）。
- **已知边界（如实）**：`pa_type_ops` 无 `__delattr__`／下标/比较钩子（本层只做属性读/写、
  方法绑定与 `__len__`/`__iter__`/`__add__` 协议；下标走 `pa_gettable` 等既有入口）；
  绑定方法要求 `pa_sig` **含 `self`**，显式 `obj.m(obj, x)` 按实参个数报错（不做双形态猜测）；
  `AB-74` 的 `type_idx` 指向**类型对象**（不是实例）；`ABI` **仍不发布**（用户 2026-11-13 口径）。
- **其他待办（承接）**：ABI `#2` `pa_exec_file`（等 `IM-15`）· ABI **发布**＝用户决定
  （`DESIGN` §13-22）· `CX-30` 白名单 C 类（低价值）· 官方测试面与性能主线（dict 哈希化）·
  本地提交 push/合并需授权。
- **口径**：`PA_ABI_VERSION`＝**1.11**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-74`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-10`；`pa_` 核心函数 **77**（含 9 个 `pa_type_expr_*`）＋
  `paL_` 辅助 **19**。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
