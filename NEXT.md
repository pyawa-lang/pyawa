# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-25 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格；r16：`time` 面 ⇒
> `import test.support` 通过；r17：P3-35 批 A＋`sys.exc_info`＋`gc`＋4 骨架；r18：**P3-34 complex**＋mock 同步＋**try/with 余部重放修**；
> r19：**P3-36 `.pyac` 修**＋**match 全形**＋`sys.monitoring`＋**`unicodedata`** ⇒ `import pdb`/`doctest`；
> r20：**doctest 链同步（×20→0）**＋**批 B/C 语法（60→40，非 surrogate 仅剩 10）**＋**`compile()`/`f_back`**（⇒ 跑通 9）＋**`hashlib`（×4→0）**）。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…20** 节 ✓（本笔追加 round 20）。

## 现在

- **HEAD**：`2e4ac19`（`dev`，本笔前）—— 本笔＝**round 20**：**doctest/pdb/bdb/_pyrepl 同步链**（14 文件，`doctest` ×20→0）＋ **批 B/C 语法缺口**（60→40；`~x`/star 表达式/PEP 646 星号下标/多切片/目标链/注解表达式重构/装饰器表达式/walrus 等；`except*` 改诚实 Unsupported）＋ **`builtins.compile()`**（exec/eval 可用）＋ **`sys._getframe` f_back 链**（跑通 7→9）＋ **`hashlib` 最小面**（Rust md5/sha1/sha256，×4→0）；细节见台账 round 20。
- **工作区**：本笔待提交 ＝ **42 项**（23 个修改文件：compile/parser＋emitter 10 件/`builtin.rs`/`compile.rs`（core 内置）/`frame.rs`/`executor.rs`/`typecheck.rs`/`lib.rs`/`builtins_module.rs`/stdlib `lib.rs`/`tools/sync_lib.py`/`tests` ＋ `NEXT.md` ＋ 台账 round 20 ＋ `ROUNDS.md`；＋ 19 个新增文件：`Lib/{bdb,doctest,pdb,hashlib}.py`＋`Lib/_pyrepl/` 11 个、`crates/pyawa-core/src/builtin/compile.rs`、`tests/builtin_compile.rs`、`crates/pyawa-stdlib/src/_hashlib_module.rs`、`tests/hashlib.rs`；`git status --porcelain` 与之一致）。
- **stash**：**空** ✓
- **round 20 新增验收（全 ✓）**：
  1. **doctest 链**：仓库 Lib 下 `import doctest`／`pdb`／`bdb`／`_pyrepl` 全通（14 文件同步、`--check` 568 文件逐字节一致）；
     survey `No module named 'doctest'` **×20 → 0**。
  2. **批 B/C 语法**：**语法错 60→40**（30 条 surrogate 之外只剩 **10 条**）、**可编译 337→357**；`~x`/star 表达式/集合星号/
     PEP 646 星号下标/多切片/`for a[i]`/`(o.a) +=`/装饰器任意表达式/walrus/元组表达式语句/`.01j`/注解表达式重构（含
     `def f(x: -1)` 注解值真 bug）与 python3 对拍；`except*` 归诚实未实现。
  3. **`builtins.compile()`**：exec/eval 可直接用 code 对象、SyntaxError 属性齐全、filename 正确 ⇒ 官方 `test_compiler_assemble`／
     `test_compiler_codegen` 转 PASS（跑通 7→**9**）。
  4. **`sys._getframe` f_back 链**：`_getframe(1).f_code.co_name`、f_back 链/越界/类型文案照参照；`test_unicodedata`/`test_doctest`
     的该格已过（卡 `test.support.os_helper`＝surrogate epic）。
  5. **`hashlib`**：Rust md5/sha1/sha256 向量与参照一致、`_hashlib` 面/`compare_digest` 可用、`Lib/hashlib.py` 同步 ⇒
     `No module named 'hashlib'` **×4 → 0**；`hmac.compare_digest` 回归转绿。
- **数字**（round 20 干净基线）：判据① **477/628＝76.0%**（通过 460＋参照口径 17，**+7**）；`Lib/` **564**（`--check` 568）、
  进度指标 **461/564＝81.7%**；语料 407／在册差异 3 未变。`Lib/test` 体检：**可编译 357/397＝89.9%**、**跑通 9/397＝2.3%**、
  语法错 40（30 surrogate＋10 非 surrogate）、未实现 43、运行期错 260、**超时 45**（`import doctest` ≈15s CPU + 重套件；与 round 19 的
  32 不可直比）。**BadCodeSection／doctest／hashlib 三个签名均为 0**。
- **缺口榜（下一轮）**：① `test.support.os_helper` **×101**（**surrogate epic：字符串表示层支持合法孤立代理** —— 最大单点，
  **需用户拍板是否启动**；不启动则 M6「新差异为零」被它卡住）；② `\N{…}` 具名转义 11；③ 非 surrogate 语法错 10（`class A(**d)`／
  泛型类 `**kwargs`、f-string 反斜杠/嵌套引号、推导式目标含下标、值模式 `case A.y as z`、非 BMP 标识符、test_re 引号、
  test_grammar/test_named_expressions 残留）；④ `except*` 6／`async` 推导式 4／PEP 695 `**P` 2；⑤ 超时 45；
  ⑥ 小格：`hmac.new` 卡 `bytes.ljust`、`sys.exc_info` 异常子类 MRO（`with_traceback`/`__cause__` 只在精确类型）、
  `set.union` 挂 stdlib 类型字典（将来与 core 遮蔽）、`test_decorators` 余 4。
- **本轮修掉的格不再出现在缺口榜**：doctest ×20、hashlib ×4、BadCodeSection（r19）、`compile` NameError、`f_back`。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序**：**surrogate epic 立项评估（需用户拍板）** → 非 surrogate 语法错 10 → `\N{…}` 名字表 → `except*`/async 推导式/
   PEP 695 → 超时 45 甄别 → 小格（`bytes.ljust`／异常子类 MRO／`set.union` 迁移）→ 报用户批准 M6 收口（`--no-ff` ＋ tag `m6`）。
3. **如实**：M6 收口目前**被 surrogate epic 硬 gate**（`os_helper` ×101＋`multibytecodec_support` 唯一阻塞都是它；`MS-19` 判据补充
   要求"合法 Python 被拒必须修、不得登记差异"），启动前需用户明确；不启动则只能给出"带已知 gate 的收口结论"。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 20 已提交）
# 下一格：非 surrogate 语法错 10（test_class/test_type_params 的 class A(**d)、test_fstring、test_listcomps、test_patma 等）
python3 tools/libtests_survey.py --jobs 8
```

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
