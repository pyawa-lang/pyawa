# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 07（Lead＋6 队员团队模式）已收 **round 10（M5 开局）**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 10（会话 07 · M5 开局）」节）。

## 现在

- **HEAD**：`367da5f`（`dev`，本笔前，已推送 origin ✓）—— 本笔＝round 10：**M5 开局（六条并行线）**。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 442 ＋ 参照口径 17 ＝ 459 ÷ 628 ⇒ 73.1%**（阈值 421/67% ✓；单次 459，按口径报区间 459–460；
  上一轮 451／71.8% ⇒ **+8**：`_codecs_jp` 的 7 个 encodings ＋ `import site`）。
- **M5 判据进展（如实）**：
  - **判据② ✓**：`crates/pyawa-runtime/tests/two_providers.rs`（同一脚本**逐字节相同**，真实 `PosixFs` vs
    受限 `RestrictedFs`，写法不变）4 绿＋2 个 `#[ignore]` 缺口探针。
  - **判据③**：`P3-28` 三档基线**已量** ✓（数字唯一出处＝`DESIGN.md` §9 的 M5 小节）；**阈值仍待用户拍板** ⇒ 判据③ 尚未裁定。
  - **判据① 未达成**：`T-CP-*` **10 绿／3 红**（红＝T-CP-3／T-CP-7 后半／T-CP-10；
    证据 `crates/pyawa-abi/tests/capability_acceptance.rs` 7 例＋`two_providers.rs`）。
- **本轮落地（round 10）**：P3-28 三档实测（`tools/measure_footprint.py`＋`footprint_host.c`＋`DESIGN.md` §9 新节）·
  T-CP 矩阵＋两 provider 验收（`capability_acceptance.rs`／`two_providers.rs`／`fs_posix.rs` 的 `RestrictedFs`）·
  B 批 `BC-62`／`BC-63`＋`TS-49`（`b_batch.rs` 18 例＋`b_batch_shapes.rs` 6 例）·
  类型级描述符＋`P3-17` 主体（`functools.wraps.__name__` 正路；`descriptor_protocol.rs`）·
  socket 首片 `socketpair` 真 I/O（`CpNetVtable`＋`net_posix.rs`＋`instance/net.rs`＋`_socket_module.rs`；
  `PA_ABI_VERSION` 1.0⇒1.1；`net_socketpair.rs` 11 例）·
  C 面 `_codecs_jp` 一族（7 个 encodings＋`codec_tables/jp.txt`＋`multibyte_module.rs`；`multibyte_codecs.rs`）。
- **未修 bug／缺口（M5 续的取活队列）**：
  ① **`CP-31` 统一实例能力表**（`fd → 能力对象`；fs／net 共用号段，销毁逐条 close）——它是 T-CP-3（`os.dup2`／`select.poll`）、
  T-CP-7 后半、T-CP-10 前件的**共同前置**；② 取消令牌＋worker（`CP-26`…`CP-28`／`CP-35`，T-CP-10）；
  ③ `CP-21`／`CP-32` 的 VM 侧路径解析（受限 provider 现是过渡档）；`CP-30` 注册时版本校验；fs/clock vtable `version` 未对齐 `PA_ABI_VERSION`；
  ④ C 面：core 的 `str.encode`／`bytes.decode` 只认 4 编码（`_codecs_jp` 端到端差这一步）＋`codecs.lookup` 需 `encodings` 先导入＋`_codecs_cn/tw/kr` 未做；
  ⑤ B 批：括号内复合语句／`??=` 非名字目标；描述符：`__annotate__` 可写（卡 `cell.cell_contents`）／`__type_params__`／类级未绑定直调；
  ⑥ 既有：函数对象 `__builtins__` 未接 · union 对象 `==`／`repr` 有差 · `sys._getframe(depth>0)`；
  ⑦ socket：`OSError.errno` 属性未填、`family/type/proto/timeout` 描述符与 `connect/bind/getaddrinfo/poll` 未接线；`getaddrinfo` 仍推迟（A2）。

## 下一阶段（M5 续；顺序见 `PLAN-milestones.md` §9.4）

1. **先落 `CP-31` 实例能力表**（T-CP-3／7／10 三红的共同前置）⇒ 再 `os.dup2`／`select.poll`／销毁逐条 close ⇒ 再取消令牌＋worker。
2. `CP-21`／`CP-32` 的 VM 侧路径解析（受限 provider 正路）；`CP-30` 注册时版本校验；fs/clock vtable version 对齐。
3. C 面续：core 委派 codecs 注册表 ⇒ `_codecs_jp` 端到端；再按同引擎加 `_codecs_cn`／`_tw`／`_kr`。
4. **M5 收口**（判据①②③ 全绿后）：更新 `PLAN`／`README` 状态 ⇒ **报用户批准**后 `--no-ff` 收录 `main` ＋ tag `m5`。

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；并发测量无效。**本轮实测 459/628＝73.1%**。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.1**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=1`；`CP-30`／`AB-39` 共用号）；
  `include/pa.h` 的 `PA_ABI_VERSION`／`PA_ABI_MINOR` 同步；**指令集版本** `INSTRUCTION_SET_VERSION`＝**2**。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-31`（新增前先查已定义；墓碑算定义）。
- **扩展特性边界**：`.py` 下扩展语法是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`cargo test -p pyawa-core --test compile` 守）。
- **socket 首片**：`socketpair` 真 I/O 已通（`CP-38`／`CP-39`）；`fd` 的 `CP-31` 统一编号**未落地**（见未修 ①）；`getaddrinfo` 仍推迟（A2）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **分支**：`main` 只在阶段完成时以 `--no-ff` 收录；**不 reset／不重写历史／不 force-push**（`agents-rules/branch-rule.md`）。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
