# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 07（Lead＋6 队员团队模式）已收 **round 10（M5 开局）／round 11／round 12（功能收口）／
> round 13（M5 收口）**；逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 13」节）。

## 现在

- **HEAD**：`42cadcb`（`dev`，本笔前，已推送 origin ✓）—— 本笔＝round 13：**M5 收口**（判据③ 阈值用户拍板＋状态同步；
  `dev` 以 `--no-ff` 收录 `main` ＋ **tag `m5`**——合并提交见 `git log --merges -1`）。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **M5 已成立 ✓（判据①②③ 全绿，2026-10-18 收口）**：
  - **判据①（`T-CP-*` 全绿）✓**：矩阵 **13 绿／0 红**。可执行证据：`crates/pyawa-abi/tests/capability_acceptance.rs`（11）／
    `crates/pyawa-runtime/tests/two_providers.rs`（7）／`t_cp10_cancel.rs`（4）／`select_dup2_shapes.rs`（1）／
    `capability_registration.rs`（6）；静态两条（T-CP-6／13）由 `tests/ci/check.py` 的 `T-CX-4` 承担。
  - **判据②（两 provider 写法不变）✓**：`two_providers.rs`——同一脚本**逐字节相同**，真实 `PosixFs` vs 受限 `RestrictedFs`。
  - **判据③（启动延迟/常驻内存升为阻塞项）✓**：**阻塞阈值经用户拍板**（2026-10-18，按建议值）——阈值表与三档基线数字的
    唯一出处＝`DESIGN.md` §9 的「Pyawa（M5 实测基线）」末节；收口时实测全部在阈值内。
  - 状态同步：`README` 当前状态＝**M0–M5 均已成立**；`PLAN` §6 的「M5 的要求」＋兼容边界声明 M5 修订行；`DESIGN` §13-17 已升记录。
- **判据①（Lib import ratio）**：**通过 449 ＋参照口径 17 ＝ 466 ÷ 628 ⇒ 74.2%**（阈值 421/67% ✓；单次 466、
  按口径报区间 465–467；进度指标 450/523＝86.0%）。
- **M5 阶段的如实缺口（不阻塞判据，留队列）**：
  ① `CP-22` 的 `os.getcwd/chdir` stdlib 面未接（cwd 核心状态已落、初值是过渡口径）。
  ② `CP-21` 的"逐段 Dir 能力链"理想形态未实现（v1＝guest 绝对名＋provider 根映射；行为满足 T-CP-4）。
  ③ `T-CP-10` 未覆盖：`net.recv/send` 等无 timeout 阻塞槽位、provider 内部长内核操作；Python 层 `KeyboardInterrupt`
     投递（本层 `pa_interrupt` 是宿主级中断，如实差异）。
  ④ `AB-43` 的消费方有界读取（`min(宿主 size, 自身 size)`）未做；`CpClockVtable::UNIMPLEMENTED` 的 `version:1` 哨兵待统一。
  ⑤ 上轮遗留：`dup/dup2` 到 0/1/2 未接；`OSError.errno` 属性未填；`os.lseek` 缺；`_codecs_hk`／iso2022／gb18030／hz 未做；
     B 批（括号内复合语句／`??=` 非名字目标）与描述符（`__annotate__` 可写／`__type_params__`）剩余形态；
     socket 的 `connect/bind/listen/accept/getaddrinfo/poll` 一族未接线（`getaddrinfo`／DNS 按 A2 推迟）。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单
   `tests/conformance/divergences.md` 的复核；`MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. 取活顺序按 `PLAN-milestones.md` §9.2 队列；M5 阶段缺口（上面 ①–⑤）按"阻塞最多模块"优先。
3. 新增能力面按需另立专项（socket 的 `connect` 一族／`getaddrinfo` 推迟项、C 模块剩余面）。

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；并发测量无效。**当前 466/628＝74.2%**。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m5`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）。合并后应复核 `git diff main dev` 为空。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.1**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=1`；`CP-30`／`AB-39` 共用号）；
  `pa_setcapability` **注册时校验** vtable 公共头（`version` 主版本＋次版本方向、`size` 8–4096）；**指令集版本** `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31`）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net 共用一张表；
  `dup`／`dup2` 表项复制＋按 `(domain, provider_handle)` 引用计数（归零才调 provider close）；`dup2` 到 0／1／2 **未接** ⇒ EBADF。
- **路径解析（`CP-21`/`CP-32`）**：VM 侧 guest 绝对路径空间解析（cwd＋`.`/`..` 夹根＋逐段 `readlink`、深度 40）；
  provider 只做根映射、不再夹根；`readlink` 槽缺失时降级纯词法（如实记）。
- **取消（`T-CP-10`）**：VM 侧 10 ms 时间片轮询式可取消等待；中断 ⇒ `ExecError::Interrupted` ⇒ `PA_ERR_INTERRUPT`；
  `select` 空列表＋正 timeout 真睡；无 timeout 的阻塞 `recv/send` 尚不可取消（如实记）。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-31`（新增前先查已定义；墓碑算定义）。
- **扩展特性边界**：`.py` 下扩展语法是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`cargo test -p pyawa-core --test compile` 守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
