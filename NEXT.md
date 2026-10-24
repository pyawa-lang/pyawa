# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-24 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格＋两处崩溃；
> r16：`time` 面 ⇒ `import test.support` 通过＋包装体面＋`try/return-finally`；r17：**P3-35 语法缺口批 A**（行内体／for-star／
> 链式与注解目标）＋ **`sys.exc_info()`** ＋ **`gc` 模块** ＋ 4 个 `test` 骨架同步）。逐轮复盘见
> `docs/rounds/03-m3-progress-05.md` 的 **round 14…17** 节 ✓（本笔追加 round 17）。

## 现在

- **HEAD**：`009b3a0`（`dev`，本笔前）—— 本笔＝**round 17**：P3-35 语法缺口批 A（行内套件体／for-star／链式元组与注解非名字目标／表达式语句等）＋ `sys.exc_info()`/`sys.exception()` 真实现（unittest 错误恢复）＋ `gc` 模块诚实面（P3-37 第一片）＋ 4 个 `Lib/test` 骨架同步；细节见台账 round 17。
- **工作区**：本笔待提交 ＝ **28 项**（22 个修改文件：编译器/执行器/stdlib/测试/`tools/sync_lib.py` ＋ `NEXT.md` ＋ 台账 round 17 ＋ `ROUNDS.md` 索引；＋ 6 个新增文件：4 个 `Lib/test` 骨架、`crates/pyawa-stdlib/src/gc_module.rs`、`crates/pyawa-stdlib/tests/gc.rs`；`git status --porcelain` 与之一致；无其他未跟踪项）。
- **stash**：**空** ✓
- **round 17 新增验收（全 ✓）**：
  1. **P3-35 批 A**：`typinganndata/ann_module.py`／`ann_module2.py` **exit 0**；`libregrtest/result.py`／
     `support/channels.py` 语法过（运行期分别卡 `.utils` 的 surrogate 与 `_interpchannels`，如实）；官方套件
     「冒号后面要换行」签名 **0 次**（基线 8 个文件，现均能进编译/运行期）。
  2. **`sys.exc_info()`/`sys.exception()`**：探针与参照逐行一致；官方 `test_decorators` 的故意失败用例输出正常
     `FAIL: … FAILED`，**不再 `issubclass() arg 1 must be a class` abort**（错误恢复打通）。
  3. **`gc` 模块**：`import gc` 成功、`isenabled()` True、`collect()` 真收环（函数域 list/对象环）、`get_threshold()` `(2000,10,0)`；
     `No module named 'gc'` ×10 归零；11 个能力如实 `NotImplementedError`（P3-37 挂账）。
  4. 既有验收仍全过：`import test.support` **ok**；`sys.exit(3)`⇒3；`unittest.main()`⇒0；`PYTHONPATH` 进 `sys.path`。
- **数字**（round 17 终审实测）：判据① **468/628＝74.5%**（通过 451＋参照口径 17，**+2**：`ann_module`/`ann_module2` 可 import）；
  `Lib/` 同步 **548**（`Lib/test` **65**；`sync_lib --check` **552 文件**逐字节一致，例外清单空）；进度指标 **452/548＝82.5%**；
  语料 **407**、在册差异 **3** 未变。
  `Lib/test` 体检：**可编译 347/397＝87.4%**（上轮 311；**MS-19 正向**）、**跑通 7/397＝1.8%**（持平）；
  **语法错 86→50**（其中 28 个是 surrogate 转义）、未实现 72／运行期错 261／**超时 7**（新增 `test_dictviews`／
  `test_exception_group`／`test_sort`／`test_weakset`，待查）。
- **官方套件缺口榜（下一轮取活顺序的实测依据，`target/libtests_survey.tsv`）**：
  ① `test.support.os_helper` ×72（＋`import_helper` 链）—— 卡 **P3-35 的 surrogate**（字面量/运行时 surrogateescape，
  需字符串表示层 WTF-8 级改造 ⇒ **独立立项评估**，不以假数据绕过）；
  ② `doctest` 19（诊断：导入 `pdb` ⇒ **match 的序列/映射模式 opcode** 未接）；
  ③ `unittest.mock` 13（诊断：`1j` 复数虚部 ⇒ **P3-34**，同时计划里压 52 个）；
  ④ `unicodedata` 4／`hashlib` 4（P3-37 其余）；⑤ `.pyac BadCodeSection` 8（P3-36；`test_dis`/`test_types` 已推进到它）；
  ⑥ `multibytecodec_support` ×11（只在 testsuite deb 解包目录，sync 来源没有 ⇒ 需**第二来源根**或一次性 CX-8 拷入，**待拍板**）；
  超时 7 待查（性能/挂死）。
