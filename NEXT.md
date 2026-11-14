# NEXT（会话 13 · 宿主协议唯一入口 oracle 已落地；§12.14 ✓）

> **本轮落地（2026-11-14）**：按用户「对象语义唯一入口＝属性查找」不变量完成三步（§12.14）：
> 1. **唯一协议清单**：`crates/pyawa-core/src/protocols.rs` 的 `PROTOCOL_CASES`（协议→dunder→
>    oracle 用例；属性钩子/描述符的排除理由写在模块注释）。
> 2. **清洗旁路**：`str`/`repr`（`override_text` 去掉类型 MRO 预判、宿主通道优先）、`format`
>    （空 spec＋f-string `FORMAT_SIMPLE`）、`hash`、`==`/`!=`（快路＋`__ne__` 默认）、`contains`、
>    `abs`、全部原位算术、异步 `__aiter__`（GET_AITER 的 `Owned` 绑定）：统一经
>    `Instance::call_protocol_method`（宿主优先→类型/实例属性通道）。
>    内建标量快路保留，`protocol_host_dispatch.rs` 钉等价；用户类 `__format__` 的 f-string 空 spec
>    也一并修正。
> 3. **oracle 进 CI**：`protocol_oracle.rs`（记录式 `getattr_fn` 对**全清单**逐条断言"被问过"；
>    清单↔片段双向一致；无 numpy）＋`protocol_host_dispatch.rs`（行为生效/无 ops 零回归/快路
>    等价）；workspace 测试、`slowcheck` 步骤 5、`gate_chunk small` 都跑。
> **本页规矩**：≤ 40 行；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`9f1fe38b`（本轮前；本轮提交见 `git log`）· **工作区**：干净后提交 ✓
- **下一条命令**：`tools/slowcheck.sh --release`（十二项，含协议 oracle 两份测试）
- **本轮验收**：`check.py` 20/20；`cargo check --workspace --all-targets` 0 警告；
  `PYAWA_DANGLING=1` 全量测试；`slowcheck --release` 十二项；未 push、ABI 不发布。
- **oracle 口径（勿弱化）**：清单↔片段双向一致（少片段/多片段都红）；记录回调一律
  `PA_ERR_NOTFOUND`（只证明"问了"，不靠返回值让机检变绿）；`__exit__` 例外返回 nil 仅为了走到
  `__enter__` 的查找；行为生效另有 `protocol_host_dispatch.rs` 固定值用例。
- **如实边界**：属性钩子（`__getattr__`/`__getattribute__`/`__setattr__`/`__delattr__`/`__dir__`）
  与描述符（`__get__`/`__set__`/`__delete__`）不在 oracle 清单：前者**就是**属性通道本身，后者是
  类属性机制；要纳入需另定"记录什么、怎么判"的口径。
- **其他待办（承接）**：ABI 发布（用户决定；版本 **1.13**）· `pa_exec_file`（等 `IM-15`）·
  `TS-45` 大整数 ABI · 官方测试面/性能主线 · push/合并需授权。
- **口径**：`PA_ABI_VERSION`＝**1.13**、指令集 **2**；编号现最大 `TS-57`／`BC-67`／`AB-79`／
  `CP-39`／`CX-30`／`P3-42`／`T-AB-19`；`pa_` 核心函数 **80**。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；动代码须 slowcheck 全绿才提交。
