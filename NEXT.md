# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 07（Lead＋6 队员团队模式）已收 **round 10（M5 开局）／round 11（M5 续）**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 11」节）。

## 现在

- **HEAD**：`4b17d1c`（`dev`，本笔前，已推送 origin ✓）—— 本笔＝round 11：**CP-31 实例能力表（T-CP-3／7 收口）＋编码委派**。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 444 ＋参照口径 17 ＝ 461 ÷ 628 ⇒ 73.4%**（阈值 421/67% ✓；单次 461，按口径报区间 460–462；
  原始两次独立测量同值 461；上一轮 459 ⇒ **+2**：`encodings.gb2312`＋`encodings.gbk`）。
- **M5 判据进展（如实）**：
  - **判据① 矩阵（`T-CP-*`）＝12 绿／1 红**，仅剩 **`T-CP-10`**（取消令牌／worker／可取消等待；`CP-26`…`CP-28`／`CP-35`）；
    另有 `CP-21`／`CP-32` 机制债（VM 侧路径解析；受限 provider 现仍是过渡档）⇒ **M5 判据① 未达成**。
  - **判据② ✓**（`two_providers.rs`：同一脚本逐字节相同、真实 `PosixFs` vs 受限 `RestrictedFs`、写法不变；仍是可执行证据）。
  - **判据③**：`P3-28` 三档基线**已量** ✓（数字唯一出处＝`DESIGN.md` §9 的 M5 小节）；**阈值仍待用户拍板** ⇒ 判据③ 尚未裁定。
- **本轮落地（round 11）**：
  - **`CP-31` 统一实例能力表**：`crates/pyawa-core/src/instance/capability_table.rs`（fd→(domain, provider_handle)、
    从 3 起、free-list、dup/dup2 引用计数、`Drop for Instance` 逐条 close）＋fs/net 适配层改换算
    （fs/net 共用一张表）；**T-CP-7 收口**；T-CP-3 的 core 入口 `capability_select`／`capability_dup2`／`Instance::POLL*`。
  - **T-CP-3 收口**：`net_posix.rs` 的 `CpNetVtable.poll` 真 `poll(2)`＋`select_module.rs` 真接线（元素 int/`fileno()`、
    返回原对象、参照错误形状）＋`posix_module.rs` 的 `os.dup`/`os.dup2`；`select_dup2_shapes.rs` 与 `python3 -S`
    逐字节一致；`two_providers.rs` 的 T-CP-3 缺口探针转正。
  - **C 面**：core 编码委派（一处：`instance/convert.rs`；内置 4 编码快路径，其余 `_codecs.lookup`）⇒
    `str.encode('cp932')`／`bytes.decode('cp932')` 端到端；`_codecs.lookup` lazy `encodings`；**`_codecs_cn`**（gb2312／gbk＋cp936 别名，
    表 `codec_tables/cn.txt` 709 KB 可复现）；`multibyte_codecs.rs` 2 例对拍。
  - **小件**：`site_import_shapes.rs`（`import site`＋`sys.copyright` 逐字节＝参照 3.14.4 回归测试）。
- **未修 bug／缺口（round 12 的取活队列）**：
  ① **`T-CP-10`**（最后一条红）：取消令牌＋worker＋可取消等待（`CP-26`…`CP-28`／`CP-35`）；`clock` 域 `sleep_ns` 未接
  ⇒ `select` 空列表＋正 timeout **不真睡**（立即返回空集，如实偏差）；`pa_interrupt` 与在途可异步化调用的协作未做。
  ② `CP-21`／`CP-32` VM 侧路径解析（受限 provider 正路）＋`CP-22` cwd 实例状态；`CP-30` 注册时版本校验；
  fs／clock vtable `version` 仍为 1 未对齐 `PA_ABI_VERSION`（1.1）。
  ③ `dup`／`dup2` 到 0／1／2 未接（标准流不占表槽）；`_socket.socket(fileno=外部 fd)` 只认表内 fd；`inheritable` 不生效。
  ④ C 面剩余：`_codecs_tw/kr/hk`、gb18030／hz、utf-16／32、`unicode_escape` 族、`xmlcharrefreplace` 等错误处理器；
  `str.encode` 未知关键字仍静默忽略（pre-existing）。
  ⑤ B 批：括号内复合语句／`??=` 非名字目标；描述符：`__annotate__` 可写（卡 `cell.cell_contents`）／`__type_params__`／类级未绑定直调。
  ⑥ 既有：函数对象 `__builtins__` 未接 · union 对象 `==`／`repr` 有差 · `sys._getframe(depth>0)` · `OSError.errno` 属性未填
  （`os.read` 的异常实例上无 `.errno`；socket 线已记）· `os.lseek` 缺。

## 下一阶段（M5 收口前最后一轮功能轮 = round 12；顺序见 `PLAN-milestones.md` §9.4）

1. **`T-CP-10`**：取消令牌＋worker＋可取消等待（`CP-26`…`CP-28`／`CP-35`）；`clock` `sleep_ns` 接线 ⇒ `select` 真睡眠；
   中断路径（`pa_interrupt` → 在途调用"已取消"返回、无后续副作用、脚本不悬挂）。
2. **`CP-21`／`CP-32`** VM 侧路径解析（受限 provider 正路）＋`CP-22` cwd 实例状态；`CP-30` 注册时版本校验；
   fs／clock vtable `version` 对齐。
3. 收尾：M5 判据①②③ 全绿（③ 阈值需用户拍板）⇒ 更新 `PLAN`／`README` 状态 ⇒ **报用户批准**后 `--no-ff` 收录 `main` ＋ tag `m5`。

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；并发测量无效。**本轮实测 461/628＝73.4%**。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.1**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=1`；`CP-30`／`AB-39` 共用号）；
  `include/pa.h` 同步；**指令集版本** `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31` 已落地）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net **共用一张表**；
  `dup`／`dup2` 是表项复制＋按 `(domain, provider_handle)` 引用计数（归零才调 provider close）；`dup2` 到 0／1／2 **未接** ⇒ EBADF。
- **`select` 现状**：`fs` 域 fd（含 0／1／2）恒就绪；`net` 域 fd 走 `CpNetVtable.poll`；**空列表＋正 timeout 不真睡**（`T-CP-10`）。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-31`（新增前先查已定义；墓碑算定义）。
- **扩展特性边界**：`.py` 下扩展语法是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`cargo test -p pyawa-core --test compile` 守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **分支**：`main` 只在阶段完成时以 `--no-ff` 收录；**不 reset／不重写历史／不 force-push**（`agents-rules/branch-rule.md`）。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
