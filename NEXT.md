# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-25 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格；r16：`time` 面 ⇒
> `import test.support` 通过；r17：P3-35 批 A＋`sys.exc_info`＋`gc`＋4 骨架；r18：**P3-34 complex**＋mock 同步＋**try/with 余部重放修**；
> r19：**P3-36 `.pyac` 修**＋**match 全形**＋`sys.monitoring`＋**`unicodedata`** ⇒ `import pdb`/`doctest`；
> r20：**doctest 链同步（×20→0）**＋批 B/C 语法＋**`compile()`/`f_back`**＋**`hashlib`（×4→0）**；
> r21：**非 surrogate 语法归零**（可编译 92.4%）＋core 小格批（异常子类 MRO/bytes.justify/set.union/`__dict__`/memoize）⇒ **`test_decorators` 16/16 OK**）。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…21** 节 ✓（本笔追加 round 21）。

## 现在

- **HEAD**：`45a1141`（`dev`，本笔前）—— 本笔＝**round 21**：**非 surrogate 语法缺口 10→0**（class `**`/星基类、f-string PEP 701、推导式下标目标、值模式 as、非 BMP 标识符、星迭代元组、genexp 海象；语法错总数 40→30＝全 surrogate）＋ **core 小格批**（异常子类 MRO、`bytes.ljust/rjust/center`、`set.union` 迁 core、函数 `__dict__`、dict 不可哈希键）＋ **裸元组语句 `x,`** ⇒ **`test_decorators` 16/16 OK**、可编译 92.4%；细节见台账 round 21。
- **工作区**：本笔待提交 ＝ **26 项**（`git status --porcelain` 实测：compile/parser 若干、`classes.rs`、`builtin/{bytes,dict,set}.rs`、`builtin_objects/scalars.rs`、`executor/{iter,subscript}.rs`、stdlib `_hashlib_module.rs`（删临时补丁）、`tests/task46_small_cells.rs` 新增、`NEXT.md`＋台账 round 21＋`ROUNDS.md` 等；与实测一致）。
- **stash**：**空** ✓
- **round 21 新增验收（全 ✓）**：
  1. **官方 `test_decorators` 16/16 OK**（`Ran 16 tests`；`PYTHONPATH=/usr/lib/python3.14` 直跑）。
  2. **非 surrogate 语法错 0**：语法错总数 40→**30（全部 surrogate）**、**可编译 337/397…→367/397＝92.4%**；10 个代表文件语法全过
     （class `**d`/`*基类`/泛型、f-string PEP 701、推导式下标目标、`case A.y as z`、非 BMP 标识符、`for x in *a,*b`、genexp 海象、test_re 引号）。
  3. **core 小格批**：异常子类 MRO（`test_dbcheck` 过）、`bytes.ljust/rjust/center`（hmac 回退实测）、`set.union` 迁 core（stdlib 临时补丁删除）、
     函数 `__dict__`（`test_double` 过）、dict 不可哈希键（`test_memoize` 过）—— 各格与 python3 对拍一致。
  4. **裸元组语句 `x,`** 修复（`compile("x,","test","exec")` 也过）；`@pass` 等如实 SyntaxError。
  5. 既有验收仍全过：`import test.support` ok；`sys.exit(3)`⇒3；`unittest.main()`⇒0；`PYTHONPATH` 进 `sys.path`。
- **数字**（round 21 干净基线）：判据① **477/628＝76.0%**（未变）；`Lib/` **564**（`--check` 568）、进度指标 **461/564＝81.7%**；
  语料 407／在册差异 3 未变。`Lib/test` 体检：**可编译 367/397＝92.4%**、**跑通 9/397＝2.3%**（`test_decorators` 新入；
  `test_bigaddrspace` 因冷编译 15.7s 超 30s 上限转 timeout，非语义回归）、**语法错 30（全部 surrogate）**、未实现 48、运行期错 271、**超时 39**
  （含 doctest 链 ≈15s 冷编译成本）。
- **MS-14 报告**（`target/conformance/report.md`，round 21 实测）：参照 Python 3.14.4、407 案例**全通过**、已知差异 **0**、**新差异 0**
  ⇒ **四要素齐、可发布**（`M3-COVERAGE` 硬门在 check.py 里同步绿）。
- **缺口榜（M6 收口前）**：① **surrogate epic**（`os_helper` ×106＋`multibytecodec_support` 唯一阻塞＝字符串表示层支持合法孤立代理，
  需 WTF-8 级 repr 改造，**必须用户拍板**）；② `\N{…}` 名字表 11；③ `except*` 6／`async` 推导式 4／PEP 695（`*Ts`/`**P`）／映射 `**rest`；
  ④ `list.__delitem__` 下标面、`bytearray.ljust` 旧口径、dict 真哈希表（自定义 `__hash__`）；⑤ 超时 39 甄别（冷编译成本/网络类）。
- **本战役 M6 结论（待用户拍板）**：MS-14 ✓／新差异 0 ✓；但**surrogate 未修前不满足** `MS-19` 判据补充"合法 Python 被拒必须修"
  ⇒ 已按要求如实报告并请示是否启动 surrogate epic（跨轮大改）。
## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 477/628＝76.0%**（通过 460 ＋ 参照口径 17）。
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
