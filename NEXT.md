# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-24 ✓）：会话 10＝**M6**（round 14 开局：`P3-32`＋`P3-33`；round 15：`else`＋`with` 编译器格
> ＋两处崩溃；round 16：**`time` 面解锁 ⇒ `import test.support` 通过** ＋ `classmethod`/`staticmethod` 包装体面 ＋
> `try: return <表达式>`/`finally` 顺序与 `return`-in-finally 崩溃）。逐轮复盘见
> `docs/rounds/03-m3-progress-05.md` 的 **round 14／15／16** 节 ✓（本笔追加 round 16）。

## 现在

- **HEAD**：`a094974`（`dev`，本笔前）—— 本笔＝**round 16**：`time` 面（`strftime`/`localtime`/`gmtime`/`mktime` 族，`clock` 表尾三槽，**ABI 1.2→1.3**）＋ `sys._jit` ＋ `classmethod`/`staticmethod` 包装体面 ＋ `try: return <表达式>`/`finally` 顺序与 `return`-in-finally 崩溃（编译器）；细节见台账 round 16。
- **工作区**：本笔待提交 ＝ **21 项**（20 个修改文件：源码/测试/三份 `SPEC` ＋ `NEXT.md` ＋ 台账 round 16 ＋ `ROUNDS.md` 索引；＋ 1 个新增测试 `crates/pyawa-runtime/tests/time_shapes.rs`；`git status --porcelain` 与之一致；无其他未跟踪项）。
- **stash**：**空** ✓
- **round 16 新增验收（全 ✓）**：
  1. **`import test.support` 通过**（`test.support ok`；`has_strftime_extensions=True`、`_JIT_ENABLED=False`）——
     这是 round 14–15 一直压着的"官方套件框架门槛"在**模块导入层**的最后一段。
  2. `time` 面与 `python3` 3.14.4 逐条一致：`%4Y`='2026'、`localtime(0)`=`(1970,1,1,8,0,0,3,1,0)`、
     `strftime("%Y-%m-%d %H:%M:%S")`=`'1970-01-01 08:00:00'`、`asctime`/`ctime`/`mktime`（float）、
     `timezone=-28800`/`tzname=('CST','CST')`/`daylight=0`；`TZ=UTC` 全表 IDENTICAL。
  3. `try: return <表达式>` ＋ `finally` ⇒ 与参照同序（`target/recon/task22_try_finally.py`：`result => 1`）；
     `return` 写在 `finally` 体内**不再栈溢出 abort**；`BaseException.with_traceback(tb, /)` 与参照逐行一致；
     `unittest.assertRaises` 端到端一致（捕获／`ValueError not raised`／异类外泄）。
  4. `classmethod`/`staticmethod` 包装体：5 属性委派、`repr` `<classmethod(<function …>)>`、`staticmethod` 可调用
     与参照逐行一致；官方 `test_decorators` 不崩、**`test_classmethod` 转 PASS**（逐测试 11 passed／5 failed）。
  5. 既有验收仍全过：`sys.exit(3)`⇒3；最小 `unittest.main()`⇒0；`PYTHONPATH` 进 `sys.path`。
- **数字**（round 16 终审实测）：判据① **466/628＝74.2%**（通过 449 ＋参照口径 17；**未变**）；`PA_ABI_VERSION` **1.3**；
  `Lib/` 同步 **544/628**（进度指标 450/544＝82.7%，不作判据）；语料 **407**、在册差异 **3** 未变。
  `Lib/test` 体检：可编译 **311/397＝78.3%**（持平）、**跑通 7/397＝1.8%**（上轮 5；新跑通
  `test_bigaddrspace`／`test_threadedtempfile`）；语法错 86／未实现 55／运行期错 247／超时 2
  （`test_configparser`／`test_zstd`）。
- **官方套件缺口榜（下一轮取活顺序的实测依据，`target/libtests_survey.tsv`）**：
  ① **`test.support.os_helper` 缺失：`ModuleNotFoundError` ×59 ＋ `ImportError: cannot import name 'os_helper'` ×24
  ＝ **83 个** —— 该文件是本轮之前因 **P3-35 的 surrogate escape（`\uDC80`）** 暂不同步的 7 个之一
  ⇒ **下一轮第一格 = `P3-35` 的 surrogate escape**（修词法期转义 → 同步 os_helper 等 7 个骨架，一次解锁 83+）。
  ② `ModuleNotFoundError` 其余：`doctest` 17／`unittest.mock` 14／`gc` 10／`unicodedata` 4／`hashlib` 4（→ **`P3-37`**）；
  `ImportError: cannot import name 'multibytecodec_support' from 'test'` ×11（同类半同步）。
  ③ 运行期 `TypeError: issubclass() arg 1 must be a class` ×**30** —— `sys.exc_info()` 的如实桩让 unittest runner
  一遇错误就 abort（**错误恢复**格；不改跑通数但会掩盖真实断言，单列小格，优先级高）。
  ④ 语法错：`invalid unicode escape \ud800` 族 ×12＋同行 `try:`·`for:` ×8（→ **`P3-35`**，与 ① 同源）。
  ⑤ `BadCodeSection` ×6（→ **`P3-36`**）；`NotImplementedError` ×4（`_socket` I/O 方法未接线）。
- **本轮修掉的格不再出现在缺口榜**：`time.strftime` ×135、`else`＋`with` ×135、`test_decorators` 内存损坏、
  `try/return-finally`、`classmethod` 包装体面。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序（据本轮实测调整）**：**`P3-35` 的 surrogate escape**（`\ud800`/`\uDC80`，一次解锁 83+ 个官方测试
   ＋ 7 个暂不同步的 `Lib/test` 骨架；与语法错 ④ 同源）→ **`P3-37`**（缺模块：doctest／unittest.mock／gc／
   unicodedata／hashlib）→ **`P3-34`**（complex，52 个）→ **`P3-36`**（`.pyac BadCodeSection`）→
   **报用户批准后 M6 收口**（`--no-ff` ＋ tag `m6`）。
3. **另立小格（按需插队，优先级从上到下）**：`sys.exc_info()` 真实现（unittest 错误恢复）；`builtins.compile` 缺失；
   嵌套类装饰器 `func.__dict__.update` 丢失；`test_memoize 3!=4`；`_functools` 模块；`struct_time` 类型（`time` 面收口）。

## 下一条命令（下一轮开工）

```text
git status --short                                      # 应干净（round 16 已提交）
python3 tools/libtests_survey.py --jobs 8               # 重跑基线
python3 -c "import test.support"                        # 应已 ok（round 16 解锁）；再量 os_helper 缺口
# 下一格：P3-35 的 surrogate escape —— 词法期接受 \ud800/\uDC80，然后 sync_lib 补 test/support/os_helper.py 等 7 个
```

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 466/628＝74.2%**（通过 449 ＋ 参照口径 17）。
- **`Lib/test` 体检口径**：`tools/libtests_survey.py`（397 个顶层 `test_*.py`；默认目录＝
  `target/upstream/extracted/usr/lib/python3.14/test`）。两个比例是**报告级**、**不得当验收或完成度**（`CM-15`）。
  **改编译器之后必须清 `.pyac` 缓存**：`find target Lib -name __pyawa__ -type d -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`（陈旧判定只看源字节，看不到编译器版本 ✗）。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m5`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）。合并后应复核 `git diff main dev` 为空。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.2**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=2`；`CP-30`／`AB-39` 共用号；
  次版本 2＝round 15 给 `fs` vtable 表尾追加 `stat_times` 槽，按 `AB-41` 只升次版本）；
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
