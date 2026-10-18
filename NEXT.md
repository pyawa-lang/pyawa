# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-18 ✓）：会话 07（Lead＋6 队员团队模式）已收 **round 10（M5 开局）／round 11／round 12（M5 功能收口）**；
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` ✓（本笔追加「round 12」节）。

## 现在

- **HEAD**：`ed7a185`（`dev`，本笔前，已推送 origin ✓）—— 本笔＝round 12：**`T-CP-10` 收口（矩阵 13/13）＋路径解析＋版本校验＋tw/kr codecs**。
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓；以本笔提交为准）。
- **stash**：**空** ✓
- **判据①（Lib import ratio）**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 449 ＋参照口径 17 ＝ 466 ÷ 628 ⇒ 74.2%**（阈值 421/67% ✓；单次 466，按口径报区间 465–467；
  上一轮 461 ⇒ **+5**：`encodings.{big5,cp950,euc_kr,cp949,johab}`）。
- **M5 判据进展（如实）**：
  - **判据①（`T-CP-*` 全绿）✓ 达成**：矩阵 **13 绿／0 红**（round 12 收 `T-CP-10`）。
    证据：`capability_acceptance.rs`（11 passed）／`two_providers.rs`（7 passed）／`select_dup2_shapes.rs`（1）／
    `t_cp10_cancel.rs`（4）／`capability_registration.rs`（6）／`check.py` 的 T-CX-4（T-CP-6／13）。
  - **判据② ✓**（`two_providers.rs`：同一脚本逐字节相同、真实 `PosixFs` vs 受限 `RestrictedFs`、写法不变）。
  - **判据③ 待用户拍板**：`P3-28` 三档基线已量（数字唯一出处＝`DESIGN.md` §9 的 M5 小节）；
    "启动延迟与常驻内存升为阻塞项"的**阈值**需用户决定 ⇒ 判据③ 尚未裁定。
- **本轮落地（round 12）**：
  - **`T-CP-10`**：`instance/wait.rs` 的 **VM 侧 10 ms 时间片可取消等待**（不改 ABI；`CP-27/28`）；`interrupted`
    原子化（`AtomicBool`）；`clock.sleep_ns` 真实现；`time.sleep`＋`select` 真睡/可取消；中断 ⇒ `PA_ERR_INTERRUPT`
    （实测挂起 300 ms 后中断 302.5 ms 返回、后续语句不执行）。
  - **`CP-21`／`CP-32`**：`instance/path.rs` VM 侧解析（guest 绝对名＋`.`/`..` 夹根＋逐段 readlink、ELOOP 上限 40）；
    `RestrictedFs` 退化为根映射（夹根判定搬 VM）；`path_resolution.rs` 的 **NaiveFs 反证**（零检查 provider 下逃逸全 `ENOENT`）；
    `CP-22` 部分（实例 cwd 状态，`os.getcwd/chdir` stdlib 面未接）。
  - **`CP-30`**：`pa_setcapability` 注册时校验 vtable `(version,size)`（`AB-41`；失败 `PA_ERR_ABI` 可诊断）；
    fs/clock/net vtable `version` 对齐 `PA_ABI_VERSION`（1.1）。
  - **C 面**：`_codecs_tw`（big5／cp950）＋`_codecs_kr`（euc_kr／cp949／johab）；4 族跨查表、按族 `getcodec`；
    `multibyte_codecs.rs` 3 例对拍（113 行输出逐字节＝参照 3.14.4）。
- **未修 bug／缺口（不阻塞 M5 收口，留后续队列）**：
  ① `CP-22` 的 `os.getcwd／os.chdir` stdlib 面未接（cwd 初值取宿主进程 cwd 是过渡口径）。
  ② `CP-21` 的"逐段 Dir 能力链"理想形态未实现（v1＝guest 绝对名＋provider 根映射；行为满足 T-CP-4）。
  ③ `T-CP-10` 未覆盖：`net.recv/send` 等无 timeout 阻塞槽位、provider 内部长内核操作（只在调用之间可取消）；
     Python 层 `KeyboardInterrupt` 投递（本层 `pa_interrupt` 是宿主级中断，如实差异）。
  ④ `AB-43` 的消费方有界读取（`min(宿主 size, 自身 size)`）未做；`CpClockVtable::UNIMPLEMENTED` 的 `version:1` 哨兵待统一。
  ⑤ 上轮遗留：`dup／dup2` 到 0／1／2 未接；`OSError.errno` 属性未填；`os.lseek` 缺；`_codecs_hk`／iso2022／
     gb18030／hz 未做；B 批（括号内复合语句／`??=` 非名字目标）与描述符（`__annotate__` 可写／`__type_params__`）剩余形态。

## 下一阶段（round 13 = **M5 收口轮**，必须用户批准）

1. **判据③ 阈值**：把"启动延迟／常驻内存升为阻塞项"的阈值提案报用户拍板，并写进 `PLAN`／`DESIGN`（一处真相）。
2. 更新 `README`／`PLAN` 状态为 **M0–M5 均已成立**（`DESIGN.md` §13-17 的"提示项"→"阻塞项"记录）。
3. **报用户批准**后：`--no-ff` 收录 `main` ＋ **tag `m5`**（`agents-rules/branch-rule.md`：收口前必须批准）。
4. 不阻塞收口的后续队列见上"未修 bug／缺口"。

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**；单次 ±1 ⇒ 报区间；并发测量无效。**本轮实测 466/628＝74.2%**。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.1**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=1`；`CP-30`／`AB-39` 共用号）；
  `pa_setcapability` **注册时校验** vtable 公共头（`version` 主版本＋次版本方向、`size` 8–4096）；**指令集版本** `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31`）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net 共用一张表；
  `dup`／`dup2` 表项复制＋按 `(domain, provider_handle)` 引用计数（归零才调 provider close）；`dup2` 到 0／1／2 **未接** ⇒ EBADF。
- **路径解析（`CP-21`/`CP-32`，round 12）**：VM 侧 guest 绝对路径空间解析（cwd＋`.`/`..` 夹根＋逐段 `readlink`、深度 40）；
  provider 只做根映射、不再夹根；`readlink` 槽缺失时降级纯词法（如实记）。
- **取消（`T-CP-10`，round 12）**：VM 侧 10 ms 时间片轮询式可取消等待；中断 ⇒ `ExecError::Interrupted` ⇒ `PA_ERR_INTERRUPT`；
  `select` 空列表＋正 timeout 真睡；无 timeout 的阻塞 `recv/send` 尚不可取消（如实记）。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-31`（新增前先查已定义；墓碑算定义）。
- **扩展特性边界**：`.py` 下扩展语法是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`cargo test -p pyawa-core --test compile` 守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋`target/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **分支**：`main` 只在阶段完成时以 `--no-ff` 收录；**不 reset／不重写历史／不 force-push**（`agents-rules/branch-rule.md`）。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