- **本轮修掉的格不再出现在缺口榜**：「冒号后面要换行」×8、`gc` ×10、`sys.exc_info` ×30、`test_classmethod`。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序（据本轮实测调整）**：**`P3-34`（complex `1j`）**（同时是 `unittest.mock` 13 个的闸，计划 52 个）→
   **`P3-36`（`.pyac BadCodeSection` 8）** → **doctest/match 模式 opcode** → 其余 P3-37（unicodedata／hashlib／
   `multibytecodec_support` 待拍板）→ 批 B/C 语法缺口（PEP 646 `*Ts` 下标／参数注解表达式／f-string 逗号／walrus-in-tuple 等）。
   之后：**surrogate 独立立项评估**（string repr 改造）→ 超时 7 排查 → 报用户批准 M6 收口（`--no-ff` ＋ tag `m6`）。
3. **另立小格**：`builtins.compile` 缺失；嵌套类装饰器 `func.__dict__.update` 丢失；`test_memoize 3!=4`；`f_back` 链；
   帧属性完整面；`.pyac` 容器 bug（P3-36 同源）。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 17 已提交）
python3 tools/libtests_survey.py --jobs 8
# 下一格：P3-34 —— lexer 接受 1j/0.0j 等复数虚部字面量、complex 类型与算术/比较/格式化面（参照 3.14.4 对拍）
```

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 468/628＝74.5%**（通过 451 ＋ 参照口径 17）。
- **`Lib/test` 体检口径**：`tools/libtests_survey.py`（397 个顶层 `test_*.py`；默认目录＝
  `target/upstream/extracted/usr/lib/python3.14/test`）。两个比例是**报告级**、**不得当验收或完成度**（`CM-15`）。
  **改编译器之后必须清 `.pyac` 缓存**：`find target Lib -name __pyawa__ -type d -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`（陈旧判定只看源字节，看不到编译器版本 ✗）。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m5`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）。合并后应复核 `git diff main dev` 为空。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.3**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=3`；`CP-30`／`AB-39` 共用号；
  次版本 2＝round 15 `fs` 表尾 `stat_times`；次版本 3＝round 16 `clock` 表尾 `localtime`/`strftime`/`mktime`；按 `AB-41` 只升次版本）；
  `pa_setcapability` **注册时校验** vtable 公共头（`version` 主版本＋次版本方向、`size` 8–4096）；**指令集版本** `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31`）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net 共用一张表；
  `dup`／`dup2` 表项复制＋按 `(domain, provider_handle)` 引用计数（归零才调 provider close）；`dup2` 到 0／1／2 **未接** ⇒ EBADF。
- **路径解析（`CP-21`/`CP-32`）**：VM 侧 guest 绝对路径空间解析（cwd＋`.`/`..` 夹根＋逐段 `readlink`、深度 40）；
  provider 只做根映射、不再夹根；`readlink` 槽缺失时降级纯词法（如实记）。`sys.path` 最终归 `site.py`（`IM-24`），
  现在是组合根的**过渡半**（`[脚本目录, PYTHONPATH…, Lib]`）。
- **取消（`T-CP-10`）**：VM 侧 10 ms 时间片轮询式可取消等待；中断 ⇒ `ExecError::Interrupted` ⇒ `PA_ERR_INTERRUPT`；
  `select` 空列表＋正 timeout 真睡；无 timeout 的阻塞 `recv/send` 尚不可取消（如实记）。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／**`P3-37`**（新增前先查已定义；墓碑算定义）。
- **扩展特性边界**：`.py` 下扩展语法是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`cargo test -p pyawa-core --test compile` 守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋ `target/**/__pyawa__` 与 `target/upstream/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
