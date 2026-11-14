# NEXT（会话 13 · ABI 边界不可感知 · A 规格补丁）

> **本轮落地（2026-11-14，第九轮）**：用户定向「ABI 边界不可感知」的 **A 规格补丁**（阶段 A，
> `PLAN` §12.11）——`AB-73` 重写：`pa_type_ops`（`size`/`context`/`getattr_fn`/`setattr_fn`，
> 回调**必须**带 `pa_state *state`；`setattr_fn` 的 `value_idx` 借用不消耗；派发顺序＋
> `PA_ERR_NOTFOUND` 回退＋错误**不静默**；getattr 返回宿主函数 ⇒ 包成**绑定方法**且 `pa_sig`
> 含 `self`，显式传 `self` 如实报错；协议复用引实现位置）；`AB-74` 重写：完整签名
> `int pa_raise_host_exception(pa_state*, int type_idx, const char *msg, size_t len)`、`type_idx`
> 须为 `BaseException` 子类、成功返 `PA_ERR_RUNTIME`、非法 `PA_ERR_INVALID`、未映射路径不变。
> `§15.2` 新增 `PA_ERR_NOTFOUND=8`；`§15.3` 75→76（`pa_newtype` 末参 `ops`、
> `pa_raise_host_exception`）；`§15.4` 18→19（`paL_getref`）；`§13` 新增 `T-AB-8/9/10`；
> `PA_ABI_VERSION` 1.10→1.11（未发布窗口签名改写，发布后只许追加）；`PLAN` §12.11 补实施分解。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`7959a171`（本轮前；本轮提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/cargo.sh check --workspace --all-targets`（B 阶段开工前确认基线 0 警告）
- **闸门**：本轮纯文档 → `check.py` 20/20 即可提交；**B 阶段动代码后**须 `slowcheck --release`
  十二项全绿＋`check.py` 全绿＋`cargo check` 0 警告。
- **下一步（B 实现）**：①`status`/`pa.h`：`PA_ERR_NOTFOUND`、`PA_ABI_MINOR=11`；②`host.rs`：
  `pa_type_ops`/回调类型＋`HostTypeInfo` 存 ops 与 state 桥接；③`lib.rs`：`pa_newtype` 收 `ops`
  并装 `getattr`/`setattr` 槽、getattr 结果为宿主函数 ⇒ `MethodObject` 绑定、`pa_raise_host_exception`
  （记录异常→边界重抛，guest `except` 可捕获）、`paL_getref`；④`pa.h` 结构与原型；⑤测试
  `host_visibility`（派发/回退/协议/零回归/异常映射/§12.11 演练：guest 源码不含 host|handle
  字样、`is`/`id` 口径登记）。
- **已知边界（A 阶段决策，如实）**：`pa_newtype` 加末参属**未发布窗口内签名改写**（发布后只许
  追加，`AB-41`）；`getattr` 返回的函数绑定后 `pa_sig` **含 `self`**；`AB-73`/`AB-74` 仍标
  "尚未实现"（`SPEC-c-abi` §16）直到 B 阶段落地。
- **其他待办（承接）**：`CX-30` 白名单 C 类 18 处转显式（低价值）· ABI `#2` `pa_exec_file`
  （等 IM-15）· ABI **发布**＝用户决定（`DESIGN` §13-22）· 官方测试面（`array.array` 元素存储等）
  与性能主线（dict 哈希化）· 本地提交 push/合并需授权。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.11（B 阶段落地）、指令集 **2**；
  编号现最大 `TS-57`／`BC-67`／`AB-74`／`CP-39`／`CX-30`／`P3-42`／`T-AB-10`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十二项全绿才提交。
